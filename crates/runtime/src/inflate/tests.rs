//! Streams made by zlib (its Python module, `zlib`), and hand-made ones for the errors. Each
//! expected text is the input of the zlib call that made the stream.

use super::*;
use crate::{allocate_native, read_native, write_native};

const STORED_RAW: &[u8] = &[
    0x01, 0x11, 0x00, 0xee, 0xff, 0x73, 0x74, 0x6f, 0x72, 0x65, 0x64, 0x20, 0x62, 0x6c, 0x6f, 0x63, 0x6b,
    0x20, 0x74, 0x65, 0x78, 0x74,
];
const STORED_ZLIB: &[u8] = &[
    0x78, 0x01, 0x01, 0x11, 0x00, 0xee, 0xff, 0x73, 0x74, 0x6f, 0x72, 0x65, 0x64, 0x20, 0x62, 0x6c, 0x6f,
    0x63, 0x6b, 0x20, 0x74, 0x65, 0x78, 0x74, 0x3c, 0x34, 0x06, 0xa2,
];
const FIXED_RAW: &[u8] = &[0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x57, 0xc8, 0x40, 0x27, 0x01];
const FIXED_ZLIB: &[u8] =
    &[0x78, 0x01, 0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x57, 0xc8, 0x40, 0x27, 0x01, 0x68, 0x03, 0x08, 0xb1];
const DYN_RAW: &[u8] = &[
    0x5d, 0x93, 0x4b, 0x4e, 0x04, 0x31, 0x0c, 0x44, 0xf7, 0x9c, 0x22, 0x07, 0x60, 0xd1, 0x2e, 0xe7, 0xcb,
    0x6d, 0x1a, 0x68, 0x60, 0xa4, 0x99, 0x59, 0xcd, 0x70, 0x7e, 0x84, 0x12, 0x57, 0xcb, 0xde, 0x56, 0x29,
    0x89, 0xde, 0xb3, 0x73, 0x79, 0x1c, 0xb7, 0xb4, 0xbd, 0xa5, 0xdf, 0xfd, 0xfa, 0x3c, 0xd2, 0xf6, 0x9a,
    0xee, 0xfb, 0xed, 0x48, 0xfb, 0xcb, 0xe5, 0x3f, 0x17, 0xcb, 0x65, 0xe5, 0xef, 0x33, 0x87, 0xe5, 0x79,
    0xe5, 0x1f, 0x33, 0x57, 0xcb, 0xc7, 0xca, 0x3f, 0x67, 0x9e, 0x79, 0x4f, 0x5d, 0xc5, 0x31, 0x8b, 0x62,
    0x05, 0xca, 0x2a, 0xbe, 0x66, 0x51, 0xad, 0x50, 0x3b, 0xf1, 0x3d, 0x8b, 0xc6, 0xa7, 0xed, 0x8d, 0x9f,
    0x59, 0x74, 0x2b, 0x6a, 0xf6, 0x10, 0xc3, 0x8a, 0x1e, 0x28, 0x84, 0xd8, 0xea, 0x31, 0x84, 0xdc, 0xc8,
    0x1e, 0x44, 0x4e, 0xf2, 0xe6, 0x49, 0x84, 0xec, 0x0d, 0x1e, 0x45, 0x48, 0x0f, 0x8f, 0x22, 0xa4, 0x57,
    0xf1, 0x2c, 0x42, 0xfc, 0x8a, 0x30, 0x11, 0xf2, 0x8f, 0x12, 0x68, 0x28, 0x40, 0x23, 0x0e, 0x0d, 0xb4,
    0xcd, 0xe3, 0x80, 0x06, 0x04, 0x1e, 0x07, 0x54, 0x50, 0xd4, 0xe3, 0x80, 0x0a, 0x46, 0x18, 0x0d, 0xa8,
    0x20, 0x67, 0xcf, 0x03, 0x2a, 0x18, 0xe2, 0x79, 0x40, 0x07, 0x59, 0xc3, 0x8e, 0xd1, 0xc1, 0x08, 0x5b,
    0x06, 0x3a, 0x28, 0x91, 0x87, 0x0e, 0x7a, 0xc0, 0xa1, 0x82, 0x1a, 0x16, 0x4d, 0xa9, 0x00, 0xcd, 0xe3,
    0x28, 0x15, 0xf4, 0xee, 0x71, 0x94, 0x0a, 0x4a, 0xd8, 0x35, 0xa5, 0x02, 0xc0, 0xe3, 0x28, 0x15, 0xf4,
    0x11, 0x3e, 0x0d, 0x15, 0x54, 0xf1, 0x38, 0x7a, 0xfe, 0x82, 0xe2, 0x79, 0x94, 0x0a, 0x44, 0x02, 0xcf,
    0xa9, 0x20, 0x8c, 0x47, 0x4f, 0x07, 0x95, 0x3c, 0x7f,
];
const DYN_ZLIB: &[u8] = &[
    0x78, 0xda, 0x5d, 0x93, 0x4b, 0x4e, 0x04, 0x31, 0x0c, 0x44, 0xf7, 0x9c, 0x22, 0x07, 0x60, 0xd1, 0x2e,
    0xe7, 0xcb, 0x6d, 0x1a, 0x68, 0x60, 0xa4, 0x99, 0x59, 0xcd, 0x70, 0x7e, 0x84, 0x12, 0x57, 0xcb, 0xde,
    0x56, 0x29, 0x89, 0xde, 0xb3, 0x73, 0x79, 0x1c, 0xb7, 0xb4, 0xbd, 0xa5, 0xdf, 0xfd, 0xfa, 0x3c, 0xd2,
    0xf6, 0x9a, 0xee, 0xfb, 0xed, 0x48, 0xfb, 0xcb, 0xe5, 0x3f, 0x17, 0xcb, 0x65, 0xe5, 0xef, 0x33, 0x87,
    0xe5, 0x79, 0xe5, 0x1f, 0x33, 0x57, 0xcb, 0xc7, 0xca, 0x3f, 0x67, 0x9e, 0x79, 0x4f, 0x5d, 0xc5, 0x31,
    0x8b, 0x62, 0x05, 0xca, 0x2a, 0xbe, 0x66, 0x51, 0xad, 0x50, 0x3b, 0xf1, 0x3d, 0x8b, 0xc6, 0xa7, 0xed,
    0x8d, 0x9f, 0x59, 0x74, 0x2b, 0x6a, 0xf6, 0x10, 0xc3, 0x8a, 0x1e, 0x28, 0x84, 0xd8, 0xea, 0x31, 0x84,
    0xdc, 0xc8, 0x1e, 0x44, 0x4e, 0xf2, 0xe6, 0x49, 0x84, 0xec, 0x0d, 0x1e, 0x45, 0x48, 0x0f, 0x8f, 0x22,
    0xa4, 0x57, 0xf1, 0x2c, 0x42, 0xfc, 0x8a, 0x30, 0x11, 0xf2, 0x8f, 0x12, 0x68, 0x28, 0x40, 0x23, 0x0e,
    0x0d, 0xb4, 0xcd, 0xe3, 0x80, 0x06, 0x04, 0x1e, 0x07, 0x54, 0x50, 0xd4, 0xe3, 0x80, 0x0a, 0x46, 0x18,
    0x0d, 0xa8, 0x20, 0x67, 0xcf, 0x03, 0x2a, 0x18, 0xe2, 0x79, 0x40, 0x07, 0x59, 0xc3, 0x8e, 0xd1, 0xc1,
    0x08, 0x5b, 0x06, 0x3a, 0x28, 0x91, 0x87, 0x0e, 0x7a, 0xc0, 0xa1, 0x82, 0x1a, 0x16, 0x4d, 0xa9, 0x00,
    0xcd, 0xe3, 0x28, 0x15, 0xf4, 0xee, 0x71, 0x94, 0x0a, 0x4a, 0xd8, 0x35, 0xa5, 0x02, 0xc0, 0xe3, 0x28,
    0x15, 0xf4, 0x11, 0x3e, 0x0d, 0x15, 0x54, 0xf1, 0x38, 0x7a, 0xfe, 0x82, 0xe2, 0x79, 0x94, 0x0a, 0x44,
    0x02, 0xcf, 0xa9, 0x20, 0x8c, 0x47, 0x4f, 0x07, 0x95, 0x3c, 0x7f, 0x54, 0xad, 0x30, 0x51,
];
const MULTI: &[u8] = &[
    0x78, 0xda, 0x4a, 0xcb, 0x2c, 0x2a, 0x2e, 0x51, 0x28, 0x48, 0x2c, 0x2a, 0x51, 0xc8, 0x4f, 0x53, 0x28,
    0xc9, 0x48, 0x55, 0x28, 0x49, 0xad, 0x28, 0xd1, 0x51, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0x2b, 0x4e,
    0x4d, 0xce, 0xcf, 0x4b, 0x81, 0x48, 0x14, 0xa5, 0x16, 0xa4, 0x26, 0x96, 0x14, 0x2b, 0xa4, 0x61, 0x55,
    0x0b, 0x00, 0x2c, 0x1f, 0x18, 0x48,
];
const REPEAT_ZLIB: &[u8] = &[
    0x78, 0xda, 0xed, 0xc1, 0x31, 0x01, 0x00, 0x00, 0x00, 0xc2, 0xa0, 0xac, 0xeb, 0x5f, 0xc2, 0x12, 0x9e,
    0x40, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x6f, 0x03, 0x47, 0x0a, 0xa1, 0x7a,
];
const DICT_ZLIB: &[u8] = &[
    0x78, 0xf9, 0xcc, 0xf1, 0x0c, 0x34, 0xcb, 0x40, 0x28, 0x50, 0x40, 0x62, 0x03, 0x00, 0x69, 0xe7, 0x08,
    0xd9,
];
const TOO_FAR: &[u8] = &[0x4b, 0x04, 0x42];
const DIST_CODE: &[u8] = &[0x4b, 0x04, 0x3e];
const LIT_CODE: &[u8] = &[0x1b, 0x03];
const TOO_MANY: &[u8] = &[0xf5, 0x00, 0x00];
const BAD_CHECK: &[u8] = &[0x78, 0x9c, 0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x07, 0x00, 0x06, 0x2c, 0x02, 0x14];
const BAD_HEADER: &[u8] = &[0x78, 0x00];
const METHOD: &[u8] = &[0x79, 0x18];
const WINDOW: &[u8] = &[0x88, 0x1c];
const STORED_BAD: &[u8] = &[0x01, 0x05, 0x00, 0x00, 0x00];
const BLOCK_TYPE: &[u8] = &[0xff, 0x00, 0x00, 0x00];

const TEXT: &[u8] = b"item 0: value 0, name a\nitem 1: value 1, name b\nitem 2: value 4, name c\nitem 3: value 9, name d\nitem 4: value 16, name e\nitem 5: value 25, name f\nitem 6: value 36, name g\nitem 7: value 49, name h\nitem 8: value 64, name a\nitem 9: value 81, name b\nitem 10: value 3, name c\nitem 11: value 24, name d\nitem 12: value 47, name e\nitem 13: value 72, name f\nitem 14: value 2, name g\nitem 15: value 31, name h\nitem 16: value 62, name a\nitem 17: value 95, name b\nitem 18: value 33, name c\nitem 19: value 70, name d\nitem 20: value 12, name e\nitem 21: value 53, name f\nitem 22: value 96, name g\nitem 23: value 44, name h\nitem 24: value 91, name a\nitem 25: value 43, name b\nitem 26: value 94, name c\nitem 27: value 50, name d\nitem 28: value 8, name e\nitem 29: value 65, name f\nitem 30: value 27, name g\nitem 31: value 88, name h\nitem 32: value 54, name a\nitem 33: value 22, name b\nitem 34: value 89, name c\nitem 35: value 61, name d\nitem 36: value 35, name e\nitem 37: value 11, name f\nitem 38: value 86, name g\nitem 39: value 66, name h\n";
const STORED_TEXT: &[u8] = b"stored block text";
const FIXED_TEXT: &[u8] = b"hello hello hello hello";
const DICT: &[u8] = b"hello world, this is a dictionary";
const DICT_TEXT: &[u8] = b"hello world hello world";
const MULTI_TEXT: &[u8] = b"first part of the text, second part repeats first part of the text";

const TEXT_ADLER: u32 = 1420636241;
const REPEAT_ADLER: u32 = 1191879034;
const DICT_ID: u32 = 3438349364;

fn signed(bytes: &[u8]) -> Vec<i8> {
    bytes.iter().map(|&b| b as i8).collect()
}

/// Gives `input` to the stream in pieces of `piece` bytes, with output buffers of `room` bytes,
/// as a Java caller does: each call takes the input that the last one left. Stops when the
/// stream is finished, asks for a dictionary, or makes no progress.
fn inflate_in(
    stream: &mut Stream,
    input: &[u8],
    piece: usize,
    room: usize,
) -> Result<(Vec<u8>, bool), &'static str> {
    let input = signed(input);
    let mut pos = 0;
    let mut out = Vec::new();
    loop {
        let end = (pos + piece).min(input.len());
        let mut buf = vec![0i8; room];
        let progress = stream.inflate(&input[pos..end], &mut buf)?;
        pos += progress.read;
        out.extend(buf[..progress.written].iter().map(|&b| b as u8));
        if progress.finished || progress.need_dict || (progress.read == 0 && progress.written == 0) {
            return Ok((out, progress.finished));
        }
    }
}

/// The whole input at once, into a buffer big enough for all of it.
fn whole(input: &[u8], wrap: bool) -> Result<(Vec<u8>, bool), &'static str> {
    inflate_in(&mut Stream::new(wrap), input, input.len(), 1 << 17)
}

fn error(input: &[u8], wrap: bool) -> &'static str {
    whole(input, wrap).expect_err("the stream is bad")
}

#[test]
fn adler32_of_text() {
    assert_eq!(adler32(&signed(b"")), 1);
    assert_eq!(adler32(&signed(TEXT)), TEXT_ADLER);
}

#[test]
fn stored_block() {
    assert_eq!(whole(STORED_RAW, false), Ok((STORED_TEXT.to_vec(), true)));
    assert_eq!(whole(STORED_ZLIB, true), Ok((STORED_TEXT.to_vec(), true)));
}

#[test]
fn fixed_block() {
    assert_eq!(whole(FIXED_RAW, false), Ok((FIXED_TEXT.to_vec(), true)));
    assert_eq!(whole(FIXED_ZLIB, true), Ok((FIXED_TEXT.to_vec(), true)));
}

#[test]
fn dynamic_block() {
    assert_eq!(whole(DYN_RAW, false), Ok((TEXT.to_vec(), true)));
    assert_eq!(whole(DYN_ZLIB, true), Ok((TEXT.to_vec(), true)));
}

#[test]
fn zlib_adler_is_the_output_adler() {
    let mut stream = Stream::new(true);
    inflate_in(&mut stream, DYN_ZLIB, DYN_ZLIB.len(), 1 << 17).unwrap();
    assert_eq!(stream.adler, TEXT_ADLER);
    let mut raw = Stream::new(false);
    inflate_in(&mut raw, DYN_RAW, DYN_RAW.len(), 1 << 17).unwrap();
    assert_eq!(raw.adler, 0);
}

#[test]
fn input_split_into_single_bytes() {
    assert_eq!(inflate_in(&mut Stream::new(false), STORED_RAW, 1, 1 << 17), Ok((STORED_TEXT.to_vec(), true)));
    assert_eq!(inflate_in(&mut Stream::new(false), FIXED_RAW, 1, 1 << 17), Ok((FIXED_TEXT.to_vec(), true)));
    assert_eq!(inflate_in(&mut Stream::new(true), DYN_ZLIB, 1, 1 << 17), Ok((TEXT.to_vec(), true)));
    assert_eq!(inflate_in(&mut Stream::new(false), DYN_RAW, 1, 1 << 17), Ok((TEXT.to_vec(), true)));
}

#[test]
fn output_split_into_small_buffers() {
    assert_eq!(inflate_in(&mut Stream::new(true), DYN_ZLIB, 1000, 7), Ok((TEXT.to_vec(), true)));
    assert_eq!(inflate_in(&mut Stream::new(false), STORED_RAW, 2, 1), Ok((STORED_TEXT.to_vec(), true)));
}

#[test]
fn several_blocks_copy_from_the_earlier_ones() {
    assert_eq!(whole(MULTI, true), Ok((MULTI_TEXT.to_vec(), true)));
    assert_eq!(inflate_in(&mut Stream::new(true), MULTI, 1, 3), Ok((MULTI_TEXT.to_vec(), true)));
}

#[test]
fn output_beyond_the_window() {
    let mut stream = Stream::new(true);
    let (out, finished) = inflate_in(&mut stream, REPEAT_ZLIB, 3, 100).unwrap();
    assert!(finished);
    assert_eq!(out, vec![b'a'; 70000]);
    assert_eq!(stream.adler, REPEAT_ADLER);
}

#[test]
fn stops_at_the_end_of_the_stream() {
    let mut input = FIXED_RAW.to_vec();
    input.extend([1, 2, 3]);
    let mut stream = Stream::new(false);
    let mut buf = [0i8; 64];
    let progress = stream.inflate(&signed(&input), &mut buf).unwrap();
    assert!(progress.finished);
    assert_eq!(progress.read, FIXED_RAW.len());
    assert_eq!(progress.written, FIXED_TEXT.len());
}

#[test]
fn preset_dictionary() {
    let mut stream = Stream::new(true);
    let mut buf = [0i8; 64];
    let progress = stream.inflate(&signed(DICT_ZLIB), &mut buf).unwrap();
    assert!(progress.need_dict);
    assert_eq!((progress.read, progress.written), (6, 0));
    assert_eq!(stream.adler, DICT_ID);
    assert_eq!(stream.set_dictionary(&signed(b"another dictionary")), Err(()));
    stream.set_dictionary(&signed(DICT)).unwrap();
    let rest = inflate_in(&mut stream, &DICT_ZLIB[progress.read..], 1, 1 << 17);
    assert_eq!(rest, Ok((DICT_TEXT.to_vec(), true)));
}

#[test]
fn dictionary_refused_without_a_request() {
    assert_eq!(Stream::new(true).set_dictionary(&signed(DICT)), Err(()));
    assert!(Stream::new(false).set_dictionary(&signed(DICT)).is_ok());
}

#[test]
fn zlib_header_messages() {
    assert_eq!(error(BAD_HEADER, true), "incorrect header check");
    assert_eq!(error(METHOD, true), "unknown compression method");
    assert_eq!(error(WINDOW, true), "invalid window size");
}

#[test]
fn block_messages() {
    assert_eq!(error(BLOCK_TYPE, false), "invalid block type");
    assert_eq!(error(STORED_BAD, false), "invalid stored block lengths");
    assert_eq!(error(TOO_MANY, false), "too many length or distance symbols");
}

#[test]
fn symbol_messages() {
    assert_eq!(error(TOO_FAR, false), "invalid distance too far back");
    assert_eq!(error(DIST_CODE, false), "invalid distance code");
    assert_eq!(error(LIT_CODE, false), "invalid literal/length code");
}

#[test]
fn trailer_message() {
    assert_eq!(error(BAD_CHECK, true), "incorrect data check");
}

#[test]
fn errors_are_the_same_in_pieces() {
    let mut stream = Stream::new(false);
    assert_eq!(inflate_in(&mut stream, TOO_FAR, 1, 8), Err("invalid distance too far back"));
    assert_eq!(inflate_in(&mut Stream::new(true), BAD_HEADER, 1, 8), Err("incorrect header check"));
}

/// The packed result of `inflate_bytes`: the bytes written, `finished` and `needDict`.
fn written_of(packed: i64) -> usize {
    ((packed >> 31) & 0x7fff_ffff) as usize
}

fn finished(packed: i64) -> bool {
    (packed >> 62) & 1 == 1
}

fn need_dict(packed: i64) -> bool {
    packed < 0
}

fn unsigned(bytes: &[i8]) -> Vec<u8> {
    bytes.iter().map(|&b| b as u8).collect()
}

/// `STORED_ZLIB` with its input and its output in memory outside the heap, as a direct buffer
/// gives them: the same text, and the same packed result as the array form.
#[test]
fn buffer_output_gets_the_text() {
    let input = signed(STORED_ZLIB);
    let mut reference = vec![0i8; 64];
    let expected = inflate_bytes(inflate_init(false), &input, &mut reference, 0, 64).unwrap();

    let output = allocate_native(64);
    let packed = inflate_bytes_buffer(inflate_init(false), &input, output, 64).unwrap();
    assert_eq!(packed, expected);
    assert_eq!(written_of(packed), STORED_TEXT.len());
    assert!(finished(packed));
    assert_eq!(read_native(output, STORED_TEXT.len()), STORED_TEXT);
}

#[test]
fn buffer_input_is_read_from_memory() {
    let input = allocate_native(FIXED_ZLIB.len() as i64);
    write_native(input, FIXED_ZLIB);
    let mut output = vec![0i8; 64];
    let packed = inflate_buffer_bytes(inflate_init(false), input, FIXED_ZLIB.len() as i32, &mut output, 4, 60)
        .unwrap();
    assert_eq!(written_of(packed), FIXED_TEXT.len());
    assert!(finished(packed));
    assert_eq!(unsigned(&output[4..4 + FIXED_TEXT.len()]), FIXED_TEXT);
}

#[test]
fn buffer_input_and_output_in_memory() {
    let input = allocate_native(FIXED_ZLIB.len() as i64);
    write_native(input, FIXED_ZLIB);
    let output = allocate_native(64);
    let packed = inflate_buffer_buffer(inflate_init(false), input, FIXED_ZLIB.len() as i32, output, 64).unwrap();
    assert_eq!(written_of(packed), FIXED_TEXT.len());
    assert!(finished(packed));
    assert_eq!(read_native(output, FIXED_TEXT.len()), FIXED_TEXT);
}

/// A stream that asks for a dictionary, given as memory outside the heap, and then the rest of
/// its input, also outside the heap.
#[test]
fn dictionary_in_memory_then_the_rest() {
    let addr = inflate_init(false);
    let input = allocate_native(DICT_ZLIB.len() as i64);
    write_native(input, DICT_ZLIB);
    let output = allocate_native(64);
    let packed = inflate_buffer_buffer(addr, input, DICT_ZLIB.len() as i32, output, 64).unwrap();
    assert!(need_dict(packed));
    assert_eq!(packed & 0x7fff_ffff, 6);
    assert_eq!(inflate_adler(addr) as u32, DICT_ID);

    let dict = allocate_native(DICT.len() as i64);
    write_native(dict, DICT);
    inflate_set_dictionary_buffer(addr, dict, DICT.len() as i32).unwrap();
    let rest = &DICT_ZLIB[6..];
    let rest_at = allocate_native(rest.len() as i64);
    write_native(rest_at, rest);
    let packed = inflate_buffer_buffer(addr, rest_at, rest.len() as i32, output, 64).unwrap();
    assert!(finished(packed));
    assert_eq!(written_of(packed), DICT_TEXT.len());
    assert_eq!(read_native(output, DICT_TEXT.len()), DICT_TEXT);
    inflate_end(addr);
}

#[test]
fn dictionary_in_memory_refused_when_wrong() {
    let addr = inflate_init(false);
    let input = allocate_native(DICT_ZLIB.len() as i64);
    write_native(input, DICT_ZLIB);
    let output = allocate_native(64);
    let packed = inflate_buffer_buffer(addr, input, DICT_ZLIB.len() as i32, output, 64).unwrap();
    assert!(need_dict(packed));
    let wrong = allocate_native(4);
    write_native(wrong, b"nope");
    assert!(inflate_set_dictionary_buffer(addr, wrong, 4).is_err());
    inflate_end(addr);
}

/// No bytes are read or written, so an address that is no block's is no error for them.
#[test]
fn empty_buffers_touch_no_memory() {
    let addr = inflate_init(false);
    let mut output = vec![0i8; 8];
    let packed = inflate_buffer_bytes(addr, 0, 0, &mut output, 0, 8).unwrap();
    assert_eq!((written_of(packed), finished(packed)), (0, false));
    let packed = inflate_bytes_buffer(addr, &[], 0, 0).unwrap();
    assert_eq!(written_of(packed), 0);
    inflate_end(addr);
}
