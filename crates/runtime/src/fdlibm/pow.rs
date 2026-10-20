//! `e_pow.c`, with the `scalbn` of `s_scalbn.c` it calls for subnormal results.

use super::{from_words, hi, lo, with_hi, with_lo};

const BP: [f64; 2] = [1.0, 1.5];
const DP_H: [f64; 2] = [0.0, 5.84962487220764160156e-01]; // 0x3FE2B803, 0x40000000
const DP_L: [f64; 2] = [0.0, 1.35003920212974897128e-08]; // 0x3E4CFDEB, 0x43CFD006
const ZERO: f64 = 0.0;
const ONE: f64 = 1.0;
const TWO: f64 = 2.0;
const TWO53: f64 = 9007199254740992.0; // 0x43400000, 0x00000000
const HUGE: f64 = 1.0e300;
const TINY: f64 = 1.0e-300;
// poly coefs for (3/2)*(log(x)-2s-2/3*s**3
const L1: f64 = 5.99999999999994648725e-01; // 0x3FE33333, 0x33333303
const L2: f64 = 4.28571428578550184252e-01; // 0x3FDB6DB6, 0xDB6FABFF
const L3: f64 = 3.33333329818377432918e-01; // 0x3FD55555, 0x518F264D
const L4: f64 = 2.72728123808534006489e-01; // 0x3FD17460, 0xA91D4101
const L5: f64 = 2.30660745775561754067e-01; // 0x3FCD864A, 0x93C9DB65
const L6: f64 = 2.06975017800338417784e-01; // 0x3FCA7E28, 0x4A454EEF
const P1: f64 = 1.66666666666666019037e-01; // 0x3FC55555, 0x5555553E
const P2: f64 = -2.77777777770155933842e-03; // 0xBF66C16C, 0x16BEBD93
const P3: f64 = 6.61375632143793436117e-05; // 0x3F11566A, 0xAF25DE2C
const P4: f64 = -1.65339022054652515390e-06; // 0xBEBBBD41, 0xC5D26BF1
const P5: f64 = 4.13813679705723846039e-08; // 0x3E663769, 0x72BEA4D0
const LG2: f64 = 6.93147180559945286227e-01; // 0x3FE62E42, 0xFEFA39EF
const LG2_H: f64 = 6.93147182464599609375e-01; // 0x3FE62E43, 0x00000000
const LG2_L: f64 = -1.90465429995776804525e-09; // 0xBE205C61, 0x0CA86C39
const OVT: f64 = 8.0085662595372944372e-0017; // -(1024-log2(ovfl+.5ulp))
const CP: f64 = 9.61796693925975554329e-01; // 0x3FEEC709, 0xDC3A03FD =2/(3ln2)
const CP_H: f64 = 9.61796700954437255859e-01; // 0x3FEEC709, 0xE0000000 =(float)cp
const CP_L: f64 = -7.02846165095275826516e-09; // 0xBE3E2FE0, 0x145B01F5 =tail of cp_h
const IVLN2: f64 = 1.44269504088896338700e+00; // 0x3FF71547, 0x652B82FE =1/ln2
const IVLN2_H: f64 = 1.44269502162933349609e+00; // 0x3FF71547, 0x60000000 =24b 1/ln2
const IVLN2_L: f64 = 1.92596299112661746887e-08; // 0x3E54AE0B, 0xF85DDF44 =1/ln2 tail

/// `__ieee754_pow`: `x` raised to the power `y`.
#[allow(clippy::eq_op)] // y-y and (x-x)/(x-x) make the NaNs arithmetically, as the C does
pub fn pow(x: f64, y: f64) -> f64 {
    // Method: let x = 2^n * (1+f)
    //   1. Compute and return log2(x) in two pieces: log2(x) = w1 + w2, where w1 has
    //      53-24 = 29 bit trailing zeros.
    //   2. Perform y*log2(x) = n+y' by simulating multi-precision arithmetic, where
    //      |y'|<=0.5.
    //   3. Return x**y = 2**n*exp(y'*log2)
    let mut z;
    let mut t;
    let mut u;
    let mut v;
    let mut j: i32;
    let mut k: i32;
    let mut n: i32;

    let hx = hi(x);
    let lx = lo(x);
    let hy = hi(y);
    let ly = lo(y);
    let mut ix = hx & 0x7fffffff;
    let iy = hy & 0x7fffffff;

    // y==zero: x**0 = 1
    if (iy as u32 | ly) == 0 {
        return ONE;
    }

    // +-NaN return x+y
    if ix > 0x7ff00000 || (ix == 0x7ff00000 && lx != 0) || iy > 0x7ff00000 || (iy == 0x7ff00000 && ly != 0) {
        return x + y;
    }

    // determine if y is an odd int when x < 0
    // yisint = 0 ... y is not an integer
    // yisint = 1 ... y is an odd int
    // yisint = 2 ... y is an even int
    let mut yisint = 0;
    if hx < 0 {
        if iy >= 0x43400000 {
            yisint = 2; // even integer y
        } else if iy >= 0x3ff00000 {
            k = (iy >> 20) - 0x3ff; // exponent
            if k > 20 {
                j = (ly >> (52 - k)) as i32;
                if ((j as u32) << (52 - k)) == ly {
                    yisint = 2 - (j & 1);
                }
            } else if ly == 0 {
                j = iy >> (20 - k);
                if (j << (20 - k)) == iy {
                    yisint = 2 - (j & 1);
                }
            }
        }
    }

    // special value of y
    if ly == 0 {
        if iy == 0x7ff00000 {
            // y is +-inf
            if ((ix - 0x3ff00000) as u32 | lx) == 0 {
                return y - y; // inf**+-1 is NaN
            } else if ix >= 0x3ff00000 {
                // (|x|>1)**+-inf = inf,0
                return if hy >= 0 { y } else { ZERO };
            } else {
                // (|x|<1)**-,+inf = inf,0
                return if hy < 0 { -y } else { ZERO };
            }
        }
        if iy == 0x3ff00000 {
            // y is  +-1
            if hy < 0 {
                return ONE / x;
            } else {
                return x;
            }
        }
        if hy == 0x40000000 {
            return x * x; // y is  2
        }
        if hy == 0x3fe00000 && hx >= 0 {
            // y is  0.5, x >= +0
            return x.sqrt();
        }
    }

    let mut ax = x.abs();
    // special value of x
    if lx == 0 && (ix == 0x7ff00000 || ix == 0 || ix == 0x3ff00000) {
        z = ax; // x is +-0,+-inf,+-1
        if hy < 0 {
            z = ONE / z; // z = (1/|x|)
        }
        if hx < 0 {
            if ((ix - 0x3ff00000) | yisint) == 0 {
                z = (z - z) / (z - z); // (-1)**non-int is NaN
            } else if yisint == 1 {
                z = -z; // (x<0)**odd = -(|x|**odd)
            }
        }
        return z;
    }

    n = (hx >> 31) + 1;

    // (x<0)**(non-int) is NaN
    if (n | yisint) == 0 {
        return (x - x) / (x - x);
    }

    let mut s = ONE; // s (sign of result -ve**odd) = -1 else = 1
    if (n | (yisint - 1)) == 0 {
        s = -ONE; // (-ve)**(odd int)
    }

    // |y| is huge
    let (t1, t2) = if iy > 0x41e00000 {
        // if |y| > 2**31
        if iy > 0x43f00000 {
            // if |y| > 2**64, must o/uflow
            if ix <= 0x3fefffff {
                return if hy < 0 { HUGE * HUGE } else { TINY * TINY };
            }
            if ix >= 0x3ff00000 {
                return if hy > 0 { HUGE * HUGE } else { TINY * TINY };
            }
        }
        // over/underflow if x is not close to one
        if ix < 0x3fefffff {
            return if hy < 0 { s * HUGE * HUGE } else { s * TINY * TINY };
        }
        if ix > 0x3ff00000 {
            return if hy > 0 { s * HUGE * HUGE } else { s * TINY * TINY };
        }
        // now |1-x| is tiny <= 2**-20, suffice to compute log(x) by x-x^2/2+x^3/3-x^4/4
        t = ax - ONE; // t has 20 trailing zeros
        let w = (t * t) * (0.5 - t * (0.3333333333333333333333 - t * 0.25));
        u = IVLN2_H * t; // ivln2_h has 21 sig. bits
        v = t * IVLN2_L - w * IVLN2;
        let t1 = with_lo(u + v, 0);
        (t1, v - (t1 - u))
    } else {
        n = 0;
        // take care subnormal number
        if ix < 0x00100000 {
            ax *= TWO53;
            n -= 53;
            ix = hi(ax);
        }
        n += (ix >> 20) - 0x3ff;
        j = ix & 0x000fffff;
        // determine interval
        ix = j | 0x3ff00000; // normalize ix
        if j <= 0x3988E {
            k = 0; // |x|<sqrt(3/2)
        } else if j < 0xBB67A {
            k = 1; // |x|<sqrt(3)
        } else {
            k = 0;
            n += 1;
            ix -= 0x00100000;
        }
        ax = with_hi(ax, ix);
        let ku = k as usize;

        // compute ss = s_h+s_l = (x-1)/(x+1) or (x-1.5)/(x+1.5)
        u = ax - BP[ku]; // bp[0]=1.0, bp[1]=1.5
        v = ONE / (ax + BP[ku]);
        let ss = u * v;
        let s_h = with_lo(ss, 0);
        // t_h=ax+bp[k] High
        let mut t_h = from_words(((ix >> 1) | 0x20000000) + 0x00080000 + (k << 18), 0);
        let mut t_l = ax - (t_h - BP[ku]);
        let s_l = v * ((u - s_h * t_h) - s_h * t_l);
        // compute log(ax)
        let mut s2 = ss * ss;
        let mut r = s2 * s2 * (L1 + s2 * (L2 + s2 * (L3 + s2 * (L4 + s2 * (L5 + s2 * L6)))));
        r += s_l * (s_h + ss);
        s2 = s_h * s_h;
        t_h = with_lo(3.0 + s2 + r, 0);
        t_l = r - ((t_h - 3.0) - s2);
        // u+v = ss*(1+...)
        u = s_h * t_h;
        v = s_l * t_h + t_l * ss;
        // 2/(3log2)*(ss+...)
        let p_h = with_lo(u + v, 0);
        let p_l = v - (p_h - u);
        let z_h = CP_H * p_h; // cp_h+cp_l = 2/(3*log2)
        let z_l = CP_L * p_h + p_l * CP + DP_L[ku];
        // log2(ax) = (ss+..)*2/(3*log2) = n + dp_h + z_h + z_l
        t = n as f64;
        let t1 = with_lo(((z_h + z_l) + DP_H[ku]) + t, 0);
        (t1, z_l - (((t1 - t) - DP_H[ku]) - z_h))
    };

    // split up y into y1+y2 and compute (y1+y2)*(t1+t2)
    let y1 = with_lo(y, 0);
    let p_l = (y - y1) * t1 + y * t2;
    let mut p_h = y1 * t1;
    z = p_l + p_h;
    j = hi(z);
    let mut i = lo(z) as i32;
    if j >= 0x40900000 {
        // z >= 1024
        if ((j - 0x40900000) | i) != 0 {
            // if z > 1024
            return s * HUGE * HUGE; // overflow
        } else if p_l + OVT > z - p_h {
            return s * HUGE * HUGE; // overflow
        }
    } else if (j & 0x7fffffff) >= 0x4090cc00 {
        // z <= -1075
        if ((j as u32).wrapping_sub(0xc090cc00) | i as u32) != 0 {
            // z < -1075
            return s * TINY * TINY; // underflow
        } else if p_l <= z - p_h {
            return s * TINY * TINY; // underflow
        }
    }

    // compute 2**(p_h+p_l)
    i = j & 0x7fffffff;
    k = (i >> 20) - 0x3ff;
    n = 0;
    if i > 0x3fe00000 {
        // if |z| > 0.5, set n = [z+0.5]
        n = j + (0x00100000 >> (k + 1));
        k = ((n & 0x7fffffff) >> 20) - 0x3ff; // new k for n
        t = from_words(n & !(0x000fffff >> k), 0);
        n = ((n & 0x000fffff) | 0x00100000) >> (20 - k);
        if j < 0 {
            n = -n;
        }
        p_h -= t;
    }
    t = with_lo(p_l + p_h, 0);
    u = t * LG2_H;
    v = (p_l - (t - p_h)) * LG2 + t * LG2_L;
    z = u + v;
    let w = v - (z - u);
    t = z * z;
    let t1 = z - t * (P1 + t * (P2 + t * (P3 + t * (P4 + t * P5))));
    let r = (z * t1) / (t1 - TWO) - (w + z * w);
    z = ONE - (r - z);
    j = hi(z);
    j = j.wrapping_add(n << 20);
    if (j >> 20) <= 0 {
        z = scalbn(z, n); // subnormal output
    } else {
        z = with_hi(z, hi(z).wrapping_add(n << 20));
    }
    s * z
}

const TWO54: f64 = 1.80143985094819840000e+16; // 0x43500000, 0x00000000
const TWOM54: f64 = 5.55111512312578270212e-17; // 0x3C900000, 0x00000000

/// `scalbn(x, n)` from `s_scalbn.c`: x * 2**n computed by exponent manipulation.
fn scalbn(x: f64, n: i32) -> f64 {
    let mut x = x;
    let mut hx = hi(x);
    let lx = lo(x);
    let mut k = (hx & 0x7ff00000) >> 20; // extract exponent
    if k == 0 {
        // 0 or subnormal x
        if (lx | (hx & 0x7fffffff) as u32) == 0 {
            return x; // +-0
        }
        x *= TWO54;
        hx = hi(x);
        k = ((hx & 0x7ff00000) >> 20) - 54;
        if n < -50000 {
            return TINY * x; // underflow
        }
    }
    if k == 0x7ff {
        return x + x; // NaN or Inf
    }
    k += n;
    if k > 0x7fe {
        return HUGE * HUGE.copysign(x); // overflow
    }
    if k > 0 {
        // normal result
        return with_hi(x, (hx & 0x800fffff_u32 as i32) | (k << 20));
    }
    if k <= -54 {
        if n > 50000 {
            // in case integer overflow in n+k
            return HUGE * HUGE.copysign(x); // overflow
        } else {
            return TINY * TINY.copysign(x); // underflow
        }
    }
    k += 54; // subnormal result
    x = with_hi(x, (hx & 0x800fffff_u32 as i32) | (k << 20));
    x * TWOM54
}

#[cfg(test)]
mod tests {
    use super::*;

    // Expected bits come from Java's StrictMath.pow (OpenJDK on x86-64). A NaN produced by
    // arithmetic is only checked to be NaN: its sign bit is the CPU's default (x86: 0xfff8...).
    #[test]
    fn pow_matches_strictmath() {
        let b = f64::from_bits;
        let cases = [
            (b(0xc020000000000000), b(0x3fd5555555555555), 0xfff8000000000000), // -8 ** 1/3 -> NaN
            (b(0xc000000000000000), b(0x433fffffffffffff), 0xfff0000000000000), // -2 ** (2**53-1) -> -inf
            (b(0xc000000000000000), b(0xc33fffffffffffff), 0x8000000000000000), // -2 ** -(2**53-1) -> -0.0
            (b(0x7ff8000000000000), b(0x0000000000000000), 0x3ff0000000000000), // NaN ** 0 -> 1.0
            (b(0x3ff0000000000000), b(0x7ff8000000000000), 0x7ff8000000000000), // 1 ** NaN -> NaN
            (b(0x8000000000000000), b(0xc008000000000000), 0xfff0000000000000), // -0.0 ** -3 -> -inf
            (b(0x8000000000000000), b(0x3fe0000000000000), 0x0000000000000000), // -0.0 ** 0.5 -> 0.0
            (b(0xbff0000000000000), b(0x7ff0000000000000), 0xfff8000000000000), // -1 ** inf -> NaN
            (b(0x4000000000000000), b(0xc090c80000000000), 0x0000000000000001), // 2 ** -1074 -> 5e-324
            (b(0x4000000000000000), b(0xc090cc0000000000), 0x0000000000000000), // 2 ** -1075 -> 0.0
            (b(0x4000000000000000), b(0x4090000000000000), 0x7ff0000000000000), // 2 ** 1024 -> inf
            (b(0x4024000000000000), b(0xc073600000000000), 0x000012688b70e62b), // 10 ** -310, subnormal
            (b(0x3fefffffffffffff), b(0xc270000000000000), 0x3ff0008002000555), // (1-ulp/2) ** -2**40
            (b(0x0000000000000001), b(0x3fd0000000000000), 0x2f26a09e667f3bcc), // 5e-324 ** 0.25
            (b(0xfff0000000000000), b(0xc008000000000000), 0x8000000000000000), // -inf ** -3 -> -0.0
            (b(0x4005bf0a8b145769), b(0x40862e42fefa39ef), 0x7feffffffffffdd6), // e ** 709.78...
        ];
        for (x, y, want) in cases {
            let got = pow(x, y);
            if b(want).is_nan() {
                assert!(got.is_nan(), "{x:e} ** {y:e}: got {got:e}");
            } else {
                assert_eq!(got.to_bits(), want, "{x:e} ** {y:e}: got {got:e}, want {:e}", b(want));
            }
        }
    }
}
