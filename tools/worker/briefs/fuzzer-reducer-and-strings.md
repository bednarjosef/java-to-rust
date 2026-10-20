# A faster reducer, and strings in the fuzzer

`tools/fuzz/` (read its README) generates random Java programs, compares Java with the
translation and reduces mismatches. The reducer took hours per program. An Opus agent is fixing
the compile errors the fuzzer found and may extend `fuzz.py` a little at the same time; keep
your generator additions in new, separate functions and a new option so merges stay easy.

## Scope
- Make `reduce.py` fast: reduce on the syntax tree level instead of lines/characters where
  possible (delete whole statements, methods, then simplify expressions), reuse one translated
  Cargo crate and only rewrite `src/main.rs` (incremental builds), check the javac and
  translation step before building Rust (a candidate that stops reproducing the same failure
  class is rejected early), classify the failure (rustc error code, rejection message, output
  mismatch) and keep only candidates with the same class, and run candidates in parallel. A
  typical reduction should take minutes, not hours; measure it.
- Extend the generator with an option (e.g. `--strings`) for strings: `String` locals and
  fields, literals with non-ASCII and surrogate pairs, concatenation of every type, `length`,
  `charAt`, `substring`, `indexOf`, `contains`, `startsWith`, `replace`, `toUpperCase`,
  `trim`, `split` with literal separators, `String.valueOf`, `String.format` with `%d %s %x %c
  %5.2f %-8s %08d`, `StringBuilder` append/insert/reverse/setCharAt/deleteCharAt/toString,
  `switch` on strings, `equals`/`compareTo`, and `java.util.Arrays.toString/sort/fill/equals`
  on primitive arrays. Keep indices in range so programs do not throw (or throw deliberately).
- Run several hundred seeds with strings on; record each distinct mismatch or rustc failure,
  reduced, in `known-failures/<DescriptiveName>.java` with a line in
  `known-failures/README.md` in its existing style (check it is not already there).
- Only change `tools/fuzz/` and `known-failures/`.

## Deliverables
- Reducer that reduces a typical failure in minutes, measured
- Generator option for strings, StringBuilder, format and Arrays
- Several hundred string seeds run, failures reduced into known-failures/
- tools/fuzz/README.md updated
