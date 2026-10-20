//! The `Character` methods that take a radix: `digit(char, int)` and `forDigit(int, int)`.

use super::*;

impl<'a> Gen<'a> {
    /// `Character.digit(c, radix)`, which takes a `char`, and `Character.forDigit(digit, radix)`,
    /// which takes `int`s; see `java::digit` and `java::for_digit`.
    pub(super) fn character_radix(&mut self, builtin: Builtin, args: &'a [Expr]) -> Code {
        let (name, first) = match builtin {
            Builtin::CharDigit => ("digit", Type::Char),
            _ => ("for_digit", Type::Int),
        };
        let c = self.value(&args[0], first);
        let radix = self.value(&args[1], Type::Int);
        Code::atom(format!("{}({}, {})", self.runtime(name), c.text, radix.text))
    }
}
