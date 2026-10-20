//! Whether a class's static initialization must run eagerly. Java runs it once, on first use of the
//! class, and anything observable in it (a static block, a call, a `new`, a throw) must happen at
//! that point. When the initializers only compute values from literals and from statics of the
//! same class that were assigned earlier, nothing is observable: each static can be computed on
//! its first access, as a lazily initialized cell, and no `init` function is needed.

use crate::ast::*;
use std::collections::HashSet;

/// Whether class `index` needs an `init` function: it has a `<clinit>` that is not lazy.
pub fn eager(class: &Class, index: usize) -> bool {
    let Some(clinit) = class.methods.iter().find(|m| m.class == index && m.name == "<clinit>") else {
        return false;
    };
    let decl = &class.types[index];
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
        assigned.insert(name.as_str());
        ok
    })
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
        ExprKind::Unary(_, x) | ExprKind::Cast(_, x) => recur(x),
        // Integer division can divide by zero, which throws.
        ExprKind::Binary(op, l, r) => !matches!(op, BinOp::Div | BinOp::Rem) && recur(l) && recur(r),
        ExprKind::Conditional(c, a, b) => recur(c) && recur(a) && recur(b),
        _ => false,
    }
}
