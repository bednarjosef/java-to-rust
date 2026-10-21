//! `synchronized (x) { ... }`. The lock does nothing yet, which is right for a program with one
//! thread (see docs/multi-threading.md, section 3, for monitors): the statement evaluates `x` once
//! and throws a `NullPointerException` when it is null, then the block runs.

use super::*;

impl<'a> Gen<'a> {
    /// Evaluates a lock and drops it, throwing if it is null.
    pub(super) fn null_check(&mut self, lock: &'a Expr) -> Code {
        if self.nullable(lock) {
            let option = self.option_ref(lock);
            return Code::atom(format!("{}({option})", self.runtime("non_null")));
        }
        let value = self.expr(lock).text;
        Code::atom(format!("{{ let _ = {value}; }}"))
    }
}
