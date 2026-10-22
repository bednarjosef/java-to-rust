# Zip decompression: java.util.zip.Inflater's natives

Whole-program translations reach `java.util.zip.Inflater` (javac reads jar files and `ct.sym`
through `ZipFile` and `InflaterInputStream`). In the JDK, Inflater's work is done by native zlib
calls; a translated program has no zlib, so the runtime needs its own decompressor for the
DEFLATE format (RFC 1951) with the zlib wrapper (RFC 1950) when `nowrap` is false. The runtime
crate has no dependencies, and must keep none: write the decompressor in Rust in the runtime.

First read how an existing native goes from recognition to runtime function to test
(`lower/natives.rs`, `ast/natives.rs`, `codegen/natives.rs`, the runtime's natives; for example
`Native::Crc32Update`, which is CRC32's native), then read
`$JAVA_BASE_SOURCES/java/util/zip/Inflater.java` (path below), especially what
`inflateBytesBytes` returns (a long that packs the bytes read, the bytes written and flags) and
which fields it sets.

## Scope
- `Inflater.init(boolean nowrap)`, `inflateBytesBytes`, `getAdler`, `reset`, `end`,
  `initIDs` (does nothing), `setDictionary(long, byte[], int, int)`. The `long addr` is a handle:
  keep each stream's state in a runtime table keyed by it, as the runtime already does for other
  handles if it has such a table (look for one before writing your own).
- The decompressor: stored, fixed-Huffman and dynamic-Huffman blocks, a 32 KiB window,
  incremental input (Java calls `inflate` again with more input when it runs out) and output
  limited to the caller's buffer, Adler-32 for the zlib wrapper, and `DataFormatException` with
  zlib's messages for bad data ("invalid stored block lengths", "invalid block type",
  "incorrect header check", ... — match what Java prints for the cases you test).
- `inflateBytesBuffer`, `inflateBufferBytes`, `inflateBufferBuffer`, `setDictionaryBuffer` work on
  direct buffers: leave them rejected unless they turn out to be trivial.
- Leave `Deflater` (compression) rejected: its output must equal zlib's byte for byte, which is a
  different, larger job.

If `Inflater`'s Java code needs something else that is rejected on the way (for example its
`Cleaner`), say what in your report; fix it only if it is small and clearly correct.

## Deliverables
- Inflater natives on a runtime decompressor, with unit tests on hand-made streams of each block
  type, including input split across calls
- DataFormatException for bad data, with Java's messages for the cases tested
- One corpus program corpus/features/InflateBytes.java: decompresses byte arrays given as literals
  in the program (produce them once with Java's Deflater and paste them in as `byte[]`
  literals: the translation can't compress), in one call and in small pieces, raw (`nowrap`) and
  zlib-wrapped, and prints the text, `getAdler()`, `getBytesRead()`/`getBytesWritten()` and a caught
  DataFormatException's message
- If it gets that far: `InflaterInputStream` over a `ByteArrayInputStream` in the same program

## How to test
- Build and test through the build-slot limiter, which keeps the machine from running out of
  memory: prefix every cargo command with
  `<scratchpad>/build-slot.sh`,
  for example `.../build-slot.sh cargo test -p java-runtime`.
  `CORPUS_FILTER=inflate` runs only programs whose lowercase name contains `inflate`.
- Whole programs: `JAVA_BASE_SOURCES=<scratchpad>/jdk-src/java.base`
  with `cargo run -q -p java-to-rust -- --main Name --source DIR --library $JAVA_BASE_SOURCES --crate OUT`
  (DIR holds only your program), through the limiter.
- Test only what you touch while working; run `scripts/check.sh` once at the end, through the
  limiter.
- Do not touch Unsafe, VarHandle, Charset, file system, reflection or ClassLoader code: other
  agents are changing it now.
