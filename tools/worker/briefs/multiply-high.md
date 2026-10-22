# Math.multiplyHigh and its relatives

Whole-program translations of `targets/ScannerGrades.java` reach `Math.multiplyHigh(long, long)`
(the high 64 bits of the 128-bit product). Add it and its relatives, which the JDK's number
formatting and parsing code also uses.

First read how an existing `Math` method that the translator writes by hand is recognised and
generated (for example `Math.floorMod`, `Math.multiplyExact` or `Math.fma`): search
`crates/translator/src` for it, and find its runtime function and tests if it has one.

## Scope
- `Math.multiplyHigh(long x, long y)`: `((x as i128 * y as i128) >> 64) as i64`.
- `Math.unsignedMultiplyHigh(long x, long y)`: the same on the unsigned values.
- `Math.multiplyFull(int x, int y)`: the exact `long` product.
- The same methods on `StrictMath` where Java has them.
- If `Long.compress`/`expand` or `Math.ceilDiv`/`ceilMod`/`ceilDivExact` are rejected and you meet
  them on the way, they may be added in the same way; list what you add.

Each must behave exactly as Java. Anything you cannot do correctly stays rejected; say so in your
report rather than guessing.

## Deliverables
- multiplyHigh, unsignedMultiplyHigh and multiplyFull (Math and StrictMath)
- One corpus program corpus/features/MultiplyHigh.java printing them for edge values (0, 1, -1,
  Long.MIN_VALUE, Long.MAX_VALUE, Integer.MIN_VALUE and a few ordinary values)

## How to test
- Build and test through the build-slot limiter, which keeps the machine from running out of
  memory: prefix every cargo command with
  `<scratchpad>/build-slot.sh`,
  for example `.../build-slot.sh cargo test -p java-to-rust --test corpus`.
  `CORPUS_FILTER=multiplyhigh` runs only programs whose lowercase name contains it.
- Test only what you touch while working; run `scripts/check.sh` once at the end, through the
  limiter.
- Do not touch Unsafe, VarHandle, Charset, file system, reflection, ClassLoader, Appendable,
  enum or analysis code: other agents are changing it now.
