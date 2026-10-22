//! Canonical Huffman codes for DEFLATE (RFC 1951, 3.2.2), decoded one bit at a time: the
//! code is read bit by bit until it matches a length's range of codes, as zlib's `puff.c` does.

use super::{Bits, R};

pub struct Huffman {
    /// How many symbols have each code length (index 0 unused).
    count: [u16; 16],
    /// The symbols, ordered by code length and then by symbol value.
    symbol: Vec<u16>,
}

impl Huffman {
    /// The code of the symbols with the given lengths (0 for an unused symbol), or `None` where
    /// zlib rejects the set: over-subscribed, or incomplete unless `codes` is false and the only
    /// length is 1 (a single code, as a distance tree may have).
    pub fn build(lengths: &[u8], codes: bool) -> Option<Huffman> {
        let mut count = [0u16; 16];
        for &len in lengths {
            count[len as usize] += 1;
        }
        count[0] = 0;
        let max = (1..16).rev().find(|&len| count[len] > 0).unwrap_or(0);
        let mut left: i32 = 1;
        for &c in &count[1..] {
            left = (left << 1) - i32::from(c);
            if left < 0 {
                return None;
            }
        }
        if left > 0 && max != 0 && (codes || max != 1) {
            return None;
        }
        let mut offset = [0usize; 16];
        for len in 1..15 {
            offset[len + 1] = offset[len] + count[len] as usize;
        }
        let used: usize = count[1..].iter().map(|&c| c as usize).sum();
        let mut symbol = vec![0u16; used];
        for (sym, &len) in lengths.iter().enumerate() {
            if len != 0 {
                symbol[offset[len as usize]] = sym as u16;
                offset[len as usize] += 1;
            }
        }
        Some(Huffman { count, symbol })
    }

    /// The next symbol, read bit by bit; `None` if no code matches (an unused code).
    pub fn decode(&self, bits: &mut Bits) -> R<Option<u16>> {
        let mut code: i32 = 0;
        let mut first: i32 = 0;
        let mut index: i32 = 0;
        for len in 1..16 {
            code |= bits.bit()? as i32;
            let count = i32::from(self.count[len]);
            if code - count < first {
                let at = index + (code - first);
                return Ok(self.symbol.get(at as usize).copied());
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }
        Ok(None)
    }
}
