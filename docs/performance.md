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
measured inside the program, and it is reported in the two kernel columns.

## Results

One run of `python3 scripts/benchmark.py` (five runs per side, medians, wall clock, in
milliseconds) at a load average of 2.5 on 16 cores; MatrixMultiply's row is from a run of it
alone at a load of 2.9.

- CPU: AMD Ryzen AI 7 350 w/ Radeon 860M (16 cores)
- JDK: OpenJDK 26.0.2.1 (`java -version`)
- Rust: rustc 1.99.0 (b940084d7 2026-09-28); the crates the translator writes build in release
  with fat LTO, for processors with AVX2 (`target-cpu=x86-64-v3`, in the crate's
  `.cargo/config.toml`)

| program        | Java ms | Rust ms | Rust/Java | Java kernel | Rust kernel | kernel ratio |
|----------------|--------:|--------:|----------:|------------:|------------:|-------------:|
| BinaryTrees    |     686 |    1879 |      2.74 |           - |           - |            - |
| Collatz        |    1131 |     567 |      0.50 |           - |           - |            - |
| Interpreter    |    1086 |    1152 |      1.06 |         335 |         382 |         1.14 |
| Mandelbrot     |    3359 |    1111 |      0.33 |        1114 |        1109 |         1.00 |
| MatrixMultiply |     117 |     175 |      1.49 |          26 |          57 |         2.19 |
| NBody          |    1776 |    1846 |      1.04 |         588 |         599 |         1.02 |
| NodeList       |     207 |     148 |      0.72 |           - |           - |            - |
| ParticleArray  |    1392 |    1019 |      0.73 |           - |           - |            - |
| Sieve          |    1166 |     951 |      0.82 |           - |           - |            - |
| SortMillion    |    1679 |    1753 |      1.04 |         519 |         581 |         1.12 |
| Statics        |    1248 |    2632 |      2.11 |           - |           - |            - |
| StringBuild    |      26 |       1 |      0.05 |           - |           - |            - |
| TokenScanner   |     958 |    1624 |      1.70 |           - |           - |            - |

Differences of 20% or less between two runs are often noise or the placement of code and data
(see "Colors" in docs/collector.md); the large ratios are not.

## What the times come from

Read from the generated Rust under `target/benchmarks/<Name>/crate/src/main.rs`, from
profiles taken with `perf`, and from counts of the objects each `new` makes: a program
translated with `JAVA_TO_RUST_COUNT_ALLOCATIONS=1` set prints, when it exits, the 60 `new`
sites (class, the method the `new` is in, and "on the stack" for a `new` whose objects go in a
slot on the stack) that made the most objects, on standard error, with the total and the share
made on the stack (`java::count_allocation`).

- **Allocation (BinaryTrees 2.7x, NodeList 0.7x).** Objects live in the runtime's garbage
  collected heap (docs/collector.md): an allocation takes a slot from a per-thread list, and a
  handle is copied without a count. BinaryTrees keeps a tree of about 89 MB alive while it builds
  and drops millions of short-lived ones; the collector marks that long-lived tree again at each
  of its 72 collections (5 ms of marking and 3 ms of sweeping each, on several threads), which is
  most of the remaining gap. A young generation, planned in docs/native-speed.md, would collect
  the short-lived trees without marking the old one. NodeList walks and rebuilds a linked list of
  about 667,000 nodes; its nodes are taken from blocks in address order, so the walk follows
  memory.
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
- **Elements of an object array read again (NBody 1.0x, ParticleArray 0.7x).** Consecutive
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
  `library` in SortMillion is given to `Arrays.sort`, so it stays a shared `java::Array`.
- **MatrixMultiply (0.6x, kernel 0.6x).** The matrices are plain
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
- **Arithmetic loops (Mandelbrot 0.3x, Collatz 0.5x, StringBuild 0.05x).** These need nothing
  from the object model. StringBuild's `charAt` and `length()` on a string made only of `(char)`
  casts proven in `0..=0x7f` (`analysis/ranges.rs`) are a byte read and the byte length.

## Translating whole programs

How fast the translator itself runs in whole-program mode (`--main`, `--library` with the JDK's
`java.base` sources), which `targets/run.sh` and `tests/whole_program.rs` use. `--timings` (or
the environment variable `JAVA_TO_RUST_TIMINGS`) prints the time of each phase to standard error:
javac with the number of units it attributed, reachability with its rounds, lowering (each
attempt of the Rust generics), the analyses and the generator.

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
guards a branch (`objectless`, which looks at the class's supertypes) and whether a class is an
exception's (`recreatable`). Nothing is then decided on a class that may still come, and each
round reaches what a walk from scratch over its units, with the same classes asked for, reaches;
`JAVA_TO_RUST_REACH_CHECK=1` makes `load` check that after every round, and it holds in every
round of javac's translation and of 18 whole programs of the corpus. Before, such a method was
taken for an external one at once, and when a later unit declared its class and it turned out
dormant, its overriders had been reached for nothing (`Walk::stale`) and the walk started again
from `main`: four times in javac's 10 rounds, two or three times in those of `ScannerGrades` and
`TextAdventure`; and an `instanceof` or exception decided without a supertype stayed decided, so
the result depended on the order the units came in (javac's translation reached
`UnixDomainSocketAddress.getPath`, which a walk over all its units does not). A walk still starts
again from `main` in two cases: when the members written into a record walked before differ with
the new units, `load` walks every unit again (once in javac's rounds and in some of the corpus
programs); and a stale walk can still happen when a unit declares a class that javac had none of
when asked, such as a second top-level class of a file that a later unit came from, which none
of these programs does. The `timing: reach` line counts the two apart. With the wait, classes
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
streams stay sequential indexes calls by the names in javac's symbols without copying them.
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
builds (`lto = "fat"`). They build for AVX2 processors (`.cargo/config.toml`, which Cargo reads
in the crate's directory); flags that a build adds go in `CARGO_BUILD_RUSTFLAGS`, which adds to
them, not `RUSTFLAGS`, which replaces them. PGO (profile-guided optimization) is three steps, which
`scripts/pgo-build.sh CRATE TRAIN_COMMAND` runs for any translated crate: a build with
`-Cprofile-generate`, a run of it over inputs that are not the timed ones (`corpus/threads` and
`corpus/whole-program` together, each project of `corpus/projects`, and the javac-check cases
other than hello; 5 s), and a build with `-Cprofile-use` of the merged profile (`llvm-profdata`
from rustup's `llvm-tools`). It takes a third off the time of fat LTO alone, and the fat LTO
build with PGO takes about 30 minutes.

javac starts one thread, the file manager's deferred close, which a normal run never starts, but
its reachable `Thread.start()` makes the program threaded and most of its classes
`java::Arc<java::Monitored<..>>` (docs/multi-threading.md, section 4), whose fields are
`java::SharedField`s: an acquire load or release store of a word, a plain move on x86-64. Until a
second thread starts, a monitor costs plain loads and stores only (`locked.rs`).
Object parameters that may be null are borrowed (`&Option<java::Arc<..>>`,
docs/supported-java.md) rather than copied. `charAt` and `String.hashCode` read a string's
leading ASCII bytes as they are.

## Measuring

- MatrixMultiply (Java about 0.12 s) and StringBuild (Java about 0.03 s) run for less than a
  second. Larger sizes would make the script take many minutes on a slow translation, so they
  are kept small.
- The Java times are whole-process, so programs that are short on Java include JVM start-up.
  Kernel columns exist only where a program has a warm-up loop.
- `scripts/benchmark.py` prints the load average at its start. On a loaded machine the same
  Rust binary has taken twice as long from one run to the next, so compare runs made at a low
  load.
