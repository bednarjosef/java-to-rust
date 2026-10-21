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

/// The op of the `int` overload of the `Character` method `name` that has a `char` form too (for
/// `isLetter(int)`, `toUpperCase(int)`, `digit(int, int)` and the like), see `character_method`.
pub(super) fn character_op(name: &str) -> Option<CodePointOp> {
    Some(match name {
        "isLetter" => CodePointOp::IsLetter,
        "isDigit" => CodePointOp::IsDigit,
        "isLetterOrDigit" => CodePointOp::IsLetterOrDigit,
        "isWhitespace" => CodePointOp::IsWhitespace,
        "isSpaceChar" => CodePointOp::IsSpaceChar,
        "isUpperCase" => CodePointOp::IsUpperCase,
        "isLowerCase" => CodePointOp::IsLowerCase,
        "isAlphabetic" => CodePointOp::IsAlphabetic,
        "toUpperCase" => CodePointOp::ToUpperCase,
        "toLowerCase" => CodePointOp::ToLowerCase,
        "getNumericValue" => CodePointOp::NumericValue,
        "digit" => CodePointOp::Digit,
        _ => return None,
    })
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
            "toChars" if args.len() == 3 => CodePointOp::ToCharsInto,
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
        let arity = match op {
            CodePointOp::ToCodePoint => 2,
            CodePointOp::ToCharsInto => 3,
            _ => 1,
        };
        if args.len() != arity {
            return unsupported(node, &format!("the overload of `{name}` with {} arguments", args.len()));
        }
        Ok(Builtin::CodePoint(op))
    }
}
