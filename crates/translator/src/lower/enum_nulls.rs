//! `null` for enums. A local of an enum type that is assigned `null` is an `Option` of the enum, as
//! a nullable string is an `Option<String>` (see `analysis::nullness`). Such a local is only set,
//! tested against `null`, printed or concatenated, which have a `None` case in the generated code.
//! Any other use would give an enum value a `None`, so it is rejected, and so is `null` given to an
//! enum anywhere else.

use super::*;
use std::cell::{Cell, RefCell};
use std::collections::HashSet;

/// The first position in method `m` where `null` is given to an enum outside the uses above, or a
/// nullable enum local is used outside them.
pub(super) fn escape(class: &Class, m: usize) -> Option<Pos> {
    let method = &class.methods[m];
    // The enum locals that are assigned `null`, with the positions of those assignments.
    let assigned = RefCell::new(Vec::new());
    walk(
        method,
        &mut |x| {
            if let ExprKind::Assign { op: None, target: Place::Var(name), value } = &x.kind
                && matches!(value.kind, ExprKind::Null)
                && matches!(x.ty, Type::Enum(_))
            {
                assigned.borrow_mut().push((name.clone(), x.pos));
            }
        },
        &mut |s| {
            if let Stmt::Local { ty: Type::Enum(_), name, init: Some(init), .. } = s
                && matches!(init.kind, ExprKind::Null)
            {
                assigned.borrow_mut().push((name.clone(), init.pos));
            }
        },
    );
    let assigned = assigned.into_inner();
    // A parameter is passed by its caller as a value, so it cannot be a `None`.
    if let Some(&(_, pos)) = assigned.iter().find(|(n, _)| method.params.iter().any(|p| &p.name == n)) {
        return Some(pos);
    }
    let names: HashSet<&str> = assigned.iter().map(|(n, _)| n.as_str()).collect();
    let is_local = |e: &Expr| matches!(&e.kind, ExprKind::Var(n) if names.contains(n.as_str()) && matches!(e.ty, Type::Enum(_)));
    let is_null = |e: &Expr| matches!(e.kind, ExprKind::Null);
    // The uses that are allowed: the `null` of an assignment or declaration, the operands of a
    // null test, of a string concatenation and of `print`, and the local in those last three.
    let allowed = RefCell::new(HashSet::new());
    let allow = |e: &Expr| {
        allowed.borrow_mut().insert(e as *const Expr);
    };
    walk(
        method,
        &mut |x| match &x.kind {
            ExprKind::Assign { op: None, target: Place::Var(_), value } if is_null(value) => allow(value),
            ExprKind::Binary(BinOp::Eq | BinOp::Ne, a, b)
                if (is_local(a) && is_null(b)) || (is_null(a) && is_local(b)) =>
            {
                allow(a);
                allow(b);
            }
            ExprKind::Binary(BinOp::Add, a, b) if x.ty == Type::String => {
                for operand in [a, b] {
                    if is_local(operand) {
                        allow(operand);
                    }
                }
            }
            ExprKind::Builtin(Builtin::Print | Builtin::Println, args) => {
                for arg in args.iter().filter(|a| is_local(a)) {
                    allow(arg);
                }
            }
            _ => {}
        },
        &mut |s| {
            if let Stmt::Local { ty: Type::Enum(_), init: Some(init), .. } = s
                && is_null(init)
            {
                allow(init);
            }
        },
    );
    let allowed = allowed.into_inner();
    let found: Cell<Option<Pos>> = Cell::new(None);
    let not_allowed = |e: &Expr| !allowed.contains(&(e as *const Expr));
    walk(
        method,
        &mut |x| {
            let escapes = (is_local(x) && not_allowed(x)) || null_enum_in(x, class, &allowed);
            if escapes && found.get().is_none() {
                found.set(Some(x.pos));
            }
        },
        &mut |s| {
            if let Stmt::Return(Some(e), _) = s
                && null_for(e, method.ret, &allowed)
                && found.get().is_none()
            {
                found.set(Some(e.pos));
            }
        },
    );
    found.get()
}

/// Calls `on_expr` on every expression and `on_stmt` on every statement of `method`.
fn walk(method: &Method, on_expr: &mut dyn FnMut(&Expr), on_stmt: &mut dyn FnMut(&Stmt)) {
    for stmt in &method.body {
        visit_stmt(stmt, &mut |e| visit_expr(e, on_expr), on_stmt);
    }
}

/// Whether `value` is the `null` literal given to an enum, and not one of the allowed uses.
fn null_for(value: &Expr, ty: Type, allowed: &HashSet<*const Expr>) -> bool {
    matches!(value.kind, ExprKind::Null)
        && matches!(ty, Type::Enum(_))
        && !allowed.contains(&(value as *const Expr))
}

/// Whether an expression gives `null` to an enum: in an assignment, a conditional, a cast, an
/// array, or an argument of a call.
fn null_enum_in(e: &Expr, class: &Class, allowed: &HashSet<*const Expr>) -> bool {
    match &e.kind {
        ExprKind::Binary(BinOp::Eq | BinOp::Ne, a, b) => {
            null_for(a, b.ty, allowed) || null_for(b, a.ty, allowed)
        }
        ExprKind::Assign { value, .. } => null_for(value, e.ty, allowed),
        ExprKind::Conditional(_, a, b) => {
            matches!(e.ty, Type::Enum(_))
                && (matches!(a.kind, ExprKind::Null) || matches!(b.kind, ExprKind::Null))
        }
        ExprKind::Cast(ty, x) => null_for(x, *ty, allowed),
        ExprKind::ArrayLit(elements) => {
            matches!(e.ty, Type::Array(el) if matches!(el, Type::Enum(_)))
                && elements.iter().any(|x| matches!(x.kind, ExprKind::Null))
        }
        ExprKind::NewArray(_) => matches!(e.ty, Type::Array(el) if matches!(el, Type::Enum(_))),
        ExprKind::Call { args, resolved: Some(r), .. }
        | ExprKind::MethodCall { args, resolved: Some(r), .. }
        | ExprKind::New { args, ctor: Some(r), .. }
        | ExprKind::ThisCall { args, resolved: Some(r) } => {
            args.iter().zip(&class.methods[*r].params).any(|(a, p)| null_for(a, p.ty, allowed))
        }
        _ => false,
    }
}
