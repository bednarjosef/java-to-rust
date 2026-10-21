//! `Integer.parseInt(s, radix)`, `Long.parseLong(s, radix)` and `Integer.valueOf(s, radix)`, which
//! read Java's syntax: an optional sign, then digits that `Character.digit` accepts in the radix.
//! Java reads the string one UTF-16 unit at a time, so a digit outside the BMP is two units and is
//! rejected, while every BMP decimal digit counts. Rust's `from_str_radix` reads only ASCII digits.

use crate::character_classes::digit;
use crate::{Exception, uncaught};

const MIN_RADIX: i32 = 2;
const MAX_RADIX: i32 = 36;

/// Past this bound a value is out of range for every integer type, so it is held here instead of
/// growing; the bound is far enough below `i128::MAX` that one more digit cannot overflow.
const CAP: i128 = 1 << 100;

/// Java's `parseInt(s, radix)` for the integer type `T`, which throws `NumberFormatException` for
/// what Java rejects. `None` is a null string.
pub fn parse_radix<T: TryFrom<i128>>(s: Option<&str>, radix: i32) -> T {
    try_parse_radix(s, radix).unwrap_or_else(uncaught)
}

/// `parse_radix` where the program observes the `NumberFormatException`.
pub fn try_parse_radix<T: TryFrom<i128>>(s: Option<&str>, radix: i32) -> Result<T, Exception> {
    let fail = |message: String| Exception::new("java.lang.NumberFormatException", message);
    let Some(s) = s else { return Err(fail("Cannot parse null string".to_string())) };
    let value = read(s, radix).map_err(fail)?;
    T::try_from(value).map_err(|_| fail(for_input(s, radix)))
}

/// `Short.parseShort(s, radix)` and `Byte.parseByte(s, radix)`, where the program does not observe
/// the `NumberFormatException`.
pub fn parse_small<T: TryFrom<i128>>(s: Option<&str>, radix: i32) -> T {
    try_parse_small(s, radix).unwrap_or_else(uncaught)
}

/// `Short.parseShort(s, radix)` and `Byte.parseByte(s, radix)`: the string is read as an `int`
/// first, so only a value that fits an `int` but not the narrower type gets Java's range message.
pub fn try_parse_small<T: TryFrom<i128>>(s: Option<&str>, radix: i32) -> Result<T, Exception> {
    let fail = |message: String| Exception::new("java.lang.NumberFormatException", message);
    let Some(s) = s else { return Err(fail("Cannot parse null string".to_string())) };
    let value = read(s, radix).map_err(fail)?;
    if i32::try_from(value).is_err() {
        return Err(fail(for_input(s, radix)));
    }
    T::try_from(value).map_err(|_| fail(format!("Value out of range. Value:\"{s}\" Radix:{radix}")))
}

/// `Integer.parseInt(s, begin, end, radix)` and `Long.parseLong(s, begin, end, radix)` for the
/// integer type `T`, where the program does not observe the exception.
pub fn parse_range<T: TryFrom<i128>>(s: &str, begin: i32, end: i32, radix: i32) -> T {
    try_parse_range(s, begin, end, radix).unwrap_or_else(uncaught)
}

/// `parse_range` where the program observes the exception. Java checks the range of the indices
/// first (`IndexOutOfBoundsException`), then the radix, then reads the units from `begin` to `end`
/// with a sign and digits; a unit that is no digit, or a value that does not fit `T`, is an error
/// at that unit, and so is a sign alone.
pub fn try_parse_range<T: TryFrom<i128>>(s: &str, begin: i32, end: i32, radix: i32) -> Result<T, Exception> {
    let units: Vec<u16> = s.encode_utf16().collect();
    let length = units.len();
    let (b, e) = (usize::try_from(begin), usize::try_from(end));
    match (b, e) {
        (Ok(b), Ok(e)) if b <= e && e <= length => {}
        _ => {
            let message = format!("Range [{begin}, {end}) out of bounds for length {length}");
            return Err(Exception::new("java.lang.IndexOutOfBoundsException", message));
        }
    }
    let (b, e) = (begin as usize, end as usize);
    let fail = |message: String| Exception::new("java.lang.NumberFormatException", message);
    check_radix(radix).map_err(fail)?;
    let text = &units[b..e];
    if text.is_empty() {
        return Err(fail(for_input("", radix)));
    }
    let error_at = |index: usize| {
        let shown = String::from_utf16_lossy(text);
        fail(format!("Error at index {index} in: \"{shown}\""))
    };
    let (sign, first) = match text[0] {
        c if c == u16::from(b'-') => (-1, 1),
        c if c == u16::from(b'+') => (1, 1),
        _ => (1, 0),
    };
    let mut value = 0i128;
    for (index, &c) in text.iter().enumerate().skip(first) {
        let d = digit(c, radix);
        if d < 0 {
            return Err(error_at(index));
        }
        value = value * i128::from(radix) + sign * i128::from(d);
        if T::try_from(value).is_err() {
            return Err(error_at(index));
        }
    }
    if first == text.len() {
        return Err(error_at(text.len()));
    }
    T::try_from(value).map_err(|_| error_at(0))
}

/// The value of a string in `radix`, or Java's message for why it is rejected. Only the range of
/// the target type can reject a number that is read, and `parse_radix` checks that.
fn read(s: &str, radix: i32) -> Result<i128, String> {
    check_radix(radix)?;
    let units: Vec<u16> = s.encode_utf16().collect();
    let (negative, digits) = match units.first() {
        Some(&c) if c == u16::from(b'-') => (true, &units[1..]),
        Some(&c) if c == u16::from(b'+') => (false, &units[1..]),
        _ => (false, &units[..]),
    };
    if digits.is_empty() {
        return Err(for_input(s, radix));
    }
    let mut value = 0i128;
    for &c in digits {
        let d = digit(c, radix);
        if d < 0 {
            return Err(for_input(s, radix));
        }
        value = (value * i128::from(radix) + i128::from(d)).min(CAP);
    }
    Ok(if negative { -value } else { value })
}

/// Java's check of the radix of the parse methods, with the message it throws.
pub(crate) fn check_radix(radix: i32) -> Result<(), String> {
    if radix < MIN_RADIX {
        return Err(format!("radix {radix} less than Character.MIN_RADIX"));
    }
    if radix > MAX_RADIX {
        return Err(format!("radix {radix} greater than Character.MAX_RADIX"));
    }
    Ok(())
}

/// Java's `NumberFormatException.forInputString`, which names the radix unless it is 10.
pub(crate) fn for_input(s: &str, radix: i32) -> String {
    let radix = if radix == 10 { String::new() } else { format!(" under radix {radix}") };
    format!("For input string: \"{s}\"{radix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message<T: std::fmt::Debug>(r: Result<T, Exception>) -> String {
        r.unwrap_err().to_string()
    }

    #[test]
    fn reads_signs_and_every_radix() {
        assert_eq!(read("-ff", 16), Ok(-255));
        assert_eq!(read("+7", 10), Ok(7));
        assert_eq!(read("zz", 36), Ok(1295));
        assert_eq!(read("1010", 2), Ok(10));
        assert_eq!(read("-0", 8), Ok(0));
        assert_eq!(parse_radix::<i32>(Some("-80000000"), 16), i32::MIN);
        assert_eq!(parse_radix::<i64>(Some("7fffffffffffffff"), 16), i64::MAX);
    }

    #[test]
    fn reads_decimal_digits_of_every_script() {
        // U+0661 ARABIC-INDIC DIGIT ONE, U+FF12 FULLWIDTH DIGIT TWO, U+0966 DEVANAGARI DIGIT ZERO.
        assert_eq!(read("\u{661}\u{ff12}\u{966}", 10), Ok(120));
        assert_eq!(read("\u{ff21}\u{ff41}", 16), Ok(0xaa));
    }

    #[test]
    fn rejects_what_java_rejects() {
        for (bad, radix) in [
            ("", 10),
            ("-", 10),
            ("+", 10),
            ("--1", 10),
            ("+-1", 10),
            (" 1", 10),
            ("1 ", 10),
            ("1_0", 10),
            ("2", 2),
            ("g", 16),
            ("1\u{1f600}", 10),
            ("\u{1d7d9}", 10),
        ] {
            assert!(read(bad, radix).is_err(), "{bad:?} in {radix}");
        }
        assert_eq!(read("g", 16).unwrap_err(), "For input string: \"g\" under radix 16");
        assert_eq!(read("", 10).unwrap_err(), "For input string: \"\"");
        assert_eq!(read("1", 1).unwrap_err(), "radix 1 less than Character.MIN_RADIX");
        assert_eq!(read("1", 37).unwrap_err(), "radix 37 greater than Character.MAX_RADIX");
    }

    #[test]
    fn range_of_the_target_type_rejects_large_values() {
        assert!(i32::try_from(read("2147483648", 10).unwrap()).is_err());
        assert!(i32::try_from(read("-2147483649", 10).unwrap()).is_err());
        assert!(i64::try_from(read("9223372036854775808", 10).unwrap()).is_err());
        assert!(i64::try_from(read(&"9".repeat(200), 10).unwrap()).is_err());
        assert_eq!(i32::try_from(read("-2147483648", 10).unwrap()), Ok(i32::MIN));
    }

    #[test]
    fn range_reads_the_indices_it_is_given() {
        assert_eq!(try_parse_range::<i32>("x-12y", 1, 4, 10), Ok(-12));
        assert_eq!(try_parse_range::<i64>("9223372036854775807", 0, 19, 10), Ok(i64::MAX));
        assert_eq!(try_parse_range::<i32>("-2147483648", 0, 11, 10), Ok(i32::MIN));
        assert_eq!(
            message(try_parse_range::<i32>("abc", 2, 1, 10)),
            "java.lang.IndexOutOfBoundsException: Range [2, 1) out of bounds for length 3"
        );
        assert_eq!(
            message(try_parse_range::<i32>("abc", 0, 4, 10)),
            "java.lang.IndexOutOfBoundsException: Range [0, 4) out of bounds for length 3"
        );
        assert_eq!(
            message(try_parse_range::<i32>("a12", 0, 3, 10)),
            "java.lang.NumberFormatException: Error at index 0 in: \"a12\""
        );
        assert_eq!(
            message(try_parse_range::<i32>("12x", 0, 3, 10)),
            "java.lang.NumberFormatException: Error at index 2 in: \"12x\""
        );
        assert_eq!(
            message(try_parse_range::<i32>("2147483648", 0, 10, 10)),
            "java.lang.NumberFormatException: Error at index 9 in: \"2147483648\""
        );
        assert_eq!(
            message(try_parse_range::<i32>("-", 0, 1, 10)),
            "java.lang.NumberFormatException: Error at index 1 in: \"-\""
        );
        assert_eq!(
            message(try_parse_range::<i32>("7", 0, 0, 10)),
            "java.lang.NumberFormatException: For input string: \"\""
        );
        assert_eq!(
            message(try_parse_range::<i32>("7", 0, 1, 1)),
            "java.lang.NumberFormatException: radix 1 less than Character.MIN_RADIX"
        );
    }

    #[test]
    fn short_and_byte_report_their_own_range() {
        assert_eq!(try_parse_small::<i16>(Some("-32768"), 10), Ok(i16::MIN));
        assert_eq!(
            message(try_parse_small::<i16>(Some("32768"), 10)),
            "java.lang.NumberFormatException: Value out of range. Value:\"32768\" Radix:10"
        );
        assert_eq!(
            message(try_parse_small::<i16>(Some("3000000000"), 10)),
            "java.lang.NumberFormatException: For input string: \"3000000000\""
        );
        assert_eq!(
            message(try_parse_small::<i16>(None, 10)),
            "java.lang.NumberFormatException: Cannot parse null string"
        );
        assert_eq!(
            message(try_parse_small::<i8>(Some("ff"), 16)),
            "java.lang.NumberFormatException: Value out of range. Value:\"ff\" Radix:16"
        );
    }
}
