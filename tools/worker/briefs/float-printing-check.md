# Check float and double printing against Java exhaustively

Java prints `double` and `float` with its own algorithm (`Double.toString`: shortest digits
that round-trip, Java's switch between plain and `E` notation, `1.0E10`, `-0.0`, `NaN`,
`Infinity`). The runtime reproduces it (`crates/runtime/src/float.rs` and wherever doubles are
formatted: printing, string concatenation, `String.valueOf`). Prove it matches, and fix it where
it does not.

## Scope
- Build a differential check outside the test suite, e.g. `tools/float-check/`: a Java program
  that prints `Double.toString` and `Float.toString` for a list of values given as raw bits, and
  a Rust program (a small crate depending on `crates/runtime` by path) that formats the same
  bits through the runtime; a script runs both and diffs. Inputs: every float near powers of
  ten from 1e-45 to 1e38 (the values next to each power, ±1, ±2 ulps), subnormals, the
  plain/E-notation boundaries (1e-3, 1e7 and their neighbours), all floats with few significant
  digits, integers up to 2^53 and their neighbours, and several million random bit patterns
  for both float and double. A full sweep of all 2^32 float bit patterns is welcome if it runs
  in reasonable time (stream through stdin/stdout, parallelise).
- Also check the paths that go through translation: string concatenation of a double and a
  float, `System.out.println(float)`, `(float) d` printed, `Float.toString`, `Double.toString`,
  `String.valueOf(double)` — with a corpus program covering a few hundred tricky values.
- Fix every mismatch in the runtime (float.rs or the formatting it uses), with a unit test per
  fix. Do not touch translator code other than what printing a float needs, and nothing in
  object, nullness, borrowing, String/StringBuilder code (other agents are changing it).
- Document in the tool's README what was checked and how long it takes.

## Deliverables
- Differential float/double printing check tool with a README
- Edge values, boundaries and millions of random values compared for float and double
- Every mismatch fixed in the runtime with unit tests
- Corpus program corpus/features/FloatingPrintingSweep.java
