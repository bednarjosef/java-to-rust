//! Whether a class's static initialization must run eagerly. Java runs it once, on first use of the
//! class, and anything observable in it (a static block, a call, a `new` of an object, a throw)
//! must happen at that point. When the initializers only compute values, or new arrays, from
//! literals and from statics of the same class that were assigned earlier and that no other code
//! assigns, nothing is observable: each static can be computed on its first access, as a lazily
//! initialized cell, and no `init` function is needed.

use crate::ast::*;
use std::collections::{HashMap, HashSet};

/// Whether class `index` needs an `init` function: it has a `<clinit>` that is not lazy.
pub fn eager(class: &Class, index: usize) -> bool {
    let Some(clinit) = class.methods.iter().find(|m| m.class == index && m.name == "<clinit>") else {
        return false;
    };
    let decl = &class.types[index];
    let changed = changed_later(class, index);
    let mut assigned: HashSet<&str> = HashSet::new();
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

/// The statics of class `index` that code outside its static initialization assigns. A lazy cell
/// computed from one of them would see its value at the cell's first access, not the value Java's
/// initialization saw.
fn changed_later(class: &Class, index: usize) -> HashSet<String> {
    let mut changed = HashSet::new();
    for m in class.methods.iter().filter(|m| !(m.class == index && m.name == "<clinit>")) {
        for stmt in &m.body {
            visit_stmt(
                stmt,
                &mut |e| {
                    visit_expr(e, &mut |x| match &x.kind {
                        ExprKind::Assign { target: Place::Static { class: c, name }, .. }
                        | ExprKind::IncDec { target: Place::Static { class: c, name }, .. }
                            if *c == index =>
                        {
                            changed.insert(name.clone());
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
fn pure(e: &Expr, index: usize, assigned: &HashSet<&str>, decl: &TypeDecl) -> bool {
    let recur = |x: &Expr| pure(x, index, assigned, decl);
    match &e.kind {
        ExprKind::Int(_)
        | ExprKind::Long(_)
        | ExprKind::Double(_)
        | ExprKind::Char(_)
        | ExprKind::Float(_)
        | ExprKind::Bool(_)
        | ExprKind::Str(_) => true,
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
/// cannot run before that has started, so it skips the check.
pub fn entry_inits(class: &Class, eager: &HashSet<usize>) -> HashSet<usize> {
    let mains: Vec<&Method> = class.methods.iter().filter(|m| m.is_main).collect();
    let main_class = match mains[..] {
        [main] => Some(main.class),
        _ => None,
    };
    // The classes whose code calls each method or constructor.
    let mut callers: HashMap<usize, HashSet<usize>> = HashMap::new();
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
    let with_tables: HashSet<usize> = class
        .methods
        .iter()
        .filter(|m| m.is_static && m.name == "initConstant" && class.types[m.class].kind == TypeKind::Enum)
        .map(|m| m.class)
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
                && (m.is_main
                    || (Some(m.class) != main_class
                        && callers.get(&i).is_some_and(|from| from.iter().any(|&c| c != m.class))))
        })
        .collect()
}
