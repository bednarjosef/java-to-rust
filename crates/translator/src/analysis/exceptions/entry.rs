//! The start of a static method or constructor that is the first use of its class: it runs the
//! class's `<clinit>` before its body (see `codegen.rs` and `analysis/statics.rs`), so what that
//! raises is raised by the method, as the first point of its body.

use super::*;

/// What the entry point raises, in words.
pub(super) const START: &str = "the start of this method";

/// The point that runs the `<clinit>` of the class that method `m` starts by initializing, when
/// it has one.
pub(super) fn entry_point(class: &Class, facts: &Facts, m: usize) -> Option<Point> {
    if !facts.entry_init.contains(&m) {
        return None;
    }
    let method = &class.methods[m];
    let init = facts.init_class(class, method.class)?;
    let clinit = class.methods.iter().position(|c| c.class == init && c.name == "<clinit>")?;
    Some(Point {
        raise: Raise::Call(vec![clinit]),
        pos: method.pos,
        what: START,
        frames: Vec::new(),
        known: Vec::new(),
    })
}
