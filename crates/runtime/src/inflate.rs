//! The decompressor behind `java.util.zip.Inflater`: DEFLATE (RFC 1951), with the zlib wrapper
//! (RFC 1950) unless the stream is `nowrap`. A translated program has no zlib to call, so this
//! reproduces its results: the bytes read and written, the Adler-32 and zlib's messages.
//!
//! Java gives the input and the output in pieces, so the decoder resumes where it stopped. It
//! parses one unit at a time (a block header, a symbol, a stored byte, the trailer). A unit that
//! runs out of input is undone and parsed again when more input comes. Only bytes that a unit
//! used are reported as read, as zlib does, so the input after the end of the stream is left.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{LazyLock, Mutex, MutexGuard};

use crate::fd::Failure;

mod huffman;
#[cfg(test)]
mod tests;

use huffman::Huffman;

/// The window of DEFLATE: a distance reaches back at most this far.
const WINDOW: usize = 32768;

const DATA_FORMAT: &str = "java.util.zip.DataFormatException";

const LENGTH_BASE: [usize; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195,
    227, 258,
];
const LENGTH_EXTRA: [u32; 29] =
    [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DIST_BASE: [usize; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073,
    4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u32; 30] =
    [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];

/// The order in which a dynamic block gives the lengths of its code-length code.
const ORDER: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

/// Why a unit could not be parsed: the input ran out (parse it again with more), or the data is
/// bad, with zlib's message.
pub(super) enum Stop {
    Input,
    Bad(&'static str),
}

pub(super) type R<T> = Result<T, Stop>;

/// The input of one call, and the bits taken from it. `held` holds the bytes from the start of
/// the unit at `at` on, so a unit that stops early leaves them for the next call.
pub(super) struct Bits<'a> {
    data: &'a [i8],
    used: usize,
    held: Vec<u8>,
    at: usize,
}

impl Bits<'_> {
    /// The next bit, least significant bit of each byte first.
    pub(super) fn bit(&mut self) -> R<u32> {
        if self.at / 8 == self.held.len() {
            let Some(&byte) = self.data.get(self.used) else { return Err(Stop::Input) };
            self.held.push(byte as u8);
            self.used += 1;
        }
        let bit = (self.held[self.at / 8] >> (self.at % 8)) & 1;
        self.at += 1;
        Ok(u32::from(bit))
    }

    /// An `n`-bit number, least significant bit first.
    pub(super) fn bits(&mut self, n: u32) -> R<u32> {
        let mut value = 0;
        for i in 0..n {
            value |= self.bit()? << i;
        }
        Ok(value)
    }

    /// Skips to the next byte boundary.
    fn align(&mut self) {
        self.at = self.at.next_multiple_of(8);
    }

    /// Drops the bytes that are used up.
    fn compact(&mut self) {
        let whole = self.at / 8;
        self.held.drain(..whole);
        self.at %= 8;
    }
}

/// Where the output goes: the caller's buffer, and how much of it is filled.
struct Sink<'a> {
    out: &'a mut [i8],
    written: usize,
}

impl Sink<'_> {
    fn full(&self) -> bool {
        self.written == self.out.len()
    }
}

/// The phase of the decoder: what the next unit of input is.
enum Phase {
    /// The two-byte zlib header.
    Header,
    /// A block header, and for a dynamic block its code tables.
    Block,
    /// A stored block, with this many bytes still to copy.
    Stored(usize),
    /// The symbols of a block coded with Huffman codes.
    Codes(Box<Tables>),
    /// The Adler-32 after the last block.
    Trailer,
    /// The header asks for a preset dictionary, whose Adler-32 is in `Stream::adler`.
    NeedDict,
    Done,
}

/// The two codes of a Huffman-coded block: literals and lengths, and distances.
struct Tables {
    lit: Huffman,
    dist: Huffman,
}

impl Tables {
    /// The codes of a fixed-Huffman block (RFC 1951, 3.2.6).
    fn fixed() -> Tables {
        let mut lengths = [0u8; 288];
        for (i, len) in lengths.iter_mut().enumerate() {
            *len = match i {
                0..=143 => 8,
                144..=255 => 9,
                256..=279 => 7,
                _ => 8,
            };
        }
        let lit = Huffman::build(&lengths, false).expect("fixed literal code");
        let dist = Huffman::build(&[5u8; 32], false).expect("fixed distance code");
        Tables { lit, dist }
    }
}

/// What a symbol did: a literal or a match, the end of the block, or a full output.
#[derive(PartialEq)]
enum Flow {
    More,
    End,
    Full,
}

/// What one call of `Stream::inflate` did.
pub struct Progress {
    /// Input bytes used.
    pub read: usize,
    /// Output bytes written.
    pub written: usize,
    pub finished: bool,
    /// The header asks for a dictionary, which `Stream::set_dictionary` must give.
    pub need_dict: bool,
}

/// A decompressor: the state that Java's `Inflater` keeps in zlib's `z_stream`.
pub struct Stream {
    wrap: bool,
    phase: Phase,
    /// The block being decoded is the last one.
    last: bool,
    /// A match not yet copied out: its distance, and the bytes left.
    copy: (usize, usize),
    /// The output so far, kept for matches: at least the last `WINDOW` bytes.
    hist: Vec<u8>,
    /// The Adler-32 of the output (of the dictionary's, while a dictionary is asked for). Zero
    /// for a raw stream.
    adler: u32,
    /// The bytes taken from the input that a unit has not used up, and the bit position in them.
    held: Vec<u8>,
    at: usize,
}

impl Stream {
    /// A stream with its zlib header, or a raw one.
    pub fn new(wrap: bool) -> Stream {
        Stream {
            wrap,
            phase: if wrap { Phase::Header } else { Phase::Block },
            last: false,
            copy: (0, 0),
            hist: Vec::new(),
            adler: if wrap { 1 } else { 0 },
            held: Vec::new(),
            at: 0,
        }
    }

    /// Decompresses from `input` into `out` as far as both allow.
    pub fn inflate(&mut self, input: &[i8], out: &mut [i8]) -> Result<Progress, &'static str> {
        let mut bits = Bits { data: input, used: 0, held: std::mem::take(&mut self.held), at: self.at };
        let mut sink = Sink { out, written: 0 };
        let result = self.run(&mut bits, &mut sink);
        self.held = bits.held;
        self.at = bits.at;
        result?;
        Ok(Progress {
            read: bits.used,
            written: sink.written,
            finished: matches!(self.phase, Phase::Done),
            need_dict: matches!(self.phase, Phase::NeedDict),
        })
    }

    /// Gives the preset dictionary that the header asked for, or any dictionary for a raw stream.
    pub fn set_dictionary(&mut self, dict: &[i8]) -> Result<(), ()> {
        if self.wrap {
            if !matches!(self.phase, Phase::NeedDict) || adler32(dict) != self.adler {
                return Err(());
            }
            self.adler = 1;
            self.phase = Phase::Block;
        }
        self.hist.extend(dict.iter().map(|&b| b as u8));
        Ok(())
    }

    fn run(&mut self, bits: &mut Bits, sink: &mut Sink) -> Result<(), &'static str> {
        loop {
            bits.compact();
            self.trim();
            let mark = bits.at;
            let step = match self.phase {
                Phase::Header => self.header(bits),
                Phase::Block => self.block(bits),
                Phase::Stored(left) => self.stored(bits, sink, left),
                Phase::Codes(_) => self.codes(bits, sink),
                Phase::Trailer => self.trailer(bits),
                Phase::NeedDict | Phase::Done => Ok(false),
            };
            match step {
                Ok(true) => {}
                Ok(false) => return Ok(()),
                Err(Stop::Input) => {
                    bits.at = mark;
                    return Ok(());
                }
                Err(Stop::Bad(message)) => return Err(message),
            }
        }
    }

    /// Keeps the history to the window, once it has grown well past it.
    fn trim(&mut self) {
        if self.hist.len() > 2 * WINDOW {
            self.hist.drain(..self.hist.len() - WINDOW);
        }
    }

    /// The zlib header: the check bits, the method, the window size, and a dictionary's id.
    fn header(&mut self, bits: &mut Bits) -> R<bool> {
        let cmf = bits.bits(8)?;
        let flg = bits.bits(8)?;
        if ((cmf << 8) | flg) % 31 != 0 {
            return Err(Stop::Bad("incorrect header check"));
        }
        if (cmf & 0xf) != 8 {
            return Err(Stop::Bad("unknown compression method"));
        }
        if (cmf >> 4) > 7 {
            return Err(Stop::Bad("invalid window size"));
        }
        if flg & 0x20 != 0 {
            let mut id = 0;
            for _ in 0..4 {
                id = (id << 8) | bits.bits(8)?;
            }
            self.adler = id;
            self.phase = Phase::NeedDict;
            return Ok(false);
        }
        self.phase = Phase::Block;
        Ok(true)
    }

    /// A block header; a dynamic block's code tables follow it in the same unit.
    fn block(&mut self, bits: &mut Bits) -> R<bool> {
        let last = bits.bits(1)? == 1;
        let phase = match bits.bits(2)? {
            0 => {
                bits.align();
                let len = bits.bits(16)?;
                let nlen = bits.bits(16)?;
                if len != !nlen & 0xffff {
                    return Err(Stop::Bad("invalid stored block lengths"));
                }
                Phase::Stored(len as usize)
            }
            1 => Phase::Codes(Box::new(Tables::fixed())),
            2 => Phase::Codes(Box::new(dynamic(bits)?)),
            _ => return Err(Stop::Bad("invalid block type")),
        };
        self.last = last;
        self.phase = phase;
        Ok(true)
    }

    /// One byte of a stored block.
    fn stored(&mut self, bits: &mut Bits, sink: &mut Sink, left: usize) -> R<bool> {
        if left == 0 {
            self.phase = self.after_block();
            return Ok(true);
        }
        if sink.full() {
            return Ok(false);
        }
        let byte = bits.bits(8)? as u8;
        self.emit(sink, byte);
        self.phase = Phase::Stored(left - 1);
        Ok(true)
    }

    /// Symbols of a Huffman-coded block. The tables leave `self.phase` while in use, and go back
    /// unless the block ends.
    fn codes(&mut self, bits: &mut Bits, sink: &mut Sink) -> R<bool> {
        let Phase::Codes(tables) = std::mem::replace(&mut self.phase, Phase::Done) else {
            unreachable!("codes without tables")
        };
        let flow = self.symbol(&tables, bits, sink);
        self.phase = match flow {
            Ok(Flow::End) => self.after_block(),
            _ => Phase::Codes(tables),
        };
        Ok(flow? != Flow::Full)
    }

    /// One symbol: a literal, the end of the block, or a match, which is copied out as far as the
    /// output takes it. A literal that does not fit is left for the next call.
    fn symbol(&mut self, tables: &Tables, bits: &mut Bits, sink: &mut Sink) -> R<Flow> {
        if self.copy.1 > 0 {
            return Ok(if self.copy_out(sink) { Flow::More } else { Flow::Full });
        }
        let mark = bits.at;
        let sym = tables.lit.decode(bits)?.ok_or(Stop::Bad("invalid literal/length code"))?;
        match sym {
            0..=255 => {
                if sink.full() {
                    bits.at = mark;
                    return Ok(Flow::Full);
                }
                self.emit(sink, sym as u8);
                Ok(Flow::More)
            }
            256 => Ok(Flow::End),
            _ => {
                let len = length(sym, bits)?;
                self.copy = (distance(&tables.dist, self, bits)?, len);
                Ok(if self.copy_out(sink) { Flow::More } else { Flow::Full })
            }
        }
    }

    /// Copies the pending match as far as the output takes it; true once all of it is copied.
    fn copy_out(&mut self, sink: &mut Sink) -> bool {
        let dist = self.copy.0;
        while self.copy.1 > 0 && !sink.full() {
            let byte = self.hist[self.hist.len() - dist];
            self.emit(sink, byte);
            self.copy.1 -= 1;
        }
        self.copy.1 == 0
    }

    /// The Adler-32 trailer, after the last block.
    fn trailer(&mut self, bits: &mut Bits) -> R<bool> {
        bits.align();
        let mut sum = 0;
        for _ in 0..4 {
            sum = (sum << 8) | bits.bits(8)?;
        }
        if sum != self.adler {
            return Err(Stop::Bad("incorrect data check"));
        }
        self.phase = Phase::Done;
        Ok(true)
    }

    fn after_block(&self) -> Phase {
        match (self.last, self.wrap) {
            (false, _) => Phase::Block,
            (true, true) => Phase::Trailer,
            (true, false) => Phase::Done,
        }
    }

    fn emit(&mut self, sink: &mut Sink, byte: u8) {
        sink.out[sink.written] = byte as i8;
        sink.written += 1;
        self.hist.push(byte);
        if self.wrap {
            self.adler = adler32_step(self.adler, byte);
        }
    }
}

/// The match length for a length symbol (257 to 285), with its extra bits.
fn length(sym: u16, bits: &mut Bits) -> R<usize> {
    let i = sym as usize - 257;
    if i >= LENGTH_BASE.len() {
        return Err(Stop::Bad("invalid literal/length code"));
    }
    Ok(LENGTH_BASE[i] + bits.bits(LENGTH_EXTRA[i])? as usize)
}

/// The match distance: a distance code and its extra bits, which must reach back into the output.
fn distance(dist: &Huffman, stream: &Stream, bits: &mut Bits) -> R<usize> {
    let Some(sym) = dist.decode(bits)? else { return Err(Stop::Bad("invalid distance code")) };
    let i = sym as usize;
    if i >= DIST_BASE.len() {
        return Err(Stop::Bad("invalid distance code"));
    }
    let distance = DIST_BASE[i] + bits.bits(DIST_EXTRA[i])? as usize;
    if distance > stream.hist.len() {
        return Err(Stop::Bad("invalid distance too far back"));
    }
    Ok(distance)
}

/// The code tables of a dynamic block (RFC 1951, 3.2.7): the code-length code, then the lengths
/// of the literal and distance codes, which it gives in runs.
fn dynamic(bits: &mut Bits) -> R<Tables> {
    let nlen = bits.bits(5)? as usize + 257;
    let ndist = bits.bits(5)? as usize + 1;
    let ncode = bits.bits(4)? as usize + 4;
    if nlen > 286 || ndist > 30 {
        return Err(Stop::Bad("too many length or distance symbols"));
    }
    let mut code_lengths = [0u8; 19];
    for &i in &ORDER[..ncode] {
        code_lengths[i] = bits.bits(3)? as u8;
    }
    let codes = Huffman::build(&code_lengths, true).ok_or(Stop::Bad("invalid code lengths set"))?;
    let total = nlen + ndist;
    let mut lengths = [0u8; 286 + 30];
    let mut i = 0;
    while i < total {
        let (value, repeat) = match codes.decode(bits)? {
            Some(sym @ 0..=15) => (sym as u8, 1),
            Some(16) => {
                if i == 0 {
                    return Err(Stop::Bad("invalid bit length repeat"));
                }
                (lengths[i - 1], 3 + bits.bits(2)? as usize)
            }
            Some(17) => (0, 3 + bits.bits(3)? as usize),
            Some(_) => (0, 11 + bits.bits(7)? as usize),
            None => return Err(Stop::Bad("invalid code lengths set")),
        };
        if i + repeat > total {
            return Err(Stop::Bad("invalid bit length repeat"));
        }
        lengths[i..i + repeat].fill(value);
        i += repeat;
    }
    if lengths[256] == 0 {
        return Err(Stop::Bad("invalid code -- missing end-of-block"));
    }
    let lit = Huffman::build(&lengths[..nlen], false).ok_or(Stop::Bad("invalid literal/lengths set"))?;
    let dist = Huffman::build(&lengths[nlen..total], false).ok_or(Stop::Bad("invalid distances set"))?;
    Ok(Tables { lit, dist })
}

/// One byte of Adler-32 (RFC 1950, 8.2).
fn adler32_step(adler: u32, byte: u8) -> u32 {
    const MOD: u32 = 65521;
    let a = ((adler & 0xffff) + u32::from(byte)) % MOD;
    let b = ((adler >> 16) + a) % MOD;
    (b << 16) | a
}

static STREAMS: LazyLock<Mutex<HashMap<i64, Stream>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static NEXT: AtomicI64 = AtomicI64::new(1);

fn streams() -> MutexGuard<'static, HashMap<i64, Stream>> {
    STREAMS.lock().unwrap_or_else(|e| e.into_inner())
}

/// `Inflater.init`: a new stream, with its handle (the `long addr` that Java keeps).
pub fn inflate_init(nowrap: bool) -> i64 {
    let addr = NEXT.fetch_add(1, Ordering::Relaxed);
    streams().insert(addr, Stream::new(!nowrap));
    addr
}

/// `Inflater.inflateBytesBytes`: the result packs the bytes read (bits 0 to 30), the bytes
/// written (31 to 61), `finished` (bit 62) and `needDict` (bit 63), as Java unpacks it. The
/// output is the array from `out_off` for `out_len` bytes; the input is copied by the caller.
pub fn inflate_bytes(
    addr: i64,
    input: &[i8],
    output: &mut [i8],
    out_off: i32,
    out_len: i32,
) -> Result<i64, Failure> {
    let output = &mut output[out_off as usize..(out_off + out_len) as usize];
    run(addr, input, output).map(|(packed, _)| packed)
}

/// `Inflater.inflateBytesBuffer`: as `inflate_bytes`, with the input copied by the caller and the
/// output in memory outside the heap, at `output_addr`.
pub fn inflate_bytes_buffer(addr: i64, input: &[i8], output_addr: i64, out_len: i32) -> Result<i64, Failure> {
    let mut output = vec![0; out_len as usize];
    let (packed, written) = run(addr, input, &mut output)?;
    write_bytes(output_addr, &output[..written]);
    Ok(packed)
}

/// `Inflater.inflateBufferBytes`: as `inflate_bytes`, with the input in memory outside the heap,
/// at `input_addr`.
pub fn inflate_buffer_bytes(
    addr: i64,
    input_addr: i64,
    in_len: i32,
    output: &mut [i8],
    out_off: i32,
    out_len: i32,
) -> Result<i64, Failure> {
    let input = read_bytes(input_addr, in_len);
    inflate_bytes(addr, &input, output, out_off, out_len)
}

/// `Inflater.inflateBufferBuffer`: as `inflate_bytes`, with the input and the output both in
/// memory outside the heap.
pub fn inflate_buffer_buffer(
    addr: i64,
    input_addr: i64,
    in_len: i32,
    output_addr: i64,
    out_len: i32,
) -> Result<i64, Failure> {
    let input = read_bytes(input_addr, in_len);
    inflate_bytes_buffer(addr, &input, output_addr, out_len)
}

/// Inflates `input` into `output` with the stream at `addr`: the result packed as `inflate_bytes`
/// gives it, and the number of bytes written.
fn run(addr: i64, input: &[i8], output: &mut [i8]) -> Result<(i64, usize), Failure> {
    let mut table = streams();
    let stream = table.get_mut(&addr).expect("Inflater used after end");
    let progress = stream
        .inflate(input, output)
        .map_err(|message| Failure { class: DATA_FORMAT, message: message.to_string() })?;
    let packed = progress.read as i64
        | (progress.written as i64) << 31
        | i64::from(progress.finished) << 62
        | i64::from(progress.need_dict) << 63;
    Ok((packed, progress.written))
}

/// The `len` bytes at `address`, read only when there are any: an empty buffer has no memory
/// to read.
fn read_bytes(address: i64, len: i32) -> Vec<i8> {
    if len <= 0 {
        return Vec::new();
    }
    crate::read_native(address, len as usize).into_iter().map(|b| b as i8).collect()
}

/// Writes `bytes` at `address`, unless there are none.
fn write_bytes(address: i64, bytes: &[i8]) {
    if bytes.is_empty() {
        return;
    }
    let bytes: Vec<u8> = bytes.iter().map(|&b| b as u8).collect();
    crate::write_native(address, &bytes);
}

/// `Inflater.getAdler`.
pub fn inflate_adler(addr: i64) -> i32 {
    streams()[&addr].adler as i32
}

/// `Inflater.reset`: the stream starts again, keeping whether it has a zlib wrapper.
pub fn inflate_reset(addr: i64) {
    let mut table = streams();
    let stream = table.get_mut(&addr).expect("Inflater used after end");
    *stream = Stream::new(stream.wrap);
}

/// `Inflater.end`.
pub fn inflate_end(addr: i64) {
    streams().remove(&addr);
}

/// `Inflater.setDictionary`, which throws `IllegalArgumentException` (with no message, as zlib
/// gives none) when the dictionary is not the one the header asks for.
pub fn inflate_set_dictionary(addr: i64, dict: &[i8], off: i32, len: i32) -> Result<(), crate::Exception> {
    let dict = &dict[off as usize..(off + len) as usize];
    set_dictionary(addr, dict)
}

/// `Inflater.setDictionaryBuffer`: as `inflate_set_dictionary`, with the dictionary in memory
/// outside the heap, at `buffer_addr`.
pub fn inflate_set_dictionary_buffer(addr: i64, buffer_addr: i64, len: i32) -> Result<(), crate::Exception> {
    set_dictionary(addr, &read_bytes(buffer_addr, len))
}

fn set_dictionary(addr: i64, dict: &[i8]) -> Result<(), crate::Exception> {
    let mut table = streams();
    let stream = table.get_mut(&addr).expect("Inflater used after end");
    stream.set_dictionary(dict).map_err(|()| crate::Exception::bare("java.lang.IllegalArgumentException"))
}

/// Adler-32 of a whole byte run, as zlib's `adler32` gives it.
pub fn adler32(bytes: &[i8]) -> u32 {
    bytes.iter().fold(1, |adler, &b| adler32_step(adler, b as u8))
}
