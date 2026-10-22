//! The code point methods of `Character` (see `lower/code_point.rs`). Each one is a runtime
//! function on the code point as an `i32`, or on the two `u16` units of `toCodePoint`.

use super::*;

impl<'a> Gen<'a> {
    /// `Character.toChars`, `charCount` and the other code point methods, see `CodePointOp`.
    pub(super) fn code_point_builtin(&mut self, op: CodePointOp, args: &'a [Expr], id: ExprId) -> Code {
        if op == CodePointOp::ToCharsInto {
            let cp = self.value(&args[0], Type::Int).text;
            let dst = self.handle(&args[1]);
            let index = self.value(&args[2], Type::Int).text;
            return self.may_fail(id, "to_chars_into", "", &format!("{cp}, {dst}, {index}"));
        }
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
            CodePointOp::IsLetter => "is_letter_code_point",
            CodePointOp::IsDigit => "is_digit_code_point",
            CodePointOp::IsLetterOrDigit => "is_letter_or_digit_code_point",
            CodePointOp::IsWhitespace => "is_whitespace_code_point",
            CodePointOp::IsSpaceChar => "is_space_char_code_point",
            CodePointOp::IsUpperCase => "is_upper_case_code_point",
            CodePointOp::IsLowerCase => "is_lower_case_code_point",
            CodePointOp::IsAlphabetic => "is_alphabetic_code_point",
            CodePointOp::ToUpperCase => "to_upper_case_code_point",
            CodePointOp::ToLowerCase => "to_lower_case_code_point",
            CodePointOp::NumericValue => "numeric_value_code_point",
            CodePointOp::Digit => "digit_code_point",
            CodePointOp::ToCharsInto => unreachable!("handled above"),
        };
        let operands = match op {
            CodePointOp::ToCodePoint => {
                vec![self.value(&args[0], Type::Char).text, self.value(&args[1], Type::Char).text]
            }
            CodePointOp::Digit => {
                vec![self.value(&args[0], Type::Int).text, self.value(&args[1], Type::Int).text]
            }
            _ => vec![self.value(&args[0], Type::Int).text],
        };
        let call = format!("{}({})", self.runtime(function), operands.join(", "));
        Code::atom(if op == CodePointOp::ToChars {
            self.made_array(call, Type::array(Type::Char))
        } else {
            call
        })
    }
}
