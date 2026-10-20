# java.util.Arrays methods and array clone

Add the `java.util.Arrays` methods that typical programs call on arrays of primitives and
strings, and `a.clone()`, with Java's exact results. First read what exists: `lower/arrays.rs`
(`arrays_call`: toString, fill, sort, copyOf), `codegen/arrays.rs`, the runtime's `array.rs` and
`slice.rs`, and README's paragraphs on arrays (shared arrays, plain `Vec`s, nullable elements).

## Scope
- `Arrays.equals` (every primitive type and `String[]`, null arrays and null elements),
  `Arrays.hashCode` (Java's exact formula), `Arrays.sort` on `long[]`, `char[]`, `byte[]`,
  `short[]`, `double[]`, `float[]` (Java's order: -0.0 before 0.0, NaN last) and `String[]`
  (UTF-16 order, as `String.compareTo`; null elements throw NullPointerException: exit code 1),
  `Arrays.sort(a, from, to)` with Java's range checks, `Arrays.fill(a, from, to, v)`,
  `Arrays.copyOfRange`, `Arrays.binarySearch` (primitives and String; Java's
  `-(insertion point) - 1` result), `Arrays.stream` is NOT needed, `Arrays.deepToString` for
  arrays of arrays of primitives/strings, `Arrays.asList` is NOT needed (it needs List).
- `a.clone()` on arrays of primitives and strings (a shallow copy; for arrays of arrays the rows
  are shared, as in Java).
- Keep within arrays of primitives and strings: arrays of objects belong to the object-model
  work that another agent is doing now in `lower/objects.rs`, `analysis/` and
  `codegen/objects.rs`; do not edit those files, nor `analysis/nullness.rs` or
  `analysis/borrowing.rs`. Other agents also edit `lower.rs`'s `call` (String, Math, Character
  methods): keep any edit there to the single `java.util.Arrays` arm.
- Two programs in `known-failures/` (repository root) are yours: `ArraysEqualsInts.java` and
  `ArraysSortStrings.java`. When they translate correctly, move them into `corpus/features/` and
  delete their lines from `known-failures/README.md`.

## Deliverables
- Arrays.equals and Arrays.hashCode for primitive and String arrays
- Arrays.sort for every primitive type and String, whole and by range
- Arrays.fill by range, copyOfRange, binarySearch, deepToString
- Array clone()
- Corpus programs corpus/features/ArraysEqualsAndHash.java, corpus/features/ArraysSortAllTypes.java, corpus/features/ArraysRanges.java, corpus/features/ArrayClone.java, plus the two moved known failures
- README "Current state" updated
