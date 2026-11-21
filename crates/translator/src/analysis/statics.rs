//! Whether a class's static initialization must run eagerly. Java runs it once, on first use of the
//! class, and anything observable in it (a static block, a call, a `new` of an object, a throw)
//! must happen at that point. When the initializers only compute values, or new arrays, from
//! literals and from statics of the same class that were assigned earlier and that no other code
//! assigns, nothing is observable: each static can be computed on its first access, as a lazily
//! initialized cell, and no `init` function is needed.

use super::{FastMap, FastSet};
use crate::ast::*;

/// The classes, by index, that need an `init` function: they have a `<clinit>` that is not lazy;
/// and the statics, as (class index, name), that are assigned once, by their class's static
/// initialization: declared `final`, not constants, and assigned by no other code (see
/// `Facts::final_statics`).
pub fn eager_and_final(class: &Class) -> (FastSet<usize>, FastSet<(usize, String)>) {
    let changed = changed_later(class);
    let none = FastSet::default();
    let eager =
        (0..class.types.len()).filter(|&t| eager(class, t, changed.get(&t).unwrap_or(&none))).collect();
    let finals = class
        .types
        .iter()
        .enumerate()
        .flat_map(|(t, decl)| decl.statics.iter().map(move |f| (t, f)))
        .filter(|(t, f)| f.is_final && !f.constant && !changed.get(t).is_some_and(|c| c.contains(&f.name)))
        .map(|(t, f)| (t, f.name.clone()))
        .collect();
    (eager, finals)
}

/// Whether class `index` needs an `init` function, given the statics of it that `changed_later`
/// finds.
fn eager(class: &Class, index: usize, changed: &FastSet<String>) -> bool {
    debug_assert_eq!(class.hierarchy.overriders.len(), class.methods.len(), "the hierarchy is out of date");
    let methods = class.hierarchy.methods_of(index).iter().map(|&m| &class.methods[m]);
    let Some(clinit) = methods.into_iter().find(|m| m.class == index && m.name == "<clinit>") else {
        return false;
    };
    let decl = &class.types[index];
    let mut assigned: FastSet<&str> = FastSet::default();
    !clinit.body.iter().all(|stmt| {
        let Stmt::Expr(Expr {
            kind: ExprKind::Assign { op: None, target: Place::Static { class: c, name }, value },
            ..
        }) = stmt
        else {
            return false;
        };
        let ok = *c == index && pure(value, index, &assigned, decl);
        if !changed.contains(name) {
            assigned.insert(name.as_str());
        }
        ok
    })
}

/// For each class, by index, the statics of it that code outside its static initialization
/// assigns. A lazy cell computed from one of them would see its value at the cell's first access,
/// not the value Java's initialization saw.
fn changed_later(class: &Class) -> FastMap<usize, FastSet<String>> {
    let mut changed: FastMap<usize, FastSet<String>> = FastMap::default();
    for m in &class.methods {
        for stmt in &m.body {
            visit_stmt(
                stmt,
                &mut |e| {
                    visit_expr(e, &mut |x| match &x.kind {
                        ExprKind::Assign { target: Place::Static { class: c, name }, .. }
                        | ExprKind::IncDec { target: Place::Static { class: c, name }, .. }
                            if !(m.class == *c && m.name == "<clinit>") =>
                        {
                            changed.entry(*c).or_default().insert(name.clone());
                        }
                        _ => {}
                    })
                },
                &mut |_| {},
            );
        }
    }
    changed
}

/// The eager classes, by index, whose static initialization `main` runs before anything else, in
/// a program that starts threads: those whose `<clinit>` has no effect Java could observe, so
/// that it may run at any time before the class's first use. Run before any thread starts, it
/// assigns the class's `java::Final` statics where every thread reads them with no test (see the
/// runtime's `Final`), and no code needs to initialize the class before it uses it: every static
/// method, constructor and static access of such a class runs with no test (`Gen::init_call`,
/// `entry_inits`). Such a `<clinit>` only assigns statics of its class values that `inert`
/// accepts, or, for an enum in table form (`lower/enum_tables.rs`), calls its constant builders
/// with such values, and builders that only fill rows of their enum's tables (`inert_builder`).
/// No superclass of the class, nor a superinterface that it initializes with it, is eager. A
/// value may name a constant of another enum only if that enum is not eager or is one of these
/// too: the value needs no initialization, and the other enum's has no effect either.
pub fn early(class: &Class, eager: &FastSet<usize>) -> FastSet<usize> {
    let changed = changed_later(class);
    let none = FastSet::default();
    // Each candidate with the other eager enums whose constants its initialization names.
    let mut candidates: FastMap<usize, FastSet<usize>> = FastMap::default();
    for &t in eager {
        let decl = &class.types[t];
        if class.hierarchy.ancestors(t).iter().any(|a| eager.contains(a))
            || crate::codegen::default_superinterfaces(class, t).iter().any(|i| eager.contains(i))
        {
            continue;
        }
        let mut enums = FastSet::default();
        let ok = class.methods.iter().filter(|m| m.class == t && m.name == "<clinit>").all(|clinit| {
            let changed = changed.get(&t).unwrap_or(&none);
            let mut assigned: FastSet<&str> = FastSet::default();
            let body = match crate::lower::rewritten_clinit(decl) {
                true => clinit_code(&clinit.body),
                false => Some(&clinit.body[..]),
            };
            // A constant of another class is inlined and runs nothing.
            let mut named = |c: usize, name: Option<&str>| match name {
                Some(name) => class.types[c].statics.iter().any(|f| f.name == name && f.constant),
                None => {
                    if c != t && eager.contains(&c) {
                        enums.insert(c);
                    }
                    true
                }
            };
            let new = |k| plain_constructor(class, eager, k);
            body.is_some_and(|body| {
                body.iter().all(|stmt| {
                    let Stmt::Expr(e) = stmt else { return false };
                    match &e.kind {
                        ExprKind::Assign { op: None, target: Place::Static { class: c, name }, value } => {
                            let ok = *c == t && inert(value, t, &assigned, decl, &new, &mut named);
                            if !changed.contains(name) {
                                assigned.insert(name.as_str());
                            }
                            ok
                        }
                        ExprKind::Call { resolved: Some(k), args, .. } => {
                            class.methods[*k].class == t
                                && inert_builder(class, *k, &mut FastSet::default(), &mut named)
                                && args.iter().all(|a| {
                                    matches!(a.kind, ExprKind::Null)
                                        || inert(a, t, &assigned, decl, &new, &mut named)
                                })
                        }
                        _ => false,
                    }
                })
            })
        });
        if ok {
            candidates.insert(t, enums);
        }
    }
    // Drop the candidates that name a constant of an eager enum that is not one, until none does.
    loop {
        let out: Vec<usize> = candidates
            .iter()
            .filter(|(_, enums)| enums.iter().any(|e| !candidates.contains_key(e)))
            .map(|(&t, _)| t)
            .collect();
        if out.is_empty() {
            break;
        }
        for t in out {
            candidates.remove(&t);
        }
    }
    candidates.into_keys().collect()
}

/// Whether method `k` is a builder of its enum's constants that only fills rows of the enum's
/// tables (`table[constantOrdinal] = v`) and calls other such builders (`this(..)` of a
/// constructor, or the builder of the fields of the constants' bodies), with values that are
/// parameters, null, or what `inert` accepts with no static but constants, no `new`, and the enum
/// constants that `named` accepts: javac's `TokenKind(String name)` is `this(name, Tag.DEFAULT)`.
/// Called with `inert` arguments, it has no effect but filling its rows, and cannot throw: the
/// ordinals are those of the constants. `seen` holds the builders being checked, against a loop.
fn inert_builder(
    class: &Class,
    k: usize,
    seen: &mut FastSet<usize>,
    named: &mut dyn FnMut(usize, Option<&str>) -> bool,
) -> bool {
    let m = &class.methods[k];
    if !class.builds_constants(k) || !seen.insert(k) {
        return false;
    }
    let decl = &class.types[m.class];
    let param = |e: &Expr| matches!(&e.kind, ExprKind::Var(v) if m.params.iter().any(|p| p.name == *v));
    let value = |e: &Expr, named: &mut dyn FnMut(usize, Option<&str>) -> bool| {
        param(e)
            || matches!(e.kind, ExprKind::Null)
            || inert(e, usize::MAX, &FastSet::default(), decl, &|_| false, named)
    };
    m.body.iter().all(|stmt| match stmt {
        Stmt::Return(None, _) => true,
        Stmt::Expr(Expr {
            kind: ExprKind::Assign { op: None, target: Place::Index(array, index), value: v },
            ..
        }) => {
            matches!(&array.kind, ExprKind::Static { class: c, name }
                if *c == m.class && crate::lower::tabled_field(name).is_some())
                && matches!(&index.kind, ExprKind::Var(v) if v == crate::lower::TABLE_ORDINAL)
                && value(v, &mut *named)
        }
        Stmt::Expr(Expr { kind: ExprKind::Call { resolved: Some(callee), args, .. }, .. }) => {
            class.methods[*callee].class == m.class
                && args.iter().all(|a| value(a, &mut *named))
                && inert_builder(class, *callee, seen, &mut *named)
        }
        _ => false,
    })
}

/// The code of a rewritten `<clinit>` (`lower::clinit_errors`), which runs inside the `try` that
/// records its failure: the class's own static initialization.
fn clinit_code(body: &[Stmt]) -> Option<&[Stmt]> {
    match body.last() {
        Some(Stmt::Try { body, .. }) => Some(body),
        _ => None,
    }
}

/// Whether constructor `k` only assigns fields of the new object its parameters or literals,
/// so that `new` of it has no effect but the allocation and cannot throw: a class that is not
/// eager, not an enum, has no superclass in the program and no field initializer but a literal.
fn plain_constructor(class: &Class, eager: &FastSet<usize>, k: usize) -> bool {
    let m = &class.methods[k];
    let decl = &class.types[m.class];
    let literal = |e: &Expr| pure(e, usize::MAX, &FastSet::default(), decl);
    m.is_ctor
        && !eager.contains(&m.class)
        && decl.enum_constants.is_none()
        && class.hierarchy.superclass(m.class).is_none()
        && decl.fields.iter().all(|f| f.init.as_ref().is_none_or(literal))
        && m.body.iter().all(|stmt| match stmt {
            Stmt::Expr(Expr {
                kind: ExprKind::Assign { op: None, target: Place::Field { object, .. }, value },
                ..
            }) => {
                matches!(object.kind, ExprKind::This)
                    && match &value.kind {
                        ExprKind::Var(v) => m.params.iter().any(|p| p.name == *v),
                        _ => literal(value),
                    }
            }
            _ => false,
        })
}

/// Whether evaluating `e` has no effect and cannot throw. A static of class `index` may be read
/// only if it is a constant or was assigned before, so that a lazy cell never needs another one
/// that is still being computed.
fn pure(e: &Expr, index: usize, assigned: &FastSet<&str>, decl: &TypeDecl) -> bool {
    inert(e, index, assigned, decl, &|_| false, &mut |_, _| false)
}

/// `pure`, where `e` may also make objects with the constructors `new` accepts, which has no
/// effect but the allocation either, and name what `named` accepts: an enum's constant
/// (`named(enum, None)`), or a static of another class (`named(class, Some(name))`).
fn inert(
    e: &Expr,
    index: usize,
    assigned: &FastSet<&str>,
    decl: &TypeDecl,
    new: &dyn Fn(usize) -> bool,
    named: &mut dyn FnMut(usize, Option<&str>) -> bool,
) -> bool {
    let mut recur = |x: &Expr| inert(x, index, assigned, decl, new, named);
    match &e.kind {
        ExprKind::New { ctor: Some(k), args, .. } => new(*k) && args.iter().all(recur),
        ExprKind::EnumConstant { class, .. } => named(*class, None),
        // The ordinal of a constant, which is never null.
        ExprKind::Builtin(Builtin::EnumOrdinal, args) => match &args[..] {
            [x @ Expr { kind: ExprKind::EnumConstant { .. }, .. }] => recur(x),
            _ => false,
        },
        ExprKind::Int(_)
        | ExprKind::Long(_)
        | ExprKind::Double(_)
        | ExprKind::Char(_)
        | ExprKind::Float(_)
        | ExprKind::Bool(_)
        | ExprKind::Str(_)
        | ExprKind::Units(_) => true,
        ExprKind::Static { class, name } if *class != index => named(*class, Some(name)),
        ExprKind::Static { name, .. } => {
            assigned.contains(name.as_str()) || decl.statics.iter().any(|f| f.name == *name && f.constant)
        }
        ExprKind::Unary(_, x) | ExprKind::Cast(_, x) | ExprKind::InstanceOf { value: x, .. } => recur(x),
        // Integer division can divide by zero, which throws.
        ExprKind::Binary(op, l, r) => !matches!(op, BinOp::Div | BinOp::Rem) && recur(l) && recur(r),
        ExprKind::Conditional(c, a, b) => recur(c) && recur(a) && recur(b),
        // A new array is not observable, unless a negative size throws.
        ExprKind::NewArray(sizes) => sizes.iter().all(|n| matches!(n.kind, ExprKind::Int(n) if n >= 0)),
        ExprKind::ArrayLit(items) => items.iter().all(recur),
        _ => false,
    }
}

/// The static methods and constructors of eagerly initialized classes that start by running
/// their class's `init`. Java initializes a class when one of them is first called; a method that
/// only methods of its own class call, or any method of the class of `main` but `main` itself,
/// cannot run before that has started, so it skips the check, as does one of a class that `main`
/// initializes first (`early`). A method that lowering adds for an expression (`Method::added`)
/// never runs it: it is the code of its call sites.
pub fn entry_inits(class: &Class, eager: &FastSet<usize>, early: &FastSet<usize>) -> FastSet<usize> {
    let mains: Vec<&Method> = class.methods.iter().filter(|m| m.is_main).collect();
    let main_class = match mains[..] {
        [main] => Some(main.class),
        _ => None,
    };
    // The classes whose code calls each method or constructor.
    let mut callers: FastMap<usize, FastSet<usize>> = FastMap::default();
    let mut scan = |e: &Expr, from: usize| {
        visit_expr(e, &mut |x| match &x.kind {
            ExprKind::Call { resolved: Some(callee), .. }
            | ExprKind::MethodCall { resolved: Some(callee), .. }
            | ExprKind::New { ctor: Some(callee), .. }
            | ExprKind::ThisCall { resolved: Some(callee), .. }
            | ExprKind::SuperInit { resolved: Some(callee), .. } => {
                callers.entry(*callee).or_default().insert(from);
            }
            _ => {}
        })
    };
    for m in &class.methods {
        for stmt in &m.body {
            visit_stmt(stmt, &mut |e| scan(e, m.class), &mut |_| {});
        }
    }
    for (t, decl) in class.types.iter().enumerate() {
        for f in decl.fields.iter().chain(&decl.statics) {
            if let Some(init) = &f.init {
                scan(init, t);
            }
        }
    }
    // An enum whose constants are built by its static initialization (`lower/enum_tables.rs`):
    // its constants exist before it, so its instance methods initialize it too.
    let with_tables: FastSet<usize> = (0..class.methods.len())
        .filter(|&m| class.builds_constants(m))
        .map(|m| class.methods[m].class)
        .collect();
    let tables = |c: usize| with_tables.contains(&c);
    (0..class.methods.len())
        .filter(|&i| {
            let m = &class.methods[i];
            let init = std::iter::once(m.class)
                .chain(class.hierarchy.ancestors(m.class))
                .find(|c| eager.contains(c));
            if init.is_some_and(|c| early.contains(&c)) {
                return false;
            }
            if !m.is_static && !m.is_abstract && eager.contains(&m.class) && tables(m.class) {
                return true;
            }
            std::iter::once(m.class).chain(class.hierarchy.ancestors(m.class)).any(|c| eager.contains(&c))
                && (m.is_static || m.is_ctor)
                && m.name != "<clinit>"
                && !m.added
                && (m.is_main
                    || (Some(m.class) != main_class
                        && callers.get(&i).is_some_and(|from| from.iter().any(|&c| c != m.class))))
        })
        .collect()
}

/// The statics among `finals` that are frozen arrays (`Facts::frozen_statics`): one-dimensional
/// arrays that every assignment, and the initializer, gives a new array (`{..}`, `new T[] {..}`,
/// `new T[n]`), and that every expression of the program only indexes to read an element or
/// measures (`S[i]`, `S.length`). Nothing else can reach such an array, so nothing writes its
/// elements once its class's static initialization has made it.
pub fn frozen<'a>(class: &'a Class, finals: &FastSet<(usize, String)>) -> FastSet<(usize, String)> {
    let new_array = |e: &Expr| matches!(e.kind, ExprKind::NewArray(_) | ExprKind::ArrayLit(_));
    let mut frozen: FastSet<(usize, String)> = class
        .types
        .iter()
        .enumerate()
        .flat_map(|(t, decl)| decl.statics.iter().map(move |f| (t, f)))
        .filter(|(t, f)| {
            matches!(f.ty, Type::Array(_))
                && !matches!(f.ty.element(), Type::Array(_))
                && f.init.as_ref().is_none_or(new_array)
                && finals.contains(&(*t, f.name.clone()))
        })
        .map(|(t, f)| (t, f.name.clone()))
        .collect();
    if frozen.is_empty() {
        return frozen;
    }
    // Each static's mentions as an expression, and those that are `S[i]` read or `S.length`.
    let mut mentions: FastMap<(usize, &'a str), (usize, usize)> = FastMap::default();
    let mut assigned_other: FastSet<(usize, String)> = FastSet::default();
    let mut visit = |x: &'a Expr| match &x.kind {
        ExprKind::Static { class: c, name } => mentions.entry((*c, name.as_str())).or_default().0 += 1,
        ExprKind::Index(a, _) | ExprKind::Length(a) => {
            if let ExprKind::Static { class: c, name } = &a.kind {
                mentions.entry((*c, name.as_str())).or_default().1 += 1;
            }
        }
        ExprKind::Assign { target: Place::Static { class: c, name }, op, value }
            if op.is_some() || !new_array(value) =>
        {
            assigned_other.insert((*c, name.clone()));
        }
        _ => {}
    };
    for m in &class.methods {
        for stmt in &m.body {
            visit_stmt_exprs(stmt, &mut visit);
        }
    }
    for decl in &class.types {
        for f in decl.fields.iter().chain(&decl.statics) {
            if let Some(init) = &f.init {
                visit_expr(init, &mut visit);
            }
        }
    }
    frozen.retain(|(t, name)| {
        !assigned_other.contains(&(*t, name.clone()))
            && mentions.get(&(*t, name.as_str())).is_none_or(|(all, read)| all == read)
    });
    frozen
}
