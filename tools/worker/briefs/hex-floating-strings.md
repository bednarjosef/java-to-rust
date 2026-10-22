# Double.toHexString and Float.toHexString

Whole-program translations of `targets/ScannerGrades.java` reach `Double.toHexString(double)`.
`Double` and `Float` are runtime classes in the translation (hand-written, not translated from
the JDK), so the method needs a runtime version, and the translator must recognise the call.

First read how an existing static method of `Double` or `Float` that the runtime implements
(for example `Double.toString`, `Double.doubleToRawLongBits` or `Float.floatToIntBits`) is
recognised by the translator and generated, and where its runtime function and tests are. Then
read the specification of `Double.toHexString` and `Float.toHexString` in the JDK sources:
`$JAVA_BASE_SOURCES/java/lang/Double.java` and `Float.java` (path below).

## Scope
- `Double.toHexString(double)`: exactly Java's text. NaN, `Infinity`, `-Infinity`, `0x0.0p0`,
  `-0x0.0p0`; normal values as `0x1.<hex digits>p<exponent>` with trailing zero digits removed
  but at least one digit; subnormals as `0x0.<hex digits>p-1022`.
- `Float.toHexString(float)`: Java's text, which for subnormals uses `p-126` and is built from the
  float's value as a double (read the JDK's code: it is not a plain reuse of the double version).
- If `Double.toHexString`'s inverse, parsing a hex floating literal with `Double.parseDouble`
  or `Double.valueOf`, is already supported, leave it; if it is rejected and small, add it with
  Java's exact rounding, otherwise list it in your report.

Each must behave exactly as Java. Anything you cannot do correctly stays rejected; say so in your
report rather than guessing.

## Deliverables
- Double.toHexString exact, with runtime unit tests for zero, negative zero, NaN, infinities,
  1.0, MIN_VALUE, MIN_NORMAL, MAX_VALUE and a few ordinary values, compared with what java prints
- Float.toHexString exact, with the same kinds of tests
- One corpus program corpus/features/HexFloatingStrings.java printing those values

## How to test
- Build and test through the build-slot limiter, which keeps the machine from running out of
  memory: prefix every cargo command with
  `<scratchpad>/build-slot.sh`,
  for example `.../build-slot.sh cargo test -p java-runtime`.
  `CORPUS_FILTER=hexfloating` runs only programs whose lowercase name contains it.
- `JAVA_BASE_SOURCES=<scratchpad>/jdk-src/java.base`
- Test only what you touch while working; run `scripts/check.sh` once at the end, through the
  limiter.
- Do not touch Unsafe, VarHandle, Charset, file system, reflection, ClassLoader, Appendable or
  enum code: other agents are changing it now.
