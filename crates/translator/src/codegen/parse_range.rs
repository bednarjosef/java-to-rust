//! The `CharSequence` forms of the parse methods, `parseInt(s, begin, end, radix)` and the like
//! (see `lower/strings.rs` and `lower/unsigned.rs`).
//! The string is an `Option`, so that null is the runtime's `NullPointerException`, as in Java.

use super::*;

impl<'a> Gen<'a> {
    /// An `int` argument of a parse method, as Rust text.
    fn int(&mut self, e: &'a Expr) -> String {
        self.value(e, Type::Int).text
    }

    /// The string argument of a parse method, as the runtime's `Option<&str>`.
    fn parse_text(&mut self, text: &'a Expr) -> String {
        if self.nullable(text) { self.option_ref(text) } else { format!("Some({})", self.str_ref(text)) }
    }

    /// `Integer.parseInt(s, begin, end, radix)` and `Long.parseLong`, see `BitsOp::ParseRange`.
    pub(super) fn parse_range_builtin(&mut self, kind: Type, args: &'a [Expr], id: ExprId) -> Code {
        let text = self.parse_text(&args[0]);
        let begin = self.int(&args[1]);
        let end = self.int(&args[2]);
        let radix = self.int(&args[3]);
        let bits = if kind == Type::Long { 64 } else { 32 };
        let generics = format!("::<{}>", rust_type(kind));
        let arguments = format!("{text}, {begin}, {end}, {radix}, {bits}");
        // A null string that a `catch` sees is a dereference of `Facts::null_checked`: the checked
        // form raises its `NullPointerException`, even where no `catch` sees the other failures.
        if self.facts.null_checked.contains_key(&id) {
            self.dereferenced.insert(id);
            let call = format!("{}{generics}({arguments})", self.runtime("try_parse_range"));
            return Code::atom(self.passed_on(Code::atom(call)));
        }
        self.may_fail(id, "parse_range", &generics, &arguments)
    }

    /// `Integer.parseUnsignedInt(s)`, with or without a radix, and the range form, see
    /// `UnsignedOp::Parse` and `UnsignedOp::ParseRange`. The `u64` of the runtime is cast to the
    /// Java type, so a value beyond the signed range wraps as it does in Java.
    pub(super) fn parse_unsigned_builtin(&mut self, kind: Type, args: &'a [Expr], id: ExprId) -> Code {
        let text = self.parse_text(&args[0]);
        let bits = if kind == Type::Long { 64 } else { 32 };
        let call = if args.len() == 4 {
            let begin = self.int(&args[1]);
            let end = self.int(&args[2]);
            let radix = self.int(&args[3]);
            self.may_fail(id, "parse_unsigned_range", "", &format!("{text}, {begin}, {end}, {radix}, {bits}"))
        } else {
            let radix = match args.get(1) {
                Some(radix) => self.int(radix),
                None => "10".to_string(),
            };
            self.may_fail(id, "parse_unsigned", "", &format!("{text}, {radix}, {bits}"))
        };
        Code::new(format!("{} as {}", call.cast_operand(), rust_type(kind)), AS)
    }
}
