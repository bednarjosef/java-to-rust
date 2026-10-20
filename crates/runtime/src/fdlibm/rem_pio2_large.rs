//! `__kernel_rem_pio2` from `k_rem_pio2.c`: the reduction of a large argument modulo pi/2.
//!
//! It returns the last three digits of N with y = x - N*pi/2, so that |y| < pi/2.
//!
//! The method is to compute the integer (mod 8) and fraction parts of (2/pi)*x without doing the
//! full multiplication. In general we skip the part of the product that is known to be a huge
//! integer (more accurately, = 0 mod 8). Thus the number of operations is independent of the
//! exponent of the input.
//!
//! (2/pi) is represented by an array of 24-bit integers in `ipio2[]`.
//!
//! Input parameters:
//! - `x[]`: the input value (must be positive) broken into pieces of 24-bit integers in double
//!   precision format; `x[i]` is the i-th 24 bits of x. The scaled exponent of `x[0]` is `e0`
//!   (i.e. `x[0]*2^e0` matches x up to 24 bits). Example of breaking a positive z into
//!   `x[0]+x[1]+x[2]`: `e0 = ilogb(z)-23; z = scalbn(z,-e0)`, then for i = 0,1,2:
//!   `x[i] = floor(z); z = (z-x[i])*2**24`.
//! - `y[]`: the result, whose actual value is the sum of its elements. Its dimension is 1 for
//!   24-bit precision, 2 for 53-bit and 64-bit, 3 for 113-bit.
//! - `e0`: the exponent of `x[0]`.
//! - `prec`: 0 for 24 bits (single), 1 for 53 bits (double), 2 for 64 bits (extended), 3 for 113
//!   bits (quad).
//! - `ipio2[]`: the (24*i)-th to (24*i+23)-th bit of 2/pi after the binary point, so the
//!   corresponding floating value is `ipio2[i] * 2^(-24(i+1))`.
//!
//! Local variables:
//! - `jk`: jk+1 is the initial number of terms of `ipio2[]` needed in the computation. The
//!   recommended value is 2,3,4,6 for single, double, extended and quad.
//! - `jz`: the number of terms of `ipio2[]` used.
//! - `jx`: nx - 1.
//! - `jv`: index of the suitable `ipio2[]` for the computation. In general we want
//!   `( 2^e0*x[0] * ipio2[jv-1]*2^(-24jv) )/8` to be an integer, so e0-3-24*jv >= 0, hence
//!   jv = max(0,(e0-3)/24).
//! - `jp`: jp+1 is the number of terms in `PIo2[]` needed, jp = jk.
//! - `q[]`: doubles with integral values, the 24-bit chunks of the product of x and 2/pi.
//! - `q0`: the exponent of `q[0]`; the exponent of `q[i]` is q0-24*i.
//! - `PIo2[]`: pi/2 cut into 24-bit chunks.
//! - `f[]`: `ipio2[]` in floating point.
//! - `iq[]`: `q[]` broken up into 24-bit integer chunks.
//! - `fq[]`: the final product of x*(2/pi) in `fq[0]`,..,`fq[jk]`.
//! - `ih`: if > 0, q[] is >= 0.5, hence it also indicates the *sign* of the result.

/// Initial value for jk.
const INIT_JK: [usize; 4] = [2, 3, 4, 6];

const PIO2: [f64; 8] = [
    1.57079625129699707031e+00, // 0x3FF921FB, 0x40000000
    7.54978941586159635335e-08, // 0x3E74442D, 0x00000000
    5.39030252995776476554e-15, // 0x3CF84698, 0x80000000
    3.28200341580791294123e-22, // 0x3B78CC51, 0x60000000
    1.27065575308067607349e-29, // 0x39F01B83, 0x80000000
    1.22933308981111328932e-36, // 0x387A2520, 0x40000000
    2.73370053816464559624e-44, // 0x36E38222, 0x80000000
    2.16741683877804819444e-51, // 0x3569F31D, 0x00000000
];

const ZERO: f64 = 0.0;
const ONE: f64 = 1.0;
const TWO24: f64 = 1.67772160000000000000e+07; // 0x41700000, 0x00000000
const TWON24: f64 = 5.96046447753906250000e-08; // 0x3E700000, 0x00000000

/// `scalbn(x, n)`, x * 2^n. A single multiplication by the power of two rounds once, as
/// fdlibm's `scalbn` does, provided 2^n is itself a normal double; the exponents used here stay
/// far inside that range.
fn scalbn(x: f64, n: i32) -> f64 {
    debug_assert!((-1022..=1023).contains(&n));
    x * f64::from_bits(((0x3ff + n) as u64) << 52)
}

/// `__kernel_rem_pio2(x, y, e0, nx, prec, ipio2)` with nx = `x.len()`: writes the remainder of
/// x*2^e0 modulo pi/2 into `y` and returns the last three bits of the quotient.
pub(super) fn kernel_rem_pio2(x: &[f64], y: &mut [f64], e0: i32, prec: usize, ipio2: &[i32]) -> i32 {
    let mut iq = [0i32; 20];
    let mut f = [0.0f64; 20];
    let mut fq = [0.0f64; 20];
    let mut q = [0.0f64; 20];

    // initialize jk
    let jk = INIT_JK[prec];
    let jp = jk;

    // determine jx, jv, q0, note that 3 > q0
    let jx = x.len() - 1;
    let mut jv = (e0 - 3) / 24;
    if jv < 0 {
        jv = 0;
    }
    let mut q0 = e0 - 24 * (jv + 1);
    let jv = jv as usize;

    // set up f[0] to f[jx+jk] where f[jx+jk] = ipio2[jv+jk]
    let m = jx + jk;
    for (i, fi) in f.iter_mut().enumerate().take(m + 1) {
        let j = (jv + i) as i32 - jx as i32;
        *fi = if j < 0 { ZERO } else { ipio2[j as usize] as f64 };
    }

    // compute q[0],q[1],...q[jk]
    for i in 0..=jk {
        let mut fw = 0.0;
        for j in 0..=jx {
            fw += x[j] * f[jx + i - j];
        }
        q[i] = fw;
    }

    let mut jz = jk;
    let mut z;
    let mut n;
    let mut ih;
    loop {
        // distill q[] into iq[] reversingly
        let mut i = 0;
        let mut j = jz;
        z = q[jz];
        while j > 0 {
            let fw = ((TWON24 * z) as i32) as f64;
            iq[i] = (z - TWO24 * fw) as i32;
            z = q[j - 1] + fw;
            i += 1;
            j -= 1;
        }

        // compute n
        z = scalbn(z, q0); // actual value of z
        z -= 8.0 * (z * 0.125).floor(); // trim off integer >= 8
        n = z as i32;
        z -= n as f64;
        ih = 0;
        if q0 > 0 {
            // need iq[jz-1] to determine n
            let i = iq[jz - 1] >> (24 - q0);
            n += i;
            iq[jz - 1] -= i << (24 - q0);
            ih = iq[jz - 1] >> (23 - q0);
        } else if q0 == 0 {
            ih = iq[jz - 1] >> 23;
        } else if z >= 0.5 {
            ih = 2;
        }

        if ih > 0 {
            // q > 0.5
            n += 1;
            let mut carry = 0;
            for iqi in iq.iter_mut().take(jz) {
                // compute 1-q
                let j = *iqi;
                if carry == 0 {
                    if j != 0 {
                        carry = 1;
                        *iqi = 0x1000000 - j;
                    }
                } else {
                    *iqi = 0xffffff - j;
                }
            }
            if q0 > 0 {
                // rare case: chance is 1 in 12
                match q0 {
                    1 => iq[jz - 1] &= 0x7fffff,
                    2 => iq[jz - 1] &= 0x3fffff,
                    _ => {}
                }
            }
            if ih == 2 {
                z = ONE - z;
                if carry != 0 {
                    z -= scalbn(ONE, q0);
                }
            }
        }

        // check if recomputation is needed
        if z == ZERO {
            let mut j = 0;
            for i in (jk..jz).rev() {
                j |= iq[i];
            }
            if j == 0 {
                // need recomputation
                let mut k = 1;
                while iq[jk - k] == 0 {
                    k += 1; // k = no. of terms needed
                }

                for i in jz + 1..=jz + k {
                    // add q[jz+1] to q[jz+k]
                    f[jx + i] = ipio2[jv + i] as f64;
                    let mut fw = 0.0;
                    for j in 0..=jx {
                        fw += x[j] * f[jx + i - j];
                    }
                    q[i] = fw;
                }
                jz += k;
                continue;
            }
        }
        break;
    }

    // chop off zero terms
    if z == 0.0 {
        jz -= 1;
        q0 -= 24;
        while iq[jz] == 0 {
            jz -= 1;
            q0 -= 24;
        }
    } else {
        // break z into 24-bit if necessary
        z = scalbn(z, -q0);
        if z >= TWO24 {
            let fw = ((TWON24 * z) as i32) as f64;
            iq[jz] = (z - TWO24 * fw) as i32;
            jz += 1;
            q0 += 24;
            iq[jz] = fw as i32;
        } else {
            iq[jz] = z as i32;
        }
    }

    // convert integer "bit" chunk to floating-point value
    let mut fw = scalbn(ONE, q0);
    for i in (0..=jz).rev() {
        q[i] = fw * iq[i] as f64;
        fw *= TWON24;
    }

    // compute PIo2[0,...,jp]*q[jz,...,0]
    for i in (0..=jz).rev() {
        let mut fw = 0.0;
        let mut k = 0;
        while k <= jp && k <= jz - i {
            fw += PIO2[k] * q[i + k];
            k += 1;
        }
        fq[jz - i] = fw;
    }

    // compress fq[] into y[]
    match prec {
        0 => {
            let mut fw = 0.0;
            for i in (0..=jz).rev() {
                fw += fq[i];
            }
            y[0] = if ih == 0 { fw } else { -fw };
        }
        1 | 2 => {
            let mut fw = 0.0;
            for i in (0..=jz).rev() {
                fw += fq[i];
            }
            y[0] = if ih == 0 { fw } else { -fw };
            fw = fq[0] - fw;
            for fqi in &fq[1..=jz] {
                fw += *fqi;
            }
            y[1] = if ih == 0 { fw } else { -fw };
        }
        3 => {
            // painful
            for i in (1..=jz).rev() {
                let fw = fq[i - 1] + fq[i];
                fq[i] += fq[i - 1] - fw;
                fq[i - 1] = fw;
            }
            for i in (2..=jz).rev() {
                let fw = fq[i - 1] + fq[i];
                fq[i] += fq[i - 1] - fw;
                fq[i - 1] = fw;
            }
            let mut fw = 0.0;
            for i in (2..=jz).rev() {
                fw += fq[i];
            }
            if ih == 0 {
                y[0] = fq[0];
                y[1] = fq[1];
                y[2] = fw;
            } else {
                y[0] = -fq[0];
                y[1] = -fq[1];
                y[2] = -fw;
            }
        }
        _ => {}
    }
    n & 7
}
