//! The code point methods of `Character` (see `lower/code_point.rs`). Each one is a runtime
//! function on the code point as an `i32`, or on the two `u16` units of `toCodePoint`.

use super::*;

impl<'a> Gen<'a> {
    /// `Character.toChars`, `charCount` and the other code point methods, see `CodePointOp`.
    pub(super) fn code_point_builtin(&mut self, op: CodePointOp, args: &'a [Expr]) -> Code {
        let function = match op {
            CodePointOp::ToChars => "to_chars",
            CodePointOp::ToString => "code_point_string",
            CodePointOp::CharCount => "char_count",
            CodePointOp::ToCodePoint => "to_code_point",
            CodePointOp::HighSurrogate => "high_surrogate",
            CodePointOp::LowSurrogate => "low_surrogate",
            CodePointOp::IsValid => "is_valid_code_point",
            CodePointOp::IsBmp => "is_bmp_code_point",
            CodePointOp::IsSupplementary => "is_supplementary_code_point",
        };
        let operands = match op {
            CodePointOp::ToCodePoint => {
                vec![self.value(&args[0], Type::Char).text, self.value(&args[1], Type::Char).text]
            }
            _ => vec![self.value(&args[0], Type::Int).text],
        };
        let call = format!("{}({})", self.runtime(function), operands.join(", "));
        Code::atom(if op == CodePointOp::ToChars { self.made_array(call) } else { call })
    }
}
