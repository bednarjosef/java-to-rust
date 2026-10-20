//! Java's `float`: printing like `Float.toString`, and `Math.max`/`min`.

use super::{layout, shortest_digits};
use std::fmt;

/// Formats a `float` like Java's `Float.toString`: the shortest decimal that reads back as the
/// same `float`, so `0.1f` prints `0.1` (where `(double) 0.1f` prints `0.10000000149011612`).
pub struct Float(pub f32);

impl fmt::Display for Float {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&float_to_string(self.0))
    }
}

pub fn float_to_string(x: f32) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    let sign = if x.is_sign_negative() { "-" } else { "" };
    if x.is_infinite() {
        return format!("{sign}Infinity");
    }
    if x == 0.0 {
        return format!("{sign}0.0");
    }
    layout(sign, shortest_digits(x.abs()))
}

/// Java's `Math.max(float, float)`: NaN wins, and `0.0f` is larger than `-0.0f`.
pub fn max_f32(a: f32, b: f32) -> f32 {
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

/// Java's `Math.min(float, float)`: NaN wins, and `-0.0f` is smaller than `0.0f`.
pub fn min_f32(a: f32, b: f32) -> f32 {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats_print_like_java() {
        // Each case is a float's bits and what Java 26 prints for it.
        let cases = [
            (1036831949, "0.1"),
            (1065353216, "1.0"),
            (1120403456, "100.0"),
            (1259902592, "1.0E7"),
            (1343554297, "1.0E10"),
            (981668463, "0.001"),
            (953267991, "1.0E-4"),
            (1234613304, "1234567.0"),
            (1262248270, "1.2345678E7"),
            (2139095039, "3.4028235E38"),
            (1, "1.4E-45"),
            (2, "2.8E-45"),
            (61, "8.5E-44"),
            (8388608, "1.1754944E-38"),
            (4194304, "5.877472E-39"),
            (0x8000_0000, "-0.0"),
            (1050253722, "0.3"),
            (1051372203, "0.33333334"),
            (1059760811, "0.6666667"),
            (1266679808, "1.6777216E7"),
            (1259902591, "9999999.0"),
            (786163455, "1.0E-10"),
            (1206984804, "123456.78"),
            (1713989654, "2.0E23"),
            (1705601046, "1.0E23"),
            (1078530011, "3.1415927"),
            (1066192077, "1.1"),
        ];
        for (bits, java) in cases {
            let x = f32::from_bits(bits);
            assert_eq!(float_to_string(x), java, "{x:e}");
        }
        assert_eq!(float_to_string(f32::NAN), "NaN");
        assert_eq!(float_to_string(f32::INFINITY), "Infinity");
        assert_eq!(float_to_string(f32::NEG_INFINITY), "-Infinity");
    }

    #[test]
    fn max_and_min_follow_java() {
        assert!(max_f32(f32::NAN, 1.0).is_nan());
        assert!(min_f32(1.0, f32::NAN).is_nan());
        assert!(max_f32(-0.0, 0.0).is_sign_positive());
        assert!(min_f32(0.0, -0.0).is_sign_negative());
        assert_eq!(max_f32(1.5, -2.0), 1.5);
    }
}
