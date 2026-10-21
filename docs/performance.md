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

## Results of 2026-10-21

Three runs of `python3 scripts/benchmark.py` on the `benchmarks-rerun` branch, which starts from
main at `877411b`, after the object model, exceptions, nested classes and lambdas landed. Each
run is five runs per side, medians. Each column below is the median of the three runs, and
Rust/Java is the ratio of those two medians. Times in milliseconds.

- CPU: AMD Ryzen AI 7 350 w/ Radeon 860M (16 cores)
- JDK: OpenJDK 26.0.2.1 (`java -version`)
- Rust: rustc 1.99.0 (b940084d7 2026-09-28), release profile
- Load average at the start of each run: 20.8, 7.1 and 10.8 (on 16 cores); run 3 ended at 22.6.
  Run 1 started at 20.8, just above the threshold of 20 for starting a run.

| program        | Java ms | Rust ms | Rust/Java | Java kernel | Rust kernel | kernel ratio |
|----------------|--------:|--------:|----------:|------------:|------------:|-------------:|
| BinaryTrees    |     774 |    7444 |      9.62 |           - |           - |            - |
| Collatz        |    1217 |     604 |      0.50 |           - |           - |            - |
| Interpreter    |    1121 |    1141 |      1.02 |         358 |         378 |         1.06 |
| Mandelbrot     |    3496 |    1218 |      0.35 |        1152 |        1216 |         1.06 |
| MatrixMultiply |     356 |     356 |      1.00 |          62 |          96 |         1.55 |
| NBody          |    1853 |    1889 |      1.02 |         616 |         630 |         1.02 |
| NodeList       |       - |       - |         - |           - |           - |            - |
| Sieve          |    1219 |    1059 |      0.87 |           - |           - |            - |
| SortMillion    |    1765 |    1799 |      1.02 |         541 |         598 |         1.11 |
| Statics        |    1331 |    2190 |      1.65 |           - |           - |            - |
| StringBuild    |      30 |       1 |      0.03 |           - |           - |            - |

NodeList compiles and runs, but it does not finish: its Rust binary aborts with a stack overflow
(exit -6), so no time is reported. See the NodeList section below.

The spread between runs is wide, so only the large differences are meaningful. Rust times of the
same program ranged from 5811 to 11902 ms for BinaryTrees, 1885 to 4171 ms for NBody (its kernel
was 625 to 630 ms in two runs and 1315 ms in the third), and 1155 to 1715 ms for Mandelbrot.

### Comparison with the earlier table

Earlier table: the array-ownership branch (`fd9bace`), a single run at load 72, shown below.

| program        | earlier Rust/Java | now Rust/Java | change  | Rust ms, earlier → now |
|----------------|------------------:|--------------:|--------:|-----------------------:|
| BinaryTrees    |              3.32 |          9.62 |   +190% |        5770 → 7444     |
| Collatz        |              0.38 |          0.50 |    +32% |         601 → 604     |
| Interpreter    |              1.04 |          1.02 |     -2% |        1321 → 1141    |
| Mandelbrot     |              0.33 |          0.35 |     +6% |        1179 → 1218    |
| MatrixMultiply |              1.47 |          1.00 |    -32% |         214 → 356     |
| NBody          |              1.02 |          1.02 |      0% |        1858 → 1889    |
| NodeList       |   does not compile |   stack overflow |       - |               - → -   |
| Sieve          |              0.71 |          0.87 |    +23% |        1060 → 1059    |
| SortMillion    |              0.72 |          1.02 |    +42% |        5658 → 1799    |
| Statics        |              0.98 |          1.65 |    +68% |        1763 → 2190    |
| StringBuild    |              0.05 |          0.03 |       - |           2 → 1        |

Causes of the changes:

- **Statics (+68%) is a real slowdown, but the hot loop does no extra work.** The generated Rust
  is the same as the earlier translator's. Counted with `perf_event_open` (`perf` is not installed
  here, so the count is read directly from the kernel), both runtimes retire the same instructions:
  47,531,803,862 for `fd9bace` and 47,531,804,122 for HEAD, in the per-commit bisect builds. The
  `statics::main` code is identical in both binaries apart from two string addresses. The cycle
  counts differ: in interleaved runs at load 15, `fd9bace` took 8.7 to 9.0 billion cycles and HEAD
  took 10.6 to 10.9 billion.
  - The hot loop starts at a different offset in the two binaries (`0x17540` and `0x173e0`), so
    the difference looks like code placement. Building the same HEAD crate with
    `-C llvm-args=-align-all-functions=6` brought it back to 8.5 to 8.9 billion cycles. That is a
    build setting, not a runtime change, and it was not applied: it affects every benchmark.
  - The throw path and the accessors are already `#[cold]`/`#[inline(never)]` (throw) and
    `#[inline]` (`position`, `get`, `set`), since before `fd9bace`, and no call into them is in
    the hot loop. So there was no throw or accessor change to fix.
  - Across the runtime commits since `fd9bace`, the instruction count is 47.5 billion from
    `3f5f715` on. Cycles for those same commits range from 7.6 to 13.5 billion on the loaded
    machine, with no commit that stands out. No commit is named as the cause.
  - The earlier split (about 170 ms for `throw`, 370 ms for `array.rs`) came from wall-clock runs.
    The instruction counts do not support it, so it should not be read as a measured cause.
  - Not done: the per-benchmark instruction comparison of the other programs in
    `scripts/benchmark.py`. No runtime change was made, so nothing could have become slower.
- **SortMillion (+42%), Collatz (+32%), Sieve (+23%), BinaryTrees (+190%) did not get slower in
  Rust.** The generated Rust is the same as the earlier translator's, except for Collatz
  (`n = n / 2` is now `n /= 2`, the same operation). BinaryTrees ran at 5.8 s on both translators
  in back-to-back runs. Their Rust times are the same or lower than before: SortMillion went from
  5658 to 1799 ms. What moved the ratio is the Java side, which was 1.5 to 4.5 times faster than in
  the earlier run at load 72 (BinaryTrees 1736 to 774 ms, SortMillion 7860 to 1765 ms).
- **MatrixMultiply (-32%)** is noise: the Rust side ranged from 182 to 467 ms across the three
  runs, and the generated Rust is the same.
- **Mandelbrot (+6%), Interpreter (-2%), NBody (0%), StringBuild** are within the noise. StringBuild
  runs in 1 ms on Rust, below the resolution of the script's rounding.

## Earlier results: array-ownership branch

One run of `python3 scripts/benchmark.py` on the `array-ownership` branch (five runs per side,
medians, load average 72 at start). Times in milliseconds.

| program        | Java ms | Rust ms | Rust/Java | Java kernel | Rust kernel | kernel ratio |
|----------------|--------:|--------:|----------:|------------:|------------:|-------------:|
| BinaryTrees    |    1736 |    5770 |      3.32 |           - |           - |            - |
| Collatz        |    1565 |     601 |      0.38 |           - |           - |            - |
| Interpreter    |    1274 |    1321 |      1.04 |         390 |         430 |         1.10 |
| Mandelbrot     |    3571 |    1179 |      0.33 |        1193 |        1177 |         0.99 |
| MatrixMultiply |     146 |     214 |      1.47 |          31 |          66 |         2.13 |
| NBody          |    1820 |    1858 |      1.02 |         605 |         618 |         1.02 |
| NodeList       |       - |       - |         - |           - |           - |            - |
| Sieve          |    1484 |    1060 |      0.71 |           - |           - |            - |
| SortMillion    |    7860 |    5658 |      0.72 |        2843 |        1319 |         0.46 |
| Statics        |    1800 |    1763 |      0.98 |           - |           - |            - |
| StringBuild    |      33 |       2 |      0.05 |           - |           - |            - |

NodeList did not compile on that branch; it compiles now and runs since the drop of long chains
was made iterative. See below.

The four programs that use arrays most, before (main at `18ab2fb`) and after the array
ownership work, run back to back in the same minutes:

| program        | before Rust/Java | before kernel ratio | after Rust/Java | after kernel ratio |
|----------------|-----------------:|--------------------:|----------------:|-------------------:|
| Interpreter    |             3.74 |                1.23 |            1.06 |               1.18 |
| MatrixMultiply |            34.92 |               76.64 |            1.41 |               2.07 |
| Sieve          |             2.10 |                   - |            0.81 |                  - |
| SortMillion    |             5.55 |                6.65 |            0.89 |               0.86 |

An earlier pair of runs, at load 74 before and 27 after, gave MatrixMultiply kernel ratios of
99.15 before and 8.68 after with the plain arrays alone (before the sliced loops below), and
Sieve 0.74 before and 0.49 after.

## Causes of the slow cases (earlier run)

Read from the generated Rust under `target/benchmarks/<Name>/crate/src/main.rs`.

- **MatrixMultiply (was 85x; kernel 140x; now 1.5x, kernel 2.1x).** Element access `c[i][j]`
  was `java::get(&java::get(&c, i), j)`: the outer `get` cloned the row's handle (an `Rc`), and
  each access borrowed a `RefCell` and checked the bounds. The matrices are now plain
  `Vec<Vec<i32>>`: `create` returns its `Vec`, `multiply` borrows `a` and `b` as `&[Vec<i32>]`
  and returns `c`, and `a = b; b = c;` moves the `Vec`s, since neither is read again before it
  is assigned. `c = new int[n][n]` with `n = a.length` puts `c[i][j]` in bounds, so it is
  `c[i as usize][j as usize]`. The rows of `b` have unknown lengths, so `b[k][j]` keeps Java's
  check, but the inner loop checks it once before it starts: when `k` is a row of `b` and `n`
  is at most the lengths of `b[k]` and `c[i]`, the loop runs over the slices
  `&b[k][..n]` and `&mut c[i][..n]`, without a check, and vectorizes; otherwise it runs with
  every check (see `analysis/arrays/slices.rs`). The remaining 2x against the JIT is not
  profiled; the loop still computes with `wrapping_add` and `wrapping_mul` on `i32`.
- **StringBuild (31x before the ASCII fix).** Before the fix, `s.charAt(i)` became
  `java::char_at`, which calls `encode_utf16().nth(i)`, so each access walked the string. The
  loop condition `i < s.length()` called `java::string_length`, `encode_utf16().count()`, so each
  iteration walked it again: the scan was quadratic. The range analysis now records each
  `(char)` cast whose value stays in `0..=0x7f` (`analysis/ranges.rs`). A string made only of
  such casts is ASCII, so `charAt` is `s.as_bytes()[i] as u16` and `length()` is `s.len()`. A
  string that may hold a non-ASCII unit keeps the UTF-16 walk, which is still linear per access.
- **Statics (3.2x before the fix).** Each static field was a `thread_local!` `RefCell`, reached
  through `with_borrow` and `with_borrow_mut`. A primitive static is now a `Cell`, read with
  `get` and written with `set`, which needs no borrow flag. Other statics stay `RefCell`.
- **BinaryTrees (3.3x here, 8x on a busier run).** `count(node: Option<Rc<..>>)` clones the
  `left` and `right` handles on every call (a refcount increment and decrement each), and checks
  for `null` at every access. Allocation goes through `Rc`. Not profiled; these are read from
  the code.
- **Interpreter (was 1.9x to 3.7x; now 1.0x, kernel 1.1x).** The stack and locals were already
  plain `Vec<i32>`, but every access went through `java::at` and `java::put`, with the code
  that builds the exception next to each access. The indexes come from the program being interpreted, so
  they cannot be proven in range; the checks stay, but the check is now an inlined compare and
  a branch to a `#[cold]`, `#[inline(never)]` throwing function (runtime `array.rs`), and
  `java::at`, `put` and `modify` are `#[inline]`.
- **Sieve (was 1.6x to 2.1x; now 0.7x to 0.8x).** `composite` is `new boolean[limit + 1]`, and
  the bounds analysis proves both loops in range: `i * i <= limit` with `i >= 2` gives
  `i <= limit`, `j <= limit` bounds the inner loop, and the size `limit + 1` is longer than any
  of them. Every access is `composite[i as usize]` with no check.
- **SortMillion (was 1.6x to 5.6x; now 0.7x to 0.9x).** `randomInts` returns a plain `Vec`,
  and `quickSort` borrows it as `&mut [i32]`. Its indexes (`i`, `j`, the pivot) cannot be proven
  in range, so they keep their checks, on the cheaper path. `library` is given to
  `Arrays.sort`, so it stays a shared `java::Array`; the library helpers do not take plain
  arrays yet.

Mandelbrot (0.33x, kernel 0.99x), Collatz (0.38x) and NBody (1.02x) are at or faster than Java
and need no change.

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
been. Two cases are not incremental. When the members written into a record walked before
differ with the new units, `load` walks every unit again; it did not happen in
`ScannerGrades`, whose rounds were timed one by one. When a method was taken for an external one, because its class was missing, and made
its overriders reachable, and then turns out to be dormant (`Walk::stale`), that walk starts
again from `main` over the units it has declared. `ScannerGrades` and `TextAdventure` do that
two or three times in their 11 rounds (`AbstractCollection.isEmpty`, `CalendarNameProvider`,
`CalendarSystem`), `StreamPipeline`, `PriorityScheduler` and `ListOperations` once; the
number varies between runs with the order of the work, but the result does not. Every round
of every target and corpus program was checked equal, `Reach` and classes asked for, to a walk
from scratch over the same units, and the translations are byte for byte those before.

Release translator on a loaded machine (load 16 to 23), the before and after runs alternated,
twice each:

| program | reachability before | after | translation before | after |
|---|---|---|---|---|
| ScannerGrades | 5.7, 6.4 s | 3.1, 2.2 s | 18.0, 20.9 s | 22.3, 15.4 s |
| TextAdventure | 6.1, 7.5 s | 2.2, 1.9 s | 18.0, 23.3 s | 14.9, 14.3 s |

For the small programs reachability was 0.1 to 0.3 s and is about two thirds of that; the
translation's time is javac's. Peak memory is unchanged (1.1 GB + 2.3 GB for these two).

Further speed-ups, not done:

- The restarts of a stale walk are now about half of what is left of reachability on the large
  programs (a round without one takes 0.06 to 0.35 s, one with one 0.5 to 0.6 s). Holding a
  method of a missing class back until its class comes, instead of taking it for external
  meanwhile, would avoid them, but it changes what the rounds in between ask for, and so the
  units' order and the output.
- javac is now most of a large program's translation (9 to 15 s of `ScannerGrades`), and at
  least 2.4 s and 0.9 GB of any. A cache of javac's trees per JDK unit would have to be keyed
  by the whole JDK tree (a unit's attributed tree depends on the declarations of others), and
  saves 0.5 to 1.3 s on a small program; a JVM kept running between translations (a server
  for the corpus and target runs) would save the start-up instead.
- Lowering of `ScannerGrades` and `TextAdventure` now takes 1 to 2.5 s, twice (two attempts
  of the Rust generics, `lower::real_generics`); keeping what the first attempt learned
  instead of lowering everything again would halve it.

## NodeList: stack overflow when the list is freed (fixed)

`benchmarks/NodeList.java` builds a linked list of about 667,000 nodes. It compiled once the
assignment from a field read was fixed on main, but the binary aborted with SIGABRT: when the
list went out of scope, Rust's drop of `next` recursed once per node and overflowed the 8 MB
main stack. With `ulimit -s unlimited` it printed Java's checksum.

A class whose fields can lead back to objects of the same class now gets a `Drop` that frees
deep chains iteratively (docs/object-model.md, "1.13 Dropping long chains"). Near the top of the
stack the `Drop` only compares the stack address with a thread-local and leaves the fields to
Rust's recursive drop; 128 KB of stack deeper it moves the links onto a heap stack. Only
classes on a cycle of field types change: both benchmarks here have one.

`python3 scripts/benchmark.py BinaryTrees NodeList`, five runs per side, medians in ms:

| run                                   | load | BinaryTrees Java | BinaryTrees Rust | NodeList Java | NodeList Rust |
|---------------------------------------|-----:|-----------------:|-----------------:|--------------:|--------------:|
| main at `45ad1c1`                     |   46 |             1873 |            11428 |             - | aborts (exit -6) |
| heap stack for every drop             |   19 |             1337 |             9610 |           230 |           481 |
| recursion counter, heap stack below 200 levels | 12 |       763 |             6977 |           234 |           466 |

The machine's load moved the Java and Rust times by up to a factor of two between these runs, so
the cost of the `Drop` was measured separately: the generated crate of each benchmark built
twice, once as generated and once with the `Unlink` and `Drop` impls removed (NodeList then run
with an unlimited stack), the two binaries run alternately, median of the runs in ms:

| version of the runtime                           | runs | BinaryTrees with / without | ratio | NodeList with / without | ratio |
|--------------------------------------------------|-----:|---------------------------:|------:|------------------------:|------:|
| heap stack for every drop                        |    7 |                6769 / 5739 |  1.18 |               471 / 480 |  0.98 |
| recursion counter, heap stack below 200 levels   |    9 |                6413 / 6059 |  1.06 |               470 / 500 |  0.94 |
| stack address test, heap stack below 128 KB      |   10 |                6745 / 6255 |  1.08 |               471 / 474 |  0.99 |
| the same, committed (deep path out of line)      |   10 |                6773 / 6539 |  1.04 |               495 / 544 |  0.91 |

BinaryTrees frees about 300 million nodes, so it is the worst case: every node runs the `Drop`.
In the last version that is eight instructions and one call more than Rust's own drop of a
node (a thread-local load, a compare, and a call into the node's drop glue that is no longer
inlined into `Rc::drop_slow`), about 1 to 1.5 ns per node, 4 to 8 percent of its run time
across the A/B runs at loads 5 to 15. NodeList is unchanged within the noise: its chain is freed on the
heap stack, which costs no more than recursion. Programs without a cycle of field types are
not touched.

## Deviations from the plan

- MatrixMultiply (Java about 0.85 s here, 0.2 s quieter) and StringBuild (Java about 0.23 s) are
  under the 1 to 5 second range asked for. Their Rust sides are super-linear in the input (the
  matrix work, and the quadratic `charAt` and `length`), so larger sizes would make the script
  take many minutes per run. They are kept small on purpose.
- The Java times are whole-process, so programs that are short on Java include JVM start-up.
  Kernel columns exist only where a program has a warm-up loop.
- The earlier table is a single run on a loaded machine. The 2026-10-21 table is three runs
  whose medians are shown; the spread between them is in its section. Re-run them on an idle machine before drawing
  conclusions about small differences; the large changes (MatrixMultiply, Sieve, SortMillion)
  are far outside the noise.
