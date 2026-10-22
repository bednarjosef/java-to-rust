//! Charsets, as the runtime's `java::Charset` values: the `StandardCharsets` constants, the
//! methods of a charset value and `Charset.forName`, and the charset argument of `getBytes` and
//! `new String`, which may also be a name.

use super::*;

impl<'a> Gen<'a> {
    /// The `java::Charset` a charset argument names: a `Type::Charset` expression, or a
    /// `Type::String` naming one (which throws `UnsupportedEncodingException` when unknown).
    pub(super) fn charset_arg(&mut self, e: &'a Expr) -> String {
        match e.ty {
            Type::String => {
                let name = self.text_ref(e);
                format!("{}::for_encoding_name({name})", self.runtime("Charset"))
            }
            _ => self.charset(e),
        }
    }

    /// A charset that a method or an operation of the library uses: one that may be null throws
    /// `NullPointerException` (in the checked form where a `catch` or a `finally` sees it).
    fn charset(&mut self, e: &'a Expr) -> String {
        if !self.nullable(e) {
            return self.expr(e).at(ATOM);
        }
        let option = self.expr(e).at(ATOM);
        self.dereference(&[e.id], &option)
    }

    /// A charset that may be null as an `Option<java::Charset>`.
    fn charset_option(&mut self, e: &'a Expr) -> String {
        match &e.kind {
            ExprKind::Null => format!("None::<{}>", self.runtime("Charset")),
            _ if self.nullable(e) => self.expr(e).at(ATOM),
            _ => format!("Some({})", self.expr(e).text),
        }
    }

    /// `==` or `!=` on two charsets, or a charset and `null`: the same charset, or both null.
    pub(super) fn charset_identity(&mut self, op: BinOp, l: &'a Expr, r: &'a Expr) -> Code {
        let symbol = if op == BinOp::Ne { "!=" } else { "==" };
        let optional = |g: &Self, e: &Expr| g.nullable(e) || matches!(e.kind, ExprKind::Null);
        if optional(self, l) || optional(self, r) {
            let (a, b) = (self.charset_option(l), self.charset_option(r));
            return infix(symbol, CMP, &Code::atom(a), &Code::atom(b));
        }
        let (a, b) = (self.expr(l), self.expr(r));
        infix(symbol, CMP, &a, &b)
    }

    /// A string that the runtime takes as an `Option<&str>`, so that a null is its exception.
    fn name_option(&mut self, e: &'a Expr) -> String {
        if self.nullable(e) { self.option_ref(e) } else { format!("Some({})", self.str_ref(e)) }
    }

    pub(super) fn charset_builtin(&mut self, op: CharsetOp, args: &'a [Expr]) -> Code {
        let charset = self.runtime("Charset");
        match op {
            CharsetOp::Utf8 => Code::atom(format!("{charset}::Utf8")),
            CharsetOp::UsAscii => Code::atom(format!("{charset}::UsAscii")),
            CharsetOp::Latin1 => Code::atom(format!("{charset}::Latin1")),
            CharsetOp::Utf16 => Code::atom(format!("{charset}::Utf16")),
            CharsetOp::Utf16Be => Code::atom(format!("{charset}::Utf16Be")),
            CharsetOp::Utf16Le => Code::atom(format!("{charset}::Utf16Le")),
            CharsetOp::ForName => {
                let name = self.name_option(&args[0]);
                Code::atom(format!("{charset}::for_name({name})"))
            }
            CharsetOp::IsSupported => {
                let name = self.name_option(&args[0]);
                Code::atom(format!("{charset}::is_supported({name})"))
            }
            CharsetOp::DefaultCharset => Code::atom(format!("{charset}::default_charset()")),
            CharsetOp::Name => {
                let recv = self.charset(&args[0]);
                Code::atom(format!("{}::from({recv}.name())", self.runtime("JString")))
            }
            CharsetOp::DisplayName => {
                let recv = self.charset(&args[0]);
                Code::atom(format!("{}::from({recv}.display_name())", self.runtime("JString")))
            }
            CharsetOp::ToString => {
                let recv = self.charset(&args[0]);
                Code::atom(format!("{}::from({recv}.to_string())", self.runtime("JString")))
            }
            CharsetOp::HashCode => {
                let recv = self.charset(&args[0]);
                Code::atom(format!("{recv}.hash_code()"))
            }
            // `equals(null)` is false.
            CharsetOp::Equals if self.nullable(&args[1]) || matches!(args[1].kind, ExprKind::Null) => {
                let left = self.charset(&args[0]);
                let right = self.charset_option(&args[1]);
                infix("==", CMP, &Code::atom(format!("Some({left})")), &Code::atom(right))
            }
            CharsetOp::Equals => {
                let left = self.charset(&args[0]);
                let right = self.expr(&args[1]).at(ATOM);
                infix("==", CMP, &Code::atom(left), &Code::atom(right))
            }
        }
    }
}
