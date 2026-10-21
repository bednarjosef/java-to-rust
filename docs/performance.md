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

- **Statics (+68%) is a real slowdown.** The generated Rust is the same as the earlier
  translator's (`diff` shows no change). In back-to-back runs of the two translators, the median
  went from 1550 to 2090 ms, so the cause is in the runtime. A copy of the runtime with the old
  `throw` body recovers about 170 ms: `throw` now calls `exception::die`. The other 370 ms are in
  the new `array.rs` (`post_update` and `set`), which was not isolated further.
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

NodeList does not compile. See below.

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

## NodeList: compiles, overflows the stack (2026-10-21)

Earlier, `benchmarks/NodeList.java` did not compile: the translator emitted
`current = java::non_null(current.as_ref()).borrow().next.clone();`, the `Ref` from `borrow()`
lived until the end of the statement, and rustc rejected the assignment with E0506. The
translator now wraps that read in a block, so the `Ref` is dropped first, and the program
compiles.

It still does not produce the right result. Java prints `checksum 8650087554848663653` and exits
0. The Rust binary prints nothing and aborts with SIGABRT (exit -6) on the default 8 MB main
thread stack. With the stack limit raised (`ulimit -s unlimited`) it prints the same checksum as
Java, so the arithmetic agrees.

The cause is the drop of the list. Each round ends by dropping a list of about 667k nodes, each
an `Rc<RefCell<NodeListNode>>` whose `next` field holds the next node. Rust's automatic drop of
`NodeListNode` drops `next` inside the drop of the node, so freeing the head recurses once per
node and overflows the stack. Java's garbage collector has no such recursion. The fix belongs in the
translator, not in this benchmark: either an iterative `Drop` for the node that walks the chain
while each node is uniquely owned (`Rc::strong_count` is 1), or the same unlinking in a loop
before the list goes out of scope. Nothing in `crates/` was changed for this benchmark.

The script reports NodeList as "output differs from Java (exit -6, Java exit 0)" and exits with
status 1.

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
