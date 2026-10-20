//! Java's `char`, an unsigned 16-bit UTF-16 code unit (`u16`): printing, `String.charAt` and
//! the `Character` methods.
//!
//! The `Character` methods are exact for Latin-1 (`\u0000` to `ÿ`). Beyond it they use
//! Rust's Unicode tables, which agree with Java's except that `isDigit` accepts every numeric
//! character and `isLetter` every alphabetic one.

use super::{string_length, throw};
use std::fmt;

/// Formats a `char` the way Java prints it. A lone surrogate, which no Rust string can hold,
/// prints as `?`, as Java's UTF-8 output does.
pub struct Char(pub u16);

impl fmt::Display for Char {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let c = char::from_u32(self.0 as u32).unwrap_or('?');
        fmt::Display::fmt(&c, f)
    }
}

pub fn char_to_string(c: u16) -> String {
    Char(c).to_string()
}

/// Java's `s.charAt(index)`: the UTF-16 code unit at `index`.
pub fn char_at(s: &str, index: i32) -> u16 {
    let unit = usize::try_from(index).ok().and_then(|i| s.encode_utf16().nth(i));
    unit.unwrap_or_else(|| out_of_bounds(index, string_length(s)))
}

/// `s.charAt(index)` on an ASCII string, where code units are bytes.
pub fn ascii_char_at(s: &str, index: i32) -> u16 {
    match usize::try_from(index).ok().and_then(|i| s.as_bytes().get(i)) {
        Some(&b) => b as u16,
        None => out_of_bounds(index, s.len() as i32),
    }
}

fn out_of_bounds(index: i32, length: i32) -> ! {
    let message = format!("Index {index} out of bounds for length {length}");
    throw("java.lang.StringIndexOutOfBoundsException", &message)
}

pub(super) fn scalar(c: u16) -> Option<char> {
    char::from_u32(c as u32)
}

/// Java's `Character.isDigit(char)`.
pub fn is_digit(c: u16) -> bool {
    if c < 0x100 { (0x30..=0x39).contains(&c) } else { scalar(c).is_some_and(char::is_numeric) }
}

/// Java's `Character.isLetter(char)`.
pub fn is_letter(c: u16) -> bool {
    scalar(c).is_some_and(char::is_alphabetic)
}

/// Java's `Character.isUpperCase(char)`.
pub fn is_upper_case(c: u16) -> bool {
    scalar(c).is_some_and(char::is_uppercase)
}

/// Java's `Character.isLowerCase(char)`.
pub fn is_lower_case(c: u16) -> bool {
    scalar(c).is_some_and(char::is_lowercase)
}

/// Java's `Character.isWhitespace(int)`: a space separator other than a no-break space, a line or
/// paragraph separator, or one of the controls `\t`, `\n`, `\u000B`, `\f`, `\r` and
/// `\u001C` to `\u001F`.
pub fn is_java_whitespace(cp: u32) -> bool {
    matches!(
        cp,
        0x09..=0x0D | 0x1C..=0x20 | 0x1680 | 0x2000..=0x2006 | 0x2008..=0x200A | 0x2028 | 0x2029 | 0x205F | 0x3000
    )
}

/// Java's `Character.toUpperCase(char)`: the simple case mapping, so a character whose upper
/// case is several characters (`ß`) stays as it is.
pub fn to_upper_case(c: u16) -> u16 {
    scalar(c).map_or(c, |ch| single(ch.to_uppercase()).unwrap_or(c))
}

/// Java's `Character.toLowerCase(char)`.
pub fn to_lower_case(c: u16) -> u16 {
    if c == 0x130 {
        // `İ` lowercases to `i` plus a combining dot in full, to `i` in the simple mapping.
        return 'i' as u16;
    }
    scalar(c).map_or(c, |ch| single(ch.to_lowercase()).unwrap_or(c))
}

/// The one UTF-16 code unit of a case mapping, if it is one.
fn single(mut mapped: impl Iterator<Item = char>) -> Option<u16> {
    let c = mapped.next()?;
    let unit = u16::try_from(c as u32).ok();
    if mapped.next().is_some() { None } else { unit }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(c: char) -> u16 {
        c as u16
    }

    #[test]
    fn chars_print_as_text() {
        assert_eq!(char_to_string(unit('a')), "a");
        assert_eq!(char_to_string(unit('é')), "é");
        assert_eq!(char_to_string(0xD800), "?");
        assert_eq!(format!("[{}]", Char(unit('\n'))), "[\n]");
    }

    #[test]
    fn char_at_counts_utf16_units() {
        assert_eq!(char_at("héllo", 1), unit('é'));
        assert_eq!(char_at("a😀b", 1), 0xD83D);
        assert_eq!(char_at("a😀b", 3), unit('b'));
        assert_eq!(ascii_char_at("abc", 2), unit('c'));
    }

    #[test]
    fn character_methods_match_java_on_latin1() {
        assert!(is_digit(unit('7')) && !is_digit(unit('²')) && !is_digit(unit('a')));
        assert!(is_letter(unit('ª')) && is_letter(unit('ÿ')) && !is_letter(unit('×')));
        assert!(is_lower_case(unit('ª')) && is_lower_case(unit('ß')) && !is_lower_case(unit('A')));
        assert!(is_upper_case(unit('Þ')) && !is_upper_case(unit('÷')));
        assert_eq!(to_upper_case(unit('µ')), 0x39C);
        assert_eq!(to_upper_case(unit('ÿ')), 0x178);
        assert_eq!(to_upper_case(unit('ß')), unit('ß'));
        assert_eq!(to_lower_case(unit('À')), unit('à'));
        assert_eq!(to_lower_case(0x130), unit('i'));
        assert_eq!(to_upper_case(0xD800), 0xD800);
    }
}
