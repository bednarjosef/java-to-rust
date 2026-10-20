//! sin, cos and tan (`s_sin.c`, `s_cos.c`, `s_tan.c`), their kernels on [-pi/4, pi/4]
//! (`k_sin.c`, `k_cos.c`, `k_tan.c`) and the argument reduction `__ieee754_rem_pio2`
//! (`e_rem_pio2.c`).
//!
//! Method: let S, C and T denote the sin, cos and tan respectively on [-pi/4, +pi/4]. Reduce the
//! argument x to y1+y2 = x-k*pi/2 in [-pi/4, +pi/4], and let n = k mod 4. We have
//!
//! ```text
//!     n        sin(x)      cos(x)        tan(x)
//!     ----------------------------------------------
//!     0          S           C             T
//!     1          C          -S            -1/T
//!     2         -S          -C             T
//!     3         -C           S            -1/T
//! ```
//!
//! Special cases: trig(+-INF) is NaN, with signals; trig(NaN) is that NaN.

use super::rem_pio2_large::kernel_rem_pio2;
use super::{from_words, hi, lo, with_lo};

// Constants of k_sin.c.
const HALF: f64 = 5.00000000000000000000e-01; // 0x3FE00000, 0x00000000
const S1: f64 = -1.66666666666666324348e-01; // 0xBFC55555, 0x55555549
const S2: f64 = 8.33333333332248946124e-03; // 0x3F811111, 0x1110F8A6
const S3: f64 = -1.98412698298579493134e-04; // 0xBF2A01A0, 0x19C161D5
const S4: f64 = 2.75573137070700676789e-06; // 0x3EC71DE3, 0x57B1FE7D
const S5: f64 = -2.50507602534068634195e-08; // 0xBE5AE5E6, 0x8A2B9CEB
const S6: f64 = 1.58969099521155010221e-10; // 0x3DE5D93A, 0x5ACFD57C

/// `__kernel_sin(x, y, iy)`: sin(x+y) for |x| ~<= pi/4, where y is the tail of x and iy = 0 says
/// y is zero.
///
/// 1. Since sin(-x) = -sin(x), we need only to consider positive x.
/// 2. If x < 2^-27, return x with inexact if x != 0.
/// 3. sin(x) is approximated by a polynomial of degree 13 on [0,pi/4]:
///    sin(x) ~ x + S1*x^3 + ... + S6*x^13, with
///    |sin(x)/x - (1+S1*x^2+S2*x^4+S3*x^6+S4*x^8+S5*x^10+S6*x^12)| <= 2^-58.
/// 4. sin(x+y) = sin(x) + sin'(x')*y ~ sin(x) + (1-x*x/2)*y. For better accuracy, let
///    r = x^3*(S2+x^2*(S3+x^2*(S4+x^2*(S5+x^2*S6)))), then
///    sin(x) = x + (S1*x^3 + (x^2*(r-y/2)+y)).
pub(super) fn kernel_sin(x: f64, y: f64, iy: i32) -> f64 {
    let ix = hi(x) & 0x7fffffff; // high word of x
    if ix < 0x3e400000 {
        // |x| < 2**-27: (int)x == 0 always holds here, the C code tests it to generate inexact
        return x;
    }
    let z = x * x;
    let v = z * x;
    let r = S2 + z * (S3 + z * (S4 + z * (S5 + z * S6)));
    if iy == 0 { x + v * (S1 + z * r) } else { x - ((z * (HALF * y - v * r) - y) - v * S1) }
}

// Constants of k_cos.c.
const ONE: f64 = 1.00000000000000000000e+00; // 0x3FF00000, 0x00000000
const C1: f64 = 4.16666666666666019037e-02; // 0x3FA55555, 0x5555554C
const C2: f64 = -1.38888888888741095749e-03; // 0xBF56C16C, 0x16C15177
const C3: f64 = 2.48015872894767294178e-05; // 0x3EFA01A0, 0x19CB1590
const C4: f64 = -2.75573143513906633035e-07; // 0xBE927E4F, 0x809C52AD
const C5: f64 = 2.08757232129817482790e-09; // 0x3E21EE9E, 0xBDB4B1C4
const C6: f64 = -1.13596475577881948265e-11; // 0xBDA8FAE9, 0xBE8838D4

/// `__kernel_cos(x, y)`: cos(x+y) for |x| ~<= pi/4, where y is the tail of x.
///
/// 1. Since cos(-x) = cos(x), we need only to consider positive x.
/// 2. If x < 2^-27, return 1 with inexact if x != 0.
/// 3. cos(x) is approximated by a polynomial of degree 14 on [0,pi/4]:
///    cos(x) ~ 1 - x*x/2 + C1*x^4 + ... + C6*x^14, with a remez error
///    |cos(x)-(1-.5*x^2+C1*x^4+C2*x^6+C3*x^8+C4*x^10+C5*x^12+C6*x^14)| <= 2^-58.
/// 4. Let r = C1*x^4+C2*x^6+C3*x^8+C4*x^10+C5*x^12+C6*x^14, then cos(x) = 1 - x*x/2 + r. Since
///    cos(x+y) ~ cos(x) - sin(x)*y ~ cos(x) - x*y, a correction term is necessary in cos(x) and
///    hence cos(x+y) = 1 - (x*x/2 - (r - x*y)). For better accuracy when x > 0.3, let qx = |x|/4
///    with the last 32 bits masked off, and if x > 0.78125, let qx = 0.28125. Then
///    cos(x+y) = (1-qx) - ((x*x/2-qx) - (r-x*y)). Note that 1-qx and (x*x/2-qx) are exact here,
///    and the magnitude of the latter is at least a quarter of x*x/2, thus reducing the rounding
///    error in the subtraction.
pub(super) fn kernel_cos(x: f64, y: f64) -> f64 {
    let ix = hi(x) & 0x7fffffff; // ix = |x|'s high word
    if ix < 0x3e400000 {
        // |x| < 2**-27: (int)x == 0 always holds here, the C code tests it to generate inexact
        return ONE;
    }
    let z = x * x;
    let r = z * (C1 + z * (C2 + z * (C3 + z * (C4 + z * (C5 + z * C6)))));
    if ix < 0x3FD33333 {
        // |x| < 0.3
        ONE - (0.5 * z - (z * r - x * y))
    } else {
        let qx = if ix > 0x3fe90000 {
            // x > 0.78125
            0.28125
        } else {
            from_words(ix - 0x00200000, 0) // x/4
        };
        let hz = 0.5 * z - qx;
        let a = ONE - qx;
        a - (hz - (z * r - x * y))
    }
}

// Constants of k_tan.c: T[0..13], then one, pio4 and pio4lo.
const T: [f64; 13] = [
    3.33333333333334091986e-01,  // 3FD55555, 55555563
    1.33333333333201242699e-01,  // 3FC11111, 1110FE7A
    5.39682539762260521377e-02,  // 3FABA1BA, 1BB341FE
    2.18694882948595424599e-02,  // 3F9664F4, 8406D637
    8.86323982359930005737e-03,  // 3F8226E3, E96E8493
    3.59207910759131235356e-03,  // 3F6D6D22, C9560328
    1.45620945432529025516e-03,  // 3F57DBC8, FEE08315
    5.88041240820264096874e-04,  // 3F4344D8, F2F26501
    2.46463134818469906812e-04,  // 3F3026F7, 1A8D1068
    7.81794442939557092300e-05,  // 3F147E88, A03792A6
    7.14072491382608190305e-05,  // 3F12B80F, 32F0A7E9
    -1.85586374855275456654e-05, // BEF375CB, DB605373
    2.59073051863633712884e-05,  // 3EFB2A70, 74BF7AD4
];
const PIO4: f64 = 7.85398163397448278999e-01; // 3FE921FB, 54442D18
const PIO4LO: f64 = 3.06161699786838301793e-17; // 3C81A626, 33145C07

/// `__kernel_tan(x, y, k)`: tan(x+y) if k = 1 or -1/tan(x+y) if k = -1, for |x| ~<= pi/4, where
/// y is the tail of x.
///
/// 1. Since tan(-x) = -tan(x), we need only to consider positive x.
/// 2. If x < 2^-28, return x with inexact if x != 0.
/// 3. tan(x) is approximated by an odd polynomial of degree 27 on [0,0.67434]:
///    tan(x) ~ x + T1*x^3 + ... + T13*x^27, with
///    |tan(x)/x - (1+T1*x^2+T2*x^4+....+T13*x^26)| <= 2^-59.2.
///    Note: tan(x+y) = tan(x) + tan'(x)*y ~ tan(x) + (1+x*x)*y. Therefore, for better accuracy in
///    computing tan(x+y), let r = x^3*(T2+x^2*(T3+x^2*(...+x^2*(T12+x^2*T13)))), then
///    tan(x+y) = x + (T1*x^3 + (x^2*(r+y)+y)).
/// 4. For x in [0.67434,pi/4], let y = pi/4 - x, then
///    tan(x) = tan(pi/4-y) = (1-tan(y))/(1+tan(y)) = 1 - 2*(tan(y) - (tan(y)^2)/(1+tan(y))).
pub(super) fn kernel_tan(mut x: f64, mut y: f64, iy: i32) -> f64 {
    let mut z;
    let mut r;
    let mut v;
    let mut w;
    let mut s;

    let hx = hi(x); // high word of x
    let ix = hx & 0x7fffffff; // high word of |x|
    if ix < 0x3e300000 {
        // x < 2**-28: (int)x == 0 always holds here, the C code tests it to generate inexact
        if ((ix as u32 | lo(x)) | (iy + 1) as u32) == 0 {
            return ONE / x.abs();
        } else if iy == 1 {
            return x;
        } else {
            // compute -1 / (x+y) carefully
            w = x + y;
            z = with_lo(w, 0);
            v = y - (z - x);
            let a = -ONE / w;
            let t = with_lo(a, 0);
            s = ONE + t * z;
            return t + a * (s + t * v);
        }
    }
    if ix >= 0x3FE59428 {
        // |x| >= 0.6744
        if hx < 0 {
            x = -x;
            y = -y;
        }
        z = PIO4 - x;
        w = PIO4LO - y;
        x = z + w;
        y = 0.0;
    }
    z = x * x;
    w = z * z;
    // Break x^5*(T[1]+x^2*T[2]+...) into
    // x^5(T[1]+x^4*T[3]+...+x^20*T[11]) + x^5(x^2*(T[2]+x^4*T[4]+...+x^22*[T12]))
    r = T[1] + w * (T[3] + w * (T[5] + w * (T[7] + w * (T[9] + w * T[11]))));
    v = z * (T[2] + w * (T[4] + w * (T[6] + w * (T[8] + w * (T[10] + w * T[12])))));
    s = z * x;
    r = y + z * (s * (r + v) + y);
    r += T[0] * s;
    w = x + r;
    if ix >= 0x3FE59428 {
        v = iy as f64;
        return (1 - ((hx >> 30) & 2)) as f64 * (v - 2.0 * (x - (w * w / (w + v) - r)));
    }
    if iy == 1 {
        w
    } else {
        // if allow error up to 2 ulp, simply return -1.0 / (x+r) here
        // compute -1.0 / (x+r) accurately
        z = with_lo(w, 0);
        v = r - (z - x); // z+v = r+x
        let a = -1.0 / w; // a = -1.0/w
        let t = with_lo(a, 0);
        s = 1.0 + t * z;
        t + a * (s + t * v)
    }
}

/// 396 hex digits (476 decimal) of 2/pi, 24 bits per entry.
const TWO_OVER_PI: [i32; 66] = [
    0xA2F983, 0x6E4E44, 0x1529FC, 0x2757D1, 0xF534DD, 0xC0DB62, 0x95993C, 0x439041, 0xFE5163, 0xABDEBB,
    0xC561B7, 0x246E3A, 0x424DD2, 0xE00649, 0x2EEA09, 0xD1921C, 0xFE1DEB, 0x1CB129, 0xA73EE8, 0x8235F5,
    0x2EBB44, 0x84E99C, 0x7026B4, 0x5F7E41, 0x3991D6, 0x398353, 0x39F49C, 0x845F8B, 0xBDF928, 0x3B1FF8,
    0x97FFDE, 0x05980F, 0xEF2F11, 0x8B5A0A, 0x6D1F6D, 0x367ECF, 0x27CB09, 0xB74F46, 0x3F669E, 0x5FEA2D,
    0x7527BA, 0xC7EBE5, 0xF17B3D, 0x0739F7, 0x8A5292, 0xEA6BFB, 0x5FB11F, 0x8D5D08, 0x560330, 0x46FC7B,
    0x6BABF0, 0xCFBC20, 0x9AF436, 0x1DA9E3, 0x91615E, 0xE61B08, 0x659985, 0x5F14A0, 0x68408D, 0xFFD880,
    0x4D7327, 0x310606, 0x1556CA, 0x73A8C9, 0x60E27B, 0xC08C6B,
];

/// The high words of n*pi/2 for n = 1..32.
const NPIO2_HW: [i32; 32] = [
    0x3FF921FB, 0x400921FB, 0x4012D97C, 0x401921FB, 0x401F6A7A, 0x4022D97C, 0x4025FDBB, 0x402921FB,
    0x402C463A, 0x402F6A7A, 0x4031475C, 0x4032D97C, 0x40346B9C, 0x4035FDBB, 0x40378FDB, 0x403921FB,
    0x403AB41B, 0x403C463A, 0x403DD85A, 0x403F6A7A, 0x40407E4C, 0x4041475C, 0x4042106C, 0x4042D97C,
    0x4043A28C, 0x40446B9C, 0x404534AC, 0x4045FDBB, 0x4046C6CB, 0x40478FDB, 0x404858EB, 0x404921FB,
];

// Constants of e_rem_pio2.c.
const ZERO: f64 = 0.00000000000000000000e+00; // 0x00000000, 0x00000000
const TWO24: f64 = 1.67772160000000000000e+07; // 0x41700000, 0x00000000
/// 53 bits of 2/pi
const INVPIO2: f64 = 6.36619772367581382433e-01; // 0x3FE45F30, 0x6DC9C883
/// first 33 bits of pi/2
const PIO2_1: f64 = 1.57079632673412561417e+00; // 0x3FF921FB, 0x54400000
/// pi/2 - pio2_1
const PIO2_1T: f64 = 6.07710050650619224932e-11; // 0x3DD0B461, 0x1A626331
/// second 33 bits of pi/2
const PIO2_2: f64 = 6.07710050630396597660e-11; // 0x3DD0B461, 0x1A600000
/// pi/2 - (pio2_1+pio2_2)
const PIO2_2T: f64 = 2.02226624879595063154e-21; // 0x3BA3198A, 0x2E037073
/// third 33 bits of pi/2
const PIO2_3: f64 = 2.02226624871116645580e-21; // 0x3BA3198A, 0x2E000000
/// pi/2 - (pio2_1+pio2_2+pio2_3)
const PIO2_3T: f64 = 8.47842766036889956997e-32; // 0x397B839A, 0x252049C1

/// `__ieee754_rem_pio2(x, y)`: the remainder of x rem pi/2 in `y[0]+y[1]`; returns the quotient's
/// low bits (n with x = n*pi/2 + y, so n mod 4 picks the quadrant).
#[allow(clippy::eq_op)] // x - x is the NaN the C code returns, kept as arithmetic
pub(super) fn rem_pio2(x: f64, y: &mut [f64; 2]) -> i32 {
    let hx = hi(x); // high word of x
    let ix = hx & 0x7fffffff;
    if ix <= 0x3fe921fb {
        // |x| ~<= pi/4, no need for reduction
        y[0] = x;
        y[1] = 0.0;
        return 0;
    }
    if ix < 0x4002d97c {
        // |x| < 3pi/4, special case with n=+-1
        if hx > 0 {
            let mut z = x - PIO2_1;
            if ix != 0x3ff921fb {
                // 33+53 bit pi is good enough
                y[0] = z - PIO2_1T;
                y[1] = (z - y[0]) - PIO2_1T;
            } else {
                // near pi/2, use 33+33+53 bit pi
                z -= PIO2_2;
                y[0] = z - PIO2_2T;
                y[1] = (z - y[0]) - PIO2_2T;
            }
            return 1;
        } else {
            // negative x
            let mut z = x + PIO2_1;
            if ix != 0x3ff921fb {
                // 33+53 bit pi is good enough
                y[0] = z + PIO2_1T;
                y[1] = (z - y[0]) + PIO2_1T;
            } else {
                // near pi/2, use 33+33+53 bit pi
                z += PIO2_2;
                y[0] = z + PIO2_2T;
                y[1] = (z - y[0]) + PIO2_2T;
            }
            return -1;
        }
    }
    if ix <= 0x413921fb {
        // |x| ~<= 2^19*(pi/2), medium size
        let mut t = x.abs();
        let n = (t * INVPIO2 + HALF) as i32;
        let fn_ = n as f64;
        let mut r = t - fn_ * PIO2_1;
        let mut w = fn_ * PIO2_1T; // 1st round good to 85 bit
        if n < 32 && ix != NPIO2_HW[(n - 1) as usize] {
            y[0] = r - w; // quick check no cancellation
        } else {
            let j = ix >> 20;
            y[0] = r - w;
            let i = j - ((hi(y[0]) >> 20) & 0x7ff);
            if i > 16 {
                // 2nd iteration needed, good to 118
                t = r;
                w = fn_ * PIO2_2;
                r = t - w;
                w = fn_ * PIO2_2T - ((t - r) - w);
                y[0] = r - w;
                let i = j - ((hi(y[0]) >> 20) & 0x7ff);
                if i > 49 {
                    // 3rd iteration need, 151 bits acc
                    t = r; // will cover all possible cases
                    w = fn_ * PIO2_3;
                    r = t - w;
                    w = fn_ * PIO2_3T - ((t - r) - w);
                    y[0] = r - w;
                }
            }
        }
        y[1] = (r - y[0]) - w;
        if hx < 0 {
            y[0] = -y[0];
            y[1] = -y[1];
            return -n;
        } else {
            return n;
        }
    }
    // all other (large) arguments
    if ix >= 0x7ff00000 {
        // x is inf or NaN
        y[0] = x - x;
        y[1] = y[0];
        return 0;
    }
    // set z = scalbn(|x|,ilogb(x)-23)
    let e0 = (ix >> 20) - 1046; // e0 = ilogb(z)-23;
    let mut z = from_words(ix - (e0 << 20), lo(x));
    let mut tx = [0.0f64; 3];
    for txi in tx.iter_mut().take(2) {
        *txi = (z as i32) as f64;
        z = (z - *txi) * TWO24;
    }
    tx[2] = z;
    let mut nx = 3;
    while tx[nx - 1] == ZERO {
        nx -= 1; // skip zero term
    }
    let n = kernel_rem_pio2(&tx[..nx], y, e0, 2, &TWO_OVER_PI);
    if hx < 0 {
        y[0] = -y[0];
        y[1] = -y[1];
        return -n;
    }
    n
}

/// `StrictMath.sin`: the sine of `x`, as fdlibm's `sin` computes it.
#[allow(clippy::eq_op)] // x - x is the NaN the C code returns, kept as arithmetic
pub fn sin(x: f64) -> f64 {
    let mut y = [0.0f64; 2];
    let z = 0.0;

    // High word of x.
    let ix = hi(x) & 0x7fffffff;

    if ix <= 0x3fe921fb {
        // |x| ~< pi/4
        kernel_sin(x, z, 0)
    } else if ix >= 0x7ff00000 {
        // sin(Inf or NaN) is NaN
        x - x
    } else {
        // argument reduction needed
        let n = rem_pio2(x, &mut y);
        match n & 3 {
            0 => kernel_sin(y[0], y[1], 1),
            1 => kernel_cos(y[0], y[1]),
            2 => -kernel_sin(y[0], y[1], 1),
            _ => -kernel_cos(y[0], y[1]),
        }
    }
}

/// `StrictMath.cos`: the cosine of `x`, as fdlibm's `cos` computes it.
#[allow(clippy::eq_op)] // x - x is the NaN the C code returns, kept as arithmetic
pub fn cos(x: f64) -> f64 {
    let mut y = [0.0f64; 2];
    let z = 0.0;

    // High word of x.
    let ix = hi(x) & 0x7fffffff;

    if ix <= 0x3fe921fb {
        // |x| ~< pi/4
        kernel_cos(x, z)
    } else if ix >= 0x7ff00000 {
        // cos(Inf or NaN) is NaN
        x - x
    } else {
        // argument reduction needed
        let n = rem_pio2(x, &mut y);
        match n & 3 {
            0 => kernel_cos(y[0], y[1]),
            1 => -kernel_sin(y[0], y[1], 1),
            2 => -kernel_cos(y[0], y[1]),
            _ => kernel_sin(y[0], y[1], 1),
        }
    }
}

/// `StrictMath.tan`: the tangent of `x`, as fdlibm's `tan` computes it.
#[allow(clippy::eq_op)] // x - x is the NaN the C code returns, kept as arithmetic
pub fn tan(x: f64) -> f64 {
    let mut y = [0.0f64; 2];
    let z = 0.0;

    // High word of x.
    let ix = hi(x) & 0x7fffffff;

    if ix <= 0x3fe921fb {
        // |x| ~< pi/4
        kernel_tan(x, z, 1)
    } else if ix >= 0x7ff00000 {
        // tan(Inf or NaN) is NaN
        x - x
    } else {
        // argument reduction needed
        let n = rem_pio2(x, &mut y);
        kernel_tan(y[0], y[1], 1 - ((n & 1) << 1)) // 1 -- n even, -1 -- n odd
    }
}

#[cfg(test)]
mod tests {
    use super::{cos, sin, tan};

    // The expected bits come from Java's StrictMath (`StrictMath.sin(Double.longBitsToDouble(x))`).

    fn check(f: fn(f64) -> f64, cases: &[(f64, u64)]) {
        for &(x, want) in cases {
            assert_eq!(f(x).to_bits(), want, "x = {:#018x}", x.to_bits());
        }
    }

    #[test]
    fn sin_matches_strict_math() {
        check(
            sin,
            &[
                (f64::from_bits(0x0000000000000000), 0x0000000000000000),
                (f64::from_bits(0x8000000000000000), 0x8000000000000000), // -0.0
                (f64::from_bits(0x0000000000000001), 0x0000000000000001), // smallest subnormal
                (f64::from_bits(0x800fffffffffffff), 0x800fffffffffffff), // largest negative subnormal
                (f64::from_bits(0x3e45798ee2308c3a), 0x3e45798ee2308c3a), // 1e-8
                (f64::from_bits(0x3fe921fb54442d18), 0x3fe6a09e667f3bcc), // pi/4
                (f64::from_bits(0x3ff921fb54442d18), 0x3ff0000000000000), // pi/2
                (f64::from_bits(0x400921fb54442d18), 0x3ca1a62633145c07), // pi
                (f64::from_bits(0x40707e4cef4cbd98), 0x3cf1abdbb9ea8e6e), // near 84*(pi/2)
                (f64::from_bits(0x7e37e43c8800759c), 0xbfea2c16b010e385), // 1e300
                (f64::from_bits(0x7fefffffffffffff), 0x3f7452fc98b34e97), // f64::MAX
                (f64::from_bits(0xd467be4226647457), 0xbfbfda2d640ea22d),
            ],
        );
    }

    #[test]
    fn cos_matches_strict_math() {
        check(
            cos,
            &[
                (f64::from_bits(0x0000000000000000), 0x3ff0000000000000),
                (f64::from_bits(0x8000000000000000), 0x3ff0000000000000), // -0.0
                (f64::from_bits(0x0000000000000001), 0x3ff0000000000000), // smallest subnormal
                (f64::from_bits(0x3e3750c66cf29859), 0x3ff0000000000000),
                (f64::from_bits(0x3fe5c3a3cc1ba9cc), 0x3fe8e13258a4ba9a),
                (f64::from_bits(0x3ff921fb54442d18), 0x3c91a62633145c07), // pi/2
                (f64::from_bits(0xc00921fb54442d18), 0xbff0000000000000), // -pi
                (f64::from_bits(0x4070976eeaa101c5), 0x3cda6af37b870c09), // near 85*(pi/2)
                (f64::from_bits(0x41c10f44422adf82), 0xbfcd479d64488da5), // medium size reduction
                (f64::from_bits(0x7e37e43c8800759c), 0xbfe2699022adc4c1), // 1e300
                (f64::from_bits(0x7fefffffffffffff), 0xbfefffe62ecfab75), // f64::MAX
            ],
        );
    }

    #[test]
    fn tan_matches_strict_math() {
        check(
            tan,
            &[
                (f64::from_bits(0x0000000000000000), 0x0000000000000000),
                (f64::from_bits(0x8000000000000000), 0x8000000000000000), // -0.0
                (f64::from_bits(0x0000000000000001), 0x0000000000000001), // smallest subnormal
                (f64::from_bits(0x3e30000000000000), 0x3e30000000000000), // 2^-28
                (f64::from_bits(0x3fe5993e9b658276), 0x3fe99c7f7c0954b0), // just above 0.6744
                (f64::from_bits(0x3fe921fb54442d18), 0x3fefffffffffffff), // pi/4
                (f64::from_bits(0x3ff921fb54442d18), 0x434d02967c31cdb5), // pi/2
                (f64::from_bits(0x4012d97c7f3321d2), 0x4333570efd768923), // 3pi/2
                (f64::from_bits(0xc00921fb54442d18), 0x3ca1a62633145c07), // -pi
                (f64::from_bits(0x7e37e43c8800759c), 0x3ff6be411f37ac77), // 1e300
                (f64::from_bits(0x7fefffffffffffff), 0xbf74530cfe729484), // f64::MAX
                (f64::from_bits(0x4c6872e9ccf6aec9), 0xc01a160d1f533046),
            ],
        );
    }

    #[test]
    fn non_finite_gives_nan() {
        for x in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
            assert!(sin(x).is_nan() && cos(x).is_nan() && tan(x).is_nan());
        }
    }
}
