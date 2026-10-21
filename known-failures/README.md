# Known failures

Minimal programs that the translator gets wrong, though the feature is in the supported subset
(see the "Current state" section of the README). Each one is kept out of `corpus/`, which must
pass. Java's output is what `java File.java` prints. When a failure is fixed, move its file into
`corpus/features/` with the fixed behaviour.

- `CompoundThroughNullableField.java`: `h.box.v += 10` where `h.box` is a field that may be null,
  in a class whose fields are assigned after construction. Java prints `true`; rustc fails with
  E0716 "temporary value dropped while borrowed".
- `FieldReadAndMoveInCall.java`: `depth(single.next)` and `depth(single)` in one expression, where
  `Node` has a field assigned after construction. Java prints `0 1`; rustc fails with E0505
  "cannot move out of `single` because it is borrowed" (the `Ref` from the field read lives to the
  end of the statement).
- `LoneSurrogateCompareTo.java`: `"😀x".substring(1)`, which starts with a lone low
  surrogate, compared with `compareTo`. Java prints `2 -56772 -30747`; the translation prints
  `2 -3 26022`: the length is right, but `java::compare_to` sees the lone surrogate as `?` (63)
  instead of the code unit 0xDE00. The strings fuzzer's seeds 18, 51 and 396 (of 600) differ this
  way.
- `MinCursorLeft.java`: a `while (t.left != null) t = t.left;` loop over a mutable class.
  Java prints `1`; rustc fails with E0506 "cannot assign to `t` because it is borrowed". Loops
  over classes that are never assigned after construction translate (their `Rc` is plain).
- `NullObjectConcat.java`: `"slot " + slots[0]` where `slots` is a `Node[]` and `Node` has no
  `toString`. Java prints `slot null`; the translator rejects it with "printing an object without
  toString not supported yet". The value is always null, so the class's own printing is never
  needed.
- `NullableFieldReadAndMoveInCall.java`: `b.v + " " + show(b)` in one `println`, where `b` is a
  local of a class that may be null (returned by a method that can return `null`). Java prints
  `3 box 3`; rustc fails with E0505 "cannot move out of `b` because it is borrowed" (the
  `non_null` borrow for `b.v` is still alive when `b` is moved into `show`).
- `NullableStaticStringAppend.java`: `s += s` and `s += "!"` on a static `String` that may be
  null (it is assigned an element of a `String[]`). Java prints `aa` and `null!`; rustc fails with
  E0277 "`Option<String>` doesn't implement `std::fmt::Display`" and E0308: the compound
  assignment to a static in `codegen/objects.rs` formats the old value as a plain string and
  stores the result without `Some`. Found by the fuzzer with `--arrays --strings`.
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
