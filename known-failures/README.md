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
- `NullableStaticStringAppend.java`: `s += s` and `s += "!"` on a static `String` that may be
  null (it is assigned an element of a `String[]`). Java prints `aa` and `null!`; rustc fails with
  E0277 "`Option<String>` doesn't implement `std::fmt::Display`" and E0308: the compound
  assignment to a static in `codegen/objects.rs` formats the old value as a plain string and
  stores the result without `Some`. Found by the fuzzer with `--arrays --strings`, where it is
  the only failure: seeds 4, 34, 184, 214, 222 and 224 of 300.
- `StringContainsCharSequence.java`: `"a\\b".replace("ab", "abc").contains(...)` with a `split`
  element as the argument. Java prints nothing; the translator rejects it with "type
  `java.lang.CharSequence` not supported yet", since `replace` with string arguments and
  `contains` both take a `CharSequence` in Java.
