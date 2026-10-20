//! Lowering of the code point methods of `Character` (`toChars`, `charCount`, `toCodePoint`, the
//! surrogate and validity tests). `Character.toString` is here only for an `int` argument: the
//! `char` overload is a boxed method (see `boxing.rs`).

use super::*;

/// Whether the static method `name` of `owner` with `args` is one of the code point methods.
pub(super) fn is_code_point_method(owner: &str, name: &str, args: &[Expr]) -> bool {
    if owner != "java.lang.Character" {
        return false;
    }
    match name {
        "toString" => args.len() == 1 && args[0].ty != Type::Char,
        "toChars"
        | "charCount"
        | "toCodePoint"
        | "highSurrogate"
        | "lowSurrogate"
        | "isValidCodePoint"
        | "isBmpCodePoint"
        | "isSupplementaryCodePoint" => true,
        _ => false,
    }
}

impl<'t> Lowering<'t> {
    /// A call of a code point method, see `is_code_point_method`.
    pub(super) fn code_point_call(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
        args: &[Expr],
    ) -> Result<Builtin, Error> {
        let name = symbol.name;
        let op = match name {
            "toChars" => CodePointOp::ToChars,
            "toString" => CodePointOp::ToString,
            "charCount" => CodePointOp::CharCount,
            "toCodePoint" => CodePointOp::ToCodePoint,
            "highSurrogate" => CodePointOp::HighSurrogate,
            "lowSurrogate" => CodePointOp::LowSurrogate,
            "isValidCodePoint" => CodePointOp::IsValid,
            "isBmpCodePoint" => CodePointOp::IsBmp,
            _ => CodePointOp::IsSupplementary,
        };
        let arity = if op == CodePointOp::ToCodePoint { 2 } else { 1 };
        if args.len() != arity {
            return unsupported(node, &format!("the overload of `{name}` with {} arguments", args.len()));
        }
        Ok(Builtin::CodePoint(op))
    }
}
