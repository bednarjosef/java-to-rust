# AGENTS.md

For coding agents that use java-to-rust on someone's Java code, or work on java-to-rust itself.
The README is the overview; the details are in [docs/](docs/), linked below.

## What it is

java-to-rust is a compiler from Java source to Rust source. It is an ordinary program: it uses
no language model, and you do not write the Rust. One command reads the Java and writes a Cargo
project whose program prints the same output and exits with the same code as the Java program.
Your job as an agent is to run it, read what it reports, and act on that, not to translate or
patch the Rust by hand.

It never writes Rust it knows to be wrong. What it cannot translate exactly, it rejects at
translation time, with an error that names the construct and its file, line and column.

## Setting it up

You need a Rust toolchain (edition 2024) and a JDK, 21 or newer, with `javac` and `java` on
`PATH`. Whole-program translation reads the JDK's own sources; the translator's patches of them
are made against JDK 26, and other versions are untested.

```bash
git clone https://github.com/bednarjosef/java-to-rust
cd java-to-rust
cargo install --path crates/translator   # puts `java-to-rust` on PATH
```

Keep the checkout where it is: translated projects build against its `crates/runtime` by path
(or the directory `JAVA_TO_RUST_RUNTIME` names). The translator unpacks its own files into
`~/.cache/java-to-rust` on first use.

## Translating a program

Pick the mode that fits the program:

| The program | Command |
|---|---|
| one file that uses only `String`, `StringBuilder`, `Math`, `System`, the boxes, the standard exceptions and `Thread` | `java-to-rust Program.java --crate out/` |
| anything that uses the JDK's library (collections, streams, `java.io`, `java.nio`, ...) | `java-to-rust --main com.example.Main --source src/ --jdk --crate out/` |
| a Maven or Gradle project | `java-to-rust --project my-app/ --crate out/` |

Then build and run the result, and compare it with Java:

```bash
cd out && cargo run --release
```

Useful flags (all are in [docs/usage.md](docs/usage.md)):

- `--library DIR` adds the sources of a library the program uses. A library is translated from
  its sources, never from its jar.
- `--census` translates past every rejection: rejected code becomes a stub that ends the program
  if it runs, and the report lists every rejection, grouped by message. Run it first on anything
  large, to see in one pass what stands in the way.
- `--approximate-messages` accepts exception messages that may differ from the JVM's. Libraries
  often need it.
- `--keep-going` goes on past code that javac cannot compile against the patched JDK, or that
  names classes missing from the sources; such code throws `NoClassDefFoundError` if it runs.
- `--crates N` splits a very large program into `N` crates that build in parallel.

A typical session on an unfamiliar project: run with `--census`, read the rejections, tell the
user which are in their own code and which in libraries, then decide with them whether to change
the Java, accept `--approximate-messages`, or stop.

## What it does not do

The full list, per area, is in [docs/supported-java.md](docs/supported-java.md). The ones agents
run into most:

- In single-file mode, any library class the runtime does not supply (`List`, `Scanner`, ...).
  Translate the program whole instead.
- Most of reflection beyond what the program's code names, method handles, Java serialization,
  loading classes at run time, and the security manager
  ([docs/reflection-and-plugins.md](docs/reflection-and-plugins.md)).
- Native methods of the program's own libraries (JNI). The JDK's natives that the translator
  supports are listed in [docs/natives-and-input.md](docs/natives-and-input.md).
- Making an object without running a constructor (`Unsafe.allocateInstance`), so gson cannot
  read classes that have no constructor without parameters.
- Libraries without a sources jar: code that uses them is rejected.
- Exception messages that the translator cannot reproduce exactly are rejected unless
  `--approximate-messages` is given.
- A translated program reads its resource files from the same directories as the Java program,
  at run time, so it runs where those are.

Large programs need a large machine: the Minecraft server translates in about 6 minutes and
builds in about 36 on 16 cores, with rustc at 5 to 7 GB of memory for each of its crates
([docs/several-crates.md](docs/several-crates.md)).

Do not edit the generated Rust to fix a failure: the next translation overwrites it. Change the
Java, the flags, or the translator.

## How it works

javac parses and type-checks the Java; reachability keeps only what `main` can reach, the JDK's
code included; lowering turns that into one typed tree; a few dozen analyses prove facts about
it (which objects are never shared, which values are never null, which arithmetic cannot
overflow); and the generator writes Rust from those facts, plain where they prove it safe and a
faithful form of Java's behaviour everywhere else. See [docs/architecture.md](docs/architecture.md).

## Working on java-to-rust

- `crates/translator` is the translator; `crates/runtime` is the library that translated
  programs use; `crates/translator/java` holds its Java helpers and its overlays and patches of
  the JDK ([docs/jdk-overlays.md](docs/jdk-overlays.md)).
- `scripts/check.sh` must pass before a change is done: formatting, clippy with warnings as
  errors, and the tests. It takes a while; while working, run the tests you touch, such as
  `CORPUS_FILTER=Name cargo test --test corpus`.
- Every fix or feature comes with a test program in `corpus/` (see
  [docs/testing.md](docs/testing.md)). Its expected output is what Java prints; never write an
  expectation by hand. Whole-program tests run only with `JAVA_BASE_SOURCES` set to the JDK's
  `java.base` sources.
- No copies of the JDK's code go into the repository: an overlay replaces a JDK class with new
  code, and a patch holds only what differs from the JDK's file.
- Files under `crates/runtime` and the Java listed in
  [crates/translator/java/LICENSE.md](crates/translator/java/LICENSE.md) become part of
  translated programs and are MIT OR Apache-2.0; the rest is GPL-3.0-or-later.
- `scripts/check-no-personal.sh` must pass: no home directory paths and no email addresses in
  committed files.
- A commit message says what changed and why.

### Documentation

- Docs describe the current state: what the translator does now, how, and why. When a change
  alters something a doc describes, edit that section in place. Never append a dated note, a
  change log, or a "this now also handles…" paragraph; the history of a change belongs in its
  commit message.
- When a change makes a sentence in a doc false, fix or delete that sentence in the same commit.
- `README.md` stays short: what the project is, how to build, run and test it, and links into
  `docs/`. Feature lists and design detail belong in `docs/`.
- Plans for work not done yet stay in the doc they belong to, marked as plans. Once the work is
  done, the plan is replaced by a description of what was built, not kept alongside it.
- Briefs in `tools/worker/briefs/` are deleted once their branch has merged.
