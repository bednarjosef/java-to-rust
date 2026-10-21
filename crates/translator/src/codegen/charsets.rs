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
            _ => self.expr(e).at(ATOM),
        }
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
                let recv = self.expr(&args[0]).at(ATOM);
                Code::atom(format!("{recv}.name().to_string()"))
            }
            CharsetOp::DisplayName => {
                let recv = self.expr(&args[0]).at(ATOM);
                Code::atom(format!("{recv}.display_name().to_string()"))
            }
            CharsetOp::ToString => {
                let recv = self.expr(&args[0]).at(ATOM);
                Code::atom(format!("{recv}.to_string()"))
            }
            CharsetOp::HashCode => {
                let recv = self.expr(&args[0]).at(ATOM);
                Code::atom(format!("{recv}.hash_code()"))
            }
            CharsetOp::Equals => {
                let left = self.expr(&args[0]).at(ATOM);
                let right = self.expr(&args[1]).at(ATOM);
                infix("==", CMP, &Code::atom(left), &Code::atom(right))
            }
        }
    }
}
