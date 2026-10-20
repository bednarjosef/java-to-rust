# Performance

How fast translated programs run compared with Java. The programs are in `benchmarks/`; the script
that times them is `scripts/benchmark.py`:

    python3 scripts/benchmark.py [--runs N] [Program ...]

With no program names it times every file in `benchmarks/`; `--runs` defaults to 5.

Each program is compiled with `javac`, translated with `java-to-rust --crate`, built with
`cargo build --release`, and run on both sides. The Rust run must print the same standard output
and exit with the same code as Java's, or the program is reported and not timed. Times are the
median of the runs, wall clock. Java runs with its default flags, so JVM start-up is included. A
program that warms up prints `kernel-ms N` on standard error; that is the time of the timed loop
measured inside the program, and it is reported in the two kernel columns.

## Environment

- CPU: AMD Ryzen AI 7 350 w/ Radeon 860M (16 cores)
- JDK: OpenJDK 26.0.2.1 (`java -version`)
- Rust: rustc 1.99.0 (b940084d7 2026-09-28), release profile
- Load: the machine was busy during the run. The load average was 101.8 at start (16 cores),
  from other agents' JVMs and builds. The absolute times are inflated, and the Java side moved
  more than the Rust side between runs (BinaryTrees on Java: 794 ms on a quieter run, 4359 ms
  here), so read the ratios as rough.

## Results

One run of `python3 scripts/benchmark.py` (five runs per side, medians). Times in milliseconds.

| program        | Java ms | Rust ms | Rust/Java | Java kernel | Rust kernel | kernel ratio |
|----------------|--------:|--------:|----------:|------------:|------------:|-------------:|
| BinaryTrees    |    4359 |   34649 |      7.95 |           - |           - |            - |
| Collatz        |    4524 |    1564 |      0.35 |           - |           - |            - |
| Interpreter    |   14014 |   26334 |      1.88 |        2162 |        7341 |         3.40 |
| Mandelbrot     |   10099 |    3611 |      0.36 |        3677 |        3607 |         0.98 |
| MatrixMultiply |     852 |   72810 |     85.46 |         163 |       22781 |       139.76 |
| NBody          |    6763 |    7668 |      1.13 |        1873 |        2392 |         1.28 |
| NodeList       |       - |       - |         - |           - |           - |            - |
| Sieve          |    3497 |    5640 |      1.61 |           - |           - |            - |
| SortMillion    |    3010 |    4725 |      1.57 |         864 |        1718 |         1.99 |
| Statics        |    1839 |    5796 |      3.15 |           - |           - |            - |
| StringBuild    |     230 |    7235 |     31.46 |           - |           - |            - |

NodeList does not compile. See below.

The table predates the StringBuild and Statics changes in "Causes" below. Those changes are not
measured yet: the benchmark script was not run on this branch. To measure them, run
`python3 scripts/benchmark.py StringBuild Statics` (the two changed cases), then
`python3 scripts/benchmark.py` for the full table and the check that nothing got slower.

A second run, taken earlier when the machine was less loaded, gave the same order of slowdowns
with different absolute numbers: BinaryTrees 7.82, Collatz 0.90, Interpreter 5.65, Mandelbrot
0.55, MatrixMultiply 58.40, NBody 0.65, Sieve 2.00, SortMillion 3.88, Statics 4.49, StringBuild
105.77. Collatz and NBody moved most, which is what load does to short runs.

## Causes of the slow cases

Read from the generated Rust under `target/benchmarks/<Name>/crate/src/main.rs`.

- **MatrixMultiply (85x; kernel 140x).** Element access `c[i][j]` becomes
  `java::get(&java::get(&c, i), j)`. The outer `get` clones the row's array handle (an `Rc`),
  and each access borrows its `RefCell` and checks the bounds. A multiply-add is therefore two
  borrows, two checks, and a refcount change, in the innermost loop.
- **StringBuild (31x).** Before the fix, `s.charAt(i)` became `java::char_at`, which calls
  `encode_utf16().nth(i)` (runtime `character.rs:27`), so each access walked the string. The loop
  condition `i < s.length()` called `java::string_length`, which is `encode_utf16().count()`
  (runtime `lib.rs:157`), so each iteration walked it again. Both are linear in the length, so the
  scan was quadratic. The string is built from `(char) (...)` casts, which the translator could
  not prove ASCII. The range analysis now records each `(char)` cast whose value stays in
  `0..=0x7f` (`analysis/ranges.rs`). A string made only of such casts is ASCII, so `charAt` is
  `s.as_bytes()[i] as u16` and `length()` is `s.len()`. A string that may hold a non-ASCII unit
  keeps the UTF-16 walk, which is still linear per access. Not measured yet.
- **Statics (3.2x).** Each static field was a `thread_local!` `RefCell`, reached through
  `with_borrow` and `with_borrow_mut`. `next()` reads and writes the state several times per
  call, so each access paid a thread-local lookup and a borrow. A primitive static (`int`,
  `long`, `char`, `boolean`, and the other primitives, never null) is now a `Cell`, read with
  `get` and written with `set`, which needs no borrow flag. Compound updates copy the value to a
  local and store it. Other statics stay `RefCell`. Not measured yet.
- **BinaryTrees (8x).** `count(node: Option<Rc<..>>)` clones the `left` and `right` handles on
  every call (a refcount increment and decrement each), and checks for `null` at every access.
  Allocation goes through `Rc`. Not profiled; these are read from the code.
- **Interpreter (1.9x; kernel 3.4x).** The stack and locals are plain `Vec<i32>`, which is good,
  but every access goes through `java::at` and `java::put` (runtime `slice.rs:13,18`), which
  check the index through the throwing path. Not profiled.
- **Sieve (1.6x).** `java::at` and `java::put` on a `Vec<bool>` for each element, with a checked
  index each time. Not profiled.
- **SortMillion (1.6x; kernel 2.0x).** The sort helpers work over `java::Array`, so each element
  access goes through the same checked borrow path. Not inspected in detail.
Mandelbrot (0.36x, kernel 0.98x), Collatz (0.35x) and NBody (1.13x, kernel 1.28x) are at or
faster than Java and need no change.

## NodeList: does not compile (translator bug)

`benchmarks/NodeList.java` walks a linked list with `current = current.next` inside a loop. The
translator emits

    current = java::non_null(current.as_ref()).borrow().next.clone();

The `Ref` temporary from `borrow()` lives until the end of the statement, so the assignment to
`current` happens while `current` is still borrowed. rustc rejects it with E0506 ("cannot assign
to `current` because it is borrowed"). The same shape appears in the loop of `sum`. The fix
belongs in the codegen that emits assignments from a field read through a `RefCell`: bind the
`clone` to a local before the assignment. Nothing in `crates/` was changed for this benchmark.

Because it does not compile, the script reports NodeList as "does not compile" and exits with
status 1.

## Deviations from the plan

- MatrixMultiply (Java about 0.85 s here, 0.2 s quieter) and StringBuild (Java about 0.23 s) are
  under the 1 to 5 second range asked for. Their Rust sides are super-linear in the input (the
  matrix work, and the quadratic `charAt` and `length`), so larger sizes would make the script
  take many minutes per run. They are kept small on purpose.
- The Java times are whole-process, so programs that are short on Java include JVM start-up.
  Kernel columns exist only where a program has a warm-up loop.
- The table is one run on a loaded machine. Re-run it on an idle machine before drawing
  conclusions about small differences; the large ratios (MatrixMultiply, StringBuild, BinaryTrees)
  are far outside the noise.
