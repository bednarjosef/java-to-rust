//! The bounds of a counting loop written as `for i in a..b`. Java reads each bound once, before
//! the loop. Rust keeps the temporaries of a loop's iterator expression alive for the whole loop,
//! so a bound that reads through a borrow of a shared object (`this.borrow().nextreg`) would hold
//! that borrow while the body borrows the object again, which panics. Such a bound is bound to a
//! local before the loop, which ends the borrow as Java's read does.

use super::*;

/// Whether generated text reads through a borrow of a shared object, whose guard would outlive
/// the read: `.borrow()` and `.borrow_mut()` of a `RefCell` or `java::Locked`, or the `.base()`
/// that a value of an enum borrows its fields with.
fn reads_through_borrow(text: &str) -> bool {
    [".borrow()", ".borrow_mut()", ".base()", ".base_mut()"].iter().any(|b| text.contains(b))
}

impl<'a> Gen<'a> {
    /// A bound of a range loop: written as it is, or, where it reads through a borrow, as a local
    /// `let name = ..;` written before the loop, which the loop then uses. The caller writes the
    /// bounds in Java's order, so the start is read before the end.
    pub(super) fn range_bound(&mut self, bound: Code, name: &str) -> Code {
        if !reads_through_borrow(&bound.text) {
            return bound;
        }
        let local = self.temp(name);
        self.line(&format!("let {local} = {};", bound.at(LOWEST)));
        Code::atom(local)
    }
}
