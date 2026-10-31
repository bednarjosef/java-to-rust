# Performance

How fast translated programs run compared with Java (and, in "Translating whole programs" below,
how fast whole programs translate). The programs are in `benchmarks/`; the script that times
them is `scripts/benchmark.py`:

    python3 scripts/benchmark.py [--runs N] [Program ...]

With no program names it times every file in `benchmarks/`; `--runs` defaults to 5.

Each program is compiled with `javac`, translated with `java-to-rust --crate`, built with
`cargo build --release`, and run on both sides. The Rust run must print the same standard output
and exit with the same code as Java's, or the program is reported and not timed. Times are the
median of the runs, wall clock. Java runs with its default flags, so JVM start-up is included. A
program that warms up prints `kernel-ms N` on standard error; that is the time of the timed loop
measured inside the program, and it is reported in the two kernel columns. The last two columns
are each side's peak memory: the most resident memory the process held, its `ru_maxrss`, the
median over the runs. The table below is of the times; the memory of translated programs and the
benchmarks that measure it are in "Memory".

## Results

One run of `python3 scripts/benchmark.py` (five runs per side, medians, wall clock, in
milliseconds), at a load average of 1.6 at its start and about 4 during it, on 16 cores at full
clock (4.8 GHz under load); NoiseTree's and ObjectDispatch's rows are from later runs of each
alone (three runs a side), ReflectionLookups' from one of it alone (five runs a side).

- CPU: AMD Ryzen AI 7 350 w/ Radeon 860M (16 cores)
- JDK: OpenJDK 26.0.2.1 (`java -version`)
- Rust: rustc 1.99.0 (b940084d7 2026-09-28); the crates the translator writes build in release
  with fat LTO, for processors with AVX2 (`target-cpu=x86-64-v3`, in the crate's
  `.cargo/config.toml`)

| program               | Java ms | Rust ms | Rust/Java | Java kernel | Rust kernel | kernel ratio |
|-----------------------|--------:|--------:|----------:|------------:|------------:|-------------:|
| BaseFields            |     170 |      69 |      0.41 |         145 |          66 |         0.46 |
| BinaryTrees           |     686 |    1634 |      2.38 |           - |           - |            - |
| ClassCast             |      85 |      83 |      0.97 |          37 |          71 |         1.92 |
| Collatz               |    1155 |     581 |      0.50 |           - |           - |            - |
| Interpreter           |    1046 |    1177 |      1.12 |         341 |         393 |         1.15 |
| LargeArrays           |      34 |       7 |      0.21 |           - |           - |            - |
| Mandelbrot            |    3400 |    1143 |      0.34 |        1128 |        1141 |         1.01 |
| ManyThreadsAllocating |     136 |     108 |      0.79 |           - |           - |            - |
| MatrixMultiply        |     124 |      80 |      0.65 |          27 |          25 |         0.93 |
| MonitoredObjects      |      67 |      33 |      0.48 |           - |           - |            - |
| NBody                 |    1811 |    1884 |      1.04 |         601 |         598 |         1.00 |
| NodeList              |     175 |     110 |      0.63 |           - |           - |            - |
| NoiseTree             |     369 |     442 |      1.20 |         287 |         400 |         1.39 |
| ObjectDispatch        |     169 |     376 |      2.23 |         111 |         311 |         2.80 |
| ObjectHash            |     106 |      48 |      0.45 |          59 |          41 |         0.69 |
| ParticleArray         |    1426 |     892 |      0.63 |           - |           - |            - |
| ReflectionLookups     |     207 |     210 |      1.01 |         146 |         189 |         1.29 |
| ReflectionTableMemory |      25 |       2 |      0.09 |           - |           - |            - |
| Sieve                 |    1278 |    1006 |      0.79 |           - |           - |            - |
| SmallObjects          |     147 |     203 |      1.38 |           - |           - |            - |
| SortMillion           |    1731 |    1727 |      1.00 |         535 |         577 |         1.08 |
| Statics               |    1286 |    1266 |      0.98 |           - |           - |            - |
| StringBuild           |      28 |       1 |      0.04 |           - |           - |            - |
| TokenScanner          |     994 |     775 |      0.78 |           - |           - |            - |
| UnlockedObjects       |      69 |      34 |      0.49 |           - |           - |            - |

Differences of 20% or less between two runs are often noise or the placement of code and data
(see "Colors" in docs/collector.md); the large ratios are not.

## What the times come from

Read from the generated Rust under `target/benchmarks/<Name>/crate/src/main.rs`, from
profiles taken with `perf`, and from counts of the objects each `new` makes: a program
translated with `JAVA_TO_RUST_COUNT_ALLOCATIONS=1` set prints, when it exits, the 60 `new`
sites (class, the method the `new` is in, and "on the stack" for a `new` whose objects go in a
slot on the stack) that made the most objects, on standard error, with the total and the share
made on the stack (`java::count_allocation`).

- **Allocation (BinaryTrees 2.4x, SmallObjects 1.4x, NodeList 0.6x).** Objects live in the runtime's
  garbage collected heap (docs/collector.md): an allocation takes a slot from a per-thread list
  through the global allocator, and a handle is copied without a count. The slot is zeroed word by
  word rather than with a call of `memset`: BinaryTrees took 7.8 s against 9.4 s with the call
  (alternating runs, at about 620 MHz). Taking the slot inlined at every `new` took it to 5.8 s but
  slowed other loops, which the compiler no longer optimized as well (docs/collector.md,
  "Allocation"). BinaryTrees keeps a tree of about 32 MB alive while it builds and drops millions of
  short-lived ones; the collector marks that long-lived tree again at each of its 70 collections,
  which at full clock pause the program for about 0.2 s of its 1.6 s in all (on 16 threads; 0.9 s on
  one). The rest is the program itself: half of its own time is in the allocator, most of that in
  the first store to each new slot, a cache miss, which not zeroing the slot moves to the
  constructor's stores without saving time. `SmallObjects` likewise marks a linked list of 2 million
  nodes again at each of its collections, which no helper can share. Of the 60 million nodes its
  last loop makes, the half that each iteration's `Node n` holds are made on the stack, because the
  escape analysis tells variables apart by their declaration (docs/collector.md, "Objects on the
  stack"); taking this `n` for the one of an earlier `for (Node n = head; ..)`, declared outside the
  loop, it made them all on the heap: 1.31 s and 22 collections against 0.75 s and 12 (alternating
  runs at a load of 15). Idle markers sleep instead of spinning (docs/collector.md, "Marking and
  sweeping in parallel"): spinning, at a load average of about 6, BinaryTrees took 4.27 s against
  2.16 s (Java 0.91 s at that load) and `SmallObjects` 0.64 s against 0.36 s (Java 0.17 s). The
  young generation (`JAVA_GC_GENERATIONS=1`, docs/collector.md) collects the short-lived objects
  without marking the old ones: `SmallObjects` 0.17 s against 0.20 s, with 0.30 s against 0.60 s of
  processor time, and BinaryTrees 1.5 s against 1.65 s with 3.2 s against 5.0 s of processor time
  (alternating runs at full clock); it is opt-in because some peaks rise (`SmallObjects` 170 MB
  against 200 MB). NodeList walks and rebuilds a linked list of about 667,000 nodes; its nodes are
  taken from blocks in address order, so the walk follows memory.
- **Strings read from a field (TokenScanner 0.8x).** A parser that reads its input one character
  at a time with `input.substring(index, index + 1)` and checks `index < input.length()` runs in
  linear time:
  - `java::string_length` counts ASCII text by its byte length, and other text from its bytes
    without decoding it.
  - A string field of a plain object (an owned or immutable object, or `this` in its methods) is
    borrowed where it is when the statement neither assigns that field, nor passes the object on,
    nor calls a `&mut self` method of it (`codegen/strings.rs`, `Steady`).
  - The ASCII analysis follows `String` fields (instance and static, by name, only where every
    field of that name is a `String`), the overrides of a method, enum constants, and the
    exceptions the runtime raises. On a string proved ASCII, `length()` is the byte length and
    `substring` is `java::ascii_substring`, a byte slice with Java's bounds check. A call of a
    method that returns a type variable is not taken for ASCII.
  - `ascii_substring` is inlined. Called, it returned the new one-letter string written as 8-byte
    stores, which the caller copied into the field with a 16-byte load that the processor cannot
    forward from those stores: about one blocked forward per character, 1388 ms against 757 ms
    inlined.
  - `"0123456789".contains(current)` searches for one byte without checking first that the text
    is UTF-8 (`Wtf8::find_from`).
- **Elements of an object array read again (NBody 1.0x, ParticleArray 0.6x).** Consecutive
  statements that read the same element, with an array that is a local, a field of `this` or a
  static and an index that is a local or a constant, read it once into a local, when none of
  those statements assigns the array, the index, or an element of any object array, and none of
  them calls a method (`lower/element_reads.rs`). The local is declared without `mut` when it is
  assigned once.
- **Plain arrays (Sieve 0.8x, SortMillion 1.0x, Interpreter 1.1x).** An array that is not
  shared is a `Vec`, borrowed as a slice by the methods it is passed to. Sieve's indexes are
  proven in range, so every access is `composite[i as usize]` with no check: `i * i <= limit`
  with `i >= 2` gives `i <= limit`, and the size `limit + 1` is longer than any of them.
  SortMillion's `quickSort` borrows its `Vec` as `&mut [i32]`; its indexes cannot be proven in
  range, so they keep Java's check, an inlined compare and a branch to a `#[cold]`,
  `#[inline(never)]` throwing function (runtime `array.rs`). Interpreter's stack and locals are
  plain `Vec<i32>` with the same checks, since its indexes come from the program it interprets.
  Its kernel depends on where the linker puts `run`: two builds with the same instructions in
  `run`, one starting 32 bytes past a 64-byte line and one at a line, ran the kernel in 390 ms
  and 260 ms (the same instructions and branch misses executed, 4.9 against 7.3 instructions a
  cycle). In the slow one, the block every opcode jumps back to, which reads the next opcode and
  jumps through the table, spans two 64-byte lines.
  `library` in SortMillion is given to `Arrays.sort`, so it stays a shared `java::Array`.
- **MatrixMultiply (0.65x, kernel 0.9x).** The matrices are plain
  `Vec<Vec<i32>>`: `create` returns its `Vec`, `multiply` borrows `a` and `b` as `&[Vec<i32>]`
  and returns `c`, and `a = b; b = c;` moves the `Vec`s, since neither is read again before it
  is assigned. `create`'s store `m[i][j] = (x >>> 16) % 100` stays a plain store: Java computes a
  stored value before it throws for the array, so a value that may throw is computed first, with
  the row held in a local, which gives the row a second name and makes the whole matrix shared
  (`lower/argument_order.rs`); a remainder by a nonzero constant cannot throw. When `k` is a row
  of `b` and `n` is at most the lengths of `b[k]` and `c[i]`, the inner loop runs over the slices
  `&b[k][..n]` and `&mut c[i][..n]` without a check, and vectorizes (`analysis/arrays/slices.rs`);
  otherwise it runs with every check. Built for x86-64 without AVX2, the vectorized loop has no
  4-lane 32-bit multiply (SSE2 has none) and the kernel took twice Java's time; for
  `x86-64-v3` it multiplies eight lanes at once (`vpmulld`), as the JIT does on this processor.
- **Statics (1.0x).** In a program that starts no thread, a primitive static is a
  `java::Global<Cell<_>>`, a plain `static`, read with `get` and written with `set`; the
  histogram, an `int[]`, is a `java::LazyGlobal<RefCell<_>>`. The compiler keeps the
  xorshift state in a register through the loop, which it could not do for a `thread_local!`,
  whose address it does not know: each iteration stored the state and the next read it back
  (2.6 s). Reading or writing one element of a `java::Array` checks no borrow flag
  (`array.rs`, `ArrayData::raw`), whose store and reload also chained the iterations.
- **Objects of many classes behind one type (ObjectDispatch 2.2x, kernel 2.8x).** 48 enums held as
  `Comparable` and `Object`: a palette that also holds strings searched by `==`
  (`java::same_object`), casts back to an enum (`as_<type>()`) and `ordinal()` through `Enum`, the
  operations at the top of the Minecraft server's profile. Each tests the value's class number: a
  cast tests the one number it passes, and an ordinal is the constant's bits
  (`java::Thin::ordinal`). Through `view()` and a `match` over every class, the kernel took 1132 ms
  in release (fat LTO, `opt-level = 3`) and 14751 ms at `opt-level = 1`, against 781 ms and 4183 ms
  with the class numbers tested (medians of five, at a load of 6; Java's kernel is about 120 ms).
  `same_object` compares the two values' bits first, which hold the class number: equal bits are the
  same object and different bits different objects, but where both are strings or boxes. In the
  palette's search the compiler then tests the key's class once, before the loop, and each element
  is one compare, as in Java; comparing the class numbers first, the kernel took 1.8 times as long.
  The search reads the palette, a static, at each iteration; `java::Global::cloned` reads it without
  writing its borrow flag, whose increment and decrement were stores the compiler could not tell
  apart from the array, so the loop reloaded the array's element pointer at each element.
  `java::len` stops the program for an array longer than `i32::MAX` elements, which Java's arrays
  never are, and so tells the compiler that `i < palette.length` keeps `i` in bounds: the loop has
  no bounds check left. What remains is the loop itself, about 1.3 cycles an element against Java's
  0.5: the JIT unrolls it (its kernel takes 362 ms with `-XX:LoopUnrollLimit=1`, against 112 ms),
  and LLVM does not unroll a loop with a second exit, the element found. With `-C
  llvm-args=-unroll-runtime-multi-exit` it does, and the kernel takes 104 ms against 308 ms, but the
  same flag slows SortMillion's kernel from 563 to 634 ms (its partition scans `while (a[i] < pivot)
  i++` end after a few elements, and the unrolled loop runs more instructions and mispredicts more)
  and ObjectHash's from 41 to 47 ms (its probes end after one or two slots), so the translation does
  not set it.
- **Classes known only at run time (ClassCast 1.0x, kernel 1.9x).** `Class.cast` and
  `Class.isInstance` of values of 258 classes behind `Comparable` with their own class and another,
  as the Minecraft server's `StateHolder.getValue` casts each property's value. Each asks the value
  for the names above its class, which is one read of a table by class number
  (`TYPE_NAMES_BY_CLASS_NUMBER`), and compares each name by its length and last eight bytes before
  it compares it whole. Through a `match` over the 258 classes and a `memcmp` of each name, the
  kernel took 184 ms in release, 446 ms at `opt-level = 1` and 1450 ms at `opt-level = 0` (the
  runtime at 2), against 89, 242 and 1030 ms now (medians of five, three at 0, at a load of 7;
  Java's kernel is about 48 ms).
- **Objects of many classes hashed as `Object`s (ObjectHash 0.45x, kernel 0.7x).** An
  open-addressing set of 300 objects probed for each, as `ImmutableCollections.SetN.probe` is in the
  Minecraft server: two classes in three hash by their address and the others override `hashCode`,
  numbered in turn, so the classes that hash by address are 100 runs of class numbers. With them as
  the last arm of `Object::hash_code`'s `match` (`_`) rather than 100 range tests before it, the
  kernel takes 70 ms in release against 192 ms, and 870 ms at `opt-level = 0` (the runtime at 2)
  against 1800 ms; at `opt-level = 1` both take about 140 ms (medians of five to nine, at a load of
  7 to 9; Java's kernel is about 94 ms).
- **Fields read through a type of several classes, split into crates (BaseFields 0.4x).** Two final
  fields of an abstract class read from an array of four subclasses, in a translation split into
  three body crates (`--crates 3`), as the Minecraft server's noise chunk reads its density
  buffers. `base()` is one read, and its layout checks are in a constant of their own, so it is
  small enough to stay in the header, inlined where it is called (`docs/several-crates.md`).
  When every function moved to a body crate behind a header wrapper, the kernel took about
  730 ms at `opt-level = 1` (with the release profile's fat LTO), against 87 ms now; release
  takes about 70 ms either way. Without LTO, at `opt-level = 3`, it took 93 ms while `base()`,
  its checks inside it, was too long to stay, against 70 ms (medians of five, at a load of 7;
  Java's kernel is about 160 to 220 ms there).
- **Static finals and arrays that threads share (NoiseTree 1.2x, kernel 1.4x).** A tree of density
  functions computed on a worker thread, with Perlin noise at the leaves: each noise call reads a
  byte permutation table 14 times and a `static final` array of gradient objects 8 times. The thread
  makes every array a `java::SharedArray`, so each read loads the handle, then the elements' address
  and length from the array's header, then the element, an atomic load. Java reads the element after
  the array's header in one load, and takes a `static final` array for a constant whose length it
  knows, so `GRADIENT[hash & 15]` has no check. The `static final` is a `java::Final`, which a read
  reaches with one load of a pointer and a test (docs/multi-threading.md, 2.3); inside a `LazyLock`,
  as it was, a read tested the `LazyLock` and then the `OnceLock` inside it (kernel 2502 ms against
  2480 ms with the `java::Final`, alternating runs at about 850 MHz; Java's kernel is about 1610 ms
  there). With the permutation table a plain `[u8; 256]` field and the gradients a constant table of
  numbers, edited by hand into the generated code, the kernel took 1966 ms; the table alone, 2321
  ms. `Mth.floor`'s `(int) v` is `java::double_to_int(v)`: on x86-64, one conversion instruction and
  a compare of its result with `i32::MIN`, which only NaN, values out of range and those that
  truncate to `i32::MIN` give, and which sends them to Rust's `as`. Rust's `as` alone clamps the
  value with a `max` and a `min` before it converts and tests for NaN after, a longer chain: the
  kernel took 433 ms against 400 ms (alternating runs at full clock). LLVM does not vectorize Rust's
  `as` from `f64` to `i32` for `x86-64-v3` either, so a loop of such conversions loses nothing: 4096
  of them took 1.7 µs with `as` and 1.2 µs with the call.
- **Arithmetic loops (Mandelbrot 0.3x, Collatz 0.5x, StringBuild 0.04x).** These need nothing
  from the object model. StringBuild's `charAt` and `length()` on a string made only of `(char)`
  casts proven in `0..=0x7f` (`analysis/ranges.rs`) are a byte read and the byte length.

## Memory

`scripts/benchmark.py` reports each side's peak resident memory beside its time. Six of the
programs measure memory rather than time: `ReflectionTableMemory` (a reflection table of 14,400
methods, of which three classes' are read), `LargeArrays` (arrays of 64, 128 and 256 KB alive
together), `MonitoredObjects` (3 million small objects in a program that waits and notifies, of a
class it locks, so every object keeps its monitor), `UnlockedObjects` (the same, of a class whose
objects become `Object`s but no monitor site can lock), `SmallObjects` (3 million objects of 24
and 40 bytes alive together, then 30 million that die young) and `ManyThreadsAllocating` (32
threads that allocate arrays of 1 to 32 KB and stay alive while the main thread allocates). Peak
memory in MB, before and after the change that each measures (the median of three runs; of eight
for `ManyThreadsAllocating`, 0.101 s before and 0.105 s after; of ten for `SmallObjects`, whose
time is under "Allocation" below):

| program               | before | after | Java |
|-----------------------|-------:|------:|-----:|
| ReflectionTableMemory |   20.2 |  14.9 | 42.0 |
| LargeArrays           |  134.4 |  88.4 |  143 |
| MonitoredObjects      |  124.5 |  76.6 |  148 |
| UnlockedObjects       |  124.5 |  76.5 |  166 |
| SmallObjects          |  153.2 | 145.6 |  734 |
| ManyThreadsAllocating |  235.5 | 197.7 |  417 |

### Where the Minecraft server's memory went

The release trial of the server peaked at 1,498 MB on the bench (`minecraft-server.md`, step
`bench`), Java at 1,092 MB. Measured on that binary after the tick sprint, from its block table
(the collector's live bytes per slot size), its memory map and the contents of sampled slots:

- **The binary's own pages: 485 MB**, against Java's 26 MB of mapped files. The code the server
  ran: 292 MB of its 591 MB of `.text` were in memory, 278 MB with the kernel's mapping of the
  neighbours of each faulted page turned off. The 105 MB of the loader's fix-ups of a
  position-independent binary (`.rela.dyn`, 4.6 million of them: the place of every panic
  message, every table of functions), all read at start, and the 61 MB of data that fixing them
  up writes (`.data.rel.ro`, `.got`). The translator now builds binaries at a fixed address
  (`split::CARGO_CONFIG`), which have neither: the pointers are in the file, read where used.
  The saving grows with the binary: `ReflectionTableMemory`'s 6 MB binary loses 5,357 fix-ups
  (126 KB) and 89 KB of `.data.rel.ro`, too little to move its peak (15.0 MB either way), and
  runs the same.
- **The heap: about 1,000 MB of blocks for 530 MB alive**, against Java's 186 MB alive (a full
  collection's `jmap -histo:live`) in a heap of 442 MB. Both collect when the program has taken
  about as much as is alive, so the heaps are about twice what is alive; the difference is
  what is alive. Of the 530 MB:
  - **the reflection table's objects, about 120 MB**: all 160,000 methods and 80,000 fields made
    at the first lookup, each with three classes of 88 bytes held in it, its name, and lists of
    parameter classes in which each class is a boxed copy. Java makes a class's member objects
    when it is reflected on. The translation now makes each member's object the first time a
    lookup returns it (docs/reflection-and-plugins.md).
  - **monitors**: the server waits and notifies, which kept every object's monitor, and each
    `java::Monitored` object held its lock, 16 bytes. It now holds a pointer to a lock made the
    first time a thread takes it (`LazyMonitor`), 8 bytes, which puts about half of the objects
    in a smaller slot (`MonitoredObjects`), and only the objects that a monitor site may lock
    have one (docs/multi-threading.md 3.1, `UnlockedObjects`).
  - **large allocations of whole blocks**: 361 arrays of exactly 128 KB (`float[32768]`, the
    density buffers of world generation) took three blocks of 64 KB each, the third only for
    the cache-line color of its start, 23 MB in all. A large allocation's color now stays
    within the blocks its size needs (docs/collector.md).
- **The heap by class.** A dump of the heap and its block table right after a collection, with
  the server stopped (424 MB in taken slots, against Java's 186 MB alive), each object named by
  the class number in the handles that point at it (`java::Thin`), or else by the chain of
  objects that hold it up to one that has a number:
  - the reflection table, 110 MB in this dump (above);
  - **thread spans, 28 MB**: 641 words of the blocks' bitmaps whose taken slots nothing points
    at, 1 to 2 MB in each slot size from 896 bytes to 32 KB, one 64 KB block for about each of
    the 41 threads and the sizes it allocates. A thread took every untaken slot of a bitmap
    word at once, which from slots of 1 KB is a whole block, and the collector kept all of the
    word's taken slots. A span now takes at most 8 KB of slots, and the collector keeps only
    its own (docs/collector.md, `ManyThreadsAllocating` above);
  - **the same objects, larger**: `StructureTemplate$StructureBlockInfo` (308,000 of them) is
    48 bytes against Java's 24, `BlockPos` (313,000) 32 against 24, `BlockState` 224 against
    104, DataFixerUpper's `ProductType`, `NamedType`, `SumType` and `TagType` 64 against 32. A
    handle is 8 bytes where Java's compressed references are 4, each object held a 16-byte lock
    (now 8), and slots grew by 16 bytes where Java's objects grow by 8. From the furthest
    nonzero word of each class's objects (slots that may hold pointers are zeroed when handed
    out), the 214 MB of objects reached through numbered handles take 200 MB with the 8-byte
    lock, 190 MB with no lock word in the classes no `synchronized`, `wait` or `notify`
    reaches, and 185 MB with slot sizes in steps of 8 bytes to 128 as well. The translation now
    does both (docs/multi-threading.md 3.1, docs/collector.md; `UnlockedObjects` and
    `SmallObjects` above); how much of it the server gains is not yet measured;
  - world generation's density buffers (`float[32768]` and `float[98304]`) come and go: up to
    120 MB of them in one dump, a few MB in another. Their pools drop a buffer unused for 20
    ticks, so they are not kept; Java's histogram was taken between them.
- What remains larger than Java's: the handles above, a `java::Class` is an
  88-byte value wherever it is held (a field, a boxed `Class` as an `Object`), a string field is
  a 24-byte `Vec` with its own copy of the text, and the heap keeps free blocks until a whole
  huge page of them is free (167 MB of free blocks in the sample).

## Translating whole programs

How fast the translator itself runs in whole-program mode (`--main`, `--library` with the JDK's
`java.base` sources), which `targets/run.sh` and `tests/whole_program.rs` use. `--timings` (or
the environment variable `JAVA_TO_RUST_TIMINGS`) prints the time of each phase to standard error:
javac with the number of units it attributed, reachability with its rounds, lowering (each
attempt of the Rust generics, and why the next one is needed: the covariant arrays it noted, the
generic classes and methods that failed, or an error that erases those its code uses, or all of
them outside a census and a whole program), the analyses and the
generator. A census run with `JAVA_TO_RUST_STOP_AFTER_LOWER=1` set exits once lowering is done,
which times lowering alone on a program whose census takes long.

**Cause of the slow translations.** javac, given the JDK's sources as its source path, attributes
every unit it loads, and loads every class that the code of an attributed unit names, so a
program that touches `HashMap`, `ArrayList` or only `System.out` had some 2500 of the JDK's units
attributed and exported: a 356 MB JSON tree, 3.6 GB in the java process and 3.4 GB in the
translator, which held the tree whole as `serde_json` values. Reachability then used a few dozen
of them (36 for `IntegerKeyedGroups`). Two such translations at once, with other builds, were
enough to make the machine thrash; a translation that ran then took ten minutes or more, which is
how `IntegerKeyedGroups`, later `InventoryManagement`, `ListOperations`, `CheckedExceptions` and
`EnumAbstractMethods` hit `run.sh`'s 600 s limit in turn. Nothing in those programs was slow.

**Fix.** `JavaTree --serve` (`javac::Session`) attributes the program's own files and then only
the units asked for, with javac's `JavacTaskImpl.analyze` of chosen classes (hence
`--add-exports jdk.compiler/com.sun.tools.javac.api=ALL-UNNAMED`). `program::load` runs
reachability over the units so far and asks for the classes it named that no unit declares
(`Reach::missing`), until javac gives no more; reachability only grows with the units, so the
last round reaches what it reached over all 2500. The library's units come in the order they
were asked for, so a first unsupported construct may be reported in another class than before
(`EnumAbstractMethods` now stops at `EnumMap` instead of `EnumSet`, `EventListeners` at
`ArrayDeque` instead of `ArrayList`, `ListOperations` at `ArrayList` instead of `List`), and the
three programs of `corpus/whole-program` name some overloads differently; with the units in the
same order, the output of all 28 targets and the corpus programs is byte for byte that of a
javac run over every unit. `--survey` still has javac attribute everything.

Measured on a quiet build slot, release translator, wall time of the translation and peak RSS
of the translator and of its java process:

| program | before | after |
|---|---|---|
| IntegerKeyedGroups | 20.6 s, 3.43 GB + 3.64 GB | 3.2 s, 0.09 GB + 0.98 GB |
| WordFrequency | 26.9 s, 3.43 GB + 3.65 GB | 3.2 s, 0.10 GB + 1.10 GB |
| ListOperations | 21.2 s, 3.43 GB + 3.29 GB | 3.7 s, 0.19 GB + 1.16 GB |
| CheckedExceptions | 21.7 s, 3.43 GB + 3.71 GB | 2.6 s, 3 MB + 0.89 GB |

All 28 targets and the 3 whole-program corpus programs took 20 to 37 s each before and 2.1 to
3.1 s after, except the two that read with `Scanner` and format with `MessageFormat`
(`ScannerGrades`, `TextAdventure`: 630 units, 14 s, 1.2 GB + 2.3 GB) and `StreamPipeline` (161
units, 6.6 s, 0.35 GB + 1.9 GB). Each reachability round completes the records of copies of
only the units that this changes (those with a record or without a package), which halved the
translator's peak for those three. With the debug translator, which `run.sh` uses, the first
three of the table took 36, 41 and 40 s before and 4.8, 4.2 and 5.3 s after. Most of what
remains for a small program is the JVM and javac starting and attributing the program's own
file (2.4 s and 0.9 GB for `CheckedExceptions`, which uses no library unit); for the large ones,
javac's attribution and the reachability rounds.
`-Xmx512m` halves the java process's peak (to about 0.5 GB) at no visible cost for the small
programs, and 256 MB makes it three times slower; it is not set, as the large ones need more.

**Reachability rounds that walk only what is new.** Each round used to walk every unit again
from `main`: declare them all, survey them all for the records' members, walk, and index the
reached code for the check that streams stay sequential. A round now does that for its new units
only (`reach::Rounds`): an item that needed a class no unit declared waits for that class, and
a new unit's overriders of a method whose overriders may run are reached as they would have
been. A method of a class that no unit declares waits for javac's answer: it is taken for an
external one (whose overriders may run) only once javac was asked for the class and gave no unit
that declares it (`Walk::asked`), which can take a round more. So do the other decisions that
depend on whether a class is declared: whether an `instanceof` of a class without objects
guards a branch (`objectless`, which looks at the class's supertypes), whether a class is an
exception's (`recreatable`), and which superinterfaces with code a class's initialization
initializes (`default_superinterfaces`). Nothing is then decided on a class that may still come,
and each round reaches what a walk from scratch over its units, with the same classes asked for,
reaches; `JAVA_TO_RUST_REACH_CHECK=1` makes `load` check that after every round. On javac's
census it holds in every round but for one name noted missing, which javac has no class of: the
string literal `prob.found.req`, which javac also registers as a service's class name
(`services.rs`); the rounds meet the literal before the registration and note it, a walk from
scratch meets the registration first. It held in every round of 18 whole programs of the corpus
when it was written. Before, such a method was
taken for an external one at once, and when a later unit declared its class and it turned out
dormant, its overriders had been reached for nothing (`Walk::stale`) and the walk started again
from `main`: four times in javac's 10 rounds, two or three times in those of `ScannerGrades` and
`TextAdventure`; and an `instanceof` or exception decided without a supertype stayed decided, so
the result depended on the order the units came in (javac's translation reached
`UnixDomainSocketAddress.getPath`, which a walk over all its units does not). A stale walk can
still happen when a unit declares a class that javac had none of when asked, such as a second
top-level class of a file that a later unit came from, which none of these programs does; the
walk then starts again from `main`. The members written into a record walked before
(`javac::Records`) can grow with the new units: an accessor, `hashCode` or `equals` that a new
unit calls for, or an accessor that overrides an interface method of a new unit. The round then
declares the record's unit again with them (`Rounds::replace`, `Walk::redeclare_units`): a new
method's overrides are found as a new unit's are, a method of the record that the walk took for
an external one as no unit declared it (`Object`'s `hashCode` on a live record) is visited
again, and reflection's lookups in the record and the reflection metadata that names it are
made again. A record is final, so nothing was reached through that external method that a walk
from scratch would not reach. What decides the members only grows with the units, so members are
only ever added (`javac::Completion::only_adds_to`); were one to go, `load` would walk every unit
again. The `timing: reach` line counts the walks from the start, for records and stale, and the
rounds that added members to records walked before. With the wait, classes
are asked for in other rounds, so the units come in another order, which can change the names
of overloads, the tags of the classes, and which unsupported construct is reported first.
Reachability, release translator, on a machine with other builds running, before (a method of
a missing class taken for an external one at once) and after; walks from the start in brackets:

| program | before | after |
|---|---:|---:|
| javac (`--census`, 10 rounds) | 5.9 s (5) | 3.5 s (1) |
| ExtendedAttributes | 1.70 s (5) | 1.23 s (1) |
| SymbolicLinkCopies | 1.79 s (6) | 1.41 s (2) |
| PlatformLoggerValues | 1.58 s (2) | 1.00 s (1) |
| JarManifests | 1.20 s (3) | 1.04 s (1) |
| the small programs (0.1 to 0.6 s) | 0 to 3 | 0 or 1 |

On the small programs the time is about the same: their walks from the start were short.

Before records took their new members in place, each round that added members to a record walked
before walked every unit again from `main`, as long as all the walks of the rounds before it:
once in javac's census (round 6, 2.1 s of a reachability of 6.7 to 6.9 s), and 9 times in the
Minecraft server's 18 rounds, 304 s of reachability in all. In place, javac's round 6 takes 0.3
s and its reachability 5.0 to 5.4 s, reaching the same classes and methods; javac's census output
is the same byte for byte.

Release translator on a loaded machine (load 16 to 23), the before and after runs alternated,
twice each:

| program | reachability before | after | translation before | after |
|---|---|---|---|---|
| ScannerGrades | 5.7, 6.4 s | 3.1, 2.2 s | 18.0, 20.9 s | 22.3, 15.4 s |
| TextAdventure | 6.1, 7.5 s | 2.2, 1.9 s | 18.0, 23.3 s | 14.9, 14.3 s |

For the small programs reachability was 0.1 to 0.3 s and is about two thirds of that; the
translation's time is javac's. Peak memory is unchanged (1.1 GB + 2.3 GB for these two).

**Cheaper walks.** On a large program most of reachability is walks from `main` again, so the
cost of a node is most of its time. A node of the walk has its kind read once, and the tests
of its shape run only for the kinds they can hold for, in the same order as before; owners,
names and call keys are copied only when an item is made of them; a field of a tree node is
found by its first byte before its whole name (`value::Map::search`). A stale walk stops as
soon as it goes stale, as all it reached is thrown away. Reflection's lookups use an index of
the declared methods and fields that the walk keeps as it declares them, and the check that
streams stay sequential indexes calls by the names in javac's symbols without copying them, and
copies the key of a reached method, field or class only when it was not indexed yet.
Whether the records' members changed with a round's units is told by comparing what completing
them would write (which accessors and what they override, whether `hashCode` and `equals` are
written, which components are plain values; `javac::Completion`), decided by the functions
that `complete_record` uses, instead of completing copies of the records and comparing them.
The rounds, the classes asked for and `Reach` are the same, and the translations byte for byte
(18 whole programs of the corpus, `-o`, `--crate` and `--census`); release translator, two or
three runs each on a machine with other builds running:

| program | before | after |
|---|---:|---:|
| javac (`--census`, 10 rounds, 5 walks from the start) | 12.4 to 12.8 s | 5.2 to 5.4 s |
| ExtendedAttributes | 4.2 to 4.5 s | 1.7 to 1.8 s |
| SymbolicLinkCopies | 3.8 to 4.5 s | 1.8 to 1.9 s |
| JarManifests | 2.4 to 2.6 s | 1.2 s |
| SystemLoggerValues | 2.6 to 3.4 s | 1.1 to 1.2 s |

**A walk of the methods that can run that skips repeats.** The walk of `analysis/live_methods.rs`
visits a method once for each class of object it runs on, and again whenever what it relied on
changes. A call on a receiver that may be any class runs thousands of methods, and each visit
used to enter all of them again (pass the arguments, mark each pair of method and class reached
or waiting), which was most of the Minecraft server's walk (about 1 ms a visit). A call now
remembers what it passed when it last entered its methods, and on a later visit only works out
what is known of its arguments: when that is the same, entering them again would change nothing,
so it enters none (`Walk::call`). The values and dead code of a method walked on several objects
are remembered until something they relied on changes, a type's initialization runs its loop over
the types above once, and handing the runtime objects it already holds is skipped.
`JAVA_TO_RUST_LIVE_CHECK=1` also runs the walk without these shortcuts and panics unless both
find the same methods, classes, receivers and unresolved calls in the same order; that holds for
javac's census and every test with it set, and the javac census report and the 28 targets'
translations are byte for byte the same. On javac, whose visits rarely repeat a call's targets,
the walk itself goes from 1.47 to 1.29 s (the rest of the phase is the points-to analysis).

**Walking a method again only when the literal it relied on changes.** A call such as
`buffer.isDirect()` is known when every class with objects below the receiver's type returns the
same literal from it (`same_literal` in `live_methods/known.rs`). A method whose code relied on
that used to be walked again whenever an object of any class below the type was created, so a
call on an `Object` or a wide interface walked its method again for nearly every class of the
program: on the Minecraft server, 3.5 million of the main walk's 4.2 million walks. What the
classes return is now kept for each method and type that was asked for, and brought up to date as
each class gets its first object; the methods that relied on it are walked again only when the
literal changes (none, then one, then none for good). The methods, classes, receivers and
unresolved calls are the same (`JAVA_TO_RUST_LIVE_CHECK`), and so is javac's census. On javac's
census the main walk goes from about 90,000 walks (36,800 of them for an object) to 50,000 (223),
and the phase from 2.3 to 1.6 s, 0.8 s of which is the walk of the started threads.

**A points-to analysis that works per call, not per target.** The points-to analysis for dispatch
(`object_values::analyze_for_dispatch`) runs before each walk of the methods that can run, and
on the Minecraft server it took about 290 s each time (3.3 million runs of 1.1 million visits).
A visit runs again whenever a place it read grows, and on each run its calls used to find their
targets again (an implementation looked up for each class of the receiver), pass every target
its arguments again, and read and join the return of every target, though a call on a receiver
of a wide type runs thousands of methods and most calls have the receiver and arguments they had
before. While the analysis runs, a call of four targets or more now keeps, in each context, the
receiver's flow with the targets found for it and the argument flows it last passed them: the
targets are taken again while the receiver holds the same, and arguments that hold the same are
not passed again, as places only grow. The returns of each list of sixteen targets or more are
joined into a place of their own, grown with the returns (`Joined`), which a visit reads instead
of every target's return, so that it runs again only when the join grows. A read of an array
element joins every element place, one per element type; that join is kept too, grown with the
places. The analysis reaches the same facts, as the smallest set closed under its rules does not
depend on the order, and javac's census is byte for byte the same. On javac its readers go from
442,000 to 199,000, and each run of it takes about a sixth less time (8.2 to 6.7 s on average
over four runs, on a busy machine).

**Early static reads by groups.** `analysis/early_reads.rs` walks, for each class with an eager
static initializer, what each statement of the initializer runs. It used to walk method by method
and list every method that each virtual call can run, once per call, thousands for a call on a
receiver of a wide type; on the Minecraft server it took 90 to 132 s in each round of the
analyses. The methods that a virtual call runs are now listed once per method and receiver type,
and shared by the calls that have them; methods that run each other without passing through a
static initializer are walked as one group, since reaching one reaches all (each `<clinit>`
stays a group of its own, as a class's walk cuts off its own); and the test that leaves out code
from which no read of the class's statics can run is worked out over the groups.
`Hierarchy::targets` no longer checks each implementation against the list found so far.
A group that runs thousands of others (one that holds a virtual call on a wide type) has them by
the set of initializers whose statics they may read: the sets are kept once each (javac's 34,800
groups have 158), so a class's walk tests a set once rather than every group that has it, and
the groups that read nothing are left out for good. With `JAVA_TO_RUST_TIMINGS` the step prints
how many groups and sets each walk enters and looks at.
`JAVA_TO_RUST_EARLY_CHECK=1` also walks every method one by one, leaving none out, and panics
unless both find the same reads. On javac the step goes from 0.25 to 0.11 s, and its census
report and the 28 targets' translations are byte for byte the same.

**Ranges over numbered variables.** `analysis/ranges.rs` copies the state of the variables at
every branch and joins two states wherever paths meet. The state used to be a hash map from each
variable's name to its range, so every copy allocated each name again and every join hashed
them; on the Minecraft server, where the analysis took 56 to 64 s in each round, hashing,
copying names and allocating were about half of its profile.
Names are now numbered once per analysis (`Names`), and a state is a vector of variables sorted
by number (`Env`): a copy is one allocation and a copy of memory, and a join, a widening or a
comparison of two states is one pass over both. What the body, the handlers and the `finally`
of a `try`, and the body of a counting loop, assign is found once per statement rather than once
per variable on every pass, and a counting loop looks for the counters that grow by constants
only among the variables its body assigns. The facts are the same: `JAVA_TO_RUST_RANGES_CHECK=1`
checks each set of assigned variables against a scan of the statements for each variable, and
javac's whole translation, its census report and the 28 targets' translations are byte for byte
the same. On javac the analysis goes from 1.05 to 1.16 s to 0.55 to 0.60 s.

**Census rounds that stop at a check.** A census runs the analyses again after each round of
stubs (`program/census.rs`), and on the Minecraft server most rounds end at a check: a pattern
variable, then `synchronized` on a string, then exceptions. The checks (`analysis::analyze_from`)
run in their order as soon as what they read is known: the validation of constructors and enums
and the checks of strings and boxes as `Object`s, of what threads share and of monitors read only
the code and the representation's facts. The range analysis, fixed fields, statics, early static
reads and nullness, which none of them reads, come after them, so a round that one of these
checks ends no longer runs them. The result is the same by construction: each check sees the
facts it saw before, and the steps moved after the checks see the checks' own facts (the
identity checks, the boxes compared by identity) only once they have run, as before. The range
analysis reads no facts, and a census keeps its facts with the representation's while no stub
changes the program. javac's census report and whole translation, the 28 targets' translations,
and the translations and census reports of the whole-program corpus are byte for byte the same.
`a_census_stubs_what_each_check_of_the_analyses_rejects_round_by_round` (`tests/translation.rs`)
has a round for each kind; its first two rounds run none of the steps moved after the checks.

**Errors of every check in one round.** A round of a census goes on past the errors it finds
(`analysis::analyze_collecting`): each check, the analysis of exceptions, the messages of
`NullPointerException`s and then the generator run as if the errors before them had passed, on
the facts found so far (the analysis of exceptions keeps its facts with its errors, which the
messages read), and the round stubs the code of all of their errors at once. An error found in
code that another error's stub takes away comes to nothing; a stub that changes the facts of the
rest of the program (a stubbed constructor can leave a field null, whose dereferences then need
messages) shows in the next round. A panic of the generator outside a method, after errors, ends
the round with those errors. On the Minecraft server, pass 3 took a round each for the checks
(28 pieces), the messages (633), the generator (171) and a cast (1), and a fifth that passed,
about 6 minutes each. `a_census_finds_the_same_with_the_errors_of_every_check_in_one_round`
(`tests/translation.rs`) finds the same census and translation in one round as round by round
(`program::round_by_round`), where each round ends at the first check that rejects code.

**Every missed cast and operation in one round.** After writing, the generator checks that
each cast whose `ClassCastException` a `catch` sees, and each operation whose failure one sees
(`Facts::raising`, `Facts::catching`), was written in its checked form (`Gen::check_casts`,
`Gen::check_catching`). Each check rejects every one it misses, first to last by line and
column, as the check of dereferences does, so that a census stubs them all in one round
rather than one per round: on the Minecraft server, two casts in the arguments of natives that
the runtime lacks (`XInputMethod.java:89`, `BufImgSurfaceData.java:125`), where a round after
the first takes about 690 s (analyses 430 s, generator 256 s). The census stubs the same code
in the same order as a round per cast would, so its report and translation are the same.
`a_census_stubs_every_cast_the_generator_misses_in_one_round`
(`tests/translation.rs`) has two such casts, stubbed in one round.

**Order of the methods solved for the objects on the stack.** The analysis of the objects
that may live on the stack (`analysis/escape.rs`) solves each method's summary again whenever
the summary of a method it may call changes, taking first the components of calls that others
call and, in a component, the smaller methods. A method's size counts the methods its calls may
run as well as its steps: `Dynamic.into` of Mojang's serialization library is one call,
`f.apply(this)`, that may run thousands of lambdas, and as a method of one step it was solved
again after each of them changed, 8,253 times and 45 s in each round of the Minecraft server's
census (alongside the other analyses). Taken after them, it waits until they settle. The
summaries are the least fixpoint in any order, so the facts are the same: javac's census is
byte for byte the same, with 61,500 solves instead of 70,800 (1.3 s instead of 1.6 s), and a
program of 2,000 lambdas called through one such method takes 12,000 solves instead of 2
million (0.17 s instead of 3.9 s).

**Steps of the analyses alongside each other.** Every check reads facts that the steps before
it find, so none can move ahead of another; what a round can do is find its facts sooner. The
steps that read none of each other's
facts run at once, on threads of their own (`analysis/alongside.rs`): from the start of a round,
alongside the representation and the checks, the ranges, fixed fields, statics and early reads,
which read the code and the ranges alone, the ASCII strings on a thread of their own from when the
ranges are found (nothing reads them before the generator, and on the thread of the early reads
they held up nullness, which waits for those, by 12 s on the Minecraft server), and the objects on
the stack, which the census takes once the messages of `NullPointerException`s pass; once the representation and the
pattern variables are checked, nullness and then the borrowed parameters, alongside the object
values and the checks after them. Each step's result is taken where the step ran before, so a
round finds the same facts and stops at the same error or panic (a panic carries its place in
the translator to the thread that takes it), and the result of a step that a check before it
ends the round without is dropped. The type hierarchy keeps its answers behind locks, and the
translator allocates from one arena of glibc's, as with one thread: with an arena per thread,
javac's census took a third more memory. A round that a check ends gains nothing; one that gets
past the checks no longer waits for these steps, which on the Minecraft server took about 75 s
of each such round of about 390 s (ranges 16 s, early reads 11 s, nullness 18 s, borrowing
10 s, objects on the stack 19 s, and the ASCII strings, untimed then). On javac's census from
its checkpoint, a round of the analyses goes from 5.9 to 6.2 s to 4.4 to 4.8 s, and the peak
memory from 0.50 to 0.51 or 0.52 GB.
`a_census_finds_the_same_with_the_steps_of_the_analyses_one_after_another`
(`tests/translation.rs`) compares a census of five rounds, the generator's among them, with the
steps run alongside each other and one after another.

**Generator rounds that write only what is new.** The enums of class types get the items the
code asks for (conversions, dispatch, bodies of methods), which only writing the code finds, so
the generator (`codegen::generate_all`) writes the whole program, then writes the enum items
alone until they ask for nothing new, and then all of them once more with the final set.
Only the enum items read which items are wanted, and what an item asks for depends on the item
alone, so a round of enum items now writes only the enums of the types asked for anew, and in
each only the items asked for anew (`Gen::only_enums`); the rest would ask for what it asked
for before. What does not depend on the wanted items (the live types, the names of methods, their
receivers, the homes of methods, the classes' numbers) is worked out once for all the rounds
(`Setup`), not once per round. The last round writes only the enum items too, all of them,
and puts them among the other items of the first round, whose text it keeps (`Kept`,
`modules::merge`). Labels are numbered per item of a module, so an item's text depends on the
items before it only through the blank line that may start it (`gap`): of the first round's
items, only the one right after a module's enum items, where those end otherwise than what was
before them, is written again. What the checks after the writing read (the bodies written, the
dereferences and casts written in their checked forms) and the classes named as values are
carried over from the first round. A name made once for the whole program records nothing in
the item that first asks for it (`modules::name_in_package`), as which item that is depends on
what a round writes. javac's census from its checkpoint saves the same tree (`--save-tree`)
byte for byte, and its generator takes 7.4 s: 3.4 s for the first round, 2.0 s for its ten
rounds of enum items together, and 1.9 s for the last, which wrote the whole program again in
5.3 s.

**Generator threads.** Each round writes the program's modules one by one, and what it writes
of a module depends on the module alone (`Gen::module`), so the modules of a round can be
written on several threads (`codegen/threads.rs`): each thread has a generator of its own and
takes the next module not yet written, the modules are put back in their order, and what the
generators found besides the text (the items asked for, the bodies written, the dereferences
and casts written in their checked forms, the classes named as values) is joined, as each only
grows. A panic outside the methods comes out of the first module that panicked, as with one
thread. javac's census saves the same tree byte for byte on one thread and on eight, and
`the_generator_writes_the_same_on_several_threads` (`tests/translation.rs`) compares programs of
several packages and a census on one thread and on three. The generator still writes on one
thread unless `JAVA_TO_RUST_GENERATOR_THREADS` asks for more, because the translator allocates
from one arena of glibc's (`main.rs`), and the generator allocates so much that its threads wait
for each other at the allocator: on javac's census from its checkpoint, eight threads make the
first round 3.1 s instead of 3.4 s, and each small round 0.3 s slower. With an arena for each
thread (`M_ARENA_MAX` of 9) the first round takes 1.3 s and the generator 4.4 s instead of 7.4
s, but the census's peak memory goes from 0.53 to 0.79 GB, 0.14 GB of that the analyses'
threads. `MALLOC_ARENA_MAX` in the environment, which glibc reads itself, sets the number of
arenas in place of the one: on javac's census from its checkpoint, four generator threads and
five arenas take the generator's two passes from 6.0 to 3.5 s and the census from 25.6 to 21.3
s, at a peak of 0.78 GB instead of 0.63 GB; with mimalloc as the translator's allocator the first round takes 1.1 s, the generator
3.8 s (6.5 s on one thread) and the analyses 6.3 s instead of 7.9 s, at a peak of 0.64 GB (0.58
GB on one thread). Plan: pick the allocator by a census of the Minecraft server, whose peak of 16
GB must stay well under the machine's 24 GB, and then write on several threads by default.

**Messages kept from round to round.** Pass 3 of a census runs the analyses, the messages of
`NullPointerException`s and the generator once per round, and the stubs between the rounds change
only a few hundred methods; on the Minecraft server, the messages took about 100 s in each of the
three rounds. What a method's messages are depends on its own code (and, for the first
constructor of a class, the initializers of its instance fields), the declarations and the class
files' names alone, not on the facts, which only pick the messages needed afterwards. So a census
keeps each method's messages (`analysis::KeptMessages`) and, after each round of stubs, forgets
those of the methods whose code changed (`Stubs::changed`): the methods stubbed, the abstract
methods made concrete, and the methods of a class whose field initializer a stub dropped. An
expression's id is unique in the program and a stub's expressions get new ones, so a kept
message can only be one of code that is still there. `timing: messages: N of M methods found
anew` shows how many were found again. The census is the same:
`a_census_finds_the_same_with_the_messages_of_unchanged_code_kept_from_round_to_round`
(`tests/translation.rs`) compares a census of two rounds with one that finds every method's
messages anew in each round (`program::messages_found_anew`), and with
`JAVA_TO_RUST_CHECK_KEPT_MESSAGES=1` each round also finds every method's messages anew and
panics if they are not the ones kept, which on the Minecraft server costs back the 100 s a round.

**Facts kept across rounds of stubs.** On the Minecraft server, pass 3 of a census took two or
three rounds, each running the analyses (about 430 to 490 s), the messages and the generator
(about 260 s), only to take away a few pieces each: a `toString` whose exception is caught,
messages of `NullPointerException`s, casts whose `ClassCastException` is caught in the
arguments of natives the runtime lacks. A stub ends the program where its code would run
(`java::not_translated`), so it only takes behaviour away: the facts of the analyses still hold
for the program after the stubs, which may do less than they say but nothing more, as they held
for the generator that ran on them in the same round past those errors. So a round whose errors
the stubs all took away by replacing method bodies keeps its facts, and goes on with the
messages of the changed methods and the generator (`program/census.rs`, `analyse`; see the next
paragraph). The facts that name the expressions of the code a stub replaced, which is no longer
in the program, are taken out of the kept facts (`forget_replaced`): the dereferences that the
generator checks it wrote (`Facts::null_checked`) and those that need a message
(`Facts::npe_message_needed`), which would otherwise be rejected as not written or without a
message. A stub that changes more than a body (an abstract method made concrete, a field
initializer changed: `Stubs::reshaped`), an error that no stub took away, or a round that
stopped before the analyses finished runs the analyses again, as does a census round by round.
The translation of a census that kept its facts may differ from one that ran the analyses again
where the facts of stubbed code reach, as a stubbed method keeps the form its callers saw (it
may still return a `Result`); a stub never runs past its first line, so the program behaves the
same. `a_census_stubs_every_cast_the_generator_misses_in_one_round` (`tests/translation.rs`)
runs the analyses and the generator once for its two rounds of stubs, one of them in a method
with a dereference whose `NullPointerException` is caught.

**Stubs written in the same run of the generator.** The round after stubs that only replaced
method bodies would run the whole generator again (about 260 s on the Minecraft server) only to
write the stubbed methods. Instead, a census's generator (`codegen::generate_all_stubbing`)
hands the errors of its first round, with those of the analyses and the messages before it, to
the census, which stubs them there and then and records them as a round of its own would. Where
the stubs replaced only method bodies and took every error away, the census keeps the facts (as
above), finds the messages of the changed methods again, and the generator goes on in the same
run: a module's text depends on its own code alone, given what the generator works out of the
whole program before it writes (`Setup`), so it writes again only the modules that the stubs
reach (`restubbed`). Those are the modules that hold a changed method (its class's, its home
class's, its free functions'), or code that calls one or creates an object of its class (the
generator reads the bodies of a lambda's methods where it is created, and those of constructors
for objects on the stack); those that hold a class that the stubs left with no `new`, or that
asked for the variants of an enum that may hold one (`Found::variant_types`); and those that
rejected code in the first round. Every other module keeps its first round's text and what
writing it found (`Found`); the checks after the writing run again over all of them, and the
rounds of enum items follow as in any run. Where the stubs change anything else of `Setup`, the
enums whose `valueOf` is checked, or a message of a dereference left in the program, the
generator writes the whole program again in the same run, as the next round would have. Where
the stubs changed more than a body or an error stayed, the generator stops after its first round
and the census goes on as before, to a round that runs the analyses again. So a census whose
stubs only replace bodies runs the generator once. The Minecraft server's pass 3 has had only
such stubs (a `toString` whose exception is caught, messages of `NullPointerException`s, casts
in the arguments of natives the runtime lacks). javac's census is the same byte for byte,
report and translation: of its 18 errors in pass 3, 5 are in methods that an earlier error of the
same round stubbed, which counts as an error no stub took away, so it runs the analyses again as
before, and its first run of the generator now stops after the first round (2.7 s rather than
4.4 s). Taking those errors as clean instead, the generator writes 102 of javac's 169 modules
again in 2.3 s, where a whole run takes 4.3 s, and its translation is the same byte for byte as
the whole generator's on the same stubbed program.
`a_census_stubs_every_cast_the_generator_misses_in_one_round` compares a census with one that
runs the generator again for the stubs (`program::generator_again`): report, JSON and
translation are the same.

Further speed-ups, not done:

- The walk from scratch when the members written into a record change is what is left of the
  restarts. Declaring again only the units whose records changed, and visiting again the
  methods of theirs that were reached, would make it incremental too.
- javac is now most of a large program's translation (9 to 15 s of `ScannerGrades`), and at
  least 2.4 s and 0.9 GB of any. A cache of javac's trees per JDK unit would have to be keyed
  by the whole JDK tree (a unit's attributed tree depends on the declarations of others), and
  saves 0.5 to 1.3 s on a small program; a JVM kept running between translations (a server
  for the corpus and target runs) would save the start-up instead.
- Lowering of `ScannerGrades` and `TextAdventure` now takes 1 to 2.5 s, twice (two attempts
  of the Rust generics, `lower::real_generics`); keeping what the first attempt learned
  instead of lowering everything again would halve it.
- An error in code that an earlier error of the same round stubbed (javac has 5 of its 18 in
  pass 3) counts as one that no stub took away, so the census runs the analyses and the
  generator again. Taking such an error as taken away by that stub would let the census keep its
  facts and write the stubs in the same run of the generator.
- The analyses' rounds in a census, where a stub changes more than a body or an error stays.
  Ways to make them cheaper, each to be checked by a census that stays byte for byte the same:
  - Incremental analyses: rerun the object values and the exceptions only over the stubbed
    methods and what their values and exceptions reach (a worklist seeded with them) rather
    than over everything. As the stubs only take facts away and the analyses grow their facts
    to a fixpoint, they cannot start from the last round's facts; they would need to find the
    facts that depended on the stubbed code and drop them before growing again (delete and
    derive again), which is a large change to each analysis.
  - Keep the representation and the range facts (`analysis::Represented`), which a census
    drops whenever a stub changes the program, for the methods no stub changed, where they are
    facts of a method's own code. The representation is mostly whole-program (borrowing,
    owned, flows), so this saves part of its 34 s at most.
  - Find the messages of the first round alongside the analyses, since the messages read no
    facts until they are picked: their results are taken only where a `catch` reads a message,
    as now. This needs `Names` made safe to share between threads. The class files that the
    messages need (56 s for the Minecraft server) already compile on a thread of their own
    while the analyses run (`Names::early`), javac beside them.

## The translated javac

javac translated as a whole program with `--census` (`scripts/javac-check.sh` checks its output
against the real javac's), timed against the real javac on four inputs:
`scripts/javac-check/hello` (one class), `corpus/programs` compiled together (47 files, 121 class
files), the files of `corpus/features` at 21d7678e that javac compiles together (540 files,
1,509 class files; the 13 left out declare a public class under another file's name or one that
another file declares too), and a large server code base outside the repository (about 4,700
files). Wall clock, the best of three rounds for the first three; peak memory is the resident
set of the process. Every run gives the same class files and the same standard error as the real
javac.

| build | hello | 47 programs | 540 features | large code base |
|---|---:|---:|---:|---:|
| real javac (JDK 26) | 0.19 s | 0.63 s, 210 MB | 1.85 s, 430 MB | 11.3 s, 4.1 GB |
| reference counts, `lto = "fat"` and PGO | 0.022 s | 0.36 s | 2.43 s | 60.2 s, 6.2 GB |
| reference counts, quick build | 0.035 s | 0.55 s, 86 MB | 3.93 s, 257 MB | 93.7 s, 6.2 GB |
| collector, quick build | 0.023 s | 0.34 s, 172 MB | 2.40 s, 301 MB | 59.7 s, 4.1 GB |

The quick build is `opt-level = 3` with 256 codegen units and no LTO: it builds in under three
minutes with a peak of 11 GB, and ranks changes, but runs about 1.6 times slower than a fat LTO
build with PGO. Under the collector the quick build already matches the reference-counted fat
LTO and PGO build, and on the large code base uses as much memory as the real javac. On the 47
programs the collector holds more than reference counting did because it does not collect
before 64 MB have been allocated (docs/collector.md).

The crate is one of about 537,000 lines, built with all 16 cores. Fat LTO makes the binary 22%
smaller (72 MB against 92 MB), and the crates the translator writes use it for their release
builds (`lto = "fat"`). Their debug builds are not incremental (`incremental = false`): the
data that rustc keeps for an incremental build of a generated crate is large, a new translation
changes every crate anyway, and the Minecraft server's 205 MB header crate takes rustc past
21 GB with it and to 14.2 GB without. They build for AVX2 processors (`.cargo/config.toml`, which Cargo reads
in the crate's directory); flags that a build adds go in `CARGO_BUILD_RUSTFLAGS`, which adds to
them, not `RUSTFLAGS`, which replaces them. PGO (profile-guided optimization) is three steps, which
`scripts/pgo-build.sh CRATE TRAIN_COMMAND` runs for any translated crate: a build with
`-Cprofile-generate`, a run of it over inputs that are not the timed ones (`corpus/threads` and
`corpus/whole-program` together, each project of `corpus/projects`, and the javac-check cases
other than hello; 5 s), and a build with `-Cprofile-use` of the merged profile (`llvm-profdata`
from rustup's `llvm-tools`). It takes a third off the time of fat LTO alone, and the fat LTO
build with PGO takes about 30 minutes.

javac starts one thread, the file manager's deferred close, which a normal run never starts, but
its reachable `Thread.start()` makes the program threaded and most of its classes thread-shared
(docs/multi-threading.md, section 4), whose fields are `java::SharedField`s: an acquire load or
release store of a word, a plain move on x86-64. Until a
second thread starts, a monitor costs plain loads and stores only (`locked.rs`).
Object parameters that may be null are borrowed (`&Option<java::Arc<..>>`,
docs/supported-java.md) rather than copied. `charAt` and `String.hashCode` read a string's
leading ASCII bytes as they are.

## Compile time

What rustc spends its time on in a translated program is the amount of code, not one costly
construct: on javac's translation, split into a header and 30 body crates, a debug build is
close to half type checking and borrow checking, a third LLVM and the rest metadata, code
generation and monomorphization, spread over tens of thousands of functions; a release build is
mostly LLVM. So the generated code is written in the forms that are cheapest for rustc per
function, where the optimized code is the same:

- A failure that leaves a method or a `try` block is a written-out `match` with its conversion to
  `Throwable` named (`Throwable::j2r_raised(e)`, `Throwable::j2r_null_pointer(message)`), not a
  `?` or `java::or_break!` (docs/exceptions.md). For 2,000 such statements, rustc's instructions
  went from 2.04 G to 1.48 G for null checks with Java's message, 1.39 G to 1.20 G for a `?` and
  1.59 G to 1.21 G for an `or_break!`.
- A moved method is the function that the header declares (`#[unsafe(export_name = ..)]` on the
  trait method), not a second function that calls it (docs/several-crates.md).
- The runtime chooses by a type's size or layout with `match const { .. }` and `if const { .. }`,
  so a debug build does not carry the arms for other sizes into every instance, and turns a
  `Thin` into the pointer it holds with a `transmute` of the reference, which has no debug
  assertions to compile.
- A small function keeps its code in the header (docs/several-crates.md), one item there instead
  of a declaration and a function that calls it.

On javac's translation (rustc's instructions summed over the crates, which unlike wall time do
not depend on the machine's load), against the same translation written without these forms
(the match forms and one function per moved method):

| build | without | with |
|---|---:|---:|
| debug, all crates | 822 G | 720 G |
| release without LTO, all crates | 1,736 G | 1,514 G |
| release header crate alone | 75 G, 2.29 GB | 73 G, 2.22 GB |

The release build's code runs javac's check cases with about 3% fewer instructions (its text
is 31.1 MB instead of 33.4 MB without LTO).

rustc's memory for a crate grows with its items more than with their size: on javac's header, a
method that moves costs about 19 KB (its stub and its declaration), an empty function about
13 KB and a declaration alone about 6 KB. So a function that moves is a declaration alone, by
its own name (`docs/several-crates.md`), and so are constructors and the methods that take
their object as a parameter, which are functions of their module (`Circle__new`,
`docs/object-model.md`), as are the dispatchers and bodies of a type's enum, which take its
value (`Shape__area(this: Shape)`); only a method that takes `self` keeps a stub. The header is
compiled before every body crate and holds an item for every function of the program, so it
sets a build's peak: the Minecraft server's, 226 MB, needs 16 to 20 GB in release. Half of its
text is the 285,000 functions and methods that move, of which about 68,000 took `self`, about
45,000 of them dispatchers and bodies of a type's enum (counted by kind on a translation
written before they became functions of their module). Then come the 178,000 small functions that keep their code there, and the 43,000 classes as values
(`CLASS__..`). Those are statics that name their
superclass, interfaces and declaring class by references: as constants that named them by
closures, 93,000 function bodies for rustc, they alone took rustc 2.8 GB and 15.9 s in release,
and as statics 1.8 GB and 13.0 s.

Cranelift (`-Zcodegen-backend=cranelift`, nightly) builds javac's translation in debug in 55 s
instead of 82 s with LLVM (two jobs), but the binary it builds ends with status 101 at once on
the first program, so it is not used.

## Measuring

- MatrixMultiply (Java about 0.12 s) and StringBuild (Java about 0.03 s) run for less than a
  second. Larger sizes would make the script take many minutes on a slow translation, so they
  are kept small.
- The Java times are whole-process, so programs that are short on Java include JVM start-up.
  Kernel columns exist only where a program has a warm-up loop.
- `scripts/benchmark.py` prints the load average at its start. On a loaded machine the same
  Rust binary has taken twice as long from one run to the next, so compare runs made at a low
  load.
