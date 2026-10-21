//! Java semantics that Rust does not have built in, for code produced by `java-to-rust`:
//! printing doubles the way Java does, integer division that throws `ArithmeticException`,
//! Java's `Math` rules and uncaught exceptions.

mod array;
mod slice;

pub use array::*;
pub use slice::*;

use std::fmt;

mod arguments;
mod boxed;
mod builder;
mod catching;
mod chains;
mod character;
mod character_classes;
mod character_tables;
mod charset;
mod class;
mod code_point;
mod code_point_case_tables;
mod code_point_chars;
mod code_point_tables;
mod exact;
mod exception;
mod fd;
pub mod fdlibm;
mod float;
mod float_parse;
pub mod format;
mod initializer;
mod integer;
mod integer_parse;
mod java_arrays;
mod locked;
mod math;
mod monitor;
mod natives;
mod null;
mod out;
mod parse_range;
pub mod regex;
mod statics;
pub mod string;
mod system;
mod thread;
mod unsigned;
mod vm;
pub use arguments::main_args;
pub use boxed::*;
pub use builder::{StringBuilder, same_builder};
pub use catching::catching;
pub use chains::{Chain, Link, Unlink, drop_links};
pub use character::*;
pub use character_classes::*;
pub use charset::*;
pub use class::Class;
pub use code_point::{
    char_count, code_point_string, high_surrogate, is_bmp_code_point, is_supplementary_code_point,
    is_valid_code_point, low_surrogate, to_chars, to_code_point,
};
pub use code_point_chars::*;
pub use exact::*;
pub use exception::*;
pub use fd::*;
pub use float::*;
pub use float_parse::{parse_double, parse_float, try_parse_double, try_parse_float};
pub use format::{Arg, format};
pub use initializer::*;
pub use integer::*;
pub use integer_parse::{parse_radix, parse_small_radix, try_parse_radix, try_parse_small_radix};
pub use java_arrays::*;
pub use locked::{Locked, RawLock, Ref, RefMut};
pub use math::*;
pub use monitor::*;
pub use natives::*;
pub use null::{Handle, OrNull, non_null, null_enum, or_null, same, try_non_null, try_non_null_message};
pub use out::{
    Out, drain_stdout, eprint_chars, eprintln_chars, err, err_flush, flush, out, print_chars, println_chars,
    standard_close, standard_flush, standard_write, standard_write_bytes, write_byte,
};
pub use parse_range::{parse_range, parse_unsigned_range, try_parse_range, try_parse_unsigned_range};
pub use regex::{matches_all, replace_all, replace_first, split};
pub use statics::*;
pub use string::*;
pub use system::*;
pub use thread::*;
pub use unsigned::{
    compare_unsigned, divide_unsigned, parse_unsigned, remainder_unsigned, to_unsigned_string,
    try_parse_unsigned,
};
pub use vm::*;

/// Formats a `double` like Java's `Double.toString`: `1.0`, `0.001`, `1.0E7`, `4.9E-324`.
pub struct Double(pub f64);

impl fmt::Display for Double {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&double_to_string(self.0))
    }
}

pub fn double_to_string(d: f64) -> String {
    if d.is_nan() {
        return "NaN".into();
    }
    let sign = if d.is_sign_negative() { "-" } else { "" };
    if d.is_infinite() {
        return format!("{sign}Infinity");
    }
    if d == 0.0 {
        return format!("{sign}0.0");
    }
    layout(sign, shortest_digits(d.abs()))
}

/// Lays out the digits and exponent of a positive number the way `Double.toString` and
/// `Float.toString` do: plain from `0.001` up to `10^7`, scientific (`1.0E7`) outside that.
fn layout(sign: &str, (digits, exp): (String, i32)) -> String {
    if (-3..7).contains(&exp) {
        let (int, frac) = if exp >= 0 {
            let int_len = exp as usize + 1;
            if digits.len() <= int_len {
                (format!("{digits:0<int_len$}"), "0".to_string())
            } else {
                (digits[..int_len].to_string(), digits[int_len..].to_string())
            }
        } else {
            ("0".to_string(), format!("{}{digits}", "0".repeat((-exp - 1) as usize)))
        };
        format!("{sign}{int}.{frac}")
    } else {
        let rest = if digits.len() > 1 { &digits[1..] } else { "0" };
        format!("{sign}{}.{rest}E{exp}", &digits[..1])
    }
}

/// The decimal digits and exponent Java picks for a positive finite `d`: the shortest decimal
/// that rounds to `d`, except that when one digit would do, Java takes the closest decimal of
/// at most two digits (so `Double.MIN_VALUE` prints as `4.9E-324`, not `5.0E-324`). Among
/// decimals of that length, Java takes the closest to `d`, and of two equally close, the one
/// with an even last digit (so `1100000.25f` prints as `1100000.2`).
pub(crate) fn shortest_digits<T: fmt::LowerExp + std::str::FromStr + PartialEq>(d: T) -> (String, i32) {
    let split = |s: String| {
        let (mantissa, exp) = s.split_once('e').expect("{:e} output has an exponent");
        (mantissa.replace('.', ""), exp.parse::<i32>().expect("exponent is an integer"))
    };
    let (digits, exp) = split(format!("{d:e}"));
    // Exact formatting rounds to the closest decimal of the given length, ties to even. It is
    // Java's choice whenever it still rounds to `d`; the shortest form is not always it.
    let len = digits.len().max(2);
    let closest = format!("{d:.*e}", len - 1);
    if closest.parse::<T>().ok() == Some(d) {
        let (closest_digits, closest_exp) = split(closest);
        return (closest_digits.trim_end_matches('0').to_string(), closest_exp);
    }
    (digits, exp)
}

/// Integer types with Java's division rules.
pub trait Integer: Copy {
    fn java_div(self, rhs: Self) -> Self;
    fn java_rem(self, rhs: Self) -> Self;
    fn try_div(self, rhs: Self) -> Result<Self, Exception>;
    fn try_rem(self, rhs: Self) -> Result<Self, Exception>;
}

macro_rules! integer {
    ($($t:ty),*) => {$(
        impl Integer for $t {
            fn java_div(self, rhs: Self) -> Self {
                if rhs == 0 {
                    throw("java.lang.ArithmeticException", "/ by zero");
                }
                self.wrapping_div(rhs)
            }

            fn java_rem(self, rhs: Self) -> Self {
                if rhs == 0 {
                    throw("java.lang.ArithmeticException", "/ by zero");
                }
                self.wrapping_rem(rhs)
            }

            fn try_div(self, rhs: Self) -> Result<Self, Exception> {
                if rhs == 0 {
                    return Err(Exception::new("java.lang.ArithmeticException", "/ by zero"));
                }
                Ok(self.wrapping_div(rhs))
            }

            fn try_rem(self, rhs: Self) -> Result<Self, Exception> {
                if rhs == 0 {
                    return Err(Exception::new("java.lang.ArithmeticException", "/ by zero"));
                }
                Ok(self.wrapping_rem(rhs))
            }
        }
    )*};
}

integer!(i32, i64);

/// Java's `/` on `int` and `long`: truncates, wraps `MIN / -1`, throws on zero.
pub fn div<T: Integer>(a: T, b: T) -> T {
    a.java_div(b)
}

/// Java's `%` on `int` and `long`.
pub fn rem<T: Integer>(a: T, b: T) -> T {
    a.java_rem(b)
}

/// `/` where the program observes the `ArithmeticException` of a division by zero.
pub fn try_div<T: Integer>(a: T, b: T) -> Result<T, Exception> {
    a.try_div(b)
}

/// `%` where the program observes the `ArithmeticException` of a division by zero.
pub fn try_rem<T: Integer>(a: T, b: T) -> Result<T, Exception> {
    a.try_rem(b)
}

/// Java's `Math.max(double, double)`: NaN wins, and `0.0` is larger than `-0.0`.
pub fn max(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        a
    } else if a == 0.0 && b == 0.0 && a.is_sign_negative() {
        b
    } else if a >= b {
        a
    } else {
        b
    }
}

/// Java's `Math.min(double, double)`: NaN wins, and `-0.0` is smaller than `0.0`.
pub fn min(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        a
    } else if a == 0.0 && b == 0.0 && b.is_sign_negative() {
        b
    } else if a <= b {
        a
    } else {
        b
    }
}

/// Java's `String.length()`, which counts UTF-16 code units.
pub fn string_length(s: &str) -> i32 {
    s.encode_utf16().count() as i32
}

/// Java's `System.exit`.
pub fn exit(code: i32) -> ! {
    // What `System.out` still buffers is lost, as Java loses it.
    drain_stdout();
    std::process::exit(code)
}

/// Ends the program the way an uncaught Java exception of the standard class `class` does.
pub fn throw(class: &str, message: &str) -> ! {
    catching::raise(class, message);
    // The class's name lives on for an uncaught-exception handler, while the program ends.
    let class: &'static str = Box::leak(class.to_string().into_boxed_str());
    exception::die(Exception::new(class, message))
}

/// Java's checked cast `(T) x` on a non-null object: the object as a `T` if it is one, which
/// the translation finds (`x.as_t()`), else a `ClassCastException`. `to` is the binary name of
/// `T`.
pub fn cast<T>(value: Option<T>, to: &str) -> T {
    match value {
        Some(value) => value,
        None => cast_failed(to),
    }
}

/// `cast`, where a `catch` or a `finally` sees its `ClassCastException`: the failure is
/// returned, to go on as a thrown exception does.
pub fn try_cast<T>(value: Option<T>, to: &str) -> Result<T, Exception> {
    value.ok_or_else(|| {
        Exception::new("java.lang.ClassCastException", format!("cannot be cast to class {to}"))
    })
}

/// The `ClassCastException` of a cast to class `to` (by binary name) that fails.
pub fn cast_failed(to: &str) -> ! {
    throw("java.lang.ClassCastException", &format!("cannot be cast to class {to}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ties_between_shortest_decimals_take_the_even_digit() {
        // 2^49 + 0.25 is exactly between ...312.2 and ...312.3 at 16 digits.
        assert_eq!(double_to_string(f64::from_bits(0x4300_0000_0000_0002)), "5.629499534213122E14");
    }

    #[test]
    fn doubles_print_like_java() {
        let cases = [
            (1.0, "1.0"),
            (100.0, "100.0"),
            (0.1 + 0.2, "0.30000000000000004"),
            (1.0 / 3.0, "0.3333333333333333"),
            (0.001, "0.001"),
            (0.0001, "1.0E-4"),
            (1234567.0, "1234567.0"),
            (1e7, "1.0E7"),
            (12345678.9, "1.23456789E7"),
            (-0.0, "-0.0"),
            (f64::MIN_POSITIVE, "2.2250738585072014E-308"),
            (f64::from_bits(1), "4.9E-324"),
            (f64::MAX, "1.7976931348623157E308"),
            (2e23, "2.0E23"),
            (f64::NAN, "NaN"),
            (f64::NEG_INFINITY, "-Infinity"),
        ];
        for (value, java) in cases {
            assert_eq!(double_to_string(value), java, "{value:e}");
        }
    }
}
