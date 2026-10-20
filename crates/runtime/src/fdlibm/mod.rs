//! A port of fdlibm 5.3, the C library that Java's `StrictMath` is specified to match bit for
//! bit. `Math` delegates to these routines for the functions HotSpot does not replace with an
//! intrinsic (see `natives.rs`).
//!
//! The routines follow the C sources from netlib (<https://www.netlib.org/fdlibm/>) line by
//! line, so that every intermediate rounding is the same. Their notice:
//!
//! > Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
//! >
//! > Developed at SunSoft, a Sun Microsystems, Inc. business.
//! > Permission to use, copy, modify, and distribute this
//! > software is freely granted, provided that this notice
//! > is preserved.
//!
//! (`e_pow.c` and `k_rem_pio2.c` carry the same notice dated 2004 and 1993.)

#![allow(clippy::excessive_precision, clippy::approx_constant, clippy::many_single_char_names)]

mod exp_log;
mod hyperbolic;
mod inverse;
mod misc;
mod pow;
mod rem_pio2_large;
mod trig;

pub use exp_log::{exp, expm1, log, log1p, log10};
pub use hyperbolic::{cosh, sinh, tanh};
pub use inverse::{acos, asin, atan, atan2};
pub use misc::{cbrt, hypot, remainder};
pub use pow::pow;
pub use trig::{cos, sin, tan};

/// The high 32 bits of `x`, `__HI(x)` in the C sources: sign, exponent and top of the mantissa.
pub(crate) fn hi(x: f64) -> i32 {
    (x.to_bits() >> 32) as i32
}

/// The low 32 bits of `x`, `__LO(x)`, as the unsigned word the C code mostly treats it as.
pub(crate) fn lo(x: f64) -> u32 {
    x.to_bits() as u32
}

/// `x` with its high word replaced, `__HI(x) = h`.
pub(crate) fn with_hi(x: f64, h: i32) -> f64 {
    f64::from_bits(((h as u32 as u64) << 32) | (x.to_bits() & 0xffff_ffff))
}

/// `x` with its low word replaced, `__LO(x) = l`.
pub(crate) fn with_lo(x: f64, l: u32) -> f64 {
    f64::from_bits((x.to_bits() & 0xffff_ffff_0000_0000) | l as u64)
}

/// The double with the given high and low words.
pub(crate) fn from_words(h: i32, l: u32) -> f64 {
    f64::from_bits(((h as u32 as u64) << 32) | l as u64)
}
