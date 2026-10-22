//! `Math.scalb` and `StrictMath.scalb`: `d * 2^n`, computed by the JDK's own steps (`Math.java`),
//! so each multiplication is exact or rounds once, as Java's does.

/// `2^1023`: normal and exact, the JDK's `F_UP`.
const F_UP: f64 = f64::from_bits(2046 << 52);

/// `2^-1023`: subnormal and exact, the JDK's `F_DOWN`.
const F_DOWN: f64 = f64::from_bits(1 << 51);

/// `2^n` for `-1022 <= n <= 1023`, a normal power of two (the JDK's `primPowerOfTwoD`).
fn power_of_two(n: i32) -> f64 {
    f64::from_bits(((n + 1023) as u64) << 52)
}

/// `Math.scalb(double, int)`.
pub fn scalb(d: f64, scale_factor: i32) -> f64 {
    if scale_factor > -1023 {
        if scale_factor <= 1023 {
            return d * power_of_two(scale_factor);
        }
        if scale_factor <= 2 * 1023 {
            return d * power_of_two(scale_factor - 1023) * F_UP;
        }
        if scale_factor < 2 * 1023 + 53 - 1 {
            return d * power_of_two(scale_factor - 2 * 1023) * F_UP * F_UP;
        }
        return d * F_UP * F_UP * F_UP;
    }
    if scale_factor > -2 * 1023 {
        return d * power_of_two(scale_factor + 1023) * F_DOWN;
    }
    if scale_factor > -2 * 1023 - 53 {
        return d * power_of_two(scale_factor + 2 * 1023) * F_DOWN * F_DOWN;
    }
    d * f64::from_bits(1) * f64::from_bits(1)
}

/// `Math.scalb(float, int)`: the scale is clamped to `±278` (the JDK's `MAX_SCALE`), where every
/// result is an overflow, an underflow or unchanged, and the product is exact in `double`, so the
/// one rounding to `float` is the correctly rounded result.
pub fn scalb_f32(f: f32, scale_factor: i32) -> f32 {
    const MAX_SCALE: i32 = 127 + 126 + 24 + 1;
    (f as f64 * power_of_two(scale_factor.clamp(-MAX_SCALE, MAX_SCALE))) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalb_doubles_at_the_edges() {
        assert_eq!(scalb(1.5, 10), 1536.0);
        assert_eq!(scalb(1.0, 1023), f64::from_bits(2046 << 52));
        assert_eq!(scalb(1.0, 1024), f64::INFINITY);
        assert_eq!(scalb(-1.0, i32::MAX), f64::NEG_INFINITY);
        assert_eq!(scalb(1.0, -1074), f64::from_bits(1));
        assert_eq!(scalb(1.0, -1075), 0.0);
        assert_eq!(scalb(-1.0, i32::MIN), -0.0);
        assert!(scalb(f64::NAN, 5).is_nan());
        assert_eq!(scalb(f64::INFINITY, -5), f64::INFINITY);
    }

    #[test]
    fn scalb_floats_at_the_edges() {
        assert_eq!(scalb_f32(1.5, -3), 0.1875);
        assert_eq!(scalb_f32(1.0, 128), f32::INFINITY);
        assert_eq!(scalb_f32(1.0, -149), f32::from_bits(1));
        assert_eq!(scalb_f32(1.0, -150), 0.0);
        assert_eq!(scalb_f32(-0.0, 3), -0.0);
        assert!(scalb_f32(f32::NAN, -300).is_nan());
    }
}
