//! The values that the program unboxes: a box where a primitive is wanted, which Java converts by
//! calling the box's method (`Integer.intValue()`), so that a null box throws a
//! `NullPointerException` there. The code writes some of these as a cast (`(int) o`), most not:
//! an operand of arithmetic or a comparison, a value assigned, returned or passed to a primitive,
//! an arm of a primitive `?:`, an index, a size, an element of an array of primitives, and a
//! boxed local that `++` or `+=` updates. The code generator unboxes each where it
//! converts the value (`Gen::boxing_value`), in the checked form where a `catch` may see the
//! exception (`exceptions`), whose message is Java's (`npe_messages`).

use super::*;

/// Whether `ty` is a primitive type that a box converts to.
fn primitive(ty: Type) -> bool {
    ty.is_numeric() || ty == Type::Boolean
}

/// Whether `e` is a box that is converted to the primitive type `to`.
fn unboxed_to(e: &Expr, to: Type) -> bool {
    matches!(e.ty, Type::Boxed(_)) && primitive(to)
}

/// The boxes that expression `x` unboxes itself (not those of the expressions inside it), in the
/// order Java unboxes them.
pub(crate) fn unboxed_by<'e>(class: &Class, x: &'e Expr) -> Vec<&'e Expr> {
    let mut out = Vec::new();
    let mut add = |e: &'e Expr, to: Type| {
        if unboxed_to(e, to) {
            out.push(e);
        }
    };
    // The arguments of a call, by the types of the parameters of the method it calls.
    let mut arguments = |args: &'e [Expr], callee: Option<usize>| {
        let Some(c) = callee else { return };
        let params = &class.methods[c].params;
        if params.len() == args.len() {
            for (a, p) in args.iter().zip(params) {
                add(a, p.ty);
            }
        }
    };
    match &x.kind {
        ExprKind::Cast(to, value) => add(value, *to),
        // Arithmetic and comparisons unbox their operands, but `==` and `!=` with an operand that
        // is no primitive (`v == null`) compare references, and `+` of a string makes text.
        ExprKind::Binary(op, l, r) if x.ty != Type::String => {
            for (operand, other) in [(l, r), (r, l)] {
                let reference = matches!(op, BinOp::Eq | BinOp::Ne) && !primitive(other.ty);
                if !reference {
                    add(operand, operand.ty.unboxed());
                }
            }
        }
        ExprKind::Unary(_, operand) => add(operand, operand.ty.unboxed()),
        ExprKind::Assign { value, .. } => add(value, x.ty),
        // An arm that its condition tests not to be null (`v != null ? v : 0`) is never null.
        ExprKind::Conditional(cond, a, b) => {
            for (arm, when) in [(a, true), (b, false)] {
                let narrowed =
                    matches!(&arm.kind, ExprKind::Var(v) if non_null_names(cond, when).contains(&v.as_str()));
                if !narrowed {
                    add(arm, x.ty);
                }
            }
        }
        ExprKind::Index(_, index) => add(index, Type::Int),
        ExprKind::NewArray(sizes) => sizes.iter().for_each(|s| add(s, Type::Int)),
        ExprKind::ArrayLit(items) => items.iter().for_each(|i| add(i, x.ty.element())),
        ExprKind::MethodCall { args, resolved, .. } | ExprKind::Call { args, resolved, .. } => {
            arguments(args, *resolved)
        }
        ExprKind::New { args, ctor, .. } => arguments(args, *ctor),
        _ => {}
    }
    out
}

/// The box that statement `s` of `method` unboxes itself: the value it returns or that it
/// initializes a primitive local with.
pub(crate) fn unboxed_by_stmt<'s>(method: &Method, s: &'s Stmt) -> Option<&'s Expr> {
    match s {
        Stmt::Return(Some(e), _) if unboxed_to(e, method.ret) => Some(e),
        Stmt::Local { ty, init: Some(e), .. } if unboxed_to(e, *ty) => Some(e),
        _ => None,
    }
}

/// The local that expression `x` unboxes as it updates it, `w++` or `w += 1` of a boxed `w`,
/// which it boxes again.
pub(crate) fn updated_box(x: &Expr) -> Option<&str> {
    match &x.kind {
        ExprKind::IncDec { target: Place::Var(v), .. }
        | ExprKind::Assign { op: Some(_), target: Place::Var(v), .. }
            if matches!(x.ty, Type::Boxed(_)) =>
        {
            Some(v)
        }
        _ => None,
    }
}

/// The action of the message of unboxing the null box `e`: `Cannot invoke
/// "java.lang.Integer.intValue()"`, by the method of its own box, which javac calls also for a
/// wider primitive.
pub(crate) fn unboxing_action(e: &Expr) -> String {
    format!("Cannot invoke \"java.lang.{}.{}Value()\"", e.ty, e.ty.unboxed())
}
