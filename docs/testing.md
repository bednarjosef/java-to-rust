# Testing and benchmarks


Every test program runs on Java and, translated, as Rust; printed output and exit code must match
exactly. The Java run is the expected result, so no expectations are written by hand.

- `scripts/check.sh` is the check every branch passes: formatting, clippy with warnings as
  errors, and the tests: the unit tests in a debug build, the others built optimized with debug
  assertions and overflow checks (`cargo test --profile checks`). With `JAVA_BASE_SOURCES` set
  it also checks that the translator's Java holds no copy of the JDK's
  ([docs/jdk-overlays.md](jdk-overlays.md)).
- `corpus/` holds the test programs: `features` (one or more per language feature), `programs`
  (larger realistic programs), `threads`, and `projects` (multi-file programs, each with a source
  root `src/` and a file `main` naming the main class, and optionally a library root `lib/` and
  a root `optional/` of classes that Java compiles against but runs without, which the
  translator never sees). A file `Name.args` gives a program its
  command-line arguments and `Name.input` its standard input. The corpus test
  (`cargo test --test corpus`) builds the translations with overflow checks on, so a wrong
  "cannot overflow" fact panics. `CORPUS_FILTER=text` runs only the programs whose name contains
  `text`, and `CORPUS_THREADS=n` limits the threads (one per CPU by default).
- `corpus/whole-program/` holds programs that use the JDK's own code, translated whole by
  `cargo test --test whole_program`. The JDK's sources cannot be in the repository, so the test
  runs only when `JAVA_BASE_SOURCES` names the source root of `java.base`, and otherwise says it
  skipped. The tests of other JDK modules (`jdk.unsupported`, `jdk.net`, `java.logging`) look
  for their sources next to `java.base`'s, as the JDK's `lib/src.zip` unpacks them, and skip
  without them.
- `targets/` holds typical Java programs that do not translate yet; `targets/run.sh` runs them
  one by one, or whole with a JDK root as its first argument, and
  [targets/README.md](../targets/README.md) records what rejects each one.
- `tools/fuzz` generates random programs from a seed, compares them with Java (`run.py`) and
  shrinks a failure to a minimal case (`reduce.py`). A minimal case that still fails is kept in
  `known-failures/` until it is fixed.
- `crates/translator/tests/translation.rs` and its neighbours pin down what the output looks
  like.

## How the checks are cached

Much of the checks' work gives the same result until its inputs change, so the corpus test, the
whole-program test and `targets/run.sh` keep results in `~/.cache/java-to-rust-checks`
(`CHECK_CACHE` names another directory), shared by every worktree and branch:

- `java/`, `whole-program-java/`: Java's output and exit code for each program, under a hash of
  its sources, main class, arguments, input, JVM arguments and `java -version`.
- `javac/`: javac's tree of a single source file, under a hash of the source, the translator's
  Java helpers and the Java version. Used only when `CHECK_CACHE` is set, as `scripts/check.sh`
  sets it.
- `rust/`, `whole-program-rust/`, `targets/`: the output of each translated program whose run
  matched, under a hash of everything that decides its binary and its run (generated source,
  runtime crate, toolchain, cargo configuration and variables, arguments, input). A hit skips
  building and running; the stored output is still compared with Java's. A failing run is never
  stored.

Every comparison is still made against real Java output; a missing or unreadable entry just
means doing the work. An entry is never stale, only unused, so the directory can be deleted at
any time. `CHECK_FRESH=1` (for `scripts/check.sh`, `cargo test` and `targets/run.sh`) does all
the work again and stores the results anew; `CORPUS_JAVA_FRESH=1` reruns only the Java side.

## Performance

`benchmarks/` holds small programs (sieve, n-body, Mandelbrot, sorting, binary trees, string
building, an interpreter, ...). `python3 scripts/benchmark.py [--runs N] [Program ...]` checks
that each translation prints what Java prints, then times both sides as the median of N
wall-clock runs, JVM start-up included. [performance.md](performance.md) has the
results and the causes of the slow cases. `python3 scripts/performance-charts.py` draws the
README's charts into `docs/images/` from the numbers in those tables, copied into the script. `scripts/clippy-corpus.sh` counts the clippy warnings
in the generated code of the corpus.

## Other scripts

- `scripts/jdk-coverage.py` runs the census over the target programs and rewrites
  [docs/jdk-coverage.md](jdk-coverage.md).
- `scripts/minecraft-server-survey.sh` prepares and surveys the Minecraft server
  ([docs/minecraft-server.md](minecraft-server.md)); `scripts/census-by-origin.py` splits a
  census by source root.
- `scripts/crate-groups.py` finds the groups of a translation that a split into crates must keep
  together.
- `scripts/check-no-personal.sh` checks that no committed file or commit carries personal data.

