//! The unboxing of a `Boolean` condition: `if`, `while`, `do`, `for` and `?:` test a `boolean`, so
//! a null `Boolean` there throws a `NullPointerException`, which a `catch` around the statement
//! may see (`Gen::condition` unboxes it in the checked form).

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
