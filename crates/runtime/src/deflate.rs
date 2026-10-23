//! The compressor behind `java.util.zip.Deflater`: DEFLATE (RFC 1951), with the zlib wrapper
//! (RFC 1950) unless the stream is `nowrap`. A translated program has no zlib to call, so this
//! is a compressor of its own. Its output is valid DEFLATE data that any inflater, zlib's and the
//! JDK's included, reads back to the same bytes; it is not byte for byte the output of zlib, whose
//! exact choices of matches and blocks are not reproduced.
//!
//! The input is compressed in blocks of up to `BLOCK` bytes. Matches are found as zlib finds
//! them, along chains of earlier positions with the same three bytes, within the last 32 KiB,
//! with the chain length and lazy matching of zlib's levels; each block is then written in the
//! smallest of the three forms, stored, fixed Huffman codes or dynamic Huffman codes made for the
//! block. What Java sees follows zlib: the bytes read and written of each call, the zlib header
//! with the level's flags, the flush markers of `SYNC_FLUSH` and `FULL_FLUSH`, the Adler-32 of
//! the input, preset dictionaries, and changes of level and strategy between calls.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{LazyLock, Mutex, MutexGuard};

use crate::inflate::{DIST_BASE, DIST_EXTRA, LENGTH_BASE, LENGTH_EXTRA, ORDER};

#[cfg(test)]
mod tests;

/// How far back a match may reach.
const WINDOW: usize = 32768;
/// The input bytes compressed as one block (the input is gathered until there are this many,
/// unless a flush asks for it sooner).
const BLOCK: usize = 65536;
const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 258;
const HASH_BITS: u32 = 15;

/// Java's flush modes, as zlib numbers them; `FINISH` is zlib's `Z_FINISH`.
const SYNC_FLUSH: i32 = 2;
const FULL_FLUSH: i32 = 3;
const FINISH: i32 = 4;
const HUFFMAN_ONLY: i32 = 2;

const ILLEGAL_ARGUMENT: &str = "java.lang.IllegalArgumentException";

/// zlib's parameters of a level (`configuration_table` in `deflate.c`): a match this long is
/// taken without looking for a longer one at the next position (`max_lazy`, 0 for the fast
/// levels, which do not look), a match this long ends the search (`nice_length`), and the
/// longest chain searched (`max_chain`).
struct Level {
    lazy: usize,
    nice: usize,
    chain: usize,
}

fn level_config(level: i32) -> Level {
    let (lazy, nice, chain) = match level {
        1 => (0, 8, 4),
        2 => (0, 16, 8),
        3 => (0, 32, 32),
        4 => (4, 16, 16),
        5 => (16, 32, 32),
        6 => (16, 128, 128),
        7 => (32, 128, 256),
        8 => (128, 258, 1024),
        _ => (258, 258, 4096),
    };
    Level { lazy, nice, chain }
}

/// zlib's kinds of compression, which a change of level between two of them flushes the input
/// for (`deflateParams`): stored, fast (levels 1 to 3) and slow (4 to 9).
fn kind(level: i32) -> i32 {
    match level {
        0 => 0,
        1..=3 => 1,
        _ => 2,
    }
}

/// The bits written and not yet given to Java: whole bytes, then the bits of a byte begun.
struct Output {
    bytes: Vec<u8>,
    taken: usize,
    bits: u64,
    count: u32,
}

impl Output {
    fn new() -> Output {
        Output { bytes: Vec::new(), taken: 0, bits: 0, count: 0 }
    }

    /// `n` bits of `value`, the lowest first (RFC 1951, 3.1.1).
    fn put(&mut self, value: u32, n: u32) {
        self.bits |= u64::from(value) << self.count;
        self.count += n;
        while self.count >= 8 {
            self.bytes.push(self.bits as u8);
            self.bits >>= 8;
            self.count -= 8;
        }
    }

    /// Fills the byte begun with zero bits.
    fn align(&mut self) {
        if self.count > 0 {
            self.bytes.push(self.bits as u8);
            self.bits = 0;
            self.count = 0;
        }
    }

    /// Whole bytes, at a byte boundary.
    fn push(&mut self, bytes: &[u8]) {
        debug_assert_eq!(self.count, 0);
        self.bytes.extend_from_slice(bytes);
    }

    fn pending(&self) -> bool {
        self.taken < self.bytes.len()
    }

    /// Gives as many whole bytes as `out` takes; the number given.
    fn drain(&mut self, out: &mut [i8]) -> usize {
        let n = out.len().min(self.bytes.len() - self.taken);
        for (o, &b) in out.iter_mut().zip(&self.bytes[self.taken..self.taken + n]) {
            *o = b as i8;
        }
        self.taken += n;
        if self.taken == self.bytes.len() {
            self.bytes.clear();
            self.taken = 0;
        }
        n
    }
}

/// What one call of `Stream::deflate` did.
#[derive(Debug, PartialEq)]
pub struct Progress {
    pub read: usize,
    pub written: usize,
    pub finished: bool,
}

/// A compressor: the state that Java's `Deflater` keeps in zlib's `z_stream`.
pub struct Stream {
    wrap: bool,
    level: i32,
    strategy: i32,
    /// The last bytes compressed (at most `WINDOW`, the reach of a match), then the input taken
    /// and not compressed yet, from `start`.
    data: Vec<u8>,
    start: usize,
    out: Output,
    /// The Adler-32 of the input taken (of the dictionary, until the header is written); 1 for a
    /// raw stream, as zlib leaves it.
    adler: u32,
    /// A preset dictionary was given before the header.
    dictionary: bool,
    /// zlib's `last_flush`: the flush of the last call, -2 before the first, -1 after a call that
    /// filled the output.
    last_flush: i32,
    /// The header is written (zlib's `status` is past `INIT_STATE`).
    started: bool,
    /// The last block, and the trailer, are written.
    ended: bool,
}

impl Stream {
    /// A stream with its zlib header, or a raw one, or `None` for a level or a strategy that zlib
    /// does not take.
    pub fn new(level: i32, strategy: i32, wrap: bool) -> Option<Stream> {
        if !(-1..=9).contains(&level) || !(0..=4).contains(&strategy) {
            return None;
        }
        Some(Stream {
            wrap,
            level: if level == -1 { 6 } else { level },
            strategy,
            data: Vec::new(),
            start: 0,
            out: Output::new(),
            adler: 1,
            dictionary: false,
            last_flush: -2,
            started: false,
            ended: false,
        })
    }

    /// zlib's `deflateReset`: the stream starts again, with its level, strategy and wrapper.
    pub fn reset(&mut self) {
        let (level, strategy, wrap) = (self.level, self.strategy, self.wrap);
        *self = Stream::new(level, strategy, wrap).expect("a stream's own level");
    }

    /// zlib's `deflateSetDictionary`: refused for a zlib stream once its header is written, until
    /// its trailer is (zlib then takes the stream for a raw one), and while input waits to be
    /// compressed.
    pub fn set_dictionary(&mut self, dict: &[i8]) -> Result<(), ()> {
        if (self.wrap && self.started && !self.ended) || self.start < self.data.len() {
            return Err(());
        }
        let dict: Vec<u8> = dict.iter().map(|&b| b as u8).collect();
        if self.wrap && !self.ended {
            self.adler = adler32_update(self.adler, &dict);
            self.dictionary |= !dict.is_empty();
        }
        self.data.extend_from_slice(&dict);
        self.start = self.data.len();
        self.keep_window();
        Ok(())
    }

    /// One call of zlib's `deflate` with the flush mode `flush`, from `input` into `out`.
    pub fn deflate(&mut self, input: &[i8], out: &mut [i8], flush: i32) -> Progress {
        let none = Progress { read: 0, written: 0, finished: false };
        if out.is_empty() {
            return none;
        }
        let old_flush = std::mem::replace(&mut self.last_flush, flush);
        let mut written = 0;
        if self.out.pending() {
            written = self.out.drain(out);
            if written == out.len() {
                self.last_flush = -1;
                return Progress { read: 0, written, finished: false };
            }
        } else if input.is_empty() && rank(flush) <= rank(old_flush) && flush != FINISH {
            // Nothing to do, as a repeated flush has nothing new to flush.
            return none;
        }
        if self.ended {
            return Progress { read: 0, written, finished: !self.out.pending() };
        }
        self.header();
        let mut read = 0;
        let mut flushed = false;
        loop {
            written += self.out.drain(&mut out[written..]);
            if written == out.len() || self.ended {
                break;
            }
            if read < input.len() {
                read += self.take(&input[read..]);
                if self.data.len() - self.start == BLOCK {
                    self.compress(false);
                }
                continue;
            }
            match flush {
                SYNC_FLUSH | FULL_FLUSH if !flushed => {
                    self.compress(false);
                    // An empty stored block ends at a byte boundary, so the inflater has every
                    // byte so far (zlib's `_tr_stored_block` of a flush).
                    self.out.put(0, 3);
                    self.out.align();
                    self.out.push(&[0, 0, 0xff, 0xff]);
                    if flush == FULL_FLUSH {
                        self.data.clear();
                        self.start = 0;
                    }
                    flushed = true;
                }
                FINISH => {
                    self.compress(true);
                    self.out.align();
                    if self.wrap {
                        let adler = self.adler.to_be_bytes();
                        self.out.push(&adler);
                    }
                    self.ended = true;
                }
                _ => break,
            }
        }
        let finished = self.ended && !self.out.pending();
        if written == out.len() && !finished {
            self.last_flush = -1;
        }
        Progress { read, written, finished }
    }

    /// zlib's `deflateParams`: a new level and strategy. When the kind of compression changes
    /// once compression has begun, the input so far is compressed first, as a block of the old
    /// kind (`deflate(strm, Z_BLOCK)`); otherwise no input is read.
    pub fn params(
        &mut self,
        level: i32,
        strategy: i32,
        input: &[i8],
        out: &mut [i8],
    ) -> Result<Progress, ()> {
        if !(-1..=9).contains(&level) || !(0..=4).contains(&strategy) {
            return Err(());
        }
        let level = if level == -1 { 6 } else { level };
        let mut progress = Progress { read: 0, written: 0, finished: false };
        if (strategy != self.strategy || kind(level) != kind(self.level))
            && self.last_flush != -2
            && !self.ended
        {
            self.header();
            while progress.read < input.len() {
                progress.read += self.take(&input[progress.read..]);
                if self.data.len() - self.start == BLOCK {
                    self.compress(false);
                }
            }
            self.compress(false);
            progress.written = self.out.drain(out);
            self.last_flush = 5;
        }
        self.level = level;
        self.strategy = strategy;
        Ok(progress)
    }

    pub fn adler(&self) -> u32 {
        self.adler
    }

    /// The zlib header, once, at the first call (RFC 1950, 2.2): the method and window, the
    /// level's flags, and the id of a preset dictionary.
    fn header(&mut self) {
        if self.started {
            return;
        }
        self.started = true;
        if !self.wrap {
            return;
        }
        let flags = if self.strategy >= HUFFMAN_ONLY || self.level < 2 {
            0
        } else if self.level < 6 {
            1
        } else if self.level == 6 {
            2
        } else {
            3
        };
        let mut header: u16 = 0x7800 | (flags << 6);
        if self.dictionary {
            header |= 0x20;
        }
        header += 31 - header % 31;
        self.out.push(&header.to_be_bytes());
        if self.dictionary {
            let id = self.adler.to_be_bytes();
            self.out.push(&id);
        }
        self.adler = 1;
    }

    /// Takes input until the block is full; the number of bytes taken.
    fn take(&mut self, input: &[i8]) -> usize {
        let n = (BLOCK - (self.data.len() - self.start)).min(input.len());
        let bytes: Vec<u8> = input[..n].iter().map(|&b| b as u8).collect();
        if self.wrap {
            self.adler = adler32_update(self.adler, &bytes);
        }
        self.data.extend_from_slice(&bytes);
        n
    }

    /// Compresses the input taken as one block (the stream's last one if `last`). A block that
    /// is not the last is written only when there is input for it.
    fn compress(&mut self, last: bool) {
        if self.start == self.data.len() && !last {
            return;
        }
        let tokens = if self.level == 0 { None } else { Some(self.tokens()) };
        match tokens {
            Some(tokens) => write_block(&mut self.out, &self.data[self.start..], &tokens, last),
            None => write_stored(&mut self.out, &self.data[self.start..], last),
        }
        self.start = self.data.len();
        self.keep_window();
    }

    /// Keeps only the last `WINDOW` bytes of what was compressed.
    fn keep_window(&mut self) {
        if self.start == self.data.len() && self.data.len() > WINDOW {
            self.data.drain(..self.data.len() - WINDOW);
            self.start = self.data.len();
        }
    }

    /// The literals and matches of the input taken, which matches may take from the window
    /// before it.
    fn tokens(&self) -> Vec<Token> {
        let data = &self.data;
        let (start, end) = (self.start, data.len());
        let mut tokens = Vec::new();
        if self.strategy == HUFFMAN_ONLY {
            tokens.extend(data[start..end].iter().map(|&b| Token::Literal(b)));
            return tokens;
        }
        let config = level_config(self.level);
        let mut chains = Chains::new(data, start.saturating_sub(WINDOW));
        let mut i = start;
        let mut ahead: Option<(usize, usize)> = None;
        while i < end {
            let found = ahead.take().unwrap_or_else(|| chains.find(i, end, &config));
            if found.0 < MIN_MATCH {
                tokens.push(Token::Literal(data[i]));
                i += 1;
                continue;
            }
            // Lazy matching: a longer match at the next position wins over this one.
            if found.0 < config.lazy && i + 1 < end {
                let next = chains.find(i + 1, end, &config);
                if next.0 > found.0 {
                    tokens.push(Token::Literal(data[i]));
                    i += 1;
                    ahead = Some(next);
                    continue;
                }
            }
            tokens.push(Token::Match(found.0 as u16, found.1 as u16));
            i += found.0;
        }
        tokens
    }
}

/// zlib's order of flush modes (`RANK` in `deflate.c`).
fn rank(flush: i32) -> i32 {
    flush * 2 - if flush > 4 { 9 } else { 0 }
}

#[derive(Clone, Copy, Debug)]
enum Token {
    Literal(u8),
    /// A length and a distance.
    Match(u16, u16),
}

/// Positions by the hash of their first three bytes, each chained to the one before it with the
/// same hash: zlib's `head` and `prev`.
struct Chains<'a> {
    data: &'a [u8],
    head: Vec<i32>,
    prev: Vec<i32>,
    /// Positions before this one are in the chains.
    inserted: usize,
}

impl<'a> Chains<'a> {
    fn new(data: &'a [u8], from: usize) -> Chains<'a> {
        Chains { data, head: vec![-1; 1 << HASH_BITS], prev: vec![-1; data.len()], inserted: from }
    }

    fn hash(&self, i: usize) -> usize {
        let d = self.data;
        ((usize::from(d[i]) << 10) ^ (usize::from(d[i + 1]) << 5) ^ usize::from(d[i + 2]))
            & ((1 << HASH_BITS) - 1)
    }

    /// Puts every position before `to` into its chain.
    fn insert_until(&mut self, to: usize) {
        while self.inserted < to {
            let i = self.inserted;
            if i + MIN_MATCH <= self.data.len() {
                let h = self.hash(i);
                self.prev[i] = self.head[h];
                self.head[h] = i as i32;
            }
            self.inserted += 1;
        }
    }

    /// The longest match for position `i` within the window, ending by `end`: its length (below
    /// `MIN_MATCH` for none) and distance.
    fn find(&mut self, i: usize, end: usize, config: &Level) -> (usize, usize) {
        self.insert_until(i);
        if i + MIN_MATCH > end {
            return (0, 0);
        }
        let limit = MAX_MATCH.min(end - i);
        let d = self.data;
        let mut best = (0, 0);
        let mut candidate = self.head[self.hash(i)];
        let mut chain = config.chain;
        while candidate >= 0 && chain > 0 {
            let c = candidate as usize;
            if i - c > WINDOW {
                break;
            }
            if d[c + best.0.min(limit - 1)] == d[i + best.0.min(limit - 1)] {
                let len = d[c..].iter().zip(&d[i..i + limit]).take_while(|(a, b)| a == b).count();
                if len > best.0 {
                    best = (len, i - c);
                    if len >= config.nice || len == limit {
                        break;
                    }
                }
            }
            candidate = self.prev[c];
            chain -= 1;
        }
        best
    }
}

/// The length symbol (257 to 285) of a match length, with its extra bits and their number.
fn length_code(len: usize) -> (usize, u32, u32) {
    let i = LENGTH_BASE.iter().rposition(|&base| base <= len).expect("a match of at least 3");
    (257 + i, (len - LENGTH_BASE[i]) as u32, LENGTH_EXTRA[i])
}

/// The distance symbol (0 to 29) of a match distance, with its extra bits and their number.
fn distance_code(dist: usize) -> (usize, u32, u32) {
    let i = DIST_BASE.iter().rposition(|&base| base <= dist).expect("a distance of at least 1");
    (i, (dist - DIST_BASE[i]) as u32, DIST_EXTRA[i])
}

/// The code lengths of the fixed codes (RFC 1951, 3.2.6).
fn fixed_lengths() -> (Vec<u8>, Vec<u8>) {
    let mut lit = vec![8u8; 288];
    lit[144..256].fill(9);
    lit[256..280].fill(7);
    (lit, vec![5u8; 30])
}

/// Writes a block of `bytes` as `tokens`, in whichever of the three forms is the smallest.
fn write_block(out: &mut Output, bytes: &[u8], tokens: &[Token], last: bool) {
    let mut lit_freq = vec![0u32; 286];
    let mut dist_freq = vec![0u32; 30];
    let mut extra_bits = 0u64;
    for &t in tokens {
        match t {
            Token::Literal(b) => lit_freq[usize::from(b)] += 1,
            Token::Match(len, dist) => {
                let (l, _, le) = length_code(usize::from(len));
                let (d, _, de) = distance_code(usize::from(dist));
                lit_freq[l] += 1;
                dist_freq[d] += 1;
                extra_bits += u64::from(le + de);
            }
        }
    }
    lit_freq[256] = 1;
    let cost = |lit: &[u8], dist: &[u8]| -> u64 {
        let l: u64 = lit_freq.iter().zip(lit).map(|(&f, &n)| u64::from(f) * u64::from(n)).sum();
        let d: u64 = dist_freq.iter().zip(dist).map(|(&f, &n)| u64::from(f) * u64::from(n)).sum();
        l + d + extra_bits
    };
    let (fixed_lit, fixed_dist) = fixed_lengths();
    let fixed = 3 + cost(&fixed_lit, &fixed_dist);
    let dyn_lit = code_lengths(&lit_freq, 15);
    let dyn_dist = code_lengths(&dist_freq, 15);
    let header = DynamicHeader::new(&dyn_lit, &dyn_dist);
    let dynamic = 3 + header.bits() + cost(&dyn_lit, &dyn_dist);
    let stored = stored_bits(out, bytes.len());
    if stored <= fixed.min(dynamic) {
        write_stored(out, bytes, last);
    } else if fixed <= dynamic {
        out.put(u32::from(last), 1);
        out.put(1, 2);
        write_tokens(out, tokens, &fixed_lit, &fixed_dist);
    } else {
        out.put(u32::from(last), 1);
        out.put(2, 2);
        header.write(out);
        write_tokens(out, tokens, &dyn_lit, &dyn_dist);
    }
}

/// The bits of `len` bytes as stored blocks, from where the output is.
fn stored_bits(out: &Output, len: usize) -> u64 {
    let blocks = len.div_ceil(65535).max(1) as u64;
    // The first header pads to the byte after it; the others start at a byte boundary.
    let first = 3 + (8 - (out.count + 3) % 8) % 8;
    u64::from(first) + (blocks - 1) * 8 + blocks * 32 + 8 * len as u64
}

/// Writes `bytes` as stored blocks of at most 65535 bytes (RFC 1951, 3.2.4).
fn write_stored(out: &mut Output, bytes: &[u8], last: bool) {
    let mut chunks: Vec<&[u8]> = bytes.chunks(65535).collect();
    if chunks.is_empty() {
        chunks.push(&[]);
    }
    let n = chunks.len();
    for (i, chunk) in chunks.into_iter().enumerate() {
        out.put(u32::from(last && i + 1 == n), 1);
        out.put(0, 2);
        out.align();
        let len = chunk.len() as u16;
        out.push(&len.to_le_bytes());
        out.push(&(!len).to_le_bytes());
        out.push(chunk);
    }
}

/// Writes the tokens with the codes of those lengths, then the end of the block.
fn write_tokens(out: &mut Output, tokens: &[Token], lit: &[u8], dist: &[u8]) {
    let lit_codes = codes(lit);
    let dist_codes = codes(dist);
    for &t in tokens {
        match t {
            Token::Literal(b) => out.put(lit_codes[usize::from(b)], u32::from(lit[usize::from(b)])),
            Token::Match(len, d) => {
                let (l, lx, ln) = length_code(usize::from(len));
                out.put(lit_codes[l], u32::from(lit[l]));
                out.put(lx, ln);
                let (c, dx, dn) = distance_code(usize::from(d));
                out.put(dist_codes[c], u32::from(dist[c]));
                out.put(dx, dn);
            }
        }
    }
    out.put(lit_codes[256], u32::from(lit[256]));
}

/// The header of a dynamic block (RFC 1951, 3.2.7): the code lengths of its two codes, in runs,
/// coded with a code of their own.
struct DynamicHeader {
    lit_count: usize,
    dist_count: usize,
    /// The code lengths of the code-length code, and how many of them are given, in `ORDER`.
    code_lengths: Vec<u8>,
    code_count: usize,
    /// The runs: a symbol of the code-length code and its extra bits.
    runs: Vec<(u8, u32)>,
}

impl DynamicHeader {
    fn new(lit: &[u8], dist: &[u8]) -> DynamicHeader {
        let lit_count = 257.max(lit.iter().rposition(|&n| n > 0).map_or(0, |i| i + 1));
        let dist_count = 1.max(dist.iter().rposition(|&n| n > 0).map_or(0, |i| i + 1));
        let mut runs = Vec::new();
        runs_of(&lit[..lit_count], &mut runs);
        runs_of(&dist[..dist_count], &mut runs);
        let mut freq = vec![0u32; 19];
        for &(sym, _) in &runs {
            freq[usize::from(sym)] += 1;
        }
        let code_lengths = code_lengths(&freq, 7);
        let code_count = 4.max(ORDER.iter().rposition(|&s| code_lengths[s] > 0).map_or(0, |i| i + 1));
        DynamicHeader { lit_count, dist_count, code_lengths, code_count, runs }
    }

    fn bits(&self) -> u64 {
        let runs: u64 = self
            .runs
            .iter()
            .map(|&(sym, _)| u64::from(self.code_lengths[usize::from(sym)]) + u64::from(run_extra(sym)))
            .sum();
        14 + 3 * self.code_count as u64 + runs
    }

    fn write(&self, out: &mut Output) {
        out.put((self.lit_count - 257) as u32, 5);
        out.put((self.dist_count - 1) as u32, 5);
        out.put((self.code_count - 4) as u32, 4);
        for &s in &ORDER[..self.code_count] {
            out.put(u32::from(self.code_lengths[s]), 3);
        }
        let codes = codes(&self.code_lengths);
        for &(sym, extra) in &self.runs {
            let s = usize::from(sym);
            out.put(codes[s], u32::from(self.code_lengths[s]));
            out.put(extra, run_extra(sym));
        }
    }
}

/// The number of extra bits of a symbol of the code-length code.
fn run_extra(sym: u8) -> u32 {
    match sym {
        16 => 2,
        17 => 3,
        18 => 7,
        _ => 0,
    }
}

/// The code lengths as runs of the code-length code: 16 repeats the length before it 3 to 6
/// times, 17 and 18 give 3 to 10 and 11 to 138 zeros.
fn runs_of(lengths: &[u8], runs: &mut Vec<(u8, u32)>) {
    let mut i = 0;
    while i < lengths.len() {
        let len = lengths[i];
        let run = lengths[i..].iter().take_while(|&&n| n == len).count();
        let mut left = run;
        if len == 0 {
            while left >= 11 {
                let n = left.min(138);
                runs.push((18, (n - 11) as u32));
                left -= n;
            }
            if left >= 3 {
                runs.push((17, (left - 3) as u32));
                left = 0;
            }
        } else {
            runs.push((len, 0));
            left -= 1;
            while left >= 3 {
                let n = left.min(6);
                runs.push((16, (n - 3) as u32));
                left -= n;
            }
        }
        runs.extend(std::iter::repeat_n((len, 0), left));
        i += run;
    }
}

/// Huffman code lengths for the frequencies, none longer than `limit`. At least two symbols get
/// a code, as zlib gives them (an inflater takes a code of one symbol only with special care).
/// Where the best code is too long, the frequencies are halved until it is not.
fn code_lengths(freq: &[u32], limit: u8) -> Vec<u8> {
    let mut freq = freq.to_vec();
    let used = freq.iter().filter(|&&f| f > 0).count();
    if used < 2 {
        for f in freq.iter_mut().filter(|f| **f == 0).take(2 - used) {
            *f = 1;
        }
    }
    loop {
        let lengths = huffman_lengths(&freq);
        if lengths.iter().all(|&n| n <= limit) {
            return lengths;
        }
        for f in freq.iter_mut().filter(|f| **f > 0) {
            *f = f.div_ceil(2);
        }
    }
}

/// The depths of the symbols in a Huffman tree of the frequencies (0 for an unused symbol).
fn huffman_lengths(freq: &[u32]) -> Vec<u8> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let mut parent: Vec<usize> = Vec::new();
    let mut heap = BinaryHeap::new();
    let mut leaves = Vec::new();
    for (sym, &f) in freq.iter().enumerate().filter(|(_, f)| **f > 0) {
        leaves.push(sym);
        heap.push(Reverse((u64::from(f), parent.len())));
        parent.push(usize::MAX);
    }
    while heap.len() > 1 {
        let Reverse((a, x)) = heap.pop().expect("two nodes");
        let Reverse((b, y)) = heap.pop().expect("two nodes");
        let node = parent.len();
        parent.push(usize::MAX);
        parent[x] = node;
        parent[y] = node;
        heap.push(Reverse((a + b, node)));
    }
    let mut lengths = vec![0u8; freq.len()];
    for (leaf, &sym) in leaves.iter().enumerate() {
        let mut depth = 0u32;
        let mut n = leaf;
        while parent[n] != usize::MAX {
            n = parent[n];
            depth += 1;
        }
        lengths[sym] = depth.min(255) as u8;
    }
    lengths
}

/// The canonical codes of the lengths (RFC 1951, 3.2.2), bit-reversed, as `Output::put` writes
/// the lowest bit first and a code is read from its highest.
fn codes(lengths: &[u8]) -> Vec<u32> {
    let mut count = [0u32; 16];
    for &n in lengths {
        count[usize::from(n)] += 1;
    }
    count[0] = 0;
    let mut next = [0u32; 16];
    let mut code = 0;
    for bits in 1..16 {
        code = (code + count[bits - 1]) << 1;
        next[bits] = code;
    }
    lengths
        .iter()
        .map(|&n| {
            if n == 0 {
                return 0;
            }
            let c = next[usize::from(n)];
            next[usize::from(n)] += 1;
            c.reverse_bits() >> (32 - u32::from(n))
        })
        .collect()
}

/// Adler-32 (RFC 1950, 8.2) of `bytes`, continuing from `adler`.
fn adler32_update(adler: u32, bytes: &[u8]) -> u32 {
    const MOD: u32 = 65521;
    let (mut a, mut b) = (adler & 0xffff, adler >> 16);
    // 5552 bytes is the most that cannot overflow `b` before the remainder is taken.
    for chunk in bytes.chunks(5552) {
        for &byte in chunk {
            a += u32::from(byte);
            b += a;
        }
        a %= MOD;
        b %= MOD;
    }
    (b << 16) | a
}

static STREAMS: LazyLock<Mutex<HashMap<i64, Stream>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static NEXT: AtomicI64 = AtomicI64::new(1);

fn streams() -> MutexGuard<'static, HashMap<i64, Stream>> {
    STREAMS.lock().unwrap_or_else(|e| e.into_inner())
}

/// `Deflater.init`: a new stream, with its handle (the `long addr` that Java keeps). A level
/// outside -1 to 9 throws `IllegalArgumentException` without a message, as zlib refuses it.
pub fn deflate_init(level: i32, strategy: i32, nowrap: bool) -> Result<i64, crate::Exception> {
    let stream = Stream::new(level, strategy, !nowrap).ok_or(crate::Exception::bare(ILLEGAL_ARGUMENT))?;
    let addr = NEXT.fetch_add(1, Ordering::Relaxed);
    streams().insert(addr, stream);
    Ok(addr)
}

/// `Deflater.deflateBytesBytes`: the result packs the bytes read (bits 0 to 30), the bytes
/// written (31 to 61), `finished` (bit 62) and whether the new parameters are still to be set
/// (bit 63), as Java unpacks it. `params` is Java's: bit 0 asks for a new strategy (bits 1 and 2)
/// and level (bits 3 on), which this call then sets instead of compressing.
#[allow(clippy::too_many_arguments)]
pub fn deflate_bytes(
    addr: i64,
    input: &[i8],
    output: &mut [i8],
    out_off: i32,
    out_len: i32,
    flush: i32,
    params: i32,
) -> i64 {
    let output = &mut output[out_off as usize..(out_off + out_len) as usize];
    run(addr, input, output, flush, params).0
}

/// `Deflater.deflateBytesBuffer`: as `deflate_bytes`, with the output in memory outside the heap,
/// at `output_addr`.
pub fn deflate_bytes_buffer(
    addr: i64,
    input: &[i8],
    output_addr: i64,
    out_len: i32,
    flush: i32,
    params: i32,
) -> i64 {
    let mut output = vec![0; out_len.max(0) as usize];
    let (packed, written) = run(addr, input, &mut output, flush, params);
    crate::inflate::write_bytes(output_addr, &output[..written]);
    packed
}

/// `Deflater.deflateBufferBytes`: as `deflate_bytes`, with the input in memory outside the heap,
/// at `input_addr`.
#[allow(clippy::too_many_arguments)]
pub fn deflate_buffer_bytes(
    addr: i64,
    input_addr: i64,
    in_len: i32,
    output: &mut [i8],
    out_off: i32,
    out_len: i32,
    flush: i32,
    params: i32,
) -> i64 {
    let input = crate::inflate::read_bytes(input_addr, in_len);
    deflate_bytes(addr, &input, output, out_off, out_len, flush, params)
}

/// `Deflater.deflateBufferBuffer`: as `deflate_bytes`, with the input and the output both in
/// memory outside the heap.
pub fn deflate_buffer_buffer(
    addr: i64,
    input_addr: i64,
    in_len: i32,
    output_addr: i64,
    out_len: i32,
    flush: i32,
    params: i32,
) -> i64 {
    let input = crate::inflate::read_bytes(input_addr, in_len);
    deflate_bytes_buffer(addr, &input, output_addr, out_len, flush, params)
}

/// Compresses `input` into `output` with the stream at `addr`, or sets its parameters: the
/// result packed as `deflate_bytes` gives it, and the number of bytes written. The parameters
/// are always ones zlib takes, as `setLevel` and `setStrategy` check them, so bit 63 (the
/// parameters are still to be set) is never set.
fn run(addr: i64, input: &[i8], output: &mut [i8], flush: i32, params: i32) -> (i64, usize) {
    let mut table = streams();
    let stream = table.get_mut(&addr).expect("Deflater used after end");
    let progress = if params & 1 != 0 {
        stream
            .params(params >> 3, (params >> 1) & 3, input, output)
            .expect("Deflater's setLevel and setStrategy check the parameters")
    } else {
        stream.deflate(input, output, flush)
    };
    let packed = progress.read as i64 | (progress.written as i64) << 31 | i64::from(progress.finished) << 62;
    (packed, progress.written)
}

/// `Deflater.getAdler`.
pub fn deflate_adler(addr: i64) -> i32 {
    streams()[&addr].adler() as i32
}

/// `Deflater.reset`.
pub fn deflate_reset(addr: i64) {
    streams().get_mut(&addr).expect("Deflater used after end").reset();
}

/// `Deflater.end`.
pub fn deflate_end(addr: i64) {
    streams().remove(&addr);
}

/// `Deflater.setDictionary`, which throws `IllegalArgumentException` (with no message, as zlib
/// gives none) where zlib refuses the dictionary: in a zlib stream after the first call of
/// `deflate`, and while input waits to be compressed.
pub fn deflate_set_dictionary(addr: i64, dict: &[i8], off: i32, len: i32) -> Result<(), crate::Exception> {
    set_dictionary(addr, &dict[off as usize..(off + len) as usize])
}

/// `Deflater.setDictionaryBuffer`: as `deflate_set_dictionary`, with the dictionary in memory
/// outside the heap, at `buffer_addr`.
pub fn deflate_set_dictionary_buffer(addr: i64, buffer_addr: i64, len: i32) -> Result<(), crate::Exception> {
    set_dictionary(addr, &crate::inflate::read_bytes(buffer_addr, len))
}

fn set_dictionary(addr: i64, dict: &[i8]) -> Result<(), crate::Exception> {
    let mut table = streams();
    let stream = table.get_mut(&addr).expect("Deflater used after end");
    stream.set_dictionary(dict).map_err(|()| crate::Exception::bare(ILLEGAL_ARGUMENT))
}
