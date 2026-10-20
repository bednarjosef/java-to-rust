//! The `Character` tests and conversions that Rust's `char` does not have under Java's names:
//! white space, the radix digits, and the surrogates.
//!
//! The whitespace and space tests are exact: they list Java's code points. `digit` reads the
//! decimal digits of the Basic Multilingual Plane (see `digits.rs`) and the Latin and fullwidth
//! letters.

use super::character::scalar;
use super::digits::decimal_digit;
use super::{is_digit, is_letter};

/// Java's `Character.isWhitespace(char)`: the white space of the ASCII controls and the Unicode
/// space separators, except the no-break spaces (`\u00A0`, `\u2007` and `\u202F`).
pub fn is_whitespace(c: u16) -> bool {
    matches!(c, 0x09..=0x0D | 0x1C..=0x1F) || (is_space_char(c) && !matches!(c, 0xA0 | 0x2007 | 0x202F))
}

/// Java's `Character.isSpaceChar(char)`: the Unicode space, line and paragraph separators.
pub fn is_space_char(c: u16) -> bool {
    matches!(c, 0x20 | 0xA0 | 0x1680 | 0x2000..=0x200A | 0x2028 | 0x2029 | 0x202F | 0x205F | 0x3000)
}

/// Java's `Character.isLetterOrDigit(char)`.
pub fn is_letter_or_digit(c: u16) -> bool {
    is_letter(c) || is_digit(c)
}

/// Java's `Character.isAlphabetic(int)`, on a `char`: the Unicode `Alphabetic` property.
pub fn is_alphabetic(c: u16) -> bool {
    scalar(c).is_some_and(char::is_alphabetic)
}

/// Java's `Character.digit(char, int)`: the value of `c` in `radix`, or -1. Decimal digits of
/// every script count, as do Latin and fullwidth letters, as in Java.
pub fn digit(c: u16, radix: i32) -> i32 {
    let value = match i32::from(c) {
        c @ 0x41..=0x5A => c - 0x41 + 10,
        c @ 0x61..=0x7A => c - 0x61 + 10,
        c @ 0xFF21..=0xFF3A => c - 0xFF21 + 10,
        c @ 0xFF41..=0xFF5A => c - 0xFF41 + 10,
        _ => match decimal_digit(c) {
            Some(value) => value as i32,
            None => return -1,
        },
    };
    if (2..=36).contains(&radix) && value < radix { value } else { -1 }
}

/// Java's `Character.forDigit(int, int)`: the lowercase character for `digit` in `radix`, or the
/// NUL character when the digit is not below the radix or the radix is out of range.
pub fn for_digit(digit: i32, radix: i32) -> u16 {
    if !(2..=36).contains(&radix) || !(0..radix).contains(&digit) {
        return 0;
    }
    let c = if digit < 10 { b'0' as i32 + digit } else { b'a' as i32 + digit - 10 };
    c as u16
}

/// Java's `Character.isSurrogate(char)`: a high or a low surrogate.
pub fn is_surrogate(c: u16) -> bool {
    matches!(c, 0xD800..=0xDFFF)
}

/// Java's `Character.isHighSurrogate(char)`.
pub fn is_high_surrogate(c: u16) -> bool {
    matches!(c, 0xD800..=0xDBFF)
}

/// Java's `Character.isLowSurrogate(char)`.
pub fn is_low_surrogate(c: u16) -> bool {
    matches!(c, 0xDC00..=0xDFFF)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn white_space_follows_java() {
        assert!(is_whitespace(0x09) && is_whitespace(0x1C) && is_whitespace(0x20));
        assert!(is_whitespace(0x2000) && is_whitespace(0x3000) && is_whitespace(0x2028));
        assert!(!is_whitespace(0xA0) && !is_whitespace(0x2007) && !is_whitespace(0x202F));
        assert!(!is_whitespace(0x85) && !is_whitespace(0x180E) && !is_whitespace(0x1B));
        assert!(is_space_char(0xA0) && !is_space_char(0x09));
    }

    #[test]
    fn digits_in_radixes() {
        assert_eq!(digit(b'a' as u16, 16), 10);
        assert_eq!(digit(b'Z' as u16, 36), 35);
        assert_eq!(digit(b'8' as u16, 8), -1);
        assert_eq!(digit(b'0' as u16, 1), -1);
        assert_eq!(digit(0xFF21, 16), 10);
        assert_eq!(digit(0x0669, 10), 9);
        assert_eq!(digit(0x0E59, 10), 9);
        assert_eq!(digit(0x0669, 9), -1);
        assert_eq!(for_digit(11, 16), b'b' as u16);
        assert_eq!(for_digit(36, 36), 0);
        assert_eq!(for_digit(0, 37), 0);
    }
}
