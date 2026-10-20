# Known failures

Minimal programs that the translator gets wrong, though the feature is in the supported subset
(see the "Current state" section of the README). Each one is kept out of `corpus/`, which must
pass. Java's output is what `java File.java` prints. When a failure is fixed, move its file into
`corpus/features/` with the fixed behaviour.

- `BlockExpressionBeforeCast.java`: `return (sChar++);` in a `double` method, where `sChar` is a
  `char` static. Java prints `65.0`; the translated block is taken as a statement, so the `as f64`
  after it does not parse: rustc fails with "expected expression, found `as`". The fuzzer's
  seeds hit the same parse error after a block with `>=`, `==`, `!=`, `/`, `%`, `^`, `>` and `+`
  (not reduced separately).
- `FloatIncrementValue.java`: `System.out.println(++sFloat);` on a `float` static. Java prints
  `2.5`; rustc fails with E0599 "no method named `wrapping_add` found for type `f32`" (the
  increment is translated with integer wrapping arithmetic, which `f32` lacks).
- `NegatedCharCast.java`: `sInt *= ((char)((-(3))));`, a negative int cast to `char`. Java prints
  `65533`; rustc fails with E0600 "cannot apply unary operator `-` to type `u16`".
- `NegatedLongLiteral.java`: `System.out.println(-(4294967296L));`. Java prints
  `-4294967296`; rustc fails with "literal out of range for `i32`": a `long` literal under a unary
  operator is typed as `i32`. `~(9223372036854775807L)` fails the same way, and
  `(byte)(~(9223372036854775807L))` fails with "literal out of range for `i8`".
- `NullObjectConcat.java`: `"slot " + slots[0]` where `slots` is a `Node[]` and `Node` has no
  `toString`. Java prints `slot null`; the translator rejects it with "printing an object without
  toString not supported yet". The value is always null, so the class's own printing is never
  needed.
