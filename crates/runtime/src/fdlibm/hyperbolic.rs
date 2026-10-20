//! `e_sinh.c`, `e_cosh.c` and `s_tanh.c`.

use super::exp_log::{exp, expm1};
use super::{hi, lo};

const ONE: f64 = 1.0;
const TWO: f64 = 2.0;
const HALF: f64 = 0.5;
const HUGE: f64 = 1.0e300;
const TINY: f64 = 1.0e-300;
const SHUGE: f64 = 1.0e307;

/// `__ieee754_sinh`: the hyperbolic sine of `x`.
pub fn sinh(x: f64) -> f64 {
    // Method, on |x| (sinh(-x) = -sinh(x)), with E = expm1(x):
    //   0        <= x <= 22     :  sinh(x) := (E + E/(E+1))/2
    //   22       <= x <= lnovft :  sinh(x) := exp(x)/2
    //   lnovft   <= x <= ln2ovft:  sinh(x) := exp(x/2)/2 * exp(x/2)
    //   ln2ovft  <  x           :  sinh(x) := x*shuge (overflow)

    // High word of |x|.
    let jx = hi(x);
    let ix = jx & 0x7fffffff;

    // x is INF or NaN
    if ix >= 0x7ff00000 {
        return x + x;
    }

    let mut h = 0.5;
    if jx < 0 {
        h = -h;
    }
    // |x| in [0,22], return sign(x)*0.5*(E+E/(E+1)))
    if ix < 0x40360000 {
        // |x|<22
        if ix < 0x3e300000 && SHUGE + x > ONE {
            // |x|<2**-28
            return x; // sinh(tiny) = tiny with inexact
        }
        let t = expm1(x.abs());
        if ix < 0x3ff00000 {
            return h * (2.0 * t - t * t / (t + ONE));
        }
        return h * (t + t / (t + ONE));
    }

    // |x| in [22, log(maxdouble)] return 0.5*exp(|x|)
    if ix < 0x40862E42 {
        return h * exp(x.abs());
    }

    // |x| in [log(maxdouble), overflowthresold]
    let lx = lo(x);
    if ix < 0x408633CE || (ix == 0x408633ce && lx <= 0x8fb9f87d) {
        let w = exp(0.5 * x.abs());
        let t = h * w;
        return t * w;
    }

    // |x| > overflowthresold, sinh(x) overflow
    x * SHUGE
}

/// `__ieee754_cosh`: the hyperbolic cosine of `x`.
pub fn cosh(x: f64) -> f64 {
    // Method, on |x| (cosh(x) = cosh(-x)):
    //   0        <= x <= ln2/2  :  cosh(x) := 1 + (exp(x)-1)^2 / (2*exp(x))
    //   ln2/2    <= x <= 22     :  cosh(x) := (exp(x) + 1/exp(x))/2
    //   22       <= x <= lnovft :  cosh(x) := exp(x)/2
    //   lnovft   <= x <= ln2ovft:  cosh(x) := exp(x/2)/2 * exp(x/2)
    //   ln2ovft  <  x           :  cosh(x) := huge*huge (overflow)

    // High word of |x|.
    let mut ix = hi(x);
    ix &= 0x7fffffff;

    // x is INF or NaN
    if ix >= 0x7ff00000 {
        return x * x;
    }

    // |x| in [0,0.5*ln2], return 1+expm1(|x|)^2/(2*exp(|x|))
    if ix < 0x3fd62e43 {
        let t = expm1(x.abs());
        let w = ONE + t;
        if ix < 0x3c800000 {
            return w; // cosh(tiny) = 1
        }
        return ONE + (t * t) / (w + w);
    }

    // |x| in [0.5*ln2,22], return (exp(|x|)+1/exp(|x|)/2;
    if ix < 0x40360000 {
        let t = exp(x.abs());
        return HALF * t + HALF / t;
    }

    // |x| in [22, log(maxdouble)] return half*exp(|x|)
    if ix < 0x40862E42 {
        return HALF * exp(x.abs());
    }

    // |x| in [log(maxdouble), overflowthresold]
    let lx = lo(x);
    if ix < 0x408633CE || (ix == 0x408633ce && lx <= 0x8fb9f87d) {
        let w = exp(HALF * x.abs());
        let t = HALF * w;
        return t * w;
    }

    // |x| > overflowthresold, cosh(x) overflow
    HUGE * HUGE
}

/// `tanh`: the hyperbolic tangent of `x`.
pub fn tanh(x: f64) -> f64 {
    // Method, on |x| (tanh(-x) = -tanh(x)):
    //   0      <= x <= 2**-55 : tanh(x) := x*(one+x)
    //   2**-55 <  x <=  1     : tanh(x) := -t/(t+2); t = expm1(-2x)
    //   1      <= x <=  22.0  : tanh(x) := 1 - 2/(t+2); t = expm1(2x)
    //   22.0   <  x <= INF    : tanh(x) := 1.

    // High word of |x|.
    let jx = hi(x);
    let ix = jx & 0x7fffffff;

    // x is INF or NaN
    if ix >= 0x7ff00000 {
        if jx >= 0 {
            return ONE / x + ONE; // tanh(+-inf)=+-1
        } else {
            return ONE / x - ONE; // tanh(NaN) = NaN
        }
    }

    let z;
    if ix < 0x40360000 {
        // |x|<22
        if ix < 0x3c800000 {
            // |x|<2**-55
            return x * (ONE + x); // tanh(small) = small
        }
        if ix >= 0x3ff00000 {
            // |x|>=1
            let t = expm1(TWO * x.abs());
            z = ONE - TWO / (t + TWO);
        } else {
            let t = expm1(-TWO * x.abs());
            z = -t / (t + TWO);
        }
    } else {
        // |x| > 22, return +-1
        z = ONE - TINY; // raised inexact flag
    }
    if jx >= 0 { z } else { -z }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Expected bits come from Java's StrictMath (OpenJDK on x86-64).
    fn check(f: fn(f64) -> f64, cases: &[(f64, u64)]) {
        for &(x, want) in cases {
            let got = f(x);
            assert_eq!(got.to_bits(), want, "{x:e}: got {got:e}, want {:e}", f64::from_bits(want));
        }
    }

    #[test]
    fn sinh_matches_strictmath() {
        check(
            sinh,
            &[
                (f64::from_bits(0x408633ce8fb9f87d), 0x7feffffffffffd3b), // overflow threshold
                (f64::from_bits(0x408633ce8fb9f87e), 0x7ff0000000000000), // just above it -> inf
                (f64::from_bits(0xc08633ce8fb9f87d), 0xffeffffffffffd3b), // -overflow threshold
                (f64::from_bits(0x4036000000000000), 0x41dab5adb9c43600), // 22.0
                (f64::from_bits(0x4035ffffffffffff), 0x41dab5adb9c435e5), // just below 22
                (f64::from_bits(0x3e2fffffffffffff), 0x3e2fffffffffffff), // just below 2**-28
                (f64::from_bits(0x3fefffffffffffff), 0x3ff2cd9fc44eb982), // just below 1
                (f64::from_bits(0x8000000000000001), 0x8000000000000001), // -5e-324
            ],
        );
    }

    #[test]
    fn cosh_matches_strictmath() {
        check(
            cosh,
            &[
                (f64::from_bits(0x408633ce8fb9f87d), 0x7feffffffffffd3b), // overflow threshold
                (f64::from_bits(0x408633ce8fb9f87e), 0x7ff0000000000000), // just above it -> inf
                (f64::from_bits(0xc08633ce8fb9f87d), 0x7feffffffffffd3b), // -overflow threshold
                (f64::from_bits(0x4036000000000000), 0x41dab5adb9c43600), // 22.0
                (f64::from_bits(0x3fd62e42fefa39ef), 0x3ff0f876ccdf6cd9), // 0.5*ln2
                (f64::from_bits(0x3fd62e43fefa39ef), 0x3ff0f876e3800bc8), // just above 0.5*ln2
                (f64::from_bits(0x3c7fffffffffffff), 0x3ff0000000000000), // below 2**-55 -> 1.0
                (f64::from_bits(0x0000000000000001), 0x3ff0000000000000), // 5e-324 -> 1.0
            ],
        );
    }

    #[test]
    fn tanh_matches_strictmath() {
        check(
            tanh,
            &[
                (f64::from_bits(0x3c80000000000000), 0x3c80000000000000), // 2**-55
                (f64::from_bits(0x3c7fffffffffffff), 0x3c7fffffffffffff), // just below 2**-55
                (f64::from_bits(0xbc7fffffffffffff), 0xbc7fffffffffffff), // its negation
                (f64::from_bits(0x4036000000000000), 0x3ff0000000000000), // 22.0 -> 1.0
                (f64::from_bits(0x4035ffffffffffff), 0x3ff0000000000000), // just below 22
                (f64::from_bits(0x3ff0000000000000), 0x3fe85efab514f394), // 1.0
                (f64::from_bits(0x3fefffffffffffff), 0x3fe85efab514f394), // just below 1
                (f64::from_bits(0xfff0000000000000), 0xbff0000000000000), // -inf -> -1.0
            ],
        );
    }
}
