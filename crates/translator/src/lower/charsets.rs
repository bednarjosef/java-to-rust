//! Charsets, which exist only as the charset argument of `getBytes` and `new String`. There the
//! constants of `StandardCharsets` and `Charset.forName(name)` lower to a `Builtin::Charset`
//! expression of `Type::Charset`; anywhere else a `Charset` is rejected by `ty`.

use super::*;

impl<'t> Lowering<'t> {
    /// An argument of a call or of `new`. A charset becomes its expression; anything else is
    /// lowered as an expression.
    pub(super) fn call_arg(&self, node: Node<'t>) -> Result<Expr, Error> {
        if node.java_type() != "java.nio.charset.Charset" {
            return self.expr(node);
        }
        let kind = match node.symbol() {
            Some(symbol) if symbol.owner == "java.nio.charset.StandardCharsets" => {
                let op = match symbol.name {
                    "UTF_8" => CharsetOp::Utf8,
                    "US_ASCII" => CharsetOp::UsAscii,
                    "ISO_8859_1" => CharsetOp::Latin1,
                    "UTF_16" => CharsetOp::Utf16,
                    "UTF_16BE" => CharsetOp::Utf16Be,
                    "UTF_16LE" => CharsetOp::Utf16Le,
                    _ => return unsupported(node, "this `StandardCharsets` constant"),
                };
                ExprKind::Builtin(Builtin::Charset(op), Vec::new())
            }
            Some(symbol)
                if node.kind() == "METHOD_INVOCATION"
                    && symbol.owner == "java.nio.charset.Charset"
                    && symbol.name == "forName" =>
            {
                let args: Vec<Expr> =
                    node.list("arguments").map(|a| self.expr(a)).collect::<Result<_, _>>()?;
                if args.len() != 1 {
                    return unsupported(node, "this overload of `Charset.forName`");
                }
                ExprKind::Builtin(Builtin::Charset(CharsetOp::ForName), args)
            }
            _ => {
                return unsupported(
                    node,
                    "this `Charset`, other than a `StandardCharsets` constant or `Charset.forName`",
                );
            }
        };
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        Ok(Expr { kind, pos: node.pos(), ty: Type::Charset, id })
    }
}
