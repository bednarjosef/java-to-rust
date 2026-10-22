//! `Double.toHexString` and `Float.toHexString`: a value as `0x1.<hex digits>p<exponent>`, or
//! `0x0.` for a subnormal, with the trailing zero digits left out.

use super::double_to_string;

/// `2^-896`: `Float.toHexString` scales a subnormal float by it (`Math.scalb` with
/// `Double.MIN_EXPONENT - Float.MIN_EXPONENT`), which makes a subnormal double of it exactly.
const SCALE: f64 = f64::from_bits(127 << 52);

const HEX: &[u8; 16] = b"0123456789abcdef";

/// Java's `Double.toHexString(double)`: `NaN`, `Infinity` and `-Infinity` as `Double.toString`
/// prints them, `0x0.0p0` for zero, and otherwise the significand's thirteen hex digits with the
/// zero digits at its end removed, but at least one kept.
pub fn double_to_hex_string(d: f64) -> String {
    if !d.is_finite() {
        return double_to_string(d);
    }
    let bits = d.to_bits();
    let sign = if bits >> 63 == 1 { "-" } else { "" };
    if d == 0.0 {
        return format!("{sign}0x0.0p0");
    }
    let biased = (bits >> 52) & 0x7ff;
    let significand = bits & ((1 << 52) - 1);
    // The number of zero digits at the end of the significand, which is at most 12 because the
    // bit 48 is forced on: the digit that stays is then the one at bit 48 or above.
    let zeros = ((significand | 1 << 48).trailing_zeros() / 4) as usize;
    let (lead, exponent) = if biased == 0 { ('0', -1022) } else { ('1', biased as i32 - 1023) };
    let digits: String =
        (zeros..13).rev().map(|i| HEX[((significand >> (4 * i)) & 0xf) as usize] as char).collect();
    format!("{sign}0x{lead}.{digits}p{exponent}")
}

/// Java's `Float.toHexString(float)`. A subnormal float is written as the subnormal double of
/// the same value, whose exponent `-1022` becomes `-126`; any other float is written as its
/// value as a double, which is exact.
pub fn float_to_hex_string(f: f32) -> String {
    if f.abs() < f32::MIN_POSITIVE && f != 0.0 {
        return double_to_hex_string(f as f64 * SCALE).replace("-1022", "-126");
    }
    double_to_hex_string(f as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doubles_print_like_java() {
        // Each case is a double and what Java 26 prints for it.
        let cases = [
            (0.0, "0x0.0p0"),
            (-0.0, "-0x0.0p0"),
            (f64::NAN, "NaN"),
            (f64::from_bits(0x7ff8_0000_0000_0001), "NaN"),
            (f64::INFINITY, "Infinity"),
            (f64::NEG_INFINITY, "-Infinity"),
            (1.0, "0x1.0p0"),
            (-1.0, "-0x1.0p0"),
            (f64::from_bits(1), "0x0.0000000000001p-1022"),
            (-f64::from_bits(1), "-0x0.0000000000001p-1022"),
            (f64::from_bits(3), "0x0.0000000000003p-1022"),
            (f64::MIN_POSITIVE, "0x1.0p-1022"),
            (f64::from_bits(0x0008_0000_0000_0000), "0x0.8p-1022"),
            (f64::from_bits(0x000f_ffff_ffff_ffff), "0x0.fffffffffffffp-1022"),
            (f64::from_bits(0x0010_0000_0000_0001), "0x1.0000000000001p-1022"),
            (f64::MAX, "0x1.fffffffffffffp1023"),
            (-f64::MAX, "-0x1.fffffffffffffp1023"),
            (0.1, "0x1.999999999999ap-4"),
            (0.5, "0x1.0p-1"),
            (1.5, "0x1.8p0"),
            (2.0, "0x1.0p1"),
            (3.0, "0x1.8p1"),
            (-2.5, "-0x1.4p1"),
            (255.5, "0x1.ffp7"),
            (1024.0, "0x1.0p10"),
            (1.0 / 3.0, "0x1.5555555555555p-2"),
            (std::f64::consts::PI, "0x1.921fb54442d18p1"),
            (1e300, "0x1.7e43c8800759cp996"),
            (1e-300, "0x1.56e1fc2f8f359p-997"),
        ];
        for (d, java) in cases {
            assert_eq!(double_to_hex_string(d), java, "{d:e}");
        }
    }

    #[test]
    fn floats_print_like_java() {
        // Each case is a float and what Java 26 prints for it.
        let cases = [
            (0.0, "0x0.0p0"),
            (-0.0, "-0x0.0p0"),
            (f32::NAN, "NaN"),
            (f32::from_bits(0x7fc0_0001), "NaN"),
            (f32::INFINITY, "Infinity"),
            (f32::NEG_INFINITY, "-Infinity"),
            (1.0, "0x1.0p0"),
            (-1.0, "-0x1.0p0"),
            (f32::from_bits(1), "0x0.000002p-126"),
            (-f32::from_bits(1), "-0x0.000002p-126"),
            (f32::from_bits(2), "0x0.000004p-126"),
            (f32::from_bits(3), "0x0.000006p-126"),
            (f32::MIN_POSITIVE, "0x1.0p-126"),
            (f32::from_bits(0x0040_0000), "0x0.8p-126"),
            (f32::from_bits(0x007f_ffff), "0x0.fffffep-126"),
            (f32::from_bits(0x0080_0001), "0x1.000002p-126"),
            (f32::from_bits(0x0001_16c2), "0x0.022d84p-126"),
            (f32::MAX, "0x1.fffffep127"),
            (-f32::MAX, "-0x1.fffffep127"),
            (0.1, "0x1.99999ap-4"),
            (0.5, "0x1.0p-1"),
            (1.5, "0x1.8p0"),
            (3.0, "0x1.8p1"),
            (-2.5, "-0x1.4p1"),
            (255.5, "0x1.ffp7"),
            (65504.0, "0x1.ffcp15"),
            (1e10, "0x1.2a05f2p33"),
            (0.33333334, "0x1.555556p-2"),
            (std::f32::consts::PI, "0x1.921fb6p1"),
        ];
        for (f, java) in cases {
            assert_eq!(float_to_hex_string(f), java, "{f:e}");
        }
    }

    #[test]
    fn a_float_is_its_double_unless_subnormal() {
        for f in [1.0f32, 0.1, -7.5, 1e30, f32::MAX] {
            assert_eq!(float_to_hex_string(f), double_to_hex_string(f as f64));
        }
    }
}
