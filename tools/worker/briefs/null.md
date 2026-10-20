# Null

Translate `null`: today `lower.rs` rejects it with "null not supported yet".

## Scope
This branch has only one reference type, `String` (arrays and objects are being built on other
branches at the same time). Build null support for `String` now, and write it against
`Type::is_reference()` and `Type::Null` so that arrays and classes plug in when they are merged.
- The `null` literal; `String s = null;`, passing, returning and assigning `null`.
- `s == null`, `s != null`, `null == s`; `==` between two strings stays rejected (it compares
  references) unless one side is the `null` literal.
- `println(s)` and string concatenation with a null string print `null`.
- Dereferencing null (`s.length()`, `s.equals(t)`) throws `NullPointerException`: exit code 1,
  nothing more on stdout. `"abc".equals(null)` is `false`.
- Conditional expressions mixing `null` and strings (`flag ? s : null`).

## Translation
- Faithful form: a string that can be null is `Option<String>`; dereferencing it goes through a
  runtime helper (`crates/runtime/src/null.rs`) that throws `NullPointerException` the way
  `java::throw` does, so the exit code is 1.
- Idiomatic form, by analysis (`src/analysis/nullness.rs`): a nullness analysis over the
  program (locals, parameters across calls, return values), in the style of the existing range
  analysis: a variable that is never null stays a plain `String`/`&str` as today; one that can be
  null becomes `Option<String>`; after `if (s != null)` (and `s == null` with an early return or
  `else`) the variable is known non-null in that branch, so the code uses `if let Some(s) = &s`
  or `match`, not repeated unwraps. Prefer `Option<&str>` for borrowed parameters.
- Today's output for programs without `null` must not change at all: run the corpus and
  `tests/translation.rs` before and after and compare.

## Deliverables
- `null` literal, null comparisons, null in assignments, arguments, returns, conditionals
- Printing and concatenating null strings as `null`
- `NullPointerException` on dereference with exit code 1
- Nullness analysis: never-null strings stay `String`/`&str`, maybe-null ones are `Option`, null checks narrow
- Representation written against `Type::is_reference()` so arrays and classes can use it
- Corpus programs: `corpus/features/Null.java`, `corpus/features/NullPointer.java` (ends with an NPE), and `corpus/programs/Lookup.java` (a search that returns null when not found)
- Shape tests in `tests/translation.rs` for the idiomatic forms
- README "Current state" updated
