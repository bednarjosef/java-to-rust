//! Input for the float check: bit patterns, one per line in hex (8 digits for `float`, 16 for
//! `double`), on stdout. Both the Java and the runtime side read the same stream.
//!
//! usage:
//!   gen float|double edge                 the edge values (see `edge`)
//!   gen float|double random COUNT SEED    COUNT pseudo-random bit patterns
//!   gen float|double range START COUNT    COUNT consecutive patterns from START

use std::io::{self, BufWriter, Write};

/// Values that cover the boundaries and the decimal shapes of printing, in both widths.
struct Emit {
    out: BufWriter<io::StdoutLock<'static>>,
    float: bool,
}

impl Emit {
    fn new(float: bool) -> Self {
        Emit { out: BufWriter::with_capacity(1 << 16, io::stdout().lock()), float }
    }

    fn width(&self) -> u32 {
        if self.float { 32 } else { 64 }
    }

    fn sign(&self) -> u64 {
        1 << (self.width() - 1)
    }

    /// The mask of the magnitude bits: everything but the sign.
    fn mag_mask(&self) -> u64 {
        self.sign() - 1
    }

    fn bits(&mut self, bits: u64) {
        let line = if self.float { writeln!(self.out, "{:08x}", bits as u32) } else { writeln!(self.out, "{bits:016x}") };
        line.expect("write bit pattern");
    }

    /// Emits `mag` with both signs.
    fn signed(&mut self, mag: u64) {
        self.bits(mag);
        self.bits(mag | self.sign());
    }

    /// Emits `mag` and the patterns up to `spread` steps away from it, each with both signs.
    fn around(&mut self, mag: u64, spread: u64) {
        let lo = mag.saturating_sub(spread);
        let hi = mag.saturating_add(spread).min(self.mag_mask());
        for m in lo..=hi {
            self.signed(m);
        }
    }

    /// The magnitude bits of the decimal `text`, rounded the way Rust parses it (correctly).
    fn decimal(&self, text: &str) -> u64 {
        if self.float {
            text.parse::<f32>().expect("decimal").abs().to_bits() as u64
        } else {
            text.parse::<f64>().expect("decimal").abs().to_bits()
        }
    }

    /// The magnitude bits of the integer `n`, rounded to the nearest value of this width.
    fn integer(&self, n: u64) -> u64 {
        if self.float { (n as f32).to_bits() as u64 } else { (n as f64).to_bits() }
    }

    /// The magnitude bits of `2^k`.
    fn power_of_two(&self, k: i32) -> u64 {
        if self.float { 2f32.powi(k).to_bits() as u64 } else { 2f64.powi(k).to_bits() }
    }

    /// The exponents of the decimals to try: the whole finite range of this width.
    fn decimal_exponents(&self) -> std::ops::RangeInclusive<i32> {
        if self.float { -45..=38 } else { -324..=308 }
    }

    /// Edge values: specials and extremes; decimals with at most three significant digits
    /// near every power of ten; the plain/E-notation boundaries; subnormals; integers up to
    /// 2^24 (float) and 2^20 (double), with the integers near the powers of two up to 2^53.
    fn edge(&mut self) {
        let (min_normal, max_normal, infinity) = if self.float {
            (0x0080_0000, 0x7f7f_ffff, 0x7f80_0000)
        } else {
            (0x0010_0000_0000_0000, 0x7fef_ffff_ffff_ffff, 0x7ff0_0000_0000_0000)
        };
        for mag in [0, min_normal, max_normal, infinity, self.mag_mask()] {
            self.around(mag, 2);
        }

        for e in self.decimal_exponents() {
            for d in 1..=99 {
                let mag = self.decimal(&format!("{d}e{e}"));
                self.around(mag, 2);
            }
        }
        for e in self.decimal_exponents() {
            for d in 100..=999 {
                let mag = self.decimal(&format!("{d}e{e}"));
                self.around(mag, 0);
            }
        }

        for text in ["0.001", "1e-3", "1e7", "9999999", "1e-4", "9.9999999e6"] {
            let mag = self.decimal(text);
            self.around(mag, 50);
        }

        let span = 1 << 17;
        for m in (1..=span).chain(min_normal - span..min_normal) {
            self.signed(m);
        }
        self.around(min_normal, 50);

        let integers = if self.float { 1 << 24 } else { 1 << 20 };
        for n in 0..=integers {
            let mag = self.integer(n);
            self.signed(mag);
        }
        for n in 1..=1 << 12 {
            let mag = self.integer(n);
            self.bits(mag | self.sign());
        }

        let top = if self.float { 127 } else { 1023 };
        let bits_of_int = if self.float { 31 } else { 63 };
        for k in 0..=top {
            let mag = self.power_of_two(k);
            self.around(mag, 2);
        }
        for k in 0..=bits_of_int {
            for d in -3i64..=3 {
                let n = (1u64 << k).wrapping_add_signed(d);
                let mag = self.integer(n);
                self.around(mag, 0);
            }
        }

        let mut rng = SplitMix(0x5eed_f10a);
        let mask = (1u64 << if self.float { 31 } else { 53 }) - 1;
        for _ in 0..200_000 {
            let mag = self.integer(rng.next() & mask);
            self.signed(mag);
        }
    }

    fn random(&mut self, count: u64, seed: u64) {
        let mut rng = SplitMix(seed);
        let width_mask = if self.float { u32::MAX as u64 } else { u64::MAX };
        for _ in 0..count {
            let bits = rng.next() & width_mask;
            self.bits(bits);
        }
    }

    fn range(&mut self, start: u64, count: u64) {
        for bits in start..start + count {
            self.bits(bits);
        }
    }
}

/// SplitMix64: a small, seeded generator, so both runs see the same numbers.
struct SplitMix(u64);

impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}

fn number(args: &[String], i: usize) -> u64 {
    args.get(i).and_then(|s| s.parse().ok()).unwrap_or_else(|| usage())
}

fn usage() -> ! {
    eprintln!("usage: gen float|double edge | random COUNT SEED | range START COUNT");
    std::process::exit(2)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let float = match args.first().map(String::as_str) {
        Some("float") => true,
        Some("double") => false,
        _ => usage(),
    };
    let mut emit = Emit::new(float);
    match args.get(1).map(String::as_str) {
        Some("edge") => emit.edge(),
        Some("random") => emit.random(number(&args, 2), number(&args, 3)),
        Some("range") => emit.range(number(&args, 2), number(&args, 3)),
        _ => usage(),
    }
    emit.out.flush().expect("flush bit patterns");
}
