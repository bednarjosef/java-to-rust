//! `Math` and `StrictMath` beyond what Rust's float methods already do the Java way.
//!
//! Which implementation a function needs depends on HotSpot. `StrictMath` is fdlibm, bit for bit
//! (`crate::fdlibm`). `Math` delegates to `StrictMath` except for the functions HotSpot replaces
//! with its own x86-64 code: `sin`, `cos`, `tan`, `exp`, `log`, `log10`, `pow`, `cbrt`, `sinh` and
//! `tanh` (as of JDK 26), which differ from fdlibm in the last bit for 1 to 10 percent of
//! arguments. HotSpot's code cannot be ported (its license is GPL-2.0-only), so the generated code
//! uses the closest available implementation, measured on about 90,000 arguments per function:
//! the platform's libm through Rust's float methods (`x.sin()` agrees with `Math.sin` on all but
//! about 0.1 percent of arguments, where fdlibm differs on 2 percent), and for `log10`, where
//! HotSpot is correctly rounded and libm is not, the correctly rounded `log10` below.

use crate::Integer;

/// `Math.floorDiv` on `int` and `long`: the quotient rounded toward negative infinity; throws
/// `ArithmeticException` on a zero divisor, and `MIN_VALUE / -1` wraps.
pub fn floor_div<T: Integer + PartialOrd + Default + std::ops::Sub<Output = T> + From<i8>>(a: T, b: T) -> T {
    let q = crate::div(a, b);
    let zero = T::default();
    if (a < zero) != (b < zero) && crate::rem(a, b) != zero { q - T::from(1) } else { q }
}

/// `Math.floorMod` on `int` and `long`: the remainder with the sign of the divisor.
pub fn floor_mod<T: Integer + PartialOrd + Default + std::ops::Add<Output = T>>(a: T, b: T) -> T {
    let m = crate::rem(a, b);
    let zero = T::default();
    if m != zero && (m < zero) != (b < zero) { m + b } else { m }
}

/// `Math.round(double)`: the closest `long`, ties toward positive infinity; NaN is 0 and values
/// beyond the range of `long` saturate, as Rust's `as` does.
pub fn round(x: f64) -> i64 {
    let floor = x.floor();
    // Exact: below 2^52 the difference is a fraction of the same binade, above it is zero.
    if x - floor >= 0.5 { (floor + 1.0) as i64 } else { floor as i64 }
}

/// `Math.round(float)`: the closest `int`, ties toward positive infinity.
pub fn round_f32(x: f32) -> i32 {
    let floor = x.floor();
    if x - floor >= 0.5 { (floor + 1.0) as i32 } else { floor as i32 }
}

/// `Math.signum(double)`: zero and NaN are returned as they are (Rust's `signum` gives 1.0 for
/// 0.0), anything else is 1.0 with the sign of `x`.
pub fn signum(x: f64) -> f64 {
    if x == 0.0 || x.is_nan() { x } else { 1.0f64.copysign(x) }
}

/// `Math.signum(float)`.
pub fn signum_f32(x: f32) -> f32 {
    if x == 0.0 || x.is_nan() { x } else { 1.0f32.copysign(x) }
}

/// `Math.ulp(double)`: the distance from `x` to the next double away from zero, the smallest
/// subnormal for zero and subnormals, and `|x|` for infinities and NaN.
pub fn ulp(x: f64) -> f64 {
    let exp = ((x.to_bits() >> 52) & 0x7ff) as i32 - 1023;
    match exp {
        1024 => x.abs(),
        -1023 => f64::from_bits(1),
        _ => {
            let exp = exp - 52;
            if exp >= -1022 {
                f64::from_bits(((exp + 1023) as u64) << 52)
            } else {
                f64::from_bits(1 << (exp + 1074))
            }
        }
    }
}

/// `Math.ulp(float)`.
pub fn ulp_f32(x: f32) -> f32 {
    let exp = ((x.to_bits() >> 23) & 0xff) as i32 - 127;
    match exp {
        128 => x.abs(),
        -127 => f32::from_bits(1),
        _ => {
            let exp = exp - 23;
            if exp >= -126 {
                f32::from_bits(((exp + 127) as u32) << 23)
            } else {
                f32::from_bits(1 << (exp + 149))
            }
        }
    }
}

/// `Math.nextUp(double)`: the next double toward positive infinity; NaN and infinity are
/// returned as they are, and both zeros go to the smallest subnormal.
pub fn next_up(x: f64) -> f64 {
    if x < f64::INFINITY {
        let bits = (x + 0.0).to_bits() as i64;
        f64::from_bits((bits + if bits >= 0 { 1 } else { -1 }) as u64)
    } else {
        x
    }
}

/// `Math.nextUp(float)`.
pub fn next_up_f32(x: f32) -> f32 {
    if x < f32::INFINITY {
        let bits = (x + 0.0).to_bits() as i32;
        f32::from_bits((bits + if bits >= 0 { 1 } else { -1 }) as u32)
    } else {
        x
    }
}

/// `Math.nextDown(double)`: the next double toward negative infinity, by symmetry with `next_up`.
pub fn next_down(x: f64) -> f64 {
    -next_up(-x)
}

/// `Math.nextDown(float)`.
pub fn next_down_f32(x: f32) -> f32 {
    -next_up_f32(-x)
}

/// `Math.log10`, correctly rounded as HotSpot's is (the platform's `log10` is not: it differs from
/// Java on about 4 percent of arguments). The logarithm is computed in double-double arithmetic,
/// about 104 bits, and rounded once.
pub fn log10(x: f64) -> f64 {
    if x.is_nan() || x == f64::INFINITY {
        return x + x;
    }
    if x == 0.0 {
        return f64::NEG_INFINITY;
    }
    if x < 0.0 {
        // The NaN an x86 division makes, as HotSpot's code returns (`-inf - -inf` is one too).
        #[allow(clippy::eq_op)]
        return (x - x) / 0.0;
    }
    if x == 1.0 {
        return 0.0;
    }
    // x = m * 2^e with m in [sqrt(1/2), sqrt(2)).
    let (mut bits, mut e) = (x.to_bits(), 0i32);
    if bits < 1 << 52 {
        bits = (x * f64::from_bits(0x4350_0000_0000_0000)).to_bits(); // 2^54
        e -= 54;
    }
    e += ((bits >> 52) as i32) - 1023;
    let mut m = f64::from_bits((bits & ((1 << 52) - 1)) | (1023 << 52));
    if m > std::f64::consts::SQRT_2 {
        m /= 2.0;
        e += 1;
    }
    // log(m) = 2 atanh(s) = 2 (s + s^3/3 + s^5/5 + ...) with s = (m - 1) / (m + 1), |s| < 0.172.
    let f = m - 1.0;
    let s = dd::div(dd::from(f), dd::two_sum(2.0, f));
    let s2 = dd::mul(s, s);
    let mut series = dd::from(1.0 / 45.0);
    for k in (0..22).rev() {
        series = dd::add(dd::mul(series, s2), dd::div(dd::from(1.0), dd::from((2 * k + 1) as f64)));
    }
    let log_m = dd::mul(dd::mul(s, series), dd::from(2.0));
    const LN_2: dd::Dd = (std::f64::consts::LN_2, 2.3190468138462996e-17);
    const INV_LN_10: dd::Dd = (std::f64::consts::LOG10_E, 1.098319650216765e-17);
    let log_x = dd::add(dd::mul(LN_2, dd::from(e as f64)), log_m);
    let (hi, lo) = dd::mul(log_x, INV_LN_10);
    hi + lo
}

/// Double-double arithmetic: a value is an unevaluated sum `hi + lo` with `|lo| <= ulp(hi) / 2`.
mod dd {
    pub type Dd = (f64, f64);

    pub fn from(x: f64) -> Dd {
        (x, 0.0)
    }

    /// `a + b` exactly.
    pub fn two_sum(a: f64, b: f64) -> Dd {
        let s = a + b;
        let v = s - a;
        (s, (a - (s - v)) + (b - v))
    }

    /// `a * b` exactly.
    fn two_prod(a: f64, b: f64) -> Dd {
        let p = a * b;
        (p, a.mul_add(b, -p))
    }

    pub fn add(a: Dd, b: Dd) -> Dd {
        let (s, e) = two_sum(a.0, b.0);
        let (t, f) = two_sum(a.1, b.1);
        let (s, e) = two_sum(s, e + t);
        two_sum(s, e + f)
    }

    pub fn mul(a: Dd, b: Dd) -> Dd {
        let (p, e) = two_prod(a.0, b.0);
        two_sum(p, e + (a.0 * b.1 + a.1 * b.0))
    }

    pub fn div(a: Dd, b: Dd) -> Dd {
        let q1 = a.0 / b.0;
        let r = add(a, mul(b, (-q1, 0.0)));
        let q2 = r.0 / b.0;
        let r = add(r, mul(b, (-q2, 0.0)));
        let q3 = r.0 / b.0;
        add(two_sum(q1, q2), (q3, 0.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floor_div_and_mod_match_java() {
        // Expected values from Java's Math.floorDiv and Math.floorMod.
        assert_eq!((floor_div(7, 2), floor_div(-7, 2), floor_div(7, -2), floor_div(-7, -2)), (3, -4, -4, 3));
        assert_eq!((floor_mod(7, 3), floor_mod(-7, 3), floor_mod(7, -3), floor_mod(-7, -3)), (1, 2, -2, -1));
        assert_eq!(floor_div(i32::MIN, -1), i32::MIN);
        assert_eq!(floor_mod(i64::MIN, -1), 0);
        assert_eq!(floor_div(-6i64, 3), -2);
    }

    #[test]
    fn round_matches_java() {
        // Expected values from Java's Math.round.
        let cases = [
            (0.49999999999999994, 0),
            (0.5, 1),
            (-0.5, 0),
            (-2.5, -2),
            (2.5, 3),
            (f64::NAN, 0),
            (1e19, i64::MAX),
            (-1e19, i64::MIN),
            (4503599627370497.0, 4503599627370497),
        ];
        for (x, java) in cases {
            assert_eq!(round(x), java, "{x}");
        }
        assert_eq!((round_f32(0.49999997), round_f32(-1.5), round_f32(3e9)), (0, -1, i32::MAX));
    }

    #[test]
    fn small_functions_match_java() {
        // Expected values from Java's Math.
        assert_eq!(signum(-0.0).to_bits(), (-0.0f64).to_bits());
        assert_eq!(signum(-3.0), -1.0);
        assert_eq!(ulp(1.0), 2.220446049250313e-16);
        assert_eq!(ulp(0.0), f64::from_bits(1));
        assert_eq!(ulp(f64::MAX), 1.99584030953472e292);
        assert_eq!(ulp(-f64::INFINITY), f64::INFINITY);
        assert_eq!(ulp_f32(1.0), 1.1920929e-7);
        assert_eq!(next_up(-0.0), f64::from_bits(1));
        assert_eq!(next_up(-f64::from_bits(1)).to_bits(), (-0.0f64).to_bits());
        assert_eq!(next_up(1.0), 1.0000000000000002);
        assert_eq!(next_up_f32(f32::MAX), f32::INFINITY);
    }

    #[test]
    fn log10_is_correctly_rounded() {
        // Expected values from Java's Math.log10 where the platform's log10 differs.
        let cases = [
            (0x4024000000000000, 0x3ff0000000000000),
            (0x0000000000000001, 0xc07434e6420f4374),
            (0x000fffffffffffff, 0xc0733a7146f72a42),
            (0x400921fb54442d18, 0x3fdfd14db31ba3bb),
            (0x45caa2832e8cc39e, 0x403c379521258ec0),
            (0x7fefffffffffffff, 0x40734413509f79ff),
        ];
        for (x, java) in cases {
            assert_eq!(log10(f64::from_bits(x)).to_bits(), java, "{x:x}");
        }
        for k in 0..23 {
            assert_eq!(log10(10f64.powi(k)), k as f64);
        }
    }
}
