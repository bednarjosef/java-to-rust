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

/// Whether evaluating `e` has no effect and cannot throw. A static of class `index` may be read
/// only if it is a constant or was assigned before, so that a lazy cell never needs another one
/// that is still being computed.
fn pure(e: &Expr, index: usize, assigned: &FastSet<&str>, decl: &TypeDecl) -> bool {
    let recur = |x: &Expr| pure(x, index, assigned, decl);
    match &e.kind {
        ExprKind::Int(_)
        | ExprKind::Long(_)
        | ExprKind::Double(_)
        | ExprKind::Char(_)
        | ExprKind::Float(_)
        | ExprKind::Bool(_)
        | ExprKind::Str(_)
        | ExprKind::Units(_) => true,
        ExprKind::Static { class, name } => {
            *class == index
                && (assigned.contains(name.as_str())
                    || decl.statics.iter().any(|f| f.name == *name && f.constant))
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
/// cannot run before that has started, so it skips the check. A method that lowering adds for an
/// expression (`Method::added`) never runs it: it is the code of its call sites.
pub fn entry_inits(class: &Class, eager: &FastSet<usize>) -> FastSet<usize> {
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
