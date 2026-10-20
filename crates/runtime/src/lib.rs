//! Java semantics that Rust does not have built in, for code produced by `java-to-rust`:
//! printing doubles the way Java does, integer division that throws `ArithmeticException`,
//! Java's `Math` rules and uncaught exceptions.

mod array;
mod slice;

pub use array::*;
pub use slice::*;

use std::fmt;

mod boxed;
mod character;
mod character_classes;
mod digits;
mod exact;
pub mod fdlibm;
mod float;
mod float_parse;
mod integer_parse;
mod java_arrays;
mod math;
mod natives;
mod null;
mod out;
pub use boxed::*;
pub use character::*;
pub use character_classes::*;
pub use digits::decimal_digit;
pub use exact::*;
pub use float::*;
pub use float_parse::{parse_double, parse_float};
pub use integer_parse::parse_radix;
pub use java_arrays::*;
pub use math::*;
pub use natives::*;
pub use null::{OrNull, non_null, or_null, same};
pub use out::{Out, flush, out, print_chars, println_chars, write_byte};

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
/// at most two digits (so `Double.MIN_VALUE` prints as `4.9E-324`, not `5.0E-324`).
fn shortest_digits<T: fmt::LowerExp + std::str::FromStr + PartialEq>(d: T) -> (String, i32) {
    let split = |s: String| {
        let (mantissa, exp) = s.split_once('e').expect("{:e} output has an exponent");
        (mantissa.replace('.', ""), exp.parse::<i32>().expect("exponent is an integer"))
    };
    let (digits, exp) = split(format!("{d:e}"));
    if digits.len() == 1 {
        let two = format!("{d:.1e}");
        if two.parse::<T>().ok() == Some(d) {
            let (two_digits, two_exp) = split(two);
            return (two_digits.trim_end_matches('0').to_string(), two_exp);
        }
    }
    (digits, exp)
}

/// Integer types with Java's division rules.
pub trait Integer: Copy {
    fn java_div(self, rhs: Self) -> Self;
    fn java_rem(self, rhs: Self) -> Self;
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
    flush();
    std::process::exit(code)
}

/// Ends the program the way an uncaught Java exception does.
pub fn throw(class: &str, message: &str) -> ! {
    flush();
    eprintln!("Exception in thread \"main\" {class}: {message}");
    std::process::exit(1)
}

#[cfg(test)]
mod tests {
    use super::*;

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
