//! Which strings and objects can be `null`. A local, parameter, return value or field that never
//! holds null stays a plain value (or a borrowed `&str`); only the others become `Option`. The
//! analysis is flow-insensitive, so it over-approximates: the code generator narrows a variable
//! inside a null check and may then treat it as non-null, but never the other way round.
//!
//! A field starts as Java's `null` unless something sets it first. An object field is not null
//! when every constructor sets it, among the assignments that become the constructor's struct
//! literal (see `leading_assignments`) or by its initializer, to a value that is not null, and no
//! other assignment stores a value that may be null. A static field needs a non-null initializer
//! that runs on first use (see `statics`).

use super::FastSet;
use super::{Facts, leading_assignments};
use crate::ast::*;

/// Records the variables, parameters, return values and fields that can hold null, until nothing
/// changes. A parameter is recorded under its name, as the names of a method's variables and
/// parameters are distinct.
pub fn analyze(class: &Class, facts: &mut Facts) {
    let read: Vec<FastSet<String>> = class.methods.iter().map(read_locals).collect();
    let mut ctors: Vec<Vec<usize>> = vec![Vec::new(); class.types.len()];
    for (m, method) in class.methods.iter().enumerate().filter(|(_, m)| m.is_ctor) {
        ctors[method.class].push(m);
    }
    // The override groups, by id, whose parameters (with the parameter's index) or return values
    // are recorded as nullable for every method of the group.
    let mut nullable_groups: FastSet<(usize, usize)> = FastSet::default();
    let mut nullable_group_returns: FastSet<usize> = FastSet::default();
    loop {
        let before = (
            facts.nullable_vars.len(),
            facts.nullable_returns.len(),
            facts.nullable_fields.len(),
            facts.nullable_statics.len(),
            facts.placeholder_statics.len(),
            facts.nullable_elements.len(),
        );
        let mut elements = Vec::new();
        let mut vars = Vec::new();
        let mut params = Vec::new();
        let mut declared = Vec::new();
        let mut returns = Vec::new();
        let mut fields = Vec::new();
        let mut statics = Vec::new();
        let mut placeholders = Vec::new();
        let mut on_expr = |m: usize, e: &Expr| {
            visit_expr(e, &mut |x| match &x.kind {
                ExprKind::Assign { op: None, target, value }
                    if nullable_type(x.ty) && maybe_null(facts, m, value) =>
                {
                    match target {
                        Place::Var(name) => vars.push((m, name.clone())),
                        Place::Field { object, name } => {
                            if let Some(owner) = class.field_owner_name(object.ty, name) {
                                fields.push((owner.to_string(), name.clone()));
                            }
                        }
                        Place::Static { class, name } => statics.push((*class, name.clone())),
                        Place::Index(array, _) => elements.push(array.ty.element()),
                    }
                }
                // `new T[n]` leaves its strings and objects null, and `new T[n][]` its
                // arrays.
                ExprKind::NewArray(sizes) => {
                    let elem = x.ty.element_at(sizes.len());
                    if nullable_type(elem) {
                        elements.push(elem);
                    }
                }
                // `Arrays.copyOf` and `copyOfRange` pad a longer copy with null.
                ExprKind::Builtin(Builtin::ArraysCopyOf | Builtin::Arrays(ArraysOp::CopyOfRange), _)
                    if x.ty.element().is_reference() =>
                {
                    elements.push(x.ty.element());
                }
                // A copy of an array between `String[]` and `Object[]` has the same nulls.
                ExprKind::Builtin(Builtin::StringArrayOf | Builtin::ObjectArrayOf, args)
                    if facts.nullable_elements.contains(&args[0].ty.element()) =>
                {
                    elements.push(x.ty.element());
                }
                // A copy of `System.arraycopy` stores the source's nulls in the destination.
                ExprKind::Builtin(Builtin::Native(Native::ArrayCopy), args) if matches!((args[0].ty, args[2].ty), (Type::Array(s), Type::Array(_)) if facts.nullable_elements.contains(s)) =>
                {
                    elements.push(args[2].ty.element());
                }
                // An atomic update of an element compares the element with its expected value.
                ExprKind::Builtin(Builtin::ElementUpdate(_), args)
                    if args.len() == 2 && maybe_null(facts, m, &args[0]) =>
                {
                    elements.push(updated_element(x));
                }
                // A box in an array of primitives is unboxed (`analysis::unboxing`), so the
                // array holds no null.
                ExprKind::ArrayLit(items)
                    if nullable_type(x.ty.element())
                        && items.iter().any(|item| maybe_null(facts, m, item)) =>
                {
                    elements.push(x.ty.element());
                }
                // `Array.newArray` makes an array of any of the array classes that are `Object`s,
                // with null references, and `Array.set` may store null in any of them.
                ExprKind::Builtin(Builtin::Native(Native::ReflectNewArray), _) => {
                    elements.extend(reflect_elements(class));
                }
                ExprKind::Builtin(Builtin::Native(Native::ReflectArraySet), args)
                    if maybe_null(facts, m, &args[2]) =>
                {
                    elements.extend(reflect_elements(class));
                }
                // `Arrays.fill` stores its value in every element of the range.
                ExprKind::Builtin(Builtin::ArraysFill | Builtin::Arrays(ArraysOp::FillRange), args)
                    if args.last().is_some_and(|v| maybe_null(facts, m, v)) =>
                {
                    elements.push(args[0].ty.element());
                }
                ExprKind::Call { args, resolved: Some(callee), .. }
                | ExprKind::MethodCall { args, resolved: Some(callee), .. }
                | ExprKind::New { args, ctor: Some(callee), .. }
                | ExprKind::ThisCall { args, resolved: Some(callee) }
                | ExprKind::SuperInit { args, resolved: Some(callee) } => {
                    // The argument reaches every method the call may run.
                    for (i, arg) in args.iter().enumerate() {
                        if maybe_null(facts, m, arg) {
                            params.push((*callee, i));
                        }
                    }
                }
                _ => {}
            })
        };
        for (m, method) in class.methods.iter().enumerate() {
            for stmt in &method.body {
                let mut on_stmt = |s: &Stmt| match s {
                    Stmt::Local { ty, name, init: Some(init), .. }
                        if nullable_type(*ty)
                            && (maybe_null(facts, m, init)
                                || matches!(init.kind, ExprKind::Static { .. })
                                    && !read[m].contains(name)) =>
                    {
                        // A local that nothing reads, set from a static, initializes the static's
                        // class (`MemorySegment segment = MemorySegment.NULL;` in the JDK), which
                        // may be under way, with the static still null.
                        declared.push((m, name.clone()));
                    }
                    Stmt::Return(Some(value), _)
                        if maybe_null(facts, m, value) && !never_null_result(class, m) =>
                    {
                        returns.push(m)
                    }
                    Stmt::ForEach { name, array, .. }
                        if facts.nullable_elements.contains(&array.ty.element()) =>
                    {
                        declared.push((m, name.clone()));
                    }
                    _ => {}
                };
                visit_stmt(stmt, &mut |e| on_expr(m, e), &mut on_stmt);
            }
        }
        // Field initializers run in every object's constructor: a `new String[2]` there leaves
        // its strings null too.
        for f in class.types.iter().flat_map(|t| t.fields.iter().chain(&t.statics)) {
            if let Some(init) = &f.init {
                on_expr(usize::MAX, init);
            }
        }
        for (t, decl) in class.types.iter().enumerate() {
            for f in decl.fields.iter().filter(|f| may_be_unset(f.ty) || starts_null_enum(facts, f)) {
                if !set_by_construction(class, facts, &ctors[t], t, &f.name) {
                    fields.push((decl.name.clone(), f.name.clone()));
                }
            }
            for f in decl.statics.iter().filter(|f| may_be_unset(f.ty) || matches!(f.ty, Type::Enum(_))) {
                // An eagerly initialized static starts at a placeholder, which an object has not,
                // so its cell starts empty; once the class is initialized, it holds the value.
                if !f.init.as_ref().is_some_and(|e| !maybe_null(facts, usize::MAX, e)) {
                    statics.push((t, f.name.clone()));
                } else if facts.eager_init.contains(&t) && matches!(f.ty, Type::Class(..)) {
                    placeholders.push((t, f.name.clone()));
                }
            }
        }
        // A parameter that its method sets to a value that may be null (`moment =
        // moment.plus(days)`) is an `Option`, in every method of its group as below.
        for (m, name) in &vars {
            if let Some(i) =
                class.methods.get(*m).and_then(|method| method.params.iter().position(|p| &p.name == name))
            {
                params.push((*m, i));
            }
        }
        facts.nullable_vars.extend(vars);
        facts.nullable_vars.extend(declared);
        // The methods of an override group share their parameters' and return value's nullness,
        // so that a call may run any of them.
        // Each group once: the facts only grow.
        for (m, i) in params {
            if nullable_groups.insert((class.hierarchy.group_id(m), i)) {
                for &g in class.hierarchy.group_members(m) {
                    facts.nullable_vars.insert(g, class.methods[g].params[i].name.clone());
                }
            }
        }
        for m in returns {
            if nullable_group_returns.insert(class.hierarchy.group_id(m)) {
                facts.nullable_returns.extend(class.hierarchy.group_members(m));
            }
        }
        facts.nullable_fields.extend(fields);
        facts.nullable_statics.extend(statics);
        facts.placeholder_statics.extend(placeholders);
        facts.nullable_elements.extend(elements);
        let after = (
            facts.nullable_vars.len(),
            facts.nullable_returns.len(),
            facts.nullable_fields.len(),
            facts.nullable_statics.len(),
            facts.placeholder_statics.len(),
            facts.nullable_elements.len(),
        );
        if before == after {
            break;
        }
    }
    // A static that the program may set to null holds null like any other, whatever its
    // initializer: its cell is an `Option` that is read as it is.
    let nullable = facts.nullable_statics.clone();
    facts.placeholder_statics.retain(|key| !nullable.contains(key));
}

/// The element types of references of the array classes that are `Object`s, which
/// `java.lang.reflect.Array`'s natives make and store into.
/// Whether method `m` is one of the JDK's whose result is never null, though this analysis
/// cannot tell, as it does not see that a value is not null once the code has read through it.
///
/// `sun.nio.fs.UnixPath.getByteArrayForSysCalls()`, whose result the patched
/// `UnixNativeDispatcher` gives the natives (lines of `UnixPath.java` of Java 26): it returns
/// `resolve(defaultDirectory, path)` (line 146), which returns one of its two arguments only
/// after reading the lengths of both (lines 367-372) and otherwise a new array; `path` (line
/// 149) only after `isEmpty()` has read `path.length` (lines 148, 226); or a new array (lines
/// 152-153). A null would throw before any of them returns.
fn never_null_result(class: &Class, m: usize) -> bool {
    let method = &class.methods[m];
    let decl = &class.types[method.class];
    decl.package == "sun.nio.fs"
        && decl.name == "sun.nio.fs.UnixPath"
        && method.name == "getByteArrayForSysCalls"
        && method.params.is_empty()
}

fn reflect_elements(class: &Class) -> Vec<Type> {
    (0..class.types.len())
        .filter_map(|c| class.array_of(c))
        .map(|array| array.element())
        .filter(|&element| nullable_type(element))
        .collect()
}

/// The locals and parameters that the code of `method` reads.
fn read_locals(method: &Method) -> FastSet<String> {
    let mut read = FastSet::default();
    for stmt in &method.body {
        visit_stmt(
            stmt,
            &mut |e| {
                visit_expr(e, &mut |x| match &x.kind {
                    ExprKind::Var(name)
                    | ExprKind::Assign { op: Some(_), target: Place::Var(name), .. }
                    | ExprKind::IncDec { target: Place::Var(name), .. } => {
                        read.insert(name.clone());
                    }
                    _ => {}
                })
            },
            &mut |_| {},
        );
    }
    read
}

/// The fields whose nullness is analyzed: strings, arrays, objects and charsets. A boxed field is
/// always an `Option`.
fn may_be_unset(ty: Type) -> bool {
    matches!(
        ty,
        Type::String
            | Type::StringBuilder
            | Type::Array(_)
            | Type::Class(..)
            | Type::JavaClass
            | Type::Charset
    )
}

/// An enum field that may start as `null`: with no initializer, or one that may be null (`null`,
/// or gson's `strictness = DEFAULT_STRICTNESS`, a constant that is `null`). An `Option` unless
/// every constructor sets it first (`set_by_construction`).
fn starts_null_enum(facts: &Facts, f: &Field) -> bool {
    matches!(f.ty, Type::Enum(_)) && f.init.as_ref().is_none_or(|e| maybe_null(facts, usize::MAX, e))
}

/// Whether every constructor of class `t` sets the field `name` to a value that is not null
/// before anything can read it.
fn set_by_construction(class: &Class, facts: &Facts, ctors: &[usize], t: usize, name: &str) -> bool {
    let decl = &class.types[t];
    let i = decl.fields.iter().position(|f| f.name == name).expect("a field of the class");
    let init = decl.fields[i].init.as_ref();
    if decl.fields[i].ty == Type::String && init.is_some() {
        // A string field's initializer, or the empty string that stands in for it, is not null.
        return init.is_some_and(|e| !maybe_null(facts, usize::MAX, e));
    }
    ctors.iter().map(|&index| (index, &class.methods[index])).all(|(index, m)| {
        if delegates(&m.body) {
            return true;
        }
        let leading = leading_assignments(class, m, facts);
        match leading.iter().find(|&&(j, _)| j == i) {
            Some((_, value)) => !maybe_null(facts, index, value),
            None => {
                super::inline_initializers(class, t, facts)
                    && init.is_some_and(|e| !maybe_null(facts, index, e))
            }
        }
    })
}

/// Whether the string variable or parameter `name` of method `m` can hold null.
pub fn nullable_var(facts: &Facts, m: usize, name: &str) -> bool {
    facts.nullable_vars.contains(m, name)
}

/// The types whose values can be null: references, and enums and charsets, `Copy` values, which
/// are `Option`s where they may be null as a `java::Class` is.
pub fn nullable_type(ty: Type) -> bool {
    ty.is_reference() || matches!(ty, Type::Enum(_) | Type::Charset)
}

/// Whether `e`, evaluated in method `m`, can produce null.
pub fn maybe_null(facts: &Facts, m: usize, e: &Expr) -> bool {
    // Only references, enums, charsets and the null literal can be null.
    if !nullable_type(e.ty) {
        return false;
    }
    match &e.kind {
        ExprKind::Null => true,
        ExprKind::Var(name) => nullable_var(facts, m, name),
        ExprKind::Assign { op: None, target: Place::Var(_), value } => maybe_null(facts, m, value),
        // `w++` or `w += 1` of a boxed local that may be null is the local's `Option`, which the
        // update has unboxed (`analysis::unboxing`).
        ExprKind::IncDec { target: Place::Var(name), .. }
        | ExprKind::Assign { target: Place::Var(name), .. } => nullable_var(facts, m, name),
        ExprKind::Call { resolved: Some(callee), .. }
        | ExprKind::MethodCall { resolved: Some(callee), .. } => facts.nullable_returns.contains(callee),
        // In `s != null ? s : t` the arm that is just `s` is not null.
        ExprKind::Conditional(cond, a, b) => {
            let narrowed = |arm: &Expr, when: bool| matches!(&arm.kind, ExprKind::Var(v) if non_null_names(cond, when).contains(&v.as_str()));
            (!narrowed(a, true) && maybe_null(facts, m, a))
                || (!narrowed(b, false) && maybe_null(facts, m, b))
        }
        ExprKind::Cast(_, value) => maybe_null(facts, m, value),
        // The arm of a pattern switch after the check of its selector is null where the arm's
        // value may be; the variable checked (`[v, v]`, see `Builtin::NullChecked`) is not.
        ExprKind::Builtin(Builtin::NullChecked(_), args)
            if args.len() == 2
                && !matches!((&args[0].kind, &args[1].kind), (ExprKind::Var(a), ExprKind::Var(b)) if a == b) =>
        {
            maybe_null(facts, m, &args[1])
        }
        // A copy of a `String[]` as an `Object[]` or back is null where the array is.
        ExprKind::Builtin(Builtin::StringArrayOf | Builtin::ObjectArrayOf, args) => {
            maybe_null(facts, m, &args[0])
        }
        // A switch expression is null when a value it yields may be.
        ExprKind::Switch { cases, .. } => {
            let mut null = false;
            for s in cases.iter().flat_map(|c| &c.body) {
                visit_stmt_outside_switches(s, &mut |s| {
                    null |= matches!(s, Stmt::Yield(v) if maybe_null(facts, m, v));
                });
            }
            null
        }
        ExprKind::Builtin(Builtin::ClassMethod(op), _) => op.nullable(),
        ExprKind::Builtin(Builtin::ComparableClassFor, _) => true,
        // The old element, of an atomic update of an element.
        ExprKind::Builtin(Builtin::ElementUpdate(_), _) => {
            facts.nullable_elements.contains(&updated_element(e))
        }
        // `File.list()` of what is not a readable directory.
        ExprKind::Builtin(Builtin::Native(Native::FileSystem("list0")), _) => true,
        // `TimeZone.getSystemTimeZoneID` gives `null` when the system names no zone.
        ExprKind::Builtin(Builtin::Native(Native::SystemTimeZoneId), _) => true,
        // `System.getenv` of a variable that is not set.
        ExprKind::Builtin(Builtin::Native(Native::Getenv), _) => true,
        // A boxed field is always an `Option`: its default is `null`, and so is an assignment of
        // `null` to it.
        _ if matches!(e.ty, Type::Boxed(_))
            && matches!(
                e.kind,
                ExprKind::Field { .. }
                    | ExprKind::Static { .. }
                    | ExprKind::Assign { op: None, target: Place::Field { .. } | Place::Static { .. }, .. }
            ) =>
        {
            true
        }
        ExprKind::Field { object, name } => facts.field_nullable(object.ty, name),
        ExprKind::Static { class, name } => {
            let key = (*class, name.clone());
            facts.nullable_statics.contains(&key)
                || facts.early_static_reads.contains(&e.id) && facts.placeholder_statics.contains(&key)
        }
        // An array type; a lowering that indexes a value of another type gives what may be null.
        ExprKind::Index(array, _) => match array.ty {
            Type::Array(element) => facts.nullable_elements.contains(element),
            _ => true,
        },
        ExprKind::Assign {
            op: None,
            target: Place::Field { .. } | Place::Static { .. } | Place::Index(..),
            value,
        } => maybe_null(facts, m, value),
        _ => false,
    }
}

/// The type of the element that `e`, a `Builtin::ElementUpdate`, updates.
fn updated_element(e: &Expr) -> Type {
    match &e.kind {
        ExprKind::Builtin(_, args) => match args.last().map(|a| &a.kind) {
            Some(ExprKind::Assign { target: Place::Index(array, _), .. }) => array.ty.element(),
            _ => unreachable!("an element update makes a store to an element"),
        },
        _ => unreachable!("an element update"),
    }
}

/// The variables that `cond` proves not null when it evaluates to `when`: those tested with
/// `s != null` in its top-level `&&` chain (and not assigned later in the chain) when true, the
/// one tested by a whole `s == null` when false.
pub fn non_null_names(cond: &Expr, when: bool) -> Vec<&str> {
    let mut parts = Vec::new();
    conjuncts(cond, &mut parts);
    if !when {
        return match &parts[..] {
            [test] => {
                null_test(test).filter(|&(_, non_null)| !non_null).map(|(v, _)| v).into_iter().collect()
            }
            _ => Vec::new(),
        };
    }
    let mut out = Vec::new();
    for (k, part) in parts.iter().enumerate() {
        if let Some((v, true)) = null_test(part)
            && !parts[k + 1..].iter().any(|later| assigns(later, v))
        {
            out.push(v);
        }
    }
    out
}

/// The operands of a chain of `&&`, left to right.
pub fn conjuncts<'e>(e: &'e Expr, out: &mut Vec<&'e Expr>) {
    match &e.kind {
        ExprKind::Binary(BinOp::And, l, r) => {
            conjuncts(l, out);
            conjuncts(r, out);
        }
        _ => out.push(e),
    }
}

/// `v == null` or `v != null` (either way round) on a variable: the variable and whether the test
/// is true when it is not null.
pub fn null_test(e: &Expr) -> Option<(&str, bool)> {
    let ExprKind::Binary(op @ (BinOp::Eq | BinOp::Ne), l, r) = &e.kind else { return None };
    let name = match (&l.kind, &r.kind) {
        (ExprKind::Var(v), ExprKind::Null) | (ExprKind::Null, ExprKind::Var(v)) => v,
        _ => return None,
    };
    Some((name.as_str(), *op == BinOp::Ne))
}
