# Known failures

Minimal programs that the translator gets wrong, though the feature is in the supported subset
(see docs/supported-java.md). Each one is kept out of `corpus/`, which must pass. Java's output
is what `java File.java` prints. When a failure is fixed, move its file into `corpus/features/`
with the fixed behaviour.

- `StringContainsCharSequence.java`: `"a\\b".replace("ab", "abc").contains(...)` with a `split`
  element as the argument. Java prints nothing; the translator rejects it with "type
  `java.lang.CharSequence` not supported yet", since `replace` with string arguments and
  `contains` both take a `CharSequence` in Java.
