//! `synchronized (x) { ... }`. A program has one thread, so the lock does nothing: the statement
//! evaluates `x` once and throws a `NullPointerException` when it is null, then the block runs.

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
