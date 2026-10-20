# Benchmarks: Java against the translation

Measure how fast translated programs run compared with Java, so that slow translations show up.
This brief only adds a script, benchmark programs and a document; do not edit anything under
`crates/` (other agents are changing the translator now).

## Scope
- `scripts/benchmark.sh` (or a Python script under `scripts/`, run with `python3`): for each
  program in `corpus/benchmarks/`, compile it with `javac` and translate it with the translator
  (`--crate DIR`), build the Rust in release mode, check that both print the same output, then
  time each several times (median of 5 runs, wall clock; Java with default flags, including JVM
  start-up, and also with a warm-up loop inside the program when it has one) and print a table:
  program, Java ms, Rust ms, ratio. Build directories go under `target/benchmarks/` (gitignored
  already as part of `target/`).
- `corpus/benchmarks/`: 8 to 12 programs that each run 1 to 5 seconds on Java and stay in the
  supported subset (README): integer loops (sieve, collatz), floating point (n-body,
  mandelbrot), arrays (matrix multiply, sorting a million ints), objects (a binary tree built
  and walked, a hand-written linked list), strings (building a long string with concatenation,
  charAt scans), statics, switch-heavy code (a bytecode interpreter loop). Print a checksum so
  the work cannot be optimised away. Note: `corpus.rs` runs every directory under `corpus/`
  except `projects/` as tests; make sure it does not run `corpus/benchmarks/` (they are slow).
  The cleanest way is to put them outside `corpus/`, e.g. `benchmarks/` at the repository root;
  do that instead of editing the test.
- `docs/performance.md`: the table from one run on this machine (say which CPU and JDK), and for
  every program where Rust is slower than Java or more than about 1.5 times slower than you would
  expect, look at the generated Rust and explain the cause in a sentence or two (e.g. RefCell
  borrows in a hot loop, bounds checks, `wrapping_*` calls, string copying), so the translator
  can be improved later.

## Deliverables
- Benchmark script that checks outputs match and prints the timing table
- 8 to 12 benchmark programs outside corpus/
- docs/performance.md with results and causes of slow translations
- README section pointing to the script and the document
