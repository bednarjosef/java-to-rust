//! `e_exp.c`, `e_log.c`, `e_log10.c`, `s_log1p.c` and `s_expm1.c`.

use super::{hi, lo, with_hi};

const ONE: f64 = 1.0;
const ZERO: f64 = 0.0;
const HUGE: f64 = 1.0e+300;
const TINY: f64 = 1.0e-300;
const TWOM1000: f64 = 9.33263618503218878990e-302; // 2**-1000=0x01700000,0
const TWO54: f64 = 1.80143985094819840000e+16; // 0x43500000 00000000
const O_THRESHOLD: f64 = 7.09782712893383973096e+02; // 0x40862E42, 0xFEFA39EF
const U_THRESHOLD: f64 = -7.45133219101941108420e+02; // 0xc0874910, 0xD52D3051
const LN2_HI: f64 = 6.93147180369123816490e-01; // 0x3fe62e42, 0xfee00000
const LN2_LO: f64 = 1.90821492927058770002e-10; // 0x3dea39ef, 0x35793c76
const INVLN2: f64 = 1.44269504088896338700e+00; // 0x3ff71547, 0x652b82fe

// e_exp.c
const HALF: [f64; 2] = [0.5, -0.5];
const LN2HI: [f64; 2] = [LN2_HI, -LN2_HI];
const LN2LO: [f64; 2] = [LN2_LO, -LN2_LO];
const P1: f64 = 1.66666666666666019037e-01; // 0x3FC55555, 0x5555553E
const P2: f64 = -2.77777777770155933842e-03; // 0xBF66C16C, 0x16BEBD93
const P3: f64 = 6.61375632143793436117e-05; // 0x3F11566A, 0xAF25DE2C
const P4: f64 = -1.65339022054652515390e-06; // 0xBEBBBD41, 0xC5D26BF1
const P5: f64 = 4.13813679705723846039e-08; // 0x3E663769, 0x72BEA4D0

/// `__ieee754_exp`: e raised to the power `x`.
pub fn exp(x: f64) -> f64 {
    // Method: reduce x = k*ln2 + r with |r| <= 0.5*ln2 (r carried as hi-lo), approximate
    // exp(r) = 1 + r + r*R1(r)/(2-R1(r)) where R1 is a degree 5 polynomial in r*r, then scale
    // by 2^k.

    let mut x = x;
    let mut hi_ = 0.0;
    let mut lo_ = 0.0;
    let k: i32;

    let mut hx = hi(x) as u32; // high word of x
    let xsb = ((hx >> 31) & 1) as usize; // sign bit of x
    hx &= 0x7fffffff; // high word of |x|

    // filter out non-finite argument
    if hx >= 0x40862E42 {
        // if |x|>=709.78...
        if hx >= 0x7ff00000 {
            if ((hx & 0xfffff) | lo(x)) != 0 {
                return x + x; // NaN
            } else {
                return if xsb == 0 { x } else { 0.0 }; // exp(+-inf)={inf,0}
            }
        }
        if x > O_THRESHOLD {
            return HUGE * HUGE; // overflow
        }
        if x < U_THRESHOLD {
            return TWOM1000 * TWOM1000; // underflow
        }
    }

    // argument reduction
    if hx > 0x3fd62e42 {
        // if  |x| > 0.5 ln2
        if hx < 0x3FF0A2B2 {
            // and |x| < 1.5 ln2
            hi_ = x - LN2HI[xsb];
            lo_ = LN2LO[xsb];
            k = 1 - xsb as i32 - xsb as i32;
        } else {
            k = (INVLN2 * x + HALF[xsb]) as i32;
            let t = k as f64;
            hi_ = x - t * LN2HI[0]; // t*ln2HI is exact here
            lo_ = t * LN2LO[0];
        }
        x = hi_ - lo_;
    } else if hx < 0x3e300000 {
        // when |x|<2**-28
        if HUGE + x > ONE {
            return ONE + x; // trigger inexact
        }
        k = 0;
    } else {
        k = 0;
    }

    // x is now in primary range
    let t = x * x;
    let c = x - t * (P1 + t * (P2 + t * (P3 + t * (P4 + t * P5))));
    if k == 0 {
        return ONE - ((x * c) / (c - 2.0) - x);
    }
    let y = ONE - ((lo_ - (x * c) / (2.0 - c)) - hi_);
    if k >= -1021 {
        with_hi(y, hi(y).wrapping_add(k << 20)) // add k to y's exponent
    } else {
        let y = with_hi(y, hi(y).wrapping_add((k + 1000) << 20)); // add k to y's exponent
        y * TWOM1000
    }
}

// e_log.c and s_log1p.c share their polynomial (Lg1..Lg7 = Lp1..Lp7)
const LG1: f64 = 6.666666666666735130e-01; // 3FE55555 55555593
const LG2: f64 = 3.999999999940941908e-01; // 3FD99999 9997FA04
const LG3: f64 = 2.857142874366239149e-01; // 3FD24924 94229359
const LG4: f64 = 2.222219843214978396e-01; // 3FCC71C5 1D8E78AF
const LG5: f64 = 1.818357216161805012e-01; // 3FC74664 96CB03DE
const LG6: f64 = 1.531383769920937332e-01; // 3FC39A09 D078C69F
const LG7: f64 = 1.479819860511658591e-01; // 3FC2F112 DF3E5244

/// `__ieee754_log`: the natural logarithm of `x`.
#[allow(clippy::eq_op)] // (x-x)/zero makes the NaN arithmetically, as the C does
pub fn log(x: f64) -> f64 {
    // Method: reduce x = 2^k * (1+f) with sqrt(2)/2 < 1+f < sqrt(2); with s = f/(2+f),
    // log(1+f) = 2s + s*R(s*s) for a degree 14 polynomial R; then
    // log(x) = k*ln2_hi + (f - (hfsq - (s*(hfsq+R) + k*ln2_lo))) where hfsq = f*f/2.

    let mut x = x;
    let mut hx = hi(x); // high word of x
    let lx = lo(x); // low  word of x

    let mut k: i32 = 0;
    if hx < 0x00100000 {
        // x < 2**-1022
        if ((hx & 0x7fffffff) as u32 | lx) == 0 {
            return -TWO54 / ZERO; // log(+-0)=-inf
        }
        if hx < 0 {
            return (x - x) / ZERO; // log(-#) = NaN
        }
        k -= 54;
        x *= TWO54; // subnormal number, scale up x
        hx = hi(x); // high word of x
    }
    if hx >= 0x7ff00000 {
        return x + x;
    }
    k += (hx >> 20) - 1023;
    hx &= 0x000fffff;
    let mut i = (hx + 0x95f64) & 0x100000;
    x = with_hi(x, hx | (i ^ 0x3ff00000)); // normalize x or x/2
    k += i >> 20;
    let f = x - 1.0;
    if (0x000fffff & (2 + hx)) < 3 {
        // |f| < 2**-20
        if f == ZERO {
            if k == 0 {
                return ZERO;
            } else {
                let dk = k as f64;
                return dk * LN2_HI + dk * LN2_LO;
            }
        }
        let r = f * f * (0.5 - 0.33333333333333333 * f);
        if k == 0 {
            return f - r;
        } else {
            let dk = k as f64;
            return dk * LN2_HI - ((r - dk * LN2_LO) - f);
        }
    }
    let s = f / (2.0 + f);
    let dk = k as f64;
    let z = s * s;
    i = hx - 0x6147a;
    let w = z * z;
    let j = 0x6b851 - hx;
    let t1 = w * (LG2 + w * (LG4 + w * LG6));
    let t2 = z * (LG1 + w * (LG3 + w * (LG5 + w * LG7)));
    i |= j;
    let r = t2 + t1;
    if i > 0 {
        let hfsq = 0.5 * f * f;
        if k == 0 {
            f - (hfsq - s * (hfsq + r))
        } else {
            dk * LN2_HI - ((hfsq - (s * (hfsq + r) + dk * LN2_LO)) - f)
        }
    } else if k == 0 {
        f - s * (f - r)
    } else {
        dk * LN2_HI - ((s * (f - r) - dk * LN2_LO) - f)
    }
}

// e_log10.c
const IVLN10: f64 = 4.34294481903251816668e-01; // 0x3FDBCB7B, 0x1526E50E
const LOG10_2HI: f64 = 3.01029995663611771306e-01; // 0x3FD34413, 0x509F6000
const LOG10_2LO: f64 = 3.69423907715893078616e-13; // 0x3D59FEF3, 0x11F12B36

/// `__ieee754_log10`: the base 10 logarithm of `x`.
#[allow(clippy::eq_op)] // (x-x)/zero makes the NaN arithmetically, as the C does
pub fn log10(x: f64) -> f64 {
    // Method: n = ilogb(x) (plus one if negative), x = scalbn(x,-n), then
    // log10(x) = n*log10_2hi + (n*log10_2lo + ivln10*log(x)).

    let mut x = x;
    let mut hx = hi(x); // high word of x
    let lx = lo(x); // low word of x

    let mut k: i32 = 0;
    if hx < 0x00100000 {
        // x < 2**-1022
        if ((hx & 0x7fffffff) as u32 | lx) == 0 {
            return -TWO54 / ZERO; // log(+-0)=-inf
        }
        if hx < 0 {
            return (x - x) / ZERO; // log(-#) = NaN
        }
        k -= 54;
        x *= TWO54; // subnormal number, scale up x
        hx = hi(x); // high word of x
    }
    if hx >= 0x7ff00000 {
        return x + x;
    }
    k += (hx >> 20) - 1023;
    let i = ((k as u32 & 0x80000000) >> 31) as i32;
    hx = (hx & 0x000fffff) | ((0x3ff - i) << 20);
    let y = (k + i) as f64;
    x = with_hi(x, hx);
    let z = y * LOG10_2LO + IVLN10 * log(x);
    z + y * LOG10_2HI
}

/// `log1p`: the natural logarithm of `1 + x`, accurate for small `x`.
#[allow(clippy::eq_op)] // (x-x)/(x-x) makes the NaN arithmetically, as the C does
pub fn log1p(x: f64) -> f64 {
    // Method: as `log`, on u = 1+x rounded, adding back the correction c/u with c = (1+x)-u
    // when the reduction is not exact.

    let mut f = 0.0;
    let mut c = 0.0;
    let mut hu: i32 = 0;

    let hx = hi(x); // high word of x
    let ax = hx & 0x7fffffff;

    let mut k: i32 = 1;
    if hx < 0x3FDA827A {
        // x < 0.41422
        if ax >= 0x3ff00000 {
            // x <= -1.0
            if x == -1.0 {
                return -TWO54 / ZERO; // log1p(-1)=+inf
            } else {
                // log1p(x<-1)=NaN. The C computes (x-x)/(x-x); Java's StrictMath returns its
                // canonical NaN here (also for -inf and NaNs with the sign bit set).
                return f64::from_bits(0x7ff8_0000_0000_0000);
            }
        }
        if ax < 0x3e200000 {
            // |x| < 2**-29
            if TWO54 + x > ZERO // raise inexact
                && ax < 0x3c900000
            {
                // |x| < 2**-54
                return x;
            } else {
                return x - x * x * 0.5;
            }
        }
        if hx > 0 || hx <= 0xbfd2bec3_u32 as i32 {
            // -0.2929<x<0.41422
            k = 0;
            f = x;
            hu = 1;
        }
    }
    if hx >= 0x7ff00000 {
        return x + x;
    }
    if k != 0 {
        let mut u;
        if hx < 0x43400000 {
            u = 1.0 + x;
            hu = hi(u); // high word of u
            k = (hu >> 20) - 1023;
            c = if k > 0 { 1.0 - (u - x) } else { x - (u - 1.0) }; // correction term
            c /= u;
        } else {
            u = x;
            hu = hi(u); // high word of u
            k = (hu >> 20) - 1023;
            c = 0.0;
        }
        hu &= 0x000fffff;
        if hu < 0x6a09e {
            u = with_hi(u, hu | 0x3ff00000); // normalize u
        } else {
            k += 1;
            u = with_hi(u, hu | 0x3fe00000); // normalize u/2
            hu = (0x00100000 - hu) >> 2;
        }
        f = u - 1.0;
    }
    let hfsq = 0.5 * f * f;
    let dk = k as f64;
    if hu == 0 {
        // |f| < 2**-20
        if f == ZERO {
            if k == 0 {
                return ZERO;
            } else {
                c += dk * LN2_LO;
                return dk * LN2_HI + c;
            }
        }
        let r = hfsq * (1.0 - 0.66666666666666666 * f);
        if k == 0 {
            return f - r;
        } else {
            return dk * LN2_HI - ((r - (dk * LN2_LO + c)) - f);
        }
    }
    let s = f / (2.0 + f);
    let z = s * s;
    let r = z * (LG1 + z * (LG2 + z * (LG3 + z * (LG4 + z * (LG5 + z * (LG6 + z * LG7))))));
    if k == 0 {
        f - (hfsq - s * (hfsq + r))
    } else {
        dk * LN2_HI - ((hfsq - (s * (hfsq + r) + (dk * LN2_LO + c))) - f)
    }
}

// s_expm1.c: scaled coefficients related to expm1
const Q1: f64 = -3.33333333333331316428e-02; // BFA11111 111110F4
const Q2: f64 = 1.58730158725481460165e-03; // 3F5A01A0 19FE5585
const Q3: f64 = -7.93650757867487942473e-05; // BF14CE19 9EAADBB7
const Q4: f64 = 4.00821782732936239552e-06; // 3ED0CFCA 86E65239
const Q5: f64 = -2.01099218183624371326e-07; // BE8AFDB7 6E09C32D

/// `expm1`: `exp(x) - 1`, accurate for small `x`.
pub fn expm1(x: f64) -> f64 {
    // Method: reduce x = k*ln2 + r with a correction term c for the rounding of r, approximate
    // expm1(r) with a rational function built from a degree 5 polynomial in r*r (coefficients
    // scaled by 2^i, z replaced by r*r/2), then scale back as 2^k*[expm1(r)+1] - 1 or
    // 2^k*[expm1(r) + (1-2^-k)], picking the form that loses the least.

    let mut x = x;
    let mut c = 0.0;
    let k: i32;

    let mut hx = hi(x) as u32; // high word of x
    let xsb = hx & 0x80000000; // sign bit of x
    hx &= 0x7fffffff; // high word of |x|

    // filter out huge and non-finite argument
    if hx >= 0x4043687A {
        // if |x|>=56*ln2
        if hx >= 0x40862E42 {
            // if |x|>=709.78...
            if hx >= 0x7ff00000 {
                if ((hx & 0xfffff) | lo(x)) != 0 {
                    return x + x; // NaN
                } else {
                    return if xsb == 0 { x } else { -1.0 }; // exp(+-inf)={inf,-1}
                }
            }
            if x > O_THRESHOLD {
                return HUGE * HUGE; // overflow
            }
        }
        if xsb != 0 {
            // x < -56*ln2, return -1.0 with inexact
            if x + TINY < 0.0 {
                // raise inexact
                return TINY - ONE; // return -1
            }
        }
    }

    // argument reduction
    if hx > 0x3fd62e42 {
        // if  |x| > 0.5 ln2
        let hi_;
        let lo_;
        if hx < 0x3FF0A2B2 {
            // and |x| < 1.5 ln2
            if xsb == 0 {
                hi_ = x - LN2_HI;
                lo_ = LN2_LO;
                k = 1;
            } else {
                hi_ = x + LN2_HI;
                lo_ = -LN2_LO;
                k = -1;
            }
        } else {
            k = (INVLN2 * x + if xsb == 0 { 0.5 } else { -0.5 }) as i32;
            let t = k as f64;
            hi_ = x - t * LN2_HI; // t*ln2_hi is exact here
            lo_ = t * LN2_LO;
        }
        x = hi_ - lo_;
        c = (hi_ - x) - lo_;
    } else if hx < 0x3c900000 {
        // when |x|<2**-54, return x
        let t = HUGE + x; // return x with inexact flags when x!=0
        return x - (t - (HUGE + x));
    } else {
        k = 0;
    }

    // x is now in primary range
    let hfx = 0.5 * x;
    let hxs = x * hfx;
    let r1 = ONE + hxs * (Q1 + hxs * (Q2 + hxs * (Q3 + hxs * (Q4 + hxs * Q5))));
    let t = 3.0 - r1 * hfx;
    let mut e = hxs * ((r1 - t) / (6.0 - x * t));
    if k == 0 {
        return x - (x * e - hxs); // c is 0
    }
    e = x * (e - c) - c;
    e -= hxs;
    if k == -1 {
        return 0.5 * (x - e) - 0.5;
    }
    if k == 1 {
        if x < -0.25 {
            return -2.0 * (e - (x + 0.5));
        } else {
            return ONE + 2.0 * (x - e);
        }
    }
    if k <= -2 || k > 56 {
        // suffice to return exp(x)-1
        let y = ONE - (e - x);
        let y = with_hi(y, hi(y).wrapping_add(k << 20)); // add k to y's exponent
        return y - ONE;
    }
    let mut t = ONE;
    let mut y;
    if k < 20 {
        t = with_hi(t, 0x3ff00000 - (0x200000 >> k)); // t=1-2^-k
        y = t - (e - x);
        y = with_hi(y, hi(y).wrapping_add(k << 20)); // add k to y's exponent
    } else {
        t = with_hi(t, (0x3ff - k) << 20); // 2^-k
        y = x - (e + t);
        y += ONE;
        y = with_hi(y, hi(y).wrapping_add(k << 20)); // add k to y's exponent
    }
    y
}

#[cfg(test)]
mod tests {
    use super::*;

    // Expected bits come from Java's StrictMath (OpenJDK on x86-64). A NaN produced by
    // arithmetic is only checked to be NaN: its sign bit is the CPU's default (x86: 0xfff8...).
    fn check(f: fn(f64) -> f64, cases: &[(f64, u64)]) {
        for &(x, want) in cases {
            let got = f(x);
            if f64::from_bits(want).is_nan() {
                assert!(got.is_nan(), "{x:e}: got {got:e}");
            } else {
                assert_eq!(got.to_bits(), want, "{x:e}: got {got:e}, want {:e}", f64::from_bits(want));
            }
        }
    }

    #[test]
    fn exp_matches_strictmath() {
        check(
            exp,
            &[
                (f64::from_bits(0x0000000000000001), 0x3ff0000000000000), // 5e-324 -> 1.0
                (f64::from_bits(0x40862e42fefa39ef), 0x7fefffffffffff2a), // overflow threshold
                (f64::from_bits(0x40862e42fefa39f0), 0x7ff0000000000000), // just above it -> inf
                (f64::from_bits(0xc0874910d52d3051), 0x0000000000000001), // underflow threshold -> 5e-324
                (f64::from_bits(0xc0874910d52d3052), 0x0000000000000000), // just below it -> 0.0
                (f64::from_bits(0xc086240000000000), 0x000e6cf6d08897ac), // -708.5 -> subnormal
                (f64::from_bits(0x3e30000000000000), 0x3ff0000001000000), // 2**-28
                (f64::from_bits(0x3fd62e43fefa39ef), 0x3ff6a09ec101b61c), // just above 0.5*ln2
                (f64::from_bits(0xfff0000000000000), 0x0000000000000000), // -inf -> 0.0
            ],
        );
    }

    #[test]
    fn log_matches_strictmath() {
        check(
            log,
            &[
                (f64::from_bits(0x0000000000000001), 0xc0874385446d71c3), // 5e-324
                (f64::from_bits(0x000fffffffffffff), 0xc086232bdd7abcd2), // largest subnormal
                (f64::from_bits(0x3ff0000000000001), 0x3cafffffffffffff), // 1 + ulp
                (f64::from_bits(0x3fefffffffffffff), 0xbca0000000000000), // 1 - ulp/2
                (f64::from_bits(0x7fefffffffffffff), 0x40862e42fefa39ef), // MAX_VALUE
                (f64::from_bits(0x8000000000000000), 0xfff0000000000000), // -0.0 -> -inf
                (f64::from_bits(0xbff0000000000000), 0xfff8000000000000), // -1.0 -> NaN
                (f64::from_bits(0x7ff0000000000000), 0x7ff0000000000000), // inf -> inf
            ],
        );
    }

    #[test]
    fn log10_matches_strictmath() {
        check(
            log10,
            &[
                (f64::from_bits(0x0000000000000001), 0xc07434e6420f4374), // 5e-324
                (f64::from_bits(0x44b52d02c7e14af6), 0x4037000000000000), // 1e23 -> 23.0
                (f64::from_bits(0x408f400000000000), 0x4008000000000000), // 1000.0 -> 3.0
                (f64::from_bits(0x3fefffffffffffff), 0xbc8bcb7b1526e50e), // 1 - ulp/2
                (f64::from_bits(0x7fefffffffffffff), 0x40734413509f79ff), // MAX_VALUE
                (f64::from_bits(0x0000000000000000), 0xfff0000000000000), // 0.0 -> -inf
                (f64::from_bits(0x0010000000000000), 0xc0733a7146f72a42), // MIN_NORMAL
                (f64::from_bits(0x7ff0000000000000), 0x7ff0000000000000), // inf -> inf
            ],
        );
    }

    #[test]
    fn log1p_matches_strictmath() {
        check(
            log1p,
            &[
                (f64::from_bits(0xbfefffffffffffff), 0xc0425e4f7b2737fa), // -1 + ulp/2
                (f64::from_bits(0xbff0000000000000), 0xfff0000000000000), // -1.0 -> -inf
                (f64::from_bits(0x3c90000000000000), 0x3c90000000000000), // 2**-54
                (f64::from_bits(0x3e20000000000000), 0x3e1fffffff800000), // 2**-29
                (f64::from_bits(0x4340000000000000), 0x40425e4f7b2737fa), // 2**53
                (f64::from_bits(0xbfd2bec333018866), 0xbfd62e42fefa39ee), // 1/sqrt(2) - 1
                (f64::from_bits(0x8000000000000001), 0x8000000000000001), // -5e-324
                (f64::from_bits(0x7fefffffffffffff), 0x40862e42fefa39ef), // MAX_VALUE
            ],
        );
    }

    #[test]
    fn expm1_matches_strictmath() {
        check(
            expm1,
            &[
                (f64::from_bits(0x4043687a9f1af2b1), 0x436fffffffffffec), // 56*ln2
                (f64::from_bits(0xc043687a9f1af2b1), 0xbff0000000000000), // -56*ln2 -> -1.0
                (f64::from_bits(0xbfd3333333333333), 0xbfd0966f2c7907f6), // -0.3 (k = -1)
                (f64::from_bits(0x4034000000000000), 0x41bceb088a68e804), // 20.0 (k >= 20)
                (f64::from_bits(0x4049000000000000), 0x44719103e4080b45), // 50.0 (k > 56 form)
                (f64::from_bits(0x40862e42fefa39ef), 0x7fefffffffffff2a), // overflow threshold
                (f64::from_bits(0x3c8fffffffffffff), 0x3c8fffffffffffff), // below 2**-54
                (f64::from_bits(0xfff0000000000000), 0xbff0000000000000), // -inf -> -1.0
            ],
        );
    }
}
