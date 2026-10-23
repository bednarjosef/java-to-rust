//! Java's `char`, an unsigned 16-bit UTF-16 code unit (`u16`): printing, `String.charAt` and
//! the `Character` methods.
//!
//! The `Character` methods agree with Java's for every code unit. Java's digit, letter, numeric
//! value and uppercase tables differ from Rust's, so they come from `character_tables.rs`, which
//! is generated from the JDK; the case and alphabetic tests are Rust's, which agree with Java's.

use super::character_tables::{DIGITS, FRACTIONS, LETTERS, NUMERIC_OFFSETS, UPPER_CASE_EXCEPTIONS};
use super::{Exception, JString, Wtf8, string_length};
use std::fmt;

/// Formats a `char` the way Java prints it: a lone surrogate prints as `?`, as Java's UTF-8
/// output does.
pub struct Char(pub u16);

impl fmt::Display for Char {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let c = char::from_u32(self.0 as u32).unwrap_or('?');
        fmt::Display::fmt(&c, f)
    }
}

/// `String.valueOf(c)`: the string of one code unit, a lone surrogate too.
pub fn char_to_string(c: u16) -> JString {
    JString::from_units(&[c])
}

/// Java's `s.charAt(index)`: the UTF-16 code unit at `index`.
pub fn char_at(s: &Wtf8, index: i32) -> u16 {
    try_char_at(s, index).unwrap_or_else(crate::uncaught)
}

/// `s.charAt(index)` on an ASCII string, where code units are bytes.
pub fn ascii_char_at(s: &Wtf8, index: i32) -> u16 {
    try_ascii_char_at(s, index).unwrap_or_else(crate::uncaught)
}

/// `char_at` where the program observes the `StringIndexOutOfBoundsException`.
pub fn try_char_at(s: &Wtf8, index: i32) -> Result<u16, Exception> {
    let bytes = s.as_bytes();
    // Up to an ASCII unit, units are bytes: a word-at-a-time check of the bytes before it, rather
    // than decoding every unit from the start.
    if let Ok(i) = usize::try_from(index)
        && i < bytes.len()
        && bytes[..=i].is_ascii()
    {
        return Ok(bytes[i] as u16);
    }
    let unit = usize::try_from(index).ok().and_then(|i| s.units().nth(i));
    unit.ok_or_else(|| out_of_bounds(index, string_length(s)))
}

/// `ascii_char_at` where the program observes the `StringIndexOutOfBoundsException`.
pub fn try_ascii_char_at(s: &Wtf8, index: i32) -> Result<u16, Exception> {
    match usize::try_from(index).ok().and_then(|i| s.as_bytes().get(i)) {
        Some(&b) => Ok(b as u16),
        None => Err(out_of_bounds(index, s.len() as i32)),
    }
}

fn out_of_bounds(index: i32, length: i32) -> Exception {
    let message = format!("Index {index} out of bounds for length {length}");
    Exception::new("java.lang.StringIndexOutOfBoundsException", message)
}

pub(super) fn scalar(c: u16) -> Option<char> {
    char::from_u32(c as u32)
}

/// Whether `c` lies in one of the inclusive `ranges`, which are sorted and do not overlap.
fn in_ranges(ranges: &[(u16, u16)], c: u16) -> bool {
    let i = ranges.partition_point(|&(_, last)| last < c);
    ranges.get(i).is_some_and(|&(first, _)| first <= c)
}

/// Java's `Character.isDigit(char)`: the Unicode category Nd.
pub fn is_digit(c: u16) -> bool {
    in_ranges(DIGITS, c)
}

/// Java's `Character.isLetter(char)`.
pub fn is_letter(c: u16) -> bool {
    in_ranges(LETTERS, c)
}

/// Java's `Character.getNumericValue(char)`: the value, -2 for a fraction, and -1 for a character
/// with none.
pub(super) fn numeric_value(c: u16) -> i32 {
    if in_ranges(FRACTIONS, c) {
        return -2;
    }
    let i = NUMERIC_OFFSETS.partition_point(|&(_, last, _)| last < c);
    match NUMERIC_OFFSETS.get(i) {
        Some(&(first, _, offset)) if first <= c => i32::from(c) + offset,
        _ => -1,
    }
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
    // Rust's full mapping of these is several characters, which the rule below leaves alone.
    if let Ok(i) = UPPER_CASE_EXCEPTIONS.binary_search_by_key(&c, |&(from, _)| from) {
        return UPPER_CASE_EXCEPTIONS[i].1;
    }
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
        assert_eq!(char_to_string(0xD800).units().collect::<Vec<_>>(), [0xD800]);
        assert_eq!(char_to_string(0xD800).to_string(), "?");
        assert_eq!(format!("[{}]", Char(unit('\n'))), "[\n]");
    }

    #[test]
    fn char_at_counts_utf16_units() {
        assert_eq!(char_at(crate::Wtf8::new("héllo"), 1), unit('é'));
        assert_eq!(char_at(crate::Wtf8::new("a😀b"), 1), 0xD83D);
        assert_eq!(char_at(crate::Wtf8::new("a😀b"), 3), unit('b'));
        assert_eq!(ascii_char_at(crate::Wtf8::new("abc"), 2), unit('c'));
        assert_eq!(char_at(crate::Wtf8::new("héllo"), 0), unit('h'));
        assert!(try_char_at(crate::Wtf8::new("ab"), 2).is_err());
        assert!(try_char_at(crate::Wtf8::new("ab"), -1).is_err());
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

    #[test]
    fn character_tables_match_java_beyond_latin1() {
        // Values from the JDK's sweep (tools/character-check).
        assert!(is_digit(0x663) && !is_letter(0x663));
        assert!(!is_digit(0xB2) && !is_letter(0x2160) && is_letter(0xAA));
        assert_eq!((numeric_value(0x663), numeric_value(0x2160)), (3, 1));
        assert_eq!((numeric_value(0xB2), numeric_value(0xBD), numeric_value(0x300)), (2, -2, -1));
        assert_eq!(crate::digit(0x663, 4), 3);
        assert_eq!(crate::digit(0x663, 3), -1);
        assert_eq!(crate::digit(0xFF21, 16), 10);
        assert_eq!(to_upper_case(0x1F80), 0x1F88);
        assert_eq!(to_upper_case(0x1F81), 0x1F89);
        assert_eq!(to_upper_case(0x1FF3), 0x1FFC);
    }
}
