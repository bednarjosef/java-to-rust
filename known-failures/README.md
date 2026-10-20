# Known failures

Minimal programs that the translator gets wrong, though the feature is in the supported subset
(see the "Current state" section of the README). Each one is kept out of `corpus/`, which must
pass. Java's output is what `java File.java` prints. When a failure is fixed, move its file into
`corpus/features/` with the fixed behaviour.

- `ArrayCloneInts.java`: `a.clone()` on an `int[]`. Java prints `1 9`; the translator rejects it
  with "method `Array.clone` not supported yet". Arrays are in the supported subset.
- `ArrayStoreFromMethodCall.java`: `memory[slot] = pop();` in a method of a nested class, where
  `pop` changes a field of the same object. Java prints `1`; rustc fails with E0502 "cannot
  borrow `*self` as mutable because it is also borrowed as immutable" (the generated
  `java::set(&self.memory, slot, self.pop())` borrows `self` twice). Storing the value in a local
  first avoids it.
- `ArraysEqualsInts.java`: `Arrays.equals` on an `int[]`. Java prints `true`; the translator
  rejects it with "`Arrays.equals` on this array not supported yet". Arrays given to
  `java.util.Arrays` are part of the supported subset.
- `ArraysSortStrings.java`: `Arrays.sort` on a `String[]`. Java prints `[apple, fig, pear] 3`; the
  translator rejects it with "`Arrays.sort` on this array not supported yet". (`Arrays.sort` on an
  `int[]` translates.)
- `CharacterIsLetterOrDigit.java`: `Character.isLetterOrDigit('a')`. Java prints `true`; the
  translator rejects it with "method `Character.isLetterOrDigit` not supported yet" (the
  `Character` tests are in the supported subset). `Character.isWhitespace` is rejected the same
  way, from the same match in `lower/primitives.rs`.
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
- `MinCursorLeft.java`: a `while (t.left != null) t = t.left;` loop over a mutable class.
  Java prints `1`; rustc fails with E0506 "cannot assign to `t` because it is borrowed". Loops
  over classes that are never assigned after construction translate (their `Rc` is plain).
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
- `SwitchOnEnum.java`: `switch (op)` on an enum with `case ADD:` labels. Java prints `sub`; the
  translator rejects it with "a switch on this type not supported yet" (switches on `int`,
  `char`, `String` and `ordinal()` translate).
- `StringArrayElementEqualsLoopVar.java`: `seen[k].equals(w)` in a loop over a `String[]` with
  `w` the for-each variable, where `seen` is `new String[3]` and `k < distinct` with `distinct` a
  mutable local. Java prints `distinct 0`; rustc fails with E0716 "temporary value dropped while
  borrowed" at the `equals` (the `as_deref` borrow of the element outlives its temporary). With a
  literal argument, or a literal bound, it translates.
- `StringArrayFieldInitializer.java`: `String[] items = new String[2];` as a field initializer of
  a class. Java prints `pen null 1`; rustc fails with E0308 "mismatched types" (the generated
  `filled(2, None)` gives `Option`s where `String` is expected). The same array assigned in a
  constructor translates.
- `SwitchOnFieldThenMutate.java`: `switch (m.state)` on an `int` field of a mutable class, with an
  arm that calls a method changing `m.state`. Java prints `2 2`; the translated program panics
  with "RefCell already borrowed" and exits with 101. The `Ref` of the scrutinee lives for the
  whole `match`, so the arm's `borrow_mut` fails. Storing the value in a local first avoids it.
