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
    // An instance method of an enum in table form starts by initializing it too (`statics.rs`),
    // but it runs on a constant that an earlier point got, which initialized the enum there
    // (`static_inits` of `EnumConstant`); so the call returns at once, as Java runs an instance
    // method without initializing its class.
    if !method.is_static && !method.is_ctor && !class.builds_constants(m) {
        return None;
    }
    let init = facts.init_class(class, method.class)?;
    let clinit =
        (class.hierarchy.methods_of(init).iter().copied()).find(|&c| class.methods[c].name == "<clinit>")?;
    Some(Point {
        raise: Raise::Call(super::shared_callees(vec![clinit])),
        pos: method.pos,
        what: START,
        frames: Vec::new(),
        known: Vec::new(),
    })
}
