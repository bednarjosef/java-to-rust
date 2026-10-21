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
- Load: the machine was busy during every run, from other agents' JVMs and builds: load
  averages between 27 and 74 on 16 cores. Times of the same binary moved by a factor of two
  between runs (the SortMillion kernel of one binary: 995 to 1337 ms in four runs in a row), and
  the Java side moved more than the Rust side, so read small ratios as rough. The large changes
  below are far outside that noise.

## Results

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

## Causes of the slow cases

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

## NodeList: stack overflow when the list is freed (fixed on `iterative-drop`)

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

BinaryTrees frees about 300 million nodes, so it is the worst case: every node runs the `Drop`.
In the last version that is eight instructions and one call more than Rust's own drop of a
node (a thread-local load, a compare, and a call into the node's drop glue that is no longer
inlined into `Rc::drop_slow`), about 1.5 ns per node, 6 to 8 percent of its run time across
three runs at loads 5 to 15. NodeList is unchanged within the noise: its chain is freed on the
heap stack, which costs no more than recursion. Programs without a cycle of field types are
not touched.

## Deviations from the plan

- MatrixMultiply (Java about 0.85 s here, 0.2 s quieter) and StringBuild (Java about 0.23 s) are
  under the 1 to 5 second range asked for. Their Rust sides are super-linear in the input (the
  matrix work, and the quadratic `charAt` and `length`), so larger sizes would make the script
  take many minutes per run. They are kept small on purpose.
- The Java times are whole-process, so programs that are short on Java include JVM start-up.
  Kernel columns exist only where a program has a warm-up loop.
- The tables are single runs on a loaded machine. Re-run them on an idle machine before drawing
  conclusions about small differences; the large changes (MatrixMultiply, Sieve, SortMillion)
  are far outside the noise.
