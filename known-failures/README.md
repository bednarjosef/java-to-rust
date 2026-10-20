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
- `CompoundThroughNullableField.java`: `h.box.v += 10` where `h.box` is a field that may be null,
  in a class whose fields are assigned after construction. Java prints `true`; rustc fails with
  E0716 "temporary value dropped while borrowed".
- `CopyRowOfNewArray.java`: `new Cell[2][]` followed by `copy[0] = grid[0];` where `grid` is a
  `Cell[][]`. Java prints `4`; rustc fails with E0308 "mismatched types" at the `new Cell[2][2]`
  line: the rows it builds are plain arrays, but the generated type of `grid` makes its rows
  `Option`s, because `copy[0] = grid[0]` stores a row in a nullable slot.
- `FieldReadAndMoveInCall.java`: `depth(single.next)` and `depth(single)` in one expression, where
  `Node` has a field assigned after construction. Java prints `0 1`; rustc fails with E0505
  "cannot move out of `single` because it is borrowed" (the `Ref` from the field read lives to the
  end of the statement).
- `FloatIncrementValue.java`: `System.out.println(++sFloat);` on a `float` static. Java prints
  `2.5`; rustc fails with E0599 "no method named `wrapping_add` found for type `f32`" (the
  increment is translated with integer wrapping arithmetic, which `f32` lacks).
- `MinCursorLeft.java`: a `while (t.left != null) t = t.left;` loop over a mutable class.
  Java prints `1`; rustc fails with E0506 "cannot assign to `t` because it is borrowed". Loops
  over classes that are never assigned after construction translate (their `Rc` is plain).
- `NegatedCharCast.java`: `sInt *= ((char)((-(3))));`, a negative int cast to `char`. Java prints
  `65533`; rustc fails with E0600 "cannot apply unary operator `-` to type `u16`".
- `NegatedLongLiteral.java`: `System.out.println(-(4294967296L));`. Java prints
  `-4294967296`; rustc fails with "literal out of range for `i32`": a `long` literal under a unary
  operator is typed as `i32`. `~(9223372036854775807L)` fails the same way, and
  `(byte)(~(9223372036854775807L))` fails with "literal out of range for `i8`".
- `NullEnumVariable.java`: a local `Level best = null;` of an enum type. Java prints `none` then
  `HIGH`; the translator rejects it with "null enums not supported yet" (null objects are in the
  supported subset).
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
- `StringArrayFieldInitializer.java`: `String[] items = new String[2];` as a field initializer of
  a class. Java prints `pen null 1`; rustc fails with E0308 "mismatched types" (the generated
  `filled(2, None)` gives `Option`s where `String` is expected). The same array assigned in a
  constructor translates.
- `SwitchOnFieldThenMutate.java`: `switch (m.state)` on an `int` field of a mutable class, with an
  arm that calls a method changing `m.state`. Java prints `2 2`; the translated program panics
  with "RefCell already borrowed" and exits with 101. The `Ref` of the scrutinee lives for the
  whole `match`, so the arm's `borrow_mut` fails. Storing the value in a local first avoids it.
