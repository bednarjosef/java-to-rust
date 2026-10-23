//! Every stream made here is read back with the runtime's inflater (`inflate.rs`), which rejects
//! what zlib rejects; streams of every form were also checked with zlib (Python's `zlib`).

use super::*;
use crate::inflate::Stream as Inflater;

fn signed(bytes: &[u8]) -> Vec<i8> {
    bytes.iter().map(|&b| b as i8).collect()
}

/// Text with repeats near and far, and some noise.
fn sample(len: usize) -> Vec<u8> {
    let mut seed = 12345u32;
    let words = ["deflate ", "inflate ", "stream ", "window ", "the ", "a ", "block\n"];
    let mut out = Vec::new();
    while out.len() < len {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
        if seed >> 28 == 0 {
            out.push((seed >> 16) as u8);
        } else {
            out.extend_from_slice(words[(seed >> 16) as usize % words.len()].as_bytes());
        }
    }
    out.truncate(len);
    out
}

/// Compresses `data` in pieces of `chunk` bytes into outputs of `room` bytes, finishing at the end.
fn compress(stream: &mut Stream, data: &[u8], chunk: usize, room: usize) -> Vec<u8> {
    let data = signed(data);
    let mut out = Vec::new();
    let mut buf = vec![0i8; room];
    for piece in data.chunks(chunk.max(1)) {
        let mut at = 0;
        loop {
            let p = stream.deflate(&piece[at..], &mut buf, 0);
            at += p.read;
            out.extend(buf[..p.written].iter().map(|&b| b as u8));
            if at == piece.len() && p.written < room {
                break;
            }
        }
    }
    loop {
        let p = stream.deflate(&[], &mut buf, FINISH);
        out.extend(buf[..p.written].iter().map(|&b| b as u8));
        if p.finished {
            return out;
        }
    }
}

fn inflate(stream: &[u8], wrap: bool) -> Vec<u8> {
    let mut inflater = Inflater::new(wrap);
    let mut out = vec![0i8; 1 << 22];
    let p = inflater.inflate(&signed(stream), &mut out).expect("valid deflate data");
    assert!(p.finished, "the stream ends");
    assert_eq!(p.read, stream.len(), "nothing after the stream");
    out[..p.written].iter().map(|&b| b as u8).collect()
}

#[test]
fn round_trips_at_every_level() {
    let data = sample(200_000);
    for level in -1..=9 {
        for wrap in [true, false] {
            let mut s = Stream::new(level, 0, wrap).unwrap();
            let z = compress(&mut s, &data, 70_000, 1 << 20);
            assert_eq!(inflate(&z, wrap), data, "level {level}");
            if level != 0 {
                assert!(z.len() < data.len() / 2, "level {level} compresses: {}", z.len());
            }
        }
    }
}

#[test]
fn round_trips_in_small_pieces() {
    let data = sample(50_000);
    for (chunk, room) in [(1, 1), (7, 3), (1000, 16), (100_000, 5)] {
        let mut s = Stream::new(6, 0, true).unwrap();
        let z = compress(&mut s, &data, chunk, room);
        assert_eq!(inflate(&z, true), data);
    }
}

#[test]
fn strategies() {
    let data = sample(30_000);
    for strategy in [1, 2] {
        let mut s = Stream::new(6, strategy, true).unwrap();
        let z = compress(&mut s, &data, 30_000, 1 << 16);
        assert_eq!(inflate(&z, true), data);
    }
    assert!(Stream::new(10, 0, true).is_none());
    assert!(Stream::new(-2, 0, true).is_none());
}

#[test]
fn header_and_trailer_of_empty_input() {
    // zlib's own bytes for no input: the header of the default level, an empty fixed block, and
    // the Adler-32 of nothing.
    let mut s = Stream::new(-1, 0, true).unwrap();
    assert_eq!(compress(&mut s, &[], 1, 64), [0x78, 0x9c, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01]);
    let mut s = Stream::new(0, 0, true).unwrap();
    assert_eq!(
        compress(&mut s, &[], 1, 64),
        [0x78, 0x01, 0x01, 0x00, 0x00, 0xff, 0xff, 0x00, 0x00, 0x00, 0x01]
    );
    let mut s = Stream::new(9, 0, false).unwrap();
    assert_eq!(compress(&mut s, &[], 1, 64), [0x03, 0x00]);
    for (level, flags) in [(1, 0x01), (2, 0x5e), (5, 0x5e), (6, 0x9c), (7, 0xda), (9, 0xda)] {
        let mut s = Stream::new(level, 0, true).unwrap();
        assert_eq!(compress(&mut s, b"x", 1, 64)[..2], [0x78, flags]);
    }
}

#[test]
fn stored_level_zero() {
    let data = sample(150_000);
    let mut s = Stream::new(0, 0, true).unwrap();
    let z = compress(&mut s, &data, 150_000, 1 << 20);
    // Three stored blocks of a block of input each, with their headers.
    assert!(z.len() > data.len() && z.len() < data.len() + 100);
    assert_eq!(inflate(&z, true), data);
}

#[test]
fn flushes() {
    let data = sample(10_000);
    let input = signed(&data);
    for flush in [SYNC_FLUSH, FULL_FLUSH] {
        let mut s = Stream::new(6, 0, true).unwrap();
        let mut out = vec![0i8; 1 << 16];
        let p = s.deflate(&input[..5000], &mut out, flush);
        assert_eq!(p.read, 5000);
        let first: Vec<u8> = out[..p.written].iter().map(|&b| b as u8).collect();
        // The flush marker ends what has been written so far.
        assert_eq!(first[first.len() - 4..], [0, 0, 0xff, 0xff]);
        let mut partial = Inflater::new(true);
        let mut back = vec![0i8; 10_000];
        let q = partial.inflate(&signed(&first), &mut back).unwrap();
        assert_eq!(q.written, 5000);
        // A second flush with nothing new writes nothing.
        assert_eq!(s.deflate(&[], &mut out, flush), Progress { read: 0, written: 0, finished: false });
        let mut z = first;
        z.extend(compress(&mut s, &data[5000..], 5000, 1 << 16));
        assert_eq!(inflate(&z, true), data);
    }
}

#[test]
fn full_flush_forgets_the_window() {
    let data = b"abcdefghabcdefghabcdefgh".repeat(10);
    let mut s = Stream::new(6, 0, false).unwrap();
    let mut out = vec![0i8; 1 << 12];
    let p = s.deflate(&signed(&data), &mut out, FULL_FLUSH);
    let mut tail = compress(&mut s, &data, 1000, 1 << 12);
    // The rest reads back without what came before the flush.
    let mut z: Vec<u8> = out[..p.written].iter().map(|&b| b as u8).collect();
    let mut alone = Inflater::new(false);
    let mut back = vec![0i8; 1000];
    let q = alone.inflate(&signed(&tail), &mut back).unwrap();
    assert!(q.finished);
    assert_eq!(back[..q.written], signed(&data)[..]);
    z.append(&mut tail);
    assert_eq!(inflate(&z, false), [data.clone(), data].concat());
}

#[test]
fn dictionary() {
    let dict = b"a preset dictionary of words: deflate inflate stream window block";
    let data = sample(5000);
    let mut s = Stream::new(6, 0, true).unwrap();
    s.set_dictionary(&signed(dict)).unwrap();
    assert_eq!(s.adler(), adler32_update(1, dict));
    let z = compress(&mut s, &data, 5000, 1 << 16);
    assert_eq!(z[1] & 0x20, 0x20, "the header asks for the dictionary");
    let mut inflater = Inflater::new(true);
    let mut out = vec![0i8; 10_000];
    let p = inflater.inflate(&signed(&z), &mut out).unwrap();
    assert!(p.need_dict);
    inflater.set_dictionary(&signed(dict)).unwrap();
    let q = inflater.inflate(&signed(&z)[p.read..], &mut out).unwrap();
    assert!(q.finished);
    assert_eq!(out[..q.written], signed(&data)[..]);
    // Taken again once the stream has ended, as zlib takes it, but not while it is under way.
    assert!(s.set_dictionary(&signed(dict)).is_ok());
    let mut begun = Stream::new(6, 0, true).unwrap();
    begun.deflate(&signed(&data[..10]), &mut out, SYNC_FLUSH);
    assert!(begun.set_dictionary(&signed(dict)).is_err());
}

#[test]
fn params_between_calls() {
    let data = sample(40_000);
    let input = signed(&data);
    let mut s = Stream::new(1, 0, true).unwrap();
    let mut out = vec![0i8; 1 << 17];
    // Before anything is compressed, nothing is read.
    let p = s.params(9, 0, &input, &mut out).unwrap();
    assert_eq!(p, Progress { read: 0, written: 0, finished: false });
    let mut z = Vec::new();
    let p = s.deflate(&input[..20_000], &mut out, 0);
    assert_eq!(p.read, 20_000);
    z.extend(out[..p.written].iter().map(|&b| b as u8));
    // From a slow level to stored: the input given is compressed first.
    let p = s.params(0, 0, &input[20_000..30_000], &mut out).unwrap();
    assert_eq!(p.read, 10_000);
    z.extend(out[..p.written].iter().map(|&b| b as u8));
    z.extend(compress(&mut s, &data[30_000..], 10_000, 1 << 17));
    assert_eq!(inflate(&z, true), data);
    assert!(s.params(10, 0, &[], &mut out).is_err());
}

#[test]
fn adler_of_the_input() {
    let data = sample(3000);
    let mut s = Stream::new(6, 0, true).unwrap();
    let mut out = vec![0i8; 1 << 12];
    s.deflate(&signed(&data), &mut out, 0);
    assert_eq!(s.adler(), adler32_update(1, &data));
    assert_eq!(adler32_update(1, b"Wikipedia"), 0x11e6_0398);
    let mut raw = Stream::new(6, 0, false).unwrap();
    raw.deflate(&signed(&data), &mut out, 0);
    assert_eq!(raw.adler(), 1);
}

#[test]
fn highly_repetitive_input() {
    for data in [vec![0u8; 300_000], b"ab".repeat(100_000), (0..=255).cycle().take(100_000).collect()] {
        let mut s = Stream::new(9, 0, true).unwrap();
        let z = compress(&mut s, &data, 300_000, 1 << 20);
        assert_eq!(inflate(&z, true), data);
    }
}
