//! Charsets, as the runtime's `java::Charset` values: the `StandardCharsets` constants and
//! `Charset.forName`, and the charset argument of `getBytes` and `new String`, which may also be
//! a name.

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
                let name = self.text_ref(&args[0]);
                Code::atom(format!("{charset}::for_name({name})"))
            }
        }
    }
}
