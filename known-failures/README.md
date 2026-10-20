# Known failures

Minimal programs that the translator gets wrong, though the feature is in the supported subset
(see the "Current state" section of the README). Each one is kept out of `corpus/`, which must
pass. Java's output is what `java File.java` prints. When a failure is fixed, move its file into
`corpus/features/` with the fixed behaviour.

- `LoneSurrogateCompareTo.java`: `"😀x".substring(1)`, which starts with a lone low
  surrogate, compared with `compareTo`. Java prints `2 -56772 -30747`; the translation prints
  `2 -3 26022`: the length is right, but `java::compare_to` sees the lone surrogate as `?` (63)
  instead of the code unit 0xDE00. The strings fuzzer's seeds 18, 51 and 396 (of 600) differ this
  way.
- `NullObjectConcat.java`: `"slot " + slots[0]` where `slots` is a `Node[]` and `Node` has no
  `toString`. Java prints `slot null`; the translator rejects it with "printing an object without
  toString not supported yet". The value is always null, so the class's own printing is never
  needed.
- `StringContainsCharSequence.java`: `"a\\b".replace("ab", "abc").contains(...)` with a `split`
  element as the argument. Java prints nothing; the translator rejects it with "type
  `java.lang.CharSequence` not supported yet", since `replace` with string arguments and
  `contains` both take a `CharSequence` in Java.
- `StringFormatObject.java`: `String.format("<%d|%s|%5.2f>", ...)` with an `int`, a `String` and a
  `double` argument. Java prints `<0|😀x| 0.00>`; the translator rejects it with "Object not supported
  yet", because `String.format` takes `Object...` and its varargs are not translated.
