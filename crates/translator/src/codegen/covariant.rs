//! Covariant returns: an override that returns a subtype of what the method it overrides returns.
//! A dispatcher returns the value of the overridden method's type, so the value an override
//! returns is converted to that representation: an object of a class is wrapped in its variant,
//! and a subtype's enum converts with `to_<supertype>()`.

use super::*;
use crate::analysis::hierarchy::Rep;

impl<'a> Gen<'a> {
    /// The representation of the class type that method `index` returns, if it returns one.
    fn ret_rep(&self, index: usize) -> Option<Rep> {
        let ty = self.class.methods[index].ret;
        self.class.class_index(ty).map(|t| self.rep(t))
    }

    /// Whether methods `a` and `b` return values of one representation, so that a value that one
    /// returns is a value the other returns too.
    pub(super) fn same_ret(&self, a: usize, b: usize) -> bool {
        self.ret_rep(a) == self.ret_rep(b)
            && self.facts.nullable_returns.contains(&a) == self.facts.nullable_returns.contains(&b)
    }

    /// `value`, returned by method `from`, as the value method `to` returns: the same text when
    /// both return one representation, else converted, and in `Some` when only `to` may be null.
    pub(super) fn covariant_ret(&mut self, from: usize, to: usize, value: String) -> String {
        if self.same_ret(from, to) {
            return value;
        }
        let (Some(src), Some(dst)) = (self.ret_rep(from), self.ret_rep(to)) else {
            return value;
        };
        match (self.facts.nullable_returns.contains(&from), self.facts.nullable_returns.contains(&to)) {
            (false, false) => self.convert_rep(src, dst, &value),
            (true, true) => {
                let inner = self.convert_rep(src, dst, "v");
                format!("{value}.map(|v| {inner})")
            }
            (false, true) => format!("Some({})", self.convert_rep(src, dst, &value)),
            // A method that may return null overridden by one that never does: the value is
            // already an `Option` where the dispatcher needs a plain value; left as it is.
            (true, false) => value,
        }
    }

    /// `x`, a value of representation `src`, as a value of representation `dst`.
    fn convert_rep(&mut self, src: Rep, dst: Rep, x: &str) -> String {
        match (src, dst) {
            (Rep::Object(c), Rep::Enum(t)) => self.variant(t, c, x),
            (Rep::Enum(s), Rep::Enum(t)) if s != t => {
                self.need(Need::Convert(s, t));
                format!("{x}.{}()", self.to_name(t))
            }
            _ => x.to_string(),
        }
    }
}
