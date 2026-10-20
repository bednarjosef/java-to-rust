# Known failures

Minimal programs that the translator gets wrong, though the feature is in the supported subset
(see the "Current state" section of the README). Each one is kept out of `corpus/`, which must
pass. Java's output is what `java File.java` prints. When a failure is fixed, move its file into
`corpus/features/` with the fixed behaviour.

- `AmbiguousLiteralMethod.java`: `aFloat[Math.floorMod(Math.abs((sBoolean ? 2 : -1)), ...)]++;`,
  a method call on a conditional of two `int` literals. Java prints nothing; rustc fails with
  E0689 "can't call method `abs` on ambiguous numeric type `{integer}`". The fuzzer's seed 2 hits
  the same error with `wrapping_sub` on `(~(0)) - c`, and seed 55 with `Math.abs(-(0))` alone,
  and seed 115 with `wrapping_add` on a sum of a `long` conditional and a `short`.
- `AssignmentInTernaryOr.java`: `boolean b = aBoolean[0] ? ((sBoolean = v4) || true) : false;`, an
  assignment as the left operand of `||` inside a conditional branch. Java prints `false true`;
  rustc fails with E0308 "mismatched types". The same assignment alone, or `||` alone in a
  conditional, translates.
- `BlockExpressionBeforeCast.java`: `return (sChar++);` in a `double` method, where `sChar` is a
  `char` static. Java prints `65.0`; the translated block is taken as a statement, so the `as f64`
  after it does not parse: rustc fails with "expected expression, found `as`". The fuzzer's
  seeds hit the same parse error after a block with `>=`, `==`, `!=`, `/`, `%`, `^`, `>`, `<` and
  `+` (not reduced separately). Inside a `?:` branch, `(sDouble--) < 2` fails with "expected type,
  found `2.0`" (or "found keyword `if`" when the right side is a conditional): the same block,
  followed by `<`, is read as the start of generic arguments.
- `CompoundThroughNullableField.java`: `h.box.v += 10` where `h.box` is a field that may be null,
  in a class whose fields are assigned after construction. Java prints `true`; rustc fails with
  E0716 "temporary value dropped while borrowed".
- `FieldReadAndMoveInCall.java`: `depth(single.next)` and `depth(single)` in one expression, where
  `Node` has a field assigned after construction. Java prints `0 1`; rustc fails with E0505
  "cannot move out of `single` because it is borrowed" (the `Ref` from the field read lives to the
  end of the statement).
- `FloatIncrementValue.java`: `System.out.println(++sFloat);` on a `float` static. Java prints
  `2.5`; rustc fails with E0599 "no method named `wrapping_add` found for type `f32`" (the
  increment is translated with integer wrapping arithmetic, which `f32` lacks). A decrement
  (`--`) of a `float` fails the same way with `wrapping_sub`.
- `IndexedIncrementReadsArray.java`: `a5[Math.floorMod((0), a5.length)]++;` on an `int[]` local,
  in a `switch` that is never taken. Java prints nothing; rustc fails with E0502 "cannot borrow
  `a5` as immutable because it is also borrowed as mutable": the index reads `a5.length` while the
  element is borrowed for `++`.
- `MinCursorLeft.java`: a `while (t.left != null) t = t.left;` loop over a mutable class.
  Java prints `1`; rustc fails with E0506 "cannot assign to `t` because it is borrowed". Loops
  over classes that are never assigned after construction translate (their `Rc` is plain).
- `NegatedCharCast.java`: `sInt *= ((char)((-(3))));`, a negative int cast to `char`. Java prints
  `65533`; rustc fails with E0600 "cannot apply unary operator `-` to type `u16`".
- `NegatedLongLiteral.java`: `System.out.println(-(4294967296L));`. Java prints
  `-4294967296`; rustc fails with "literal out of range for `i32`": a `long` literal under a unary
  operator is typed as `i32`. `~(9223372036854775807L)` fails the same way, and
  `(byte)(~(9223372036854775807L))` fails with "literal out of range for `i8`".
- `NegatedUnsignedShiftCount.java`: `System.out.println(1 >>> (-(0)));`, a negated shift count.
  Java prints `1`; rustc fails with E0600 "cannot apply unary operator `-` to type `u32`", the
  same message as `NegatedCharCast` with the type of the operand here.
- `NullObjectConcat.java`: `"slot " + slots[0]` where `slots` is a `Node[]` and `Node` has no
  `toString`. Java prints `slot null`; the translator rejects it with "printing an object without
  toString not supported yet". The value is always null, so the class's own printing is never
  needed.
- `NullableFieldReadAndMoveInCall.java`: `b.v + " " + show(b)` in one `println`, where `b` is a
  local of a class that may be null (returned by a method that can return `null`). Java prints
  `3 box 3`; rustc fails with E0505 "cannot move out of `b` because it is borrowed" (the
  `non_null` borrow for `b.v` is still alive when `b` is moved into `show`).
- `NullOnlyClass.java`: a class that is only used as the type of a null local (`Named n = null;`,
  never constructed). Java prints `true`; rustc fails with E0425 "cannot find type
  `NullOnlyClassNamed` in this scope" because the class is not emitted.
- `StringContainsCharSequence.java`: `"a\\b".replace("ab", "abc").contains(...)` with a `split`
  element as the argument. Java prints nothing; the translator rejects it with "type
  `java.lang.CharSequence` not supported yet", since `replace` with string arguments and
  `contains` both take a `CharSequence` in Java.
- `StringFormatObject.java`: `String.format("<%d|%s|%5.2f>", ...)` with an `int`, a `String` and a
  `double` argument. Java prints `<0|😀x| 0.00>`; the translator rejects it with "Object not supported
  yet", because `String.format` takes `Object...` and its varargs are not translated.
- `SwitchOnFieldThenMutate.java`: `switch (m.state)` on an `int` field of a mutable class, with an
  arm that calls a method changing `m.state`. Java prints `2 2`; the translated program panics
  with "RefCell already borrowed" and exits with 101. The `Ref` of the scrutinee lives for the
  whole `match`, so the arm's `borrow_mut` fails. Storing the value in a local first avoids it.
