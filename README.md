# java-to-rust

A tool that converts Java source code into Rust source code, without LLMs: a compiler-style
pipeline of parsing, type resolution, program analysis and code generation.

The goal is Rust that behaves exactly like the original program and, wherever the tool can
prove it safe, is written the way a person would write Rust: owned values instead of shared
objects, enums instead of class hierarchies, no null checks where nothing can be null.

## How it works

```
Java source -> typed syntax tree -> intermediate representation -> analyses
            -> facts about the program -> Rust code generator -> Rust crate
```

javac parses and attributes the source (`crates/translator/java/JavaTree.java` prints its tree
as JSON); `lower.rs` maps the tree onto the supported subset; the analyses prove facts and store
them in a table keyed by expression id (`analysis.rs`); and the generator (`codegen.rs`) writes
the plain form of a construct only where a fact allows it, and the faithful form otherwise. A
better analysis therefore improves the output without touching the generator.

Every value starts at a safe default: an object is `Arc<RefCell<C>>`, a reference that may be
null an `Option`, an array a shared `java::Array<T>`, arithmetic wrapping. Analyses move values
to plainer forms (`Arc<C>`, plain structs, `Vec`, plain `+`) where they prove it changes nothing.
The runtime crate (`crates/runtime`) supplies what Rust lacks: Java's number semantics and
printing, strings and builders, `String.format`, regular expressions, charsets, threads and
monitors, the native methods of the JDK, and the garbage collector that frees objects.

Two modes: a **single file** uses only the program's classes and what the runtime supplies; a
**whole program** is translated together with the JDK's own sources (`java.base`), so
collections, streams and the rest of the library come from the JDK's code. What each mode
accepts is in [docs/supported-java.md](docs/supported-java.md).

## Build and run

Building the translator and running its tests need `javac` and `java` (JDK 21 or newer) on PATH.
Translation never runs the program; it only compiles it with javac, which takes a few seconds.

```
cargo run -p java-to-rust -- Program.java                 # print the Rust translation
cargo run -p java-to-rust -- Program.java --crate out/    # write a Cargo project
cargo run -p java-to-rust -- --main com.example.Main --source src/ [--library jdk-src/java.base] [-o out.rs | --crate out/]
cargo run -p java-to-rust -- --census --main Main --source src/ --library jdk-src/java.base
cargo run -p java-to-rust -- --survey --main Main --source src/ --library jdk-src/java.base
```

- `--crate DIR` writes a Cargo project with a file per package; `-o FILE` writes one file. With
  `--crates N`, `--crate DIR` writes a workspace of a header crate, `N` body crates and the binary
  instead, for programs too large to build as one crate ([docs/several-crates.md](docs/several-crates.md)).
- `--census` translates past every rejection and reports what is rejected and why;
  `--census-json FILE` writes it as JSON. `--survey` (and `--survey-json FILE`) is a quicker
  syntactic view of what the reachable code needs.
- `--modules-json FILE` writes the module tree and what each item names
  ([docs/several-crates.md](docs/several-crates.md)).
- `--keep-going` lets javac go on past compile errors: code that names a class missing from the
  roots throws `NoClassDefFoundError` as on the JVM, and other code with an error is rejected
  ([docs/exceptions.md](docs/exceptions.md)). `--timings` prints the time of each phase.

## Testing

Every test program runs on Java and, translated, as Rust; printed output and exit code must match
exactly. The Java run is the expected result, so no expectations are written by hand.

- `scripts/check.sh` is the check every branch passes: formatting, clippy with warnings as
  errors, and the tests: the unit tests in a debug build, the others built optimized with debug
  assertions and overflow checks (`cargo test --profile checks`). With `JAVA_BASE_SOURCES` set
  it also checks that the translator's Java holds no copy of the JDK's
  ([docs/jdk-overlays.md](docs/jdk-overlays.md)).
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
  [targets/README.md](targets/README.md) records what rejects each one.
- `tools/fuzz` generates random programs from a seed, compares them with Java (`run.py`) and
  shrinks a failure to a minimal case (`reduce.py`). A minimal case that still fails is kept in
  `known-failures/` until it is fixed.
- `crates/translator/tests/translation.rs` and its neighbours pin down what the output looks
  like.

### How the checks are cached

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
wall-clock runs, JVM start-up included. [docs/performance.md](docs/performance.md) has the
results and the causes of the slow cases. `scripts/clippy-corpus.sh` counts the clippy warnings
in the generated code of the corpus.

## Other scripts

- `scripts/jdk-coverage.py` runs the census over the target programs and rewrites
  [docs/jdk-coverage.md](docs/jdk-coverage.md).
- `scripts/minecraft-server-survey.sh` prepares and surveys the Minecraft server
  ([docs/minecraft-server.md](docs/minecraft-server.md)); `scripts/census-by-origin.py` splits a
  census by source root.
- `scripts/crate-groups.py` finds the groups of a translation that a split into crates must keep
  together.
- `scripts/check-no-personal.sh` checks that no committed file or commit carries personal data.

## Documents

What the translator does:

- [docs/supported-java.md](docs/supported-java.md): per area, what translates, the Rust it
  becomes, and what is still rejected.
- [docs/architecture.md](docs/architecture.md): how the translator is built, the pipeline and the
  size of each stage.
- [docs/object-model.md](docs/object-model.md): class trees, interfaces, `Object`, generics,
  `instanceof` and casts.
- [docs/closures-and-nested-classes.md](docs/closures-and-nested-classes.md): captures, inner,
  local and anonymous classes, lambdas, method references, for-each over `Iterable`.
- [docs/exceptions.md](docs/exceptions.md): exceptions as values, `try`/`catch`/`finally`, stack
  traces and `NullPointerException` messages.
- [docs/collector.md](docs/collector.md): the garbage collector that frees objects, and handles
  without counts.
- [docs/multi-threading.md](docs/multi-threading.md): real threads, shared objects behind
  `java::Locked`, monitors.
- [docs/natives-and-input.md](docs/natives-and-input.md): the hand-written native layer and the
  standard streams.
- [docs/jdk-overlays.md](docs/jdk-overlays.md): the translator's own Java files that replace or
  patch the JDK's.
- [docs/reflection-and-plugins.md](docs/reflection-and-plugins.md): why the translation is
  closed-world, and how code reached by name still runs.
- [docs/jdk-coverage.md](docs/jdk-coverage.md): how much of the JDK code the target programs
  reach translates (generated).
- [docs/performance.md](docs/performance.md): benchmark results and the causes of slow cases.

Plans and larger goals:

- [docs/native-speed.md](docs/native-speed.md): the plan for a collected object model and
  whole-program optimizations that make translated programs faster than on the JVM.
- [docs/thorough-mode.md](docs/thorough-mode.md): a slower whole-program mode with points-to
  analysis, ownership checked by rustc and typed errors.
- [docs/natural-rust.md](docs/natural-rust.md): output that reads like hand-written Rust, and a
  later port mode.
- [docs/lists-and-streams.md](docs/lists-and-streams.md): `ArrayList` as a `Vec` and sequential
  streams as iterators.
- [docs/translator-speed-and-memory.md](docs/translator-speed-and-memory.md): the translator's
  own speed and memory, and how to cut both.
- [docs/large-projects.md](docs/large-projects.md): what large multi-file projects need.
- [docs/several-crates.md](docs/several-crates.md): splitting a translation into several crates.
- [docs/minecraft-server.md](docs/minecraft-server.md): the Minecraft server as a target.
