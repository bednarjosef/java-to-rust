//! The counting loops that become Rust ranges. `for (int i = a; i < b; i++)` is `for i in a..b`;
//! the forms beside it are `a..=b` for `<=`, `(a..b).step_by(k)` for `i += k`, and `(a..b).rev()`
//! for `for (int i = b - 1; i >= a; i--)`. A form is taken only when the range counts the same
//! values as the loop, and Java's `i` never wraps around (which would keep the Java loop running).

use super::{BinOp, Builtin, Expr, ExprKind, Place, Stmt, Type, assigns_stmt, stable};

/// A counting loop over `from..to` (or `from..=to`) in steps of `step`. A descending loop
/// `for (i = to - 1; i >= from; i--)` has `descending` set and counts down over `from..to`.
pub struct Counting<'e> {
    pub var: &'e str,
    /// The type of the loop variable, which the range has.
    pub ty: Type,
    pub from: &'e Expr,
    pub to: &'e Expr,
    pub inclusive: bool,
    /// Positive for a loop that counts up, `-1` for a descending one.
    pub step: i64,
    pub descending: bool,
}

/// Recognizes `for (int i = a; i < b; i++)` (or `long i`) where the body never assigns `i` and `b`
/// cannot change during the loop, which is exactly `for i in a..b`.
pub fn counting_loop<'e>(
    init: &'e [Stmt],
    cond: Option<&'e Expr>,
    update: &'e [Expr],
    body: &'e Stmt,
) -> Option<(&'e str, &'e Expr, &'e Expr)> {
    let c = counting(init, cond, update, body)?;
    (c.step == 1 && !c.inclusive).then_some((c.var, c.from, c.to))
}

/// Recognizes any counting loop: `i < b` or `i <= b` with `i += k` for a constant `k >= 1`, or
/// `i >= a` with `i--` and the start `b - 1`. The bound `b` (and `a`) must be stable in the body,
/// and `i` must not be assigned there. The bound has `i`'s type, or is an `int` constant in a
/// `long` loop (`i <= 5`), which Java widens.
pub fn counting<'e>(
    init: &'e [Stmt],
    cond: Option<&'e Expr>,
    update: &'e [Expr],
    body: &'e Stmt,
) -> Option<Counting<'e>> {
    let [Stmt::Local { ty, name, init: Some(start), .. }] = init else { return None };
    if !ty.is_integral() {
        return None;
    }
    let ExprKind::Binary(op, var, bound) = &cond?.kind else { return None };
    let widened = *ty == Type::Long && bound.ty == Type::Int && constant(bound).is_some();
    if !matches!(&var.kind, ExprKind::Var(v) if v == name) || (bound.ty != *ty && !widened) {
        return None;
    }
    let step = step_of(update, name)?;
    if assigns_stmt(body, name) || !stable(bound, name, body) {
        return None;
    }
    let counted = if step > 0 {
        let inclusive = match op {
            BinOp::Lt => false,
            BinOp::Le => true,
            _ => return None,
        };
        Counting { var: name, ty: *ty, from: start, to: bound, inclusive, step, descending: false }
    } else {
        // `start` is `b - 1`, and the loop counts down from it to `a`, the bound.
        let ExprKind::Binary(BinOp::Sub, to, one) = &start.kind else { return None };
        if *op != BinOp::Ge || !is_one(one) || start.ty != *ty || to.ty != *ty || !stable(to, name, body) {
            return None;
        }
        Counting { var: name, ty: *ty, from: bound, to, inclusive: false, step: -1, descending: true }
    };
    no_wrap(&counted, *ty).then_some(counted)
}

/// The step of a loop whose only update is `i++`, `i--`, `i += k` (`k >= 1`) or `i -= 1`.
fn step_of(update: &[Expr], name: &str) -> Option<i64> {
    let [Expr { kind, .. }] = update else { return None };
    let (target, step) = match kind {
        ExprKind::IncDec { target: Place::Var(target), increment, .. } => {
            (target, if *increment { 1 } else { -1 })
        }
        ExprKind::Assign { op: Some(BinOp::Add), target: Place::Var(target), value } => match value.kind {
            ExprKind::Int(k) if k >= 1 => (target, i64::from(k)),
            ExprKind::Long(k) if k >= 1 => (target, k),
            _ => return None,
        },
        ExprKind::Assign { op: Some(BinOp::Sub), target: Place::Var(target), value } if is_one(value) => {
            (target, -1)
        }
        _ => return None,
    };
    (target == name).then_some(step)
}

fn is_one(e: &Expr) -> bool {
    matches!(e.kind, ExprKind::Int(1) | ExprKind::Long(1))
}

/// Whether Java's `i` stays inside its type while the loop runs. Counting up, the last value
/// `i` takes in the body plus the step must not exceed the maximum; with `i < b` and step 1 that
/// is always so. Counting down, `b - 1` and the last `i--` must not go below the minimum.
fn no_wrap(c: &Counting, ty: Type) -> bool {
    let (min, max) = limits(ty);
    if c.descending {
        return smallest(c.to).is_some_and(|b| b > min) && smallest(c.from).is_some_and(|a| a > min);
    }
    if !c.inclusive && c.step == 1 {
        return true;
    }
    let Some(most) = largest(c.to) else { return false };
    let last = if c.inclusive { most } else { most - 1 };
    last + i128::from(c.step) <= max
}

/// The smallest and largest value of a type, as `i128`.
fn limits(ty: Type) -> (i128, i128) {
    match ty {
        Type::Byte => (i128::from(i8::MIN), i128::from(i8::MAX)),
        Type::Short => (i128::from(i16::MIN), i128::from(i16::MAX)),
        Type::Char => (0, i128::from(u16::MAX)),
        Type::Long => (i128::from(i64::MIN), i128::from(i64::MAX)),
        _ => (i128::from(i32::MIN), i128::from(i32::MAX)),
    }
}

/// The largest value `e` can take, when it is known: a constant, or a length. No array or string
/// is that long: the JVM cannot allocate one of `Integer.MAX_VALUE - 2` elements or more.
fn largest(e: &Expr) -> Option<i128> {
    match &e.kind {
        ExprKind::Length(_) | ExprKind::Builtin(Builtin::StringLength, _) => Some(i128::from(i32::MAX) - 2),
        _ => constant(e),
    }
}

/// The smallest value `e` can take, when it is known: a constant, or a length (never negative).
fn smallest(e: &Expr) -> Option<i128> {
    match &e.kind {
        ExprKind::Length(_) | ExprKind::Builtin(Builtin::StringLength, _) => Some(0),
        _ => constant(e),
    }
}

/// The value of a constant expression: a literal, or `+` and `-` of constants that javac did not
/// fold (`Integer.MAX_VALUE - 1`). A sum outside the type would wrap in Java, so it is not known.
fn constant(e: &Expr) -> Option<i128> {
    match &e.kind {
        ExprKind::Int(v) => Some(i128::from(*v)),
        ExprKind::Long(v) => Some(i128::from(*v)),
        ExprKind::Char(v) => Some(i128::from(*v)),
        ExprKind::Binary(op @ (BinOp::Add | BinOp::Sub), a, b) => {
            let (a, b) = (constant(a)?, constant(b)?);
            let value = if *op == BinOp::Add { a + b } else { a - b };
            let (min, max) = limits(e.ty);
            (min..=max).contains(&value).then_some(value)
        }
        _ => None,
    }
}
