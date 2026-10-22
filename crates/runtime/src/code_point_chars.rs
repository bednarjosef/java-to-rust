//! The `int` code point forms of the `Character` tests and conversions. A code point in the BMP
//! gets the answer of its `char` form. A supplementary one reads the JDK's answers in
//! `code_point_tables.rs` and `code_point_case_tables.rs`. Outside 0..=0x10FFFF there is no
//! character: each test is false, each conversion gives the code point back, and the numeric
//! value and the digit are -1, as in Java.

use crate::code_point::{high_surrogate, is_bmp_code_point, is_valid_code_point, low_surrogate};
use crate::code_point_case_tables::{ALPHABETIC, LOWER, LOWERCASE, UPPER, UPPERCASE};
use crate::code_point_tables::{DIGITS, FRACTIONS, LETTERS, NUMERIC};
use crate::{Elements, Exception, throw, try_set};

const MIN_SUPPLEMENTARY: i32 = 0x1_0000;
const MAX_CODE_POINT: i32 = 0x10_FFFF;

/// Whether the supplementary code point `cp` is in one of the runs `(first, last)` of a table.
fn in_runs(runs: &[(u32, u32)], cp: i32) -> bool {
    if !(MIN_SUPPLEMENTARY..=MAX_CODE_POINT).contains(&cp) {
        return false;
    }
    let cp = cp as u32;
    let i = runs.partition_point(|&(_, last)| last < cp);
    matches!(runs.get(i), Some(&(first, _)) if first <= cp)
}

/// The offset of the run of `table` that holds the supplementary code point `cp`: the value of
/// `cp` is `cp + offset`.
fn offset_in(table: &[(u32, u32, i32)], cp: i32) -> Option<i32> {
    if !(MIN_SUPPLEMENTARY..=MAX_CODE_POINT).contains(&cp) {
        return None;
    }
    let cp = cp as u32;
    let i = table.partition_point(|&(_, last, _)| last < cp);
    match table.get(i) {
        Some(&(first, _, offset)) if first <= cp => Some(offset),
        _ => None,
    }
}

/// Java's `Character.isLetter(int)`.
pub fn is_letter_code_point(cp: i32) -> bool {
    u16::try_from(cp).map_or_else(|_| in_runs(LETTERS, cp), crate::is_letter)
}

/// Java's `Character.isDigit(int)`.
pub fn is_digit_code_point(cp: i32) -> bool {
    u16::try_from(cp).map_or_else(|_| in_runs(DIGITS, cp), crate::is_digit)
}

/// Java's `Character.isLetterOrDigit(int)`.
pub fn is_letter_or_digit_code_point(cp: i32) -> bool {
    is_letter_code_point(cp) || is_digit_code_point(cp)
}

/// Java's `Character.isWhitespace(int)`. No supplementary code point is whitespace.
pub fn is_whitespace_code_point(cp: i32) -> bool {
    u16::try_from(cp).is_ok_and(crate::is_whitespace)
}

/// Java's `Character.isSpaceChar(int)`. No supplementary code point is a space character.
pub fn is_space_char_code_point(cp: i32) -> bool {
    u16::try_from(cp).is_ok_and(crate::is_space_char)
}

/// Java's `Character.isUpperCase(int)`.
pub fn is_upper_case_code_point(cp: i32) -> bool {
    u16::try_from(cp).map_or_else(|_| in_runs(UPPERCASE, cp), crate::is_upper_case)
}

/// Java's `Character.isLowerCase(int)`.
pub fn is_lower_case_code_point(cp: i32) -> bool {
    u16::try_from(cp).map_or_else(|_| in_runs(LOWERCASE, cp), crate::is_lower_case)
}

/// Java's `Character.isAlphabetic(int)`.
pub fn is_alphabetic_code_point(cp: i32) -> bool {
    u16::try_from(cp).map_or_else(|_| in_runs(ALPHABETIC, cp), crate::is_alphabetic)
}

/// Java's `Character.toUpperCase(int)`: the simple mapping, or the code point when it has none.
pub fn to_upper_case_code_point(cp: i32) -> i32 {
    match u16::try_from(cp) {
        Ok(c) => i32::from(crate::to_upper_case(c)),
        Err(_) => offset_in(UPPER, cp).map_or(cp, |offset| cp + offset),
    }
}

/// Java's `Character.toLowerCase(int)`: the simple mapping, or the code point when it has none.
pub fn to_lower_case_code_point(cp: i32) -> i32 {
    match u16::try_from(cp) {
        Ok(c) => i32::from(crate::to_lower_case(c)),
        Err(_) => offset_in(LOWER, cp).map_or(cp, |offset| cp + offset),
    }
}

/// Java's `Character.getNumericValue(int)`: -1 for a code point with no numeric value, -2 for a
/// fraction.
pub fn numeric_value_code_point(cp: i32) -> i32 {
    match u16::try_from(cp) {
        Ok(c) => crate::character::numeric_value(c),
        Err(_) if in_runs(FRACTIONS, cp) => -2,
        Err(_) => offset_in(NUMERIC, cp).map_or(-1, |offset| cp + offset),
    }
}

/// Java's `Character.digit(int, int)`: the value of a decimal digit in `radix`, else -1.
pub fn digit_code_point(cp: i32, radix: i32) -> i32 {
    if !(2..=36).contains(&radix) {
        return -1;
    }
    match u16::try_from(cp) {
        Ok(c) => crate::character_classes::digit(c, radix),
        Err(_) => {
            let value = numeric_value_code_point(cp);
            if in_runs(DIGITS, cp) && (0..radix).contains(&value) { value } else { -1 }
        }
    }
}

/// Java's `Character.toChars(cp, dst, index)`: writes the one or two units of `cp` into `dst` from
/// `index` and returns their number. A code point that is not valid throws
/// `IllegalArgumentException` before anything is written. A supplementary one writes its low unit
/// before its high one, as the JDK does, so a write that does not fit leaves `dst` partly written.
pub fn try_to_chars_into(cp: i32, dst: &impl Elements<u16>, index: i32) -> Result<i32, Exception> {
    if is_bmp_code_point(cp) {
        try_set(dst, index, cp as u16)?;
        Ok(1)
    } else if is_valid_code_point(cp) {
        try_set(dst, index.wrapping_add(1), low_surrogate(cp))?;
        try_set(dst, index, high_surrogate(cp))?;
        Ok(2)
    } else {
        Err(Exception::new(
            "java.lang.IllegalArgumentException",
            format!("Not a valid Unicode code point: 0x{cp:X}"),
        ))
    }
}

/// The unchecked form of `try_to_chars_into`, for a call where the program cannot see the failure.
pub fn to_chars_into(cp: i32, dst: &impl Elements<u16>, index: i32) -> i32 {
    try_to_chars_into(cp, dst, index).unwrap_or_else(|e| throw(e.class, &e.message.unwrap_or_default()))
}
