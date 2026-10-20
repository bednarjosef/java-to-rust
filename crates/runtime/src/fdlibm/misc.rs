//! Cube root, hypotenuse and IEEE remainder: `s_cbrt.c`, `e_hypot.c` and `e_remainder.c`.

use super::{hi, lo, with_hi, with_lo};

const B1: u32 = 715094163; // B1 = (682-0.03306235651)*2**20
const B2: u32 = 696219795; // B2 = (664-0.03306235651)*2**20

const C: f64 = 5.42857142857142815906e-01; // 19/35     = 0x3FE15F15, 0xF15F15F1
const D: f64 = -7.05306122448979611050e-01; // -864/1225 = 0xBFE691DE, 0x2532C834
const E: f64 = 1.41428571428571436819e+00; // 99/70     = 0x3FF6A0EA, 0x0EA0EA0F
const F: f64 = 1.60714285714285720630e+00; // 45/28     = 0x3FF9B6DB, 0x6DB6DB6E
const G: f64 = 3.57142857142857150787e-01; // 5/14      = 0x3FD6DB6D, 0xB6DB6DB7

/// Cube root of `x`, `cbrt` from `s_cbrt.c`.
pub fn cbrt(x: f64) -> f64 {
    let mut x = x;
    let mut hx = hi(x); // high word of x
    let sign = hx as u32 & 0x80000000; // sign = sign(x)
    hx ^= sign as i32;
    if hx >= 0x7ff00000 {
        // cbrt(NaN,INF) is itself. The C returns x+x, which quiets a signalling NaN; Java returns
        // x unchanged.
        return x;
    }
    if (hx as u32 | lo(x)) == 0 {
        return x; // cbrt(0) is itself
    }

    x = with_hi(x, hx); // x <- |x|
    // rough cbrt to 5 bits
    let mut t = 0.0;
    if hx < 0x00100000 {
        // subnormal number
        t = with_hi(t, 0x43500000); // set t = 2**54
        t *= x;
        t = with_hi(t, ((hi(t) / 3) as u32).wrapping_add(B2) as i32);
    } else {
        t = with_hi(t, ((hx / 3) as u32).wrapping_add(B1) as i32);
    }

    // new cbrt to 23 bits, may be implemented in single precision
    let mut r = t * t / x;
    let mut s = C + r * t;
    t *= G + F / (s + E + D / s);

    // chopped to 20 bits and make it larger than cbrt(x)
    t = with_lo(t, 0);
    t = with_hi(t, hi(t) + 0x00000001);

    // one step newton iteration to 53 bits with error less than 0.667 ulps
    s = t * t; // t*t is exact
    r = x / s;
    let w = t + t;
    r = (r - t) / (w + r); // r-s is exact
    t = t + t * r;

    // restore the sign bit
    with_hi(t, hi(t) | sign as i32)
}

/// `sqrt(x*x+y*y)` without undue overflow or underflow, `__ieee754_hypot`.
///
/// Method: with x > y > 0, compute x*x+y*y with error below sqrt(2)/2 ulp, so its sqrt is
/// within 1 ulp:
///   1. if x > 2y use x1*x1+(y*y+(x2*(x+x1))), x1 = x with lower 32 bits cleared, x2 = x-x1;
///   2. else use t1*y1+((x-y)*(x-y)+(t1*y2+t2*y)), t1 = 2x with lower 32 bits cleared,
///      t2 = 2x-t1, y1 = y with lower 32 bits chopped, y2 = y-y1.
///
/// Arguments too large or too tiny are scaled first. hypot is INF if x or y is +-INF, else
/// NaN if x or y is NaN.
pub fn hypot(x: f64, y: f64) -> f64 {
    let mut a;
    let mut b;
    let mut ha = hi(x) & 0x7fffffff; // high word of x
    let mut hb = hi(y) & 0x7fffffff; // high word of y
    if hb > ha {
        a = y;
        b = x;
        std::mem::swap(&mut ha, &mut hb);
    } else {
        a = x;
        b = y;
    }
    a = with_hi(a, ha); // a <- |a|
    b = with_hi(b, hb); // b <- |b|
    if (ha - hb) > 0x3c00000 {
        return a + b; // x/y > 2**60
    }
    let mut k: i32 = 0;
    if ha > 0x5f300000 {
        // a > 2**500
        if ha >= 0x7ff00000 {
            // Inf or NaN
            let mut w = a + b; // for sNaN
            if ((ha & 0xfffff) as u32 | lo(a)) == 0 {
                w = a;
            }
            if ((hb ^ 0x7ff00000) as u32 | lo(b)) == 0 {
                w = b;
            }
            return w;
        }
        // scale a and b by 2**-600
        ha -= 0x25800000;
        hb -= 0x25800000;
        k += 600;
        a = with_hi(a, ha);
        b = with_hi(b, hb);
    }
    if hb < 0x20b00000 {
        // b < 2**-500
        if hb <= 0x000fffff {
            // subnormal b or 0
            if (hb as u32 | lo(b)) == 0 {
                return a;
            }
            let t1 = with_hi(0.0, 0x7fd00000); // t1 = 2^1022
            b *= t1;
            a *= t1;
            k -= 1022;
        } else {
            // scale a and b by 2^600
            ha += 0x25800000; // a *= 2^600
            hb += 0x25800000; // b *= 2^600
            k -= 600;
            a = with_hi(a, ha);
            b = with_hi(b, hb);
        }
    }
    // medium size a and b
    let mut w = a - b;
    if w > b {
        let t1 = with_hi(0.0, ha);
        let t2 = a - t1;
        w = (t1 * t1 - (b * (-b) - t2 * (a + t1))).sqrt();
    } else {
        a += a;
        let y1 = with_hi(0.0, hb);
        let y2 = b - y1;
        let t1 = with_hi(0.0, ha + 0x00100000);
        let t2 = a - t1;
        w = (t1 * y1 - (w * (-w) - (t1 * y2 + t2 * b))).sqrt();
    }
    if k != 0 {
        let t1 = with_hi(1.0, hi(1.0).wrapping_add(k << 20));
        t1 * w
    } else {
        w
    }
}

const ZERO: f64 = 0.0;

/// IEEE remainder `x - [x/p]*p`, `[x/p]` the integer nearest `x/p` (ties to even),
/// `__ieee754_remainder`.
///
/// Method: based on fmod, which returns x-[x/p]chopped*p exactly.
pub fn remainder(x: f64, p: f64) -> f64 {
    let mut x = x;
    let mut p = p;
    let mut hx = hi(x); // high word of x
    let lx = lo(x); // low word of x
    let mut hp = hi(p); // high word of p
    let lp = lo(p); // low word of p
    let sx = hx as u32 & 0x80000000;
    hp &= 0x7fffffff;
    hx &= 0x7fffffff;

    // purge off exception values
    // (the NaNs must come from arithmetic, as in C, hence the eq_op allowances)
    if (hp as u32 | lp) == 0 {
        #[allow(clippy::eq_op)]
        return (x * p) / (x * p); // p = 0
    }
    // x not finite, or p is NaN
    if hx >= 0x7ff00000 || (hp >= 0x7ff00000 && ((hp - 0x7ff00000) as u32 | lp) != 0) {
        #[allow(clippy::eq_op)]
        return (x * p) / (x * p);
    }

    if hp <= 0x7fdfffff {
        x %= p + p; // now x < 2p; Rust's % on f64 is C's fmod
    }
    if ((hx - hp) as u32 | lx.wrapping_sub(lp)) == 0 {
        return ZERO * x;
    }
    x = x.abs();
    p = p.abs();
    if hp < 0x00200000 {
        if x + x > p {
            x -= p;
            if x + x >= p {
                x -= p;
            }
        }
    } else {
        let p_half = 0.5 * p;
        if x > p_half {
            x -= p;
            if x >= p_half {
                x -= p;
            }
        }
    }
    with_hi(x, hi(x) ^ sx as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The expected bits come from Java's StrictMath.

    fn check2(f: fn(f64, f64) -> f64, cases: &[(f64, f64, u64)]) {
        for &(a, b, want) in cases {
            assert_eq!(f(a, b).to_bits(), want, "arguments {:016x} {:016x}", a.to_bits(), b.to_bits());
        }
    }

    #[test]
    fn cbrt_matches_java() {
        let cases = [
            (f64::from_bits(0x0000000000000001), 0x2990000000000000),
            (f64::from_bits(0x800fffffffffffff), 0xaaa428a2f98d728b),
            (f64::from_bits(0x8000000000000000), 0x8000000000000000),
            (f64::from_bits(0x3e30000000000000), 0x3f5965fea53d6e3d),
            (f64::from_bits(0x4008000000000000), 0x3ff7137449123ef6),
            (f64::from_bits(0xc087490a3d70a3d7), 0xc02221bcfc0b4e1f),
            (f64::from_bits(0x7fefffffffffffff), 0x554428a2f98d728b),
            (f64::from_bits(0xfff0000000000000), 0xfff0000000000000),
        ];
        for (x, want) in cases {
            assert_eq!(cbrt(x).to_bits(), want, "argument {:016x}", x.to_bits());
        }
        assert!(cbrt(f64::NAN).is_nan());
    }

    #[test]
    fn hypot_matches_java() {
        check2(
            hypot,
            &[
                (f64::from_bits(0x0000000000000000), f64::from_bits(0x8000000000000000), 0x0000000000000000),
                (f64::from_bits(0x000012688b70e62b), f64::from_bits(0x800012688b70e62b), 0x00001a088b6bf34f),
                (f64::from_bits(0x000012688b70e62b), f64::from_bits(0x000fffffffffffff), 0x0010000a96eeeaa2),
                (f64::from_bits(0x4008000000000000), f64::from_bits(0xbff0000000000000), 0x40094c583ada5b53),
                (f64::from_bits(0x4000000000000000), f64::from_bits(0x4000000000000000), 0x4006a09e667f3bcd),
                (f64::from_bits(0x2071d53f74443522), f64::from_bits(0xff9363b7bd4f725e), 0x7f9363b7bd4f725e),
                (f64::from_bits(0x207ef78fdbd83d43), f64::from_bits(0x1e8d26a442383336), 0x207ef78fdbd83d43),
                (f64::from_bits(0x7ff0000000000000), f64::from_bits(0x7fefffffffffffff), 0x7ff0000000000000),
                (f64::from_bits(0x7ff8000000000000), f64::from_bits(0xfff0000000000000), 0x7ff0000000000000),
            ],
        );
    }

    #[test]
    fn remainder_matches_java() {
        check2(
            remainder,
            &[
                (f64::from_bits(0x4008000000000000), f64::from_bits(0xc000000000000000), 0xbff0000000000000),
                (f64::from_bits(0xc000000000000000), f64::from_bits(0x3ff0000000000000), 0x8000000000000000),
                (f64::from_bits(0xc000000000000000), f64::from_bits(0x3fb999999999999a), 0x3ca0000000000000),
                (f64::from_bits(0xc000000000000000), f64::from_bits(0x3ff921fb54442d18), 0xbfdb7812aeef4ba0),
                (f64::from_bits(0x7fefffffffffffff), f64::from_bits(0x4008000000000000), 0xbff0000000000000),
                (f64::from_bits(0x000fffffffffffff), f64::from_bits(0x800012688b70e62b), 0x8000091177587f76),
                (f64::from_bits(0x0000000000000001), f64::from_bits(0x000fffffffffffff), 0x0000000000000001),
                (f64::from_bits(0xc008000000000000), f64::from_bits(0xfff0000000000000), 0xc008000000000000),
            ],
        );
        assert!(remainder(1.0, 0.0).is_nan());
        assert!(remainder(f64::INFINITY, 2.0).is_nan());
    }
}
