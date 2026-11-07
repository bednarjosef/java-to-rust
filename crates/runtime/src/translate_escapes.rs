//! `s.translateEscapes()` of the JDK: the escape sequences of a string, as a text block or a string
//! literal has them, become the characters they stand for. Java reads the string by UTF-16 code
//! unit, and so does this; a malformed escape throws `IllegalArgumentException`.

use crate::string::{from_units, units};
use crate::{Exception, JString, Wtf8};

/// `s.translateEscapes()`, which throws `IllegalArgumentException` for a malformed escape.
pub fn translate_escapes(s: &Wtf8) -> JString {
    try_translate_escapes(s).unwrap_or_else(|e| crate::throw_exception(e))
}

/// `translate_escapes` where the program observes the `IllegalArgumentException`, with the JDK's
/// message for the first malformed escape.
pub fn try_translate_escapes(s: &Wtf8) -> Result<JString, Exception> {
    const BACKSLASH: u16 = b'\\' as u16;
    let chars = units(s);
    let mut out: Vec<u16> = Vec::with_capacity(chars.len());
    let mut from = 0;
    while from < chars.len() {
        let mut ch = chars[from];
        from += 1;
        if ch == BACKSLASH {
            // A backslash at the end of the text is followed by NUL, which is no escape.
            ch = match chars.get(from) {
                Some(&next) => {
                    from += 1;
                    next
                }
                None => 0,
            };
            // The message shows a lone surrogate as `?`.
            let shown = char::from_u32(u32::from(ch)).unwrap_or('?');
            ch = match shown {
                'b' => 0x08,
                'f' => 0x0C,
                'n' => 0x0A,
                'r' => 0x0D,
                's' => 0x20,
                't' => 0x09,
                '\'' | '"' | '\\' => ch,
                '0'..='7' => {
                    // A first digit up to 3 may be followed by two more, any other by one: `\400`
                    // is a space and a `0`.
                    let limit = (from + if shown <= '3' { 2 } else { 1 }).min(chars.len());
                    let mut code = u32::from(ch - u16::from(b'0'));
                    while from < limit {
                        let digit = chars[from];
                        if !(u16::from(b'0')..=u16::from(b'7')).contains(&digit) {
                            break;
                        }
                        from += 1;
                        code = (code << 3) | u32::from(digit - u16::from(b'0'));
                    }
                    code as u16
                }
                // A line terminator after a backslash is discarded, with a `\n` after a `\r`.
                '\n' => continue,
                '\r' => {
                    if chars.get(from) == Some(&u16::from(b'\n')) {
                        from += 1;
                    }
                    continue;
                }
                _ => return Err(invalid_escape(shown, ch)),
            };
        }
        out.push(ch);
    }
    Ok(from_units(&out))
}

/// The `IllegalArgumentException` of the escape `\` followed by `shown` (the unit `ch`), with the
/// JDK's message.
fn invalid_escape(shown: char, ch: u16) -> Exception {
    let message = format!("Invalid escape sequence: \\{shown} \\\\u{ch:04X}");
    Exception::new("java.lang.IllegalArgumentException", message)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The code points of the translation, so that invisible characters show.
    fn points(s: &str) -> Vec<u32> {
        translate_escapes(Wtf8::new(s)).code_points().collect()
    }

    fn message(s: &str) -> String {
        try_translate_escapes(Wtf8::new(s)).unwrap_err().into_message().unwrap_or_default()
    }

    #[test]
    fn letter_escapes_stand_for_their_characters() {
        assert_eq!(points(r"\b\f\n\r\s\t"), [0x08, 0x0C, 0x0A, 0x0D, 0x20, 0x09]);
        assert_eq!(points(r#"\'\"\\"#), [0x27, 0x22, 0x5C]);
    }

    #[test]
    fn octal_escapes_take_one_to_three_digits() {
        assert_eq!(points(r"\7"), [7]);
        assert_eq!(points(r"\77"), [63]);
        assert_eq!(points(r"\101"), [65]);
        assert_eq!(points(r"\377"), [255]);
        assert_eq!(points(r"\0"), [0]);
        assert_eq!(points(r"\08"), [0, 0x38]);
    }

    #[test]
    fn a_first_digit_above_3_takes_two_digits_in_all() {
        // `\4` takes at most one more digit, so `\400` is a space and a `0`.
        assert_eq!(points(r"\400"), [0x20, 0x30]);
        assert_eq!(points(r"\7777"), [0o77, 0x37, 0x37]);
    }

    #[test]
    fn a_space_escape_is_a_space() {
        assert_eq!(translate_escapes(crate::Wtf8::new(r"a\sb")), "a b");
    }

    #[test]
    fn a_backslash_before_a_line_end_is_discarded() {
        assert_eq!(translate_escapes(crate::Wtf8::new("a\\\nb")), "ab");
        assert_eq!(translate_escapes(crate::Wtf8::new("a\\\r\nb")), "ab");
        assert_eq!(translate_escapes(crate::Wtf8::new("a\\\rb")), "ab");
        assert_eq!(translate_escapes(crate::Wtf8::new("\\\n\\\n")), "");
        // Only the `\n` after a `\r` belongs to the terminator; a second `\r` stays.
        assert_eq!(translate_escapes(crate::Wtf8::new("\\\r\r")), "\r");
    }

    #[test]
    fn text_without_escapes_is_unchanged() {
        assert_eq!(translate_escapes(crate::Wtf8::new("")), "");
        assert_eq!(translate_escapes(crate::Wtf8::new("plain \u{1F600} text")), "plain \u{1F600} text");
        assert_eq!(translate_escapes(crate::Wtf8::new("\\\\n")), "\\n");
    }

    #[test]
    fn a_malformed_escape_throws_with_the_jdk_message() {
        assert_eq!(message(&["\\", "u0041"].concat()), "Invalid escape sequence: \\u \\\\u0075");
        assert_eq!(message(r"ok\x"), "Invalid escape sequence: \\x \\\\u0078");
        assert_eq!(message("ab\\"), "Invalid escape sequence: \\\u{0} \\\\u0000");
        assert_eq!(message("\\\u{1F600}"), "Invalid escape sequence: \\? \\\\uD83D");
    }

    #[test]
    fn a_malformed_escape_is_an_illegal_argument_exception() {
        let e = crate::catching(|| translate_escapes(crate::Wtf8::new(r"\q"))).unwrap_err();
        assert_eq!(e.class, "java.lang.IllegalArgumentException");
        assert_eq!(e.message.as_deref(), Some("Invalid escape sequence: \\q \\\\u0071"));
    }
}
