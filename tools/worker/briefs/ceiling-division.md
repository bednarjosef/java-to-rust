# Math.ceilDiv, ceilMod and ceilDivExact

`Math.floorDiv` and `Math.floorMod` translate; their ceiling counterparts don't. Add
`Math.ceilDiv`, `Math.ceilMod` and `Math.ceilDivExact` for `int` and `long` (and the mixed
`(long, int)` forms Java has), on `Math` and `StrictMath`.

First read how `Math.floorDiv`, `Math.floorMod` and `Math.multiplyHigh` (added tonight: see
`git log --oneline -5 -- crates/translator/src/codegen/math.rs`) are recognised and generated,
where their runtime functions and tests are, and how a call that can throw
`ArithmeticException` is declared to the exception analysis: search
`crates/translator/src/analysis/exceptions/failures.rs` and `crates/translator/src/lower/exceptions.rs`
for `floorDiv` or `ArithmeticException`.

## Scope
- `ceilDiv(x, y)`: the smallest integer not below the exact quotient; division by zero throws
  `ArithmeticException` with Java's message ("/ by zero"); `ceilDiv(MIN_VALUE, -1)` is
  `MIN_VALUE`, as Java's.
- `ceilMod(x, y)`: `x - ceilDiv(x, y) * y`; zero divisor as above.
- `ceilDivExact(x, y)`: like ceilDiv but `MIN_VALUE / -1` throws `ArithmeticException` with Java's
  message (read the JDK's `Math.java` under `$JAVA_BASE_SOURCES`, path below, for the exact text).
- You may add the entries these need in `failures.rs` and `lower/exceptions.rs` (the
  `ArithmeticException` they can throw), next to `floorDiv`'s.

Each must behave exactly as Java, including caught exceptions. Anything you cannot do correctly
stays rejected; say so in your report rather than guessing.

## Deliverables
- ceilDiv, ceilMod, ceilDivExact for int, long and the mixed forms, with their exceptions
- One corpus program corpus/features/CeilingDivision.java printing results for signs, zero
  remainders and MIN_VALUE cases, and catching each ArithmeticException and printing its message

## How to test
- Build and test through the build-slot limiter, which keeps the machine from running out of
  memory: prefix every cargo command with
  `<scratchpad>/build-slot.sh`,
  for example `.../build-slot.sh cargo test -p java-to-rust --test corpus`.
  `CORPUS_FILTER=ceilingdivision` runs only programs whose lowercase name contains it.
- `JAVA_BASE_SOURCES=<scratchpad>/jdk-src/java.base`
- Test only what you touch while working; run `scripts/check.sh` once at the end, through the
  limiter.
- Do not touch Unsafe, VarHandle, Charset, file system, reflection, ClassLoader, Appendable or
  enum code, and change nothing in the analyses beyond the entries named above: other agents are
  changing them now.
