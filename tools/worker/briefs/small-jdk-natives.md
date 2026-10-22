# Small JDK methods and natives that whole programs reach

Whole-program translations of `targets/ScannerGrades.java` and javac reject a few small JDK
methods and natives. Each is a small, self-contained addition. First read `lower/natives.rs`,
`ast/natives.rs`, `codegen/natives.rs` and the runtime's natives, and look at how an existing
native (for example `Native::NanoTimeAdjustment` or `Native::Crc32Update`) goes from recognition
to runtime function to test.

## Scope
- `Math.scalb(double, int)` and `Math.scalb(float, int)` (and `StrictMath`): Java's exact
  semantics (`d * 2^n` with correct rounding, overflow to infinity, underflow to zero, NaN and
  infinities unchanged). Rust's `f64::powi` is NOT exact for large `n`; build the result from the
  exponent bits or by repeated scaling as the JDK's own `Math.scalb` source does (read it in the
  JDK sources under `$JAVA_BASE_SOURCES/java/lang/Math.java`, where JAVA_BASE_SOURCES is given
  below). If the JDK's Java code for `scalb` translates as is, that is the preferred route:
  check first why it is rejected ("method `java.lang.Math.scalb` not supported yet").
- `jdk.internal.loader.BootLoader.loadLibrary(String)`: a translated program has no native
  libraries to load; it does nothing.
- `java.security.SecureRandom.getSeed(int)` / the seed generator: bytes from the operating
  system's random source (`/dev/urandom`, or the `getrandom` system call), as Java's default
  seed generator on Linux does.
- `java.lang.ref.Reference.hasReferencePendingList()`: always false (the translation has no
  garbage collector discovering references).

For each, find the exact place it is rejected by translating a small whole program that uses it
(see "How to test"), and make the rejection go away with correct behaviour. Anything you cannot
do correctly stays rejected; say so in your report rather than guessing.

## Deliverables
- Math.scalb (double and float) exact, with tests of edge cases (overflow, underflow, NaN, subnormals) compared with Java
- BootLoader.loadLibrary does nothing
- SecureRandom.getSeed from the OS random source
- Reference.hasReferencePendingList is false
- One corpus program corpus/features/ScalbAndSeeds.java (prints scalb results and the LENGTH of a seed, never the seed)

## How to test
- Build and test through the build-slot limiter, which keeps the machine from running out of
  memory: prefix every cargo command with
  `<scratchpad>/build-slot.sh`,
  for example `.../build-slot.sh cargo test -p java-to-rust --test corpus`.
  `CORPUS_FILTER=scalb` runs only programs whose lowercase name contains `scalb`.
- Whole programs: `JAVA_BASE_SOURCES=<scratchpad>/jdk-src/java.base`
  with `cargo run -q -p java-to-rust -- --main Name --source DIR --library $JAVA_BASE_SOURCES --crate OUT`
  (DIR holds only your program), through the limiter.
- Run `scripts/check.sh` once at the end, through the limiter.
- Do not touch Unsafe, VarHandle, Charset, file system, reflection or ClassLoader code: other
  agents are changing it now.
