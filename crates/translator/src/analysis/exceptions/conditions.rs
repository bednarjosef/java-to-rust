//! The unboxings that statements make themselves (`analysis::unboxing`): of a `Boolean`
//! condition, as `if`, `while`, `do`, `for` and `?:` test a `boolean`, and of the value that a
//! statement returns or initializes a primitive local with. A null box there throws a
//! `NullPointerException`, which a `catch` around the statement may see (`Gen::condition` and
//! `Gen::boxing_value` unbox it in the checked form).

use super::*;

/// The point where the condition `cond` is unboxed, if it is a `Boolean` that may be null.
pub(super) fn condition_points(
    facts: &Facts,
    m: usize,
    cond: &Expr,
    frames: &[Frame],
    safe: &[String],
    out: &mut Vec<Point>,
) {
    if matches!(cond.ty, Type::Boxed(_))
        && cond.ty.unboxed() == Type::Boolean
        && !is_tested(cond, safe)
        && maybe_null(facts, m, cond)
    {
        let raise = Raise::Runtime { class: "java.lang.NullPointerException", checked: true, id: cond.id };
        out.push(Point {
            raise,
            pos: cond.pos,
            what: "this unboxing",
            frames: frames.to_vec(),
            known: Vec::new(),
        });
    }
}

/// The point where statement `s` of method `m` unboxes the value it returns or initializes a
/// local with, if that is a box that may be null.
pub(super) fn statement_unboxing_points(
    class: &Class,
    facts: &Facts,
    m: usize,
    s: &Stmt,
    frames: &[Frame],
    safe: &[String],
    out: &mut Vec<Point>,
) {
    let Some(method) = class.methods.get(m) else { return };
    if let Some(value) = crate::analysis::unboxing::unboxed_by_stmt(method, s)
        && !is_tested(value, safe)
        && maybe_null(facts, m, value)
    {
        let raise = Raise::Runtime { class: "java.lang.NullPointerException", checked: true, id: value.id };
        out.push(Point {
            raise,
            pos: value.pos,
            what: "this unboxing",
            frames: frames.to_vec(),
            known: Vec::new(),
        });
    }
}
