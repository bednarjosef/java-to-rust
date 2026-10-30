//! Calls of the field initializers of an object, `field_inits`. It returns a `Result` when one of
//! the initializers raises (see `analysis::exceptions::field_inits`); the constructor that runs it
//! then passes the failure on, as it does for any call that may fail.

use super::*;

impl<'a> Gen<'a> {
    /// Whether `field_inits` of class `t` returns a `Result`.
    pub(super) fn inits_throw(&self, t: usize) -> bool {
        self.facts.init_throwing.contains(&t)
    }

    /// The statement that runs the field initializers of class `t`, which is the call `call`.
    pub(super) fn run_inits(&mut self, t: usize, call: String) -> String {
        if !self.inits_throw(t) {
            return format!("{call};");
        }
        format!("{};", self.passed_on_thrown(Code::atom(call)))
    }
}
