//! Where class hierarchies live in the Rust: which classes have a fields struct that their
//! subclasses embed, which methods only touch fields (and so live on that struct), how a value
//! of each class type is represented (one concrete class's object, or an enum over several),
//! and where each method's body is emitted.

use super::FastMap;
use super::{Facts, FastSet};
use crate::ast::*;

/// How values of a class type are represented.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rep {
    /// The object of one concrete class (index into `Class::types`), as that class is
    /// represented: every value of the type is an object of that class.
    Object(usize),
    /// The enum named after the type, with a variant per concrete class it can hold.
    Enum(usize),
}

/// Where the body of a method is emitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Home {
    /// In the `impl` of the struct that holds objects of exactly this concrete class.
    Class(usize),
    /// On the fields struct of this class, taking `&self` or `&mut self` of it.
    Fields(usize),
    /// On the enum of this type, taking `&self` of the enum.
    Enum(usize),
    /// A static method: a function of its class's module.
    Free,
    /// Nowhere: an abstract method, or a method no object can run.
    Nowhere,
}

/// Whether type `t` is a class with subclasses: its objects' fields are a struct of their own.
pub fn has_subclasses(class: &Class, t: usize) -> bool {
    class.types[t].kind == TypeKind::Class && class.hierarchy.has_subtypes(t)
}

/// The classes with subclasses that have a fields struct: those that declare instance fields,
/// have objects of their own, or have a constructor that does something. Its subclasses embed
/// it as their field `base`. So does an abstract class without subclasses that declares
/// instance fields: its type is an enum without variants, whose values are always null, and its
/// fields are read and written through `base()` all the same (as the reflection table's cases
/// do), which never returns.
pub fn fields_structs(class: &Class) -> FastSet<usize> {
    let mut out = FastSet::default();
    // Superclasses first, so that a constructor's `super(...)` is known to do something or not.
    let mut order: Vec<usize> = (0..class.types.len()).collect();
    order.sort_by_key(|&t| class.hierarchy.ancestors(t).len());
    for t in order {
        let decl = &class.types[t];
        let objectless = decl.kind == TypeKind::Class && !class.hierarchy.is_concrete(t);
        if !has_subclasses(class, t) {
            if objectless && !decl.fields.is_empty() {
                out.insert(t);
            }
            continue;
        }
        let trivial = |m: &Method| match &m.body[..] {
            [] => true,
            [Stmt::Expr(Expr { kind: ExprKind::SuperInit { args, resolved }, .. })] => {
                resolved.is_none_or(|r| !out.contains(&class.methods[r].class))
                    && !args.iter().any(has_effects)
            }
            _ => false,
        };
        let has = !decl.fields.is_empty()
            || class.hierarchy.is_concrete(t)
            || class.hierarchy.ancestors(t).iter().any(|a| out.contains(a))
            || class.methods.iter().any(|m| m.class == t && m.is_ctor && !trivial(m));
        if has {
            out.insert(t);
        }
    }
    out
}

/// The class that declares each field visible in each class, by (class name, field name).
pub fn field_owners(class: &Class) -> FastMap<(String, String), String> {
    let mut out = FastMap::default();
    for (t, decl) in class.types.iter().enumerate() {
        let mut chain = vec![t];
        chain.extend(class.hierarchy.ancestors(t));
        for &a in chain.iter().rev() {
            for f in &class.types[a].fields {
                out.insert((decl.name.clone(), f.name.clone()), class.types[a].name.clone());
            }
        }
    }
    out
}

/// The superclass whose fields struct the struct of class `t` embeds as `base`.
pub fn embedded(facts: &Facts, class: &Class, t: usize) -> Option<usize> {
    class.hierarchy.ancestors(t).into_iter().find(|a| facts.fields_structs.contains(a))
}

/// The path from the struct of class `c` to the fields struct of its superclass `d`, as field
/// accesses with a trailing dot: `base.base.`; empty when `d` is `c`.
pub fn base_path(facts: &Facts, class: &Class, c: usize, d: usize) -> String {
    let mut path = String::new();
    if c == d {
        return path;
    }
    for a in class.hierarchy.ancestors(c) {
        if facts.fields_structs.contains(&a) {
            path.push_str("base.");
        }
        if a == d {
            break;
        }
    }
    path
}

/// The concrete classes whose objects the values of type `t` can be: those under it whose
/// objects reach a value of `t` (see `flows`) and are not plain values (an owned class is never
/// converted to another type), and the enums that implement it, whose constants are `Copy`
/// values.
pub fn variants(class: &Class, facts: &Facts, t: usize) -> Vec<usize> {
    class
        .hierarchy
        .concretes(t)
        .into_iter()
        .filter(|&c| facts.flows[t].contains(&c))
        .filter(|&c| {
            matches!(class.types[c].kind, TypeKind::Enum | TypeKind::Runtime)
                || !facts.owned.contains(&class.types[c].name)
        })
        .collect()
}

/// For each type, the concrete classes (and enums) whose objects become values of that type: a
/// `new` makes an object of its class, a cast (or a pattern's binding) makes the objects of its
/// operand's type that are of the target type values of it, and an object that runs a method its class inherits is the `this` of
/// that method, a value of the type that declares it (unless the method lives on a fields
/// struct, which borrows only the fields). A call of an inherited method, or `super.m()`, makes
/// the receiver a value of the type that declares the method. An enum's constants are values of
/// the enum.
/// The classes in `raised` are those whose objects the runtime creates when an operation fails.
pub fn flows(
    class: &Class,
    field_level: &FastSet<usize>,
    raised: &std::collections::BTreeSet<usize>,
) -> Vec<FastSet<usize>> {
    let h = &class.hierarchy;
    let n = class.types.len();
    let mut flows: Vec<FastSet<usize>> = vec![FastSet::default(); n];
    for &c in raised {
        flows[c].insert(c);
    }
    let mut edges: FastSet<(usize, usize)> = FastSet::default();
    for (t, decl) in class.types.iter().enumerate() {
        if matches!(decl.kind, TypeKind::Enum | TypeKind::Runtime) {
            flows[t].insert(t);
        }
    }
    // Object code that a class inherits: its `this` is a value of the declaring type.
    for (m, method) in class.methods.iter().enumerate() {
        if method.is_static || method.is_ctor || method.is_abstract || field_level.contains(&m) {
            continue;
        }
        for c in h.concretes(method.class) {
            if c != method.class && h.implementation(m, c) == Some(m) {
                edges.insert((c, method.class));
            }
        }
    }
    // Every exception may be thrown, which makes it a `Throwable`, and a `catch` clause makes the
    // `Throwable`s under the classes it names values of those classes.
    let throwable = class.type_index("java.lang.Throwable");
    if let Some(throwable) = throwable {
        for t in (0..n).filter(|&t| t != throwable && h.is_subtype(t, throwable)) {
            edges.insert((t, throwable));
        }
        for m in &class.methods {
            for s in &m.body {
                visit_stmt(s, &mut |_| {}, &mut |s| {
                    if let Stmt::Try { catches, .. } = s {
                        for catch in catches {
                            for &t in catch.types.iter().chain(&class.class_index(catch.ty)) {
                                edges.insert((throwable, t));
                            }
                        }
                    }
                });
            }
        }
    }
    // A string, a box or a primitive is a value of its runtime class, beside `Object`.
    let index = |ty: Type| class.type_of(ty);
    let mut copies_objects = false;
    let mut note = |e: &Expr, at: usize| match &e.kind {
        ExprKind::New { class: c, .. } => _ = flows[*c].insert(*c),
        // A cast, and a pattern's binding, make the value a value of the target type.
        ExprKind::Cast(to, value) | ExprKind::InstanceOf { value, ty: to, binding: Some(_) } => {
            if let (Some(t), Some(s)) = (index(*to), index(value.ty)) {
                edges.insert((s, t));
                // An exception class's value may be held as a `Throwable` (see `rep`), and then
                // converts as one, with every variant of `Throwable`.
                if let Some(throwable) = throwable
                    && s != throwable
                    && h.is_subtype(s, throwable)
                {
                    edges.insert((throwable, t));
                }
            }
            // The rows of `new int[n][m]` as an `Object[]` are `Object`s (`lower::arrays`), and
            // so are those of each level whose arrays are `Object[]`s (`Class::object_rows`).
            if let (ExprKind::NewArray(_), Type::Array(element), Type::Array(row)) =
                (&value.kind, *to, value.ty)
                && let (Some(t), Some(s)) = (index(*element), index(*row))
            {
                edges.insert((s, t));
            }
            for pair in class.object_rows.get(&value.id).into_iter().flat_map(|levels| levels.windows(2)) {
                let (element, row) = (pair[0].0.element(), pair[1].0);
                if element != row
                    && let (Some(t), Some(s)) = (index(element), index(row))
                {
                    edges.insert((s, t));
                }
            }
        }
        // A `String[]` that is an `Object[]` (`lower::covariant`), and `main`'s arguments as one:
        // its strings are `Object`s.
        ExprKind::Builtin(Builtin::StringArrayOf | Builtin::ObjectArrayOf, args) => {
            if let (Some(s), Some(t)) = (index(args[0].ty.element()), index(e.ty.element())) {
                edges.insert((s, t));
            }
        }
        // `Array.newArray` makes an array of any of the array classes as an `Object`.
        ExprKind::Builtin(Builtin::Native(Native::ReflectNewArray), _) => {
            if let Some(object) = index(e.ty) {
                for c in (0..n).filter(|&c| class.array_of(c).is_some()) {
                    edges.insert((c, object));
                }
            }
        }
        ExprKind::Builtin(Builtin::MainArgs, _) => {
            if let (Some(s), Some(t)) = (index(Type::String), index(e.ty.element())) {
                edges.insert((s, t));
            }
        }
        // A `System.arraycopy` between arrays of classes: the objects of the source's element class
        // are values of the destination's, where they pass the check (`lower::arraycopy_classes`).
        // One between `Object`s copies through `Object` (`codegen::arraycopy`), below.
        ExprKind::Builtin(Builtin::Native(Native::ArrayCopy), args) => {
            copies_objects |= matches!(args[0].ty, Type::Class(name, _) if name == OBJECT);
            if let (Type::Array(s), Type::Array(d)) = (args[0].ty, args[2].ty)
                && let (Type::Class(..), Type::Class(..)) = (*s, *d)
                && let (Some(s), Some(d)) = (index(*s), index(*d))
            {
                edges.insert((s, d));
            }
        }
        // `super.m()`: `this` as a value of the superclass.
        ExprKind::This => {
            if let Some(t) = index(e.ty)
                && t != at
            {
                edges.insert((at, t));
            }
        }
        ExprKind::MethodCall { receiver, resolved: Some(r), .. } => {
            let d = class.methods[*r].class;
            if let Some(s) = index(receiver.ty)
                && s != d
                && !class.methods[*r].is_static
                && !field_level.contains(r)
            {
                edges.insert((s, d));
            }
        }
        _ => {}
    };
    for m in &class.methods {
        for s in &m.body {
            visit_stmt(s, &mut |e| visit_expr(e, &mut |x| note(x, m.class)), &mut |_| {});
        }
    }
    for (t, decl) in class.types.iter().enumerate() {
        let constants = decl.enum_constants.iter().flatten().flat_map(|c| &c.values);
        for init in decl.fields.iter().chain(&decl.statics).filter_map(|f| f.init.as_ref()).chain(constants) {
            visit_expr(init, &mut |x| note(x, t));
        }
    }
    // Each class that reaches a type moves on along its edges, once: to the targets that it is
    // a subtype of, found among the edges' targets or among its supertypes, whichever are fewer
    // (`Object` has an edge to the target of every cast from it, and a class few supertypes).
    let mut out: Vec<Vec<usize>> = vec![Vec::new(); n];
    for &(s, t) in &edges {
        out[s].push(t);
    }
    let mut work: Vec<(usize, usize)> =
        flows.iter().enumerate().flat_map(|(s, f)| f.iter().map(move |&c| (s, c))).collect();
    let mut supertypes: Vec<Option<Vec<usize>>> = vec![None; n];
    let spread = |flows: &mut Vec<FastSet<usize>>,
                  out: &Vec<Vec<usize>>,
                  edges: &FastSet<(usize, usize)>,
                  supertypes: &mut Vec<Option<Vec<usize>>>,
                  work: &mut Vec<(usize, usize)>| {
        while let Some((s, c)) = work.pop() {
            let ups = supertypes[c].get_or_insert_with(|| h.subtype_of(c));
            if out[s].len() <= ups.len() {
                for &t in &out[s] {
                    if h.is_subtype(c, t) && flows[t].insert(c) {
                        work.push((t, c));
                    }
                }
            } else {
                for &t in ups.iter() {
                    if edges.contains(&(s, t)) && flows[t].insert(c) {
                        work.push((t, c));
                    }
                }
            }
        }
    };
    spread(&mut flows, &out, &edges, &mut supertypes, &mut work);
    // A `System.arraycopy` between `Object`s reads the elements of any array that is an `Object`
    // as `Object`s, and stores `Object`s as the elements of any such array where they pass the
    // check: the element class of each array class among `Object`'s values flows both ways with
    // `Object`. The elements may be arrays, which become `Object`s in turn.
    if copies_objects && let Some(object) = class.type_index(OBJECT) {
        let mut done: FastSet<usize> = FastSet::default();
        loop {
            let arrays: Vec<usize> = flows[object]
                .iter()
                .copied()
                .filter(|&a| class.array_of(a).is_some() && !done.contains(&a))
                .collect();
            if arrays.is_empty() {
                break;
            }
            for a in arrays {
                done.insert(a);
                let Some(Type::Array(element)) = class.array_of(a) else { continue };
                if element.is_numeric() || *element == Type::Boolean {
                    continue;
                }
                let Some(e) = index(*element).filter(|&e| e != object) else { continue };
                for (s, t) in [(e, object), (object, e)] {
                    if edges.insert((s, t)) {
                        out[s].push(t);
                        work.extend(flows[s].iter().map(|&c| (s, c)));
                    }
                }
            }
            spread(&mut flows, &out, &edges, &mut supertypes, &mut work);
        }
    }
    flows
}

/// How values of class type `t` are represented.
pub fn rep(class: &Class, facts: &Facts, t: usize) -> Rep {
    // A thrown value is a `Throwable`, which `catch` clauses match by class; a standard exception
    // class that several classes extend is one too, rather than an enum of its own.
    let name = &class.types[t].name;
    if let Some(throwable) = class.type_index("java.lang.Throwable")
        && (t == throwable
            || (crate::lower::is_standard(name)
                && crate::lower::standard_subclass(name, "java.lang.Throwable")
                && variants(class, facts, t).len() > 1))
    {
        return Rep::Enum(throwable);
    }
    if class.hierarchy.is_concrete(t) && !class.hierarchy.has_subtypes(t) {
        return Rep::Object(t);
    }
    // A value of an interface that only an enum implements is still the interface's enum: a
    // value of a class type is an object everywhere else.
    match variants(class, facts, t)[..] {
        [only] if class.types[only].kind == TypeKind::Class => Rep::Object(only),
        _ => Rep::Enum(t),
    }
}

/// Where method `m` is emitted.
pub fn home(class: &Class, facts: &Facts, m: usize) -> Home {
    let method = &class.methods[m];
    let t = method.class;
    if method.is_static {
        return Home::Free;
    }
    if method.is_abstract {
        return Home::Nowhere;
    }
    // A default method of a closure type is a method of its extension trait.
    if facts.closure_types.contains(&t) {
        return Home::Enum(t);
    }
    if method.is_ctor || class.types[t].enum_constants.is_some() || !class.hierarchy.has_subtypes(t) {
        return Home::Class(t);
    }
    if facts.field_level.contains(&m) {
        return Home::Fields(t);
    }
    match rep(class, facts, t) {
        Rep::Object(c) => Home::Class(c),
        Rep::Enum(t) if variants(class, facts, t).is_empty() => Home::Nowhere,
        Rep::Enum(t) => Home::Enum(t),
    }
}

/// The `toString` that an object of concrete class `c` runs, if the program declares one.
pub fn to_string_impl(class: &Class, c: usize) -> Option<usize> {
    declared_impl(class, c, "toString")
}

/// The method that making the text of an object of concrete class `c` runs: its `toString`, or
/// where it has `Object`'s, the `hashCode` that one shows, if the program declares one.
pub fn text_impl(class: &Class, c: usize) -> Option<usize> {
    to_string_impl(class, c).or_else(|| declared_impl(class, c, "hashCode"))
}

/// The method without parameters named `name` that an object of concrete class `c` runs, if the
/// program declares one.
fn declared_impl(class: &Class, c: usize, name: &str) -> Option<usize> {
    debug_assert_eq!(class.hierarchy.overriders.len(), class.methods.len(), "the hierarchy is out of date");
    let chain = std::iter::successors(Some(c), |&t| class.hierarchy.superclass(t));
    chain.into_iter().find_map(|t| {
        class.hierarchy.methods_of(t).iter().copied().find(|&i| {
            let m = &class.methods[i];
            m.class == t && m.name == name && m.params.is_empty() && !m.is_static && !m.is_abstract
        })
    })
}

/// The methods of classes with subclasses that use their object only to read and write its
/// fields and to call such methods that nothing overrides: they live on the class's fields
/// struct and borrow it, as `&self` or `&mut self`. That is only safe when nothing they run
/// reaches an object of their class tree through another handle (see `borrowing`), unless the
/// tree is immutable, whose objects have no `RefCell`. A method of a class that threads share
/// borrows only when it is short (`long` has those that are not; see `threads`).
pub fn field_level(
    class: &Class,
    reach: &[super::closure::Bits],
    immutable: &FastSet<String>,
    structs: &FastSet<usize>,
    long: &FastSet<usize>,
) -> FastSet<usize> {
    let h = &class.hierarchy;
    let mut level: FastSet<usize> = (0..class.methods.len())
        .filter(|&m| {
            let method = &class.methods[m];
            let t = method.class;
            if method.is_static
                || method.is_ctor
                || method.is_abstract
                || !has_subclasses(class, t)
                || !structs.contains(&t)
            {
                return false;
            }
            let family = h.family(t);
            family.iter().all(|&c| immutable.contains(&class.types[c].name))
                || (!long.contains(&m) && !family.iter().any(|&c| reach[m].contains(c)))
        })
        .collect();
    loop {
        let losing: Vec<usize> = level
            .iter()
            .copied()
            .filter(|&m| {
                let method = &class.methods[m];
                let mut chain = vec![method.class];
                chain.extend(h.ancestors(method.class));
                !method.body.iter().all(|s| {
                    let mut ok = true;
                    visit_stmt(
                        s,
                        &mut |e| {
                            ok &= uses_this_for_fields(e, &|r, is_super| {
                                level.contains(&r)
                                    && chain.contains(&class.methods[r].class)
                                    && (is_super || !h.is_overridden(r))
                            })
                        },
                        &mut |_| {},
                    );
                    ok
                })
            })
            .collect();
        if losing.is_empty() {
            return level;
        }
        for m in losing {
            level.remove(&m);
        }
    }
}

/// Whether `e` uses `this` only as the object of a field access or the receiver of a call that
/// `allowed` accepts (given the callee and whether it is a `super` call).
fn uses_this_for_fields(e: &Expr, allowed: &dyn Fn(usize, bool) -> bool) -> bool {
    let mut ok_ids = FastSet::default();
    let mut selves = Vec::new();
    visit_expr(e, &mut |x| match &x.kind {
        ExprKind::This => selves.push(x.id),
        ExprKind::Field { object, .. }
        | ExprKind::Assign { target: Place::Field { object, .. }, .. }
        | ExprKind::IncDec { target: Place::Field { object, .. }, .. }
            if matches!(object.kind, ExprKind::This) =>
        {
            ok_ids.insert(object.id);
        }
        ExprKind::MethodCall { receiver, resolved: Some(r), is_super, .. }
            if matches!(receiver.kind, ExprKind::This) && allowed(*r, *is_super) =>
        {
            ok_ids.insert(receiver.id);
        }
        _ => {}
    });
    selves.iter().all(|id| ok_ids.contains(id))
}
