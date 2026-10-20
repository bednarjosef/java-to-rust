//! The inverse trigonometric functions: `e_asin.c`, `e_acos.c`, `s_atan.c` and `e_atan2.c`.

use super::{hi, lo, with_hi, with_lo};

const ONE: f64 = 1.00000000000000000000e+00; // 0x3FF00000, 0x00000000
const PI: f64 = 3.14159265358979311600e+00; // 0x400921FB, 0x54442D18
const PIO2_HI: f64 = 1.57079632679489655800e+00; // 0x3FF921FB, 0x54442D18
const PIO2_LO: f64 = 6.12323399573676603587e-17; // 0x3C91A626, 0x33145C07
const PIO4_HI: f64 = 7.85398163397448278999e-01; // 0x3FE921FB, 0x54442D18

// Coefficients of R(x^2), shared by asin and acos.
const PS0: f64 = 1.66666666666666657415e-01; // 0x3FC55555, 0x55555555
const PS1: f64 = -3.25565818622400915405e-01; // 0xBFD4D612, 0x03EB6F7D
const PS2: f64 = 2.01212532134862925881e-01; // 0x3FC9C155, 0x0E884455
const PS3: f64 = -4.00555345006794114027e-02; // 0xBFA48228, 0xB5688F3B
const PS4: f64 = 7.91534994289814532176e-04; // 0x3F49EFE0, 0x7501B288
const PS5: f64 = 3.47933107596021167570e-05; // 0x3F023DE1, 0x0DFDF709
const QS1: f64 = -2.40339491173441421878e+00; // 0xC0033A27, 0x1C8A2D4B
const QS2: f64 = 2.02094576023350569471e+00; // 0x40002AE5, 0x9C598AC8
const QS3: f64 = -6.88283971605453293030e-01; // 0xBFE6066C, 0x1B8D0159
const QS4: f64 = 7.70381505559019352791e-02; // 0x3FB3B8C5, 0xB12E9282

/// Arc sine of `x`, `__ieee754_asin`.
///
/// Method: asin(x) = x + x*x^2*R(x^2) on [0,0.5], R a rational approximation of
/// (asin(x)-x)/x^3. For x in [0.5,1], asin(x) = pi/2-2*asin(sqrt((1-x)/2)); with y = 1-x,
/// z = y/2, s = sqrt(z):
///   for x > 0.975, asin(x) = pio2_hi - (2*(s+s*z*R(z)) - pio2_lo);
///   otherwise, with f = hi part of s and c = (z-f*f)/(s+f) (so f+c = sqrt(z)),
///   asin(x) = pio4_hi+(pio4-2f)-(2s*z*R(z)-(pio2_lo+2c)).
/// asin(NaN) is NaN, asin(|x|>1) is NaN with invalid.
pub fn asin(x: f64) -> f64 {
    let hx = hi(x);
    let ix = hx & 0x7fffffff;
    if ix >= 0x3ff00000 {
        // |x| >= 1
        if ((ix - 0x3ff00000) as u32 | lo(x)) == 0 {
            // asin(1) = +-pi/2 with inexact
            return x * PIO2_HI + x * PIO2_LO;
        }
        #[allow(clippy::eq_op)] // the NaN must come from arithmetic, as in C
        return (x - x) / (x - x); // asin(|x|>1) is NaN
    } else if ix < 0x3fe00000 {
        // |x| < 0.5
        if ix < 0x3e400000 {
            // |x| < 2**-27: the C code returns x when huge+x > one, which always holds here
            return x;
        }
        let t = x * x;
        let p = t * (PS0 + t * (PS1 + t * (PS2 + t * (PS3 + t * (PS4 + t * PS5)))));
        let q = ONE + t * (QS1 + t * (QS2 + t * (QS3 + t * QS4)));
        let w = p / q;
        return x + x * w;
    }
    // 1 > |x| >= 0.5
    let w = ONE - x.abs();
    let mut t = w * 0.5;
    let mut p = t * (PS0 + t * (PS1 + t * (PS2 + t * (PS3 + t * (PS4 + t * PS5)))));
    let mut q = ONE + t * (QS1 + t * (QS2 + t * (QS3 + t * QS4)));
    let s = t.sqrt();
    if ix >= 0x3FEF3333 {
        // |x| > 0.975
        let w = p / q;
        t = PIO2_HI - (2.0 * (s + s * w) - PIO2_LO);
    } else {
        let w = with_lo(s, 0);
        let c = (t - w * w) / (s + w);
        let r = p / q;
        p = 2.0 * s * r - (PIO2_LO - 2.0 * c);
        q = PIO4_HI - 2.0 * w;
        t = PIO4_HI - (p - q);
    }
    if hx > 0 { t } else { -t }
}

/// Arc cosine of `x`, `__ieee754_acos`.
///
/// Method: acos(x) = pi/2 - asin(x), acos(-x) = pi/2 + asin(x).
///   For |x| <= 0.5, acos(x) = pi/2 - (x + x*x^2*R(x^2)) (see asin).
///   For x > 0.5, acos(x) = 2asin(sqrt((1-x)/2)) = 2f + (2c + 2s*z*R(z)), z = (1-x)/2,
///   s = sqrt(z), f = hi part of s and c = (z-f*f)/(s+f) the correction term so f+c ~ sqrt(z).
///   For x < -0.5, acos(x) = pi - 2asin(sqrt((1-|x|)/2)) = pi - 0.5*(s+s*z*R(z)).
/// acos(NaN) is NaN, acos(|x|>1) is NaN with invalid.
pub fn acos(x: f64) -> f64 {
    let hx = hi(x);
    let ix = hx & 0x7fffffff;
    if ix >= 0x3ff00000 {
        // |x| >= 1
        if ((ix - 0x3ff00000) as u32 | lo(x)) == 0 {
            // |x| == 1
            if hx > 0 {
                return 0.0; // acos(1) = 0
            } else {
                return PI + 2.0 * PIO2_LO; // acos(-1) = pi
            }
        }
        #[allow(clippy::eq_op)] // the NaN must come from arithmetic, as in C
        return (x - x) / (x - x); // acos(|x|>1) is NaN
    }
    if ix < 0x3fe00000 {
        // |x| < 0.5
        if ix <= 0x3c600000 {
            return PIO2_HI + PIO2_LO; // |x| < 2**-57
        }
        let z = x * x;
        let p = z * (PS0 + z * (PS1 + z * (PS2 + z * (PS3 + z * (PS4 + z * PS5)))));
        let q = ONE + z * (QS1 + z * (QS2 + z * (QS3 + z * QS4)));
        let r = p / q;
        PIO2_HI - (x - (PIO2_LO - x * r))
    } else if hx < 0 {
        // x < -0.5
        let z = (ONE + x) * 0.5;
        let p = z * (PS0 + z * (PS1 + z * (PS2 + z * (PS3 + z * (PS4 + z * PS5)))));
        let q = ONE + z * (QS1 + z * (QS2 + z * (QS3 + z * QS4)));
        let s = z.sqrt();
        let r = p / q;
        let w = r * s - PIO2_LO;
        PI - 2.0 * (s + w)
    } else {
        // x > 0.5
        let z = (ONE - x) * 0.5;
        let s = z.sqrt();
        let df = with_lo(s, 0);
        let c = (z - df * df) / (s + df);
        let p = z * (PS0 + z * (PS1 + z * (PS2 + z * (PS3 + z * (PS4 + z * PS5)))));
        let q = ONE + z * (QS1 + z * (QS2 + z * (QS3 + z * QS4)));
        let r = p / q;
        let w = r * s + c;
        2.0 * (df + w)
    }
}

const ATANHI: [f64; 4] = [
    4.63647609000806093515e-01, // atan(0.5)hi 0x3FDDAC67, 0x0561BB4F
    7.85398163397448278999e-01, // atan(1.0)hi 0x3FE921FB, 0x54442D18
    9.82793723247329054082e-01, // atan(1.5)hi 0x3FEF730B, 0xD281F69B
    1.57079632679489655800e+00, // atan(inf)hi 0x3FF921FB, 0x54442D18
];

const ATANLO: [f64; 4] = [
    2.26987774529616870924e-17, // atan(0.5)lo 0x3C7A2B7F, 0x222F65E2
    3.06161699786838301793e-17, // atan(1.0)lo 0x3C81A626, 0x33145C07
    1.39033110312309984516e-17, // atan(1.5)lo 0x3C700788, 0x7AF0CBBD
    6.12323399573676603587e-17, // atan(inf)lo 0x3C91A626, 0x33145C07
];

const AT: [f64; 11] = [
    3.33333333333329318027e-01,  // 0x3FD55555, 0x5555550D
    -1.99999999998764832476e-01, // 0xBFC99999, 0x9998EBC4
    1.42857142725034663711e-01,  // 0x3FC24924, 0x920083FF
    -1.11111104054623557880e-01, // 0xBFBC71C6, 0xFE231671
    9.09088713343650656196e-02,  // 0x3FB745CD, 0xC54C206E
    -7.69187620504482999495e-02, // 0xBFB3B0F2, 0xAF749A6D
    6.66107313738753120669e-02,  // 0x3FB10D66, 0xA0D03D51
    -5.83357013379057348645e-02, // 0xBFADDE2D, 0x52DEFD9A
    4.97687799461593236017e-02,  // 0x3FA97B4B, 0x24760DEB
    -3.65315727442169155270e-02, // 0xBFA2B444, 0x2C6A6C2F
    1.62858201153657823623e-02,  // 0x3F90AD3A, 0xE322DA11
];

/// Arc tangent of `x`, `atan` from `s_atan.c`.
///
/// Method: reduce x to positive by atan(x) = -atan(-x). Then, by the interval t = |x| lies in:
///   [0,7/16]      atan(x) = t-t^3*(a1+t^2*(a2+...(a10+t^2*a11)...)
///   [7/16,11/16]  atan(x) = atan(1/2) + atan( (t-0.5)/(1+t/2) )
///   [11/16,19/16] atan(x) = atan( 1 ) + atan( (t-1)/(1+t) )
///   [19/16,39/16] atan(x) = atan(3/2) + atan( (t-1.5)/(1+1.5t) )
///   [39/16,INF]   atan(x) = atan(INF) + atan( -1/t )
pub fn atan(x: f64) -> f64 {
    let mut x = x;
    let hx = hi(x);
    let ix = hx & 0x7fffffff;
    let id: i32;
    if ix >= 0x44100000 {
        // |x| >= 2^66
        if ix > 0x7ff00000 || (ix == 0x7ff00000 && lo(x) != 0) {
            return x + x; // NaN
        }
        if hx > 0 {
            return ATANHI[3] + ATANLO[3];
        } else {
            return -ATANHI[3] - ATANLO[3];
        }
    }
    if ix < 0x3fdc0000 {
        // |x| < 0.4375
        if ix < 0x3e200000 {
            // |x| < 2^-29: the C code returns x when huge+x > one, which always holds here
            return x;
        }
        id = -1;
    } else {
        x = x.abs();
        if ix < 0x3ff30000 {
            // |x| < 1.1875
            if ix < 0x3fe60000 {
                // 7/16 <= |x| < 11/16
                id = 0;
                x = (2.0 * x - ONE) / (2.0 + x);
            } else {
                // 11/16 <= |x| < 19/16
                id = 1;
                x = (x - ONE) / (x + ONE);
            }
        } else if ix < 0x40038000 {
            // |x| < 2.4375
            id = 2;
            x = (x - 1.5) / (ONE + 1.5 * x);
        } else {
            // 2.4375 <= |x| < 2^66
            id = 3;
            x = -1.0 / x;
        }
    }
    // end of argument reduction
    let z = x * x;
    let w = z * z;
    // break sum from i=0 to 10 aT[i]z**(i+1) into odd and even poly
    let s1 = z * (AT[0] + w * (AT[2] + w * (AT[4] + w * (AT[6] + w * (AT[8] + w * AT[10])))));
    let s2 = w * (AT[1] + w * (AT[3] + w * (AT[5] + w * (AT[7] + w * AT[9]))));
    if id < 0 {
        x - x * (s1 + s2)
    } else {
        let id = id as usize;
        let z = ATANHI[id] - ((x * (s1 + s2) - ATANLO[id]) - x);
        if hx < 0 { -z } else { z }
    }
}

const TINY: f64 = 1.0e-300;
const ZERO: f64 = 0.0;
const PI_O_4: f64 = 7.8539816339744827900E-01; // 0x3FE921FB, 0x54442D18
const PI_O_2: f64 = 1.5707963267948965580E+00; // 0x3FF921FB, 0x54442D18
const PI_LO: f64 = 1.2246467991473531772E-16; // 0x3CA1A626, 0x33145C07

/// Arc tangent of `y/x` in (-pi, pi], `__ieee754_atan2`.
///
/// Method: reduce y to positive by atan2(y,x) = -atan2(-y,x), then x to positive (when x and y
/// are unexceptional) by ARG(x+iy) = arctan(y/x) if x > 0, pi - arctan[y/(-x)] if x < 0.
/// Special cases: either argument NaN gives NaN; atan2(+-0, +x) = +-0, atan2(+-0, -x) = +-pi;
/// atan2(+-y, 0) = +-pi/2; atan2(+-y, +INF) = +-0, atan2(+-y, -INF) = +-pi;
/// atan2(+-INF, +INF) = +-pi/4, atan2(+-INF, -INF) = +-3pi/4, atan2(+-INF, x) = +-pi/2.
pub fn atan2(y: f64, x: f64) -> f64 {
    let hx = hi(x);
    let ix = hx & 0x7fffffff;
    let lx = lo(x);
    let hy = hi(y);
    let iy = hy & 0x7fffffff;
    let ly = lo(y);
    if (ix as u32 | ((lx | lx.wrapping_neg()) >> 31)) > 0x7ff00000
        || (iy as u32 | ((ly | ly.wrapping_neg()) >> 31)) > 0x7ff00000
    {
        // x or y is NaN
        return x + y;
    }
    if (hx.wrapping_sub(0x3ff00000) as u32 | lx) == 0 {
        return atan(y); // x = 1.0
    }
    let m = ((hy >> 31) & 1) | ((hx >> 30) & 2); // 2*sign(x)+sign(y)

    // when y = 0
    if (iy as u32 | ly) == 0 {
        match m {
            0 | 1 => return y,      // atan(+-0,+anything) = +-0
            2 => return PI + TINY,  // atan(+0,-anything) = pi
            _ => return -PI - TINY, // atan(-0,-anything) = -pi
        }
    }
    // when x = 0
    if (ix as u32 | lx) == 0 {
        return if hy < 0 { -PI_O_2 - TINY } else { PI_O_2 + TINY };
    }

    // when x is INF
    if ix == 0x7ff00000 {
        if iy == 0x7ff00000 {
            match m {
                0 => return PI_O_4 + TINY,        // atan(+INF,+INF)
                1 => return -PI_O_4 - TINY,       // atan(-INF,+INF)
                2 => return 3.0 * PI_O_4 + TINY,  // atan(+INF,-INF)
                _ => return -3.0 * PI_O_4 - TINY, // atan(-INF,-INF)
            }
        } else {
            match m {
                0 => return ZERO,       // atan(+...,+INF)
                1 => return -ZERO,      // atan(-...,+INF)
                2 => return PI + TINY,  // atan(+...,-INF)
                _ => return -PI - TINY, // atan(-...,-INF)
            }
        }
    }
    // when y is INF
    if iy == 0x7ff00000 {
        return if hy < 0 { -PI_O_2 - TINY } else { PI_O_2 + TINY };
    }

    // compute y/x
    let k = (iy - ix) >> 20;
    let z = if k > 60 {
        PI_O_2 + 0.5 * PI_LO // |y/x| > 2**60
    } else if hx < 0 && k < -60 {
        0.0 // |y|/x < -2**60
    } else {
        atan((y / x).abs()) // safe to do y/x
    };
    match m {
        0 => z,                            // atan(+,+)
        1 => with_hi(z, hi(z) ^ i32::MIN), // atan(-,+)
        2 => PI - (z - PI_LO),             // atan(+,-)
        _ => (z - PI_LO) - PI,             // atan(-,-)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The expected bits come from Java's StrictMath.

    fn check(f: fn(f64) -> f64, cases: &[(f64, u64)]) {
        for &(x, want) in cases {
            assert_eq!(f(x).to_bits(), want, "argument {:016x}", x.to_bits());
        }
    }

    fn check2(f: fn(f64, f64) -> f64, cases: &[(f64, f64, u64)]) {
        for &(a, b, want) in cases {
            assert_eq!(f(a, b).to_bits(), want, "arguments {:016x} {:016x}", a.to_bits(), b.to_bits());
        }
    }

    #[test]
    fn asin_matches_java() {
        check(
            asin,
            &[
                (f64::from_bits(0x0000000000000001), 0x0000000000000001),
                (f64::from_bits(0x800fffffffffffff), 0x800fffffffffffff),
                (f64::from_bits(0x8000000000000000), 0x8000000000000000),
                (f64::from_bits(0x3c5124174347263b), 0x3c5124174347263b),
                (f64::from_bits(0x3fe0000000000000), 0x3fe0c152382d7366),
                (f64::from_bits(0xbfe84b854030db5a), 0xbfeb966be1b17fd6),
                (f64::from_bits(0x3fefac78db8bdfe8), 0x3ff6d89088046368),
                (f64::from_bits(0xbff0000000000000), 0xbff921fb54442d18),
            ],
        );
        assert!(asin(f64::from_bits(0x3ff0000000000001)).is_nan());
        assert!(asin(f64::INFINITY).is_nan());
    }

    #[test]
    fn acos_matches_java() {
        check(
            acos,
            &[
                (f64::from_bits(0x0000000000000001), 0x3ff921fb54442d18),
                (f64::from_bits(0x8000000000000000), 0x3ff921fb54442d18),
                (f64::from_bits(0x3e30000000000000), 0x3ff921fb53442d18),
                (f64::from_bits(0x3fe0000000000000), 0x3ff0c152382d7366),
                (f64::from_bits(0xbfe89a788d4ed9f0), 0x40039530c42e060a),
                (f64::from_bits(0x3fef4ef96aa39a2a), 0x3fcaa88a0bcfc11d),
                (f64::from_bits(0x3ff0000000000000), 0x0000000000000000),
                (f64::from_bits(0xbff0000000000000), 0x400921fb54442d18),
            ],
        );
        assert!(acos(f64::from_bits(0xc000000000000000)).is_nan());
    }

    #[test]
    fn atan_matches_java() {
        check(
            atan,
            &[
                (f64::from_bits(0x800fffffffffffff), 0x800fffffffffffff),
                (f64::from_bits(0x3e1a1cf48fd21f2d), 0x3e1a1cf48fd21f2d),
                (f64::from_bits(0x3fe0000000000000), 0x3fddac670561bb4f),
                (f64::from_bits(0x3fe6666666666666), 0x3fe38b112d7bd4ad),
                (f64::from_bits(0xbff0000000000000), 0xbfe921fb54442d18),
                (f64::from_bits(0xc000000000000000), 0xbff1b6e192ebbe44),
                (f64::from_bits(0xc024000000000000), 0xbff789bd2c160054),
                (f64::from_bits(0x4415f3a0fe95ae55), 0x3ff921fb54442d18),
                (f64::from_bits(0xfff0000000000000), 0xbff921fb54442d18),
            ],
        );
    }

    #[test]
    fn atan2_matches_java() {
        // (y, x, expected) for atan2(y, x)
        check2(
            atan2,
            &[
                (f64::from_bits(0x8000000000000000), f64::from_bits(0x8000000000000000), 0xc00921fb54442d18),
                (f64::from_bits(0x0000000000000000), f64::from_bits(0x7ff0000000000000), 0x0000000000000000),
                (f64::from_bits(0x0000000000000001), f64::from_bits(0xbff0000000000000), 0x400921fb54442d18),
                (f64::from_bits(0x000fffffffffffff), f64::from_bits(0x8000000000000000), 0x3ff921fb54442d18),
                (f64::from_bits(0x7ff0000000000000), f64::from_bits(0xfff0000000000000), 0x4002d97c7f3321d2),
                (f64::from_bits(0x7ff0000000000000), f64::from_bits(0x7ff0000000000000), 0x3fe921fb54442d18),
                (f64::from_bits(0xbff0000000000000), f64::from_bits(0xbff0000000000000), 0xc002d97c7f3321d2),
                (f64::from_bits(0x4008000000000000), f64::from_bits(0x7fefffffffffffff), 0x000c000000000000),
                (f64::from_bits(0xc000000000000000), f64::from_bits(0x3fb999999999999a), 0xbff8555a2787981f),
            ],
        );
        assert!(atan2(f64::NAN, 1.0).is_nan());
    }
}
