//! `Integer.parseInt(s, begin, end, radix)` and `Long.parseLong(s, begin, end, radix)` on a
//! `String`: a null string is a `NullPointerException` in Java, which a `catch` sees as it sees the
//! other dereferences of a null object (`Gen::dereference`).

use super::*;

impl<'a> Gen<'a> {
    /// The parse of a substring of `kind`, which throws what `try_parse_range` reports.
    pub(super) fn parse_range(&mut self, kind: Type, args: &'a [Expr], id: ExprId) -> Code {
        let text = if self.nullable(&args[0]) {
            let option = self.option_ref(&args[0]);
            self.dereference(&[id, args[0].id], &option)
        } else {
            self.str_ref(&args[0])
        };
        let begin = self.value(&args[1], Type::Int).text;
        let end = self.value(&args[2], Type::Int).text;
        let radix = self.value(&args[3], Type::Int).text;
        let arguments = format!("{text}, {begin}, {end}, {radix}");
        self.may_fail(id, "parse_range", &format!("::<{}>", rust_type(kind)), &arguments)
    }
}
