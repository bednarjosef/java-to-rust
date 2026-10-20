# Idiomatic loops and statements in the generated Rust

The generated Rust behaves like Java, but its statements still read like Java. Make the statement
and loop forms read like Rust a person would write, without changing behaviour, and measure it
with clippy.

## Scope
- First, a measurement script `scripts/clippy-corpus.sh`: translate every program in `corpus/`
  (as `crates/translator/tests/corpus.rs` does; it may reuse its helpers but must not change that
  file, another worker is rewriting it), run `cargo clippy` on the generated crates with
  the generated `#![allow(...)]` lines removed or overridden, and print warnings per lint and per
  program, plus a total. Record the starting total in your notes.
- Counting loops: `for (int i = a; i < b; i++)` becomes `for i in a..b` when the body never
  assigns `i`, the bound `b` cannot change while the loop runs (a local or parameter not assigned
  in the body or update, `arr.length` of an array local not assigned in the body, `s.length()` of a
  string local not assigned in the body, a constant), and evaluating `a` and `b` has no side
  effects. Also `i <= b` (as `a..=b`) only when `b` is a constant below the type's maximum or
  otherwise cannot be the maximum (Java would loop forever there by wrapping), `i += k` with a
  positive constant `k` (`.step_by(k)`), and the downward `for (int i = b - 1; i >= a; i--)` as
  `(a..b).rev()`. Keep `i` an `i32`/`i64` as today. `continue`, `break` and labels must keep
  working. Keep the existing in-bounds facts working: `analysis/indexing.rs` and
  `analysis/arrays.rs` recognise counting loops to drop bounds checks; the new form must not lose
  them (put the shared loop recognition in one place).
- Other statement forms clippy flags or a person would not write: `while true` as `loop`, a
  needless trailing `return`, `mut` on locals never reassigned, needless labels, empty `else`,
  `if c { true } else { false }` at statement level, `let x; x = v;` as `let x = v;`, and similar
  ones the measurement shows are common. Leave expressions (arithmetic, casts, parentheses,
  `x = x + 1` to `x += 1`) for now: another agent is changing `codegen/expr.rs`, `primitives.rs`,
  `code.rs`, `analysis/ranges.rs` and `lower/primitives.rs`; do not edit those files. You will be
  told when they are free.
- Do not touch object, exception, nullness or borrowing code (`lower/objects.rs`,
  `lower/hierarchy.rs`, `codegen/objects.rs`, `codegen/dispatch.rs`, `analysis/borrowing.rs`,
  `analysis/nullness.rs`) nor `analysis/arrays.rs`, `codegen/arrays.rs`, `codegen/java_arrays.rs`
  beyond a minimal hook (another agent is about to rewrite array ownership).
- Each change keeps the corpus green; add corpus programs for loop edge cases (bound changed in
  the body, `i` assigned in the body, `<= Integer.MAX_VALUE`, nested labelled `continue`, empty
  ranges, negative steps). Put temporary files in the scratchpad, not the repository.

## Deliverables
- scripts/clippy-corpus.sh with the starting clippy total
- Counting loops as Rust ranges, with corpus edge cases
- Other statement forms cleaned up, guided by the measurement
- Clippy total after, and README paragraph on idiomatic output
