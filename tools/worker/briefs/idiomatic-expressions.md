# Idiomatic expressions in the generated Rust

The statements and loops of the generated Rust now read like Rust (see README, "Idiomatic
statements and loops"), and `scripts/clippy-corpus.sh` counts clippy warnings over the corpus
output: 201 at the start of this brief. Most of the rest are in expressions. Make expressions
read like Rust a person would write, without changing behaviour.

## Scope
- Run `scripts/clippy-corpus.sh` first and record the total and the top lints in your notes.
- Clippy-driven cleanups in expressions: needless parentheses (`unused_parens`, and the
  parentheses `Code::at`/`left_at` add where precedence does not need them), comparisons with
  `true`/`false` (`bool_comparison`), casts to the same type, `x = x op y` as `x op= y` where the
  operation is plain, redundant `.clone()` on `Copy` values, `!(a == b)` as `a != b`, and similar
  ones the measurement shows are common. Lints that come from the Java source itself
  (`eq_op` on `x == x`, `zero_divided_by_zero` on `0.0 / 0.0`, `approx_constant`) stay: Java
  computes them, so the generated file may `#![allow]` them.
- Plain arithmetic where the range analysis proves no overflow: `analysis/ranges.rs` already
  proves some ranges, and proven operations are written as plain `+ - *` instead of
  `wrapping_*`. Find the common cases in the corpus where it does not prove an obvious bound
  (a loop counter from a counting loop, see `ast/counting.rs`; `% n` of a small constant; a
  `char` minus `'a'`; sums of two small-type values) and extend it. Each proof must be sound: a
  value that could wrap in Java must stay wrapping.
- Do not touch object, exception, nullness or borrowing code (`lower/objects.rs`,
  `lower/hierarchy.rs`, `codegen/objects.rs`, `codegen/dispatch.rs`, `analysis/borrowing.rs`,
  `analysis/nullness.rs`), nor array code (`analysis/arrays.rs`, `codegen/arrays.rs`,
  `codegen/java_arrays.rs`, `codegen/array_writes.rs`, runtime `array.rs`/`slice.rs`): other
  agents are rewriting them now. Keep edits to shared files small and say which in your report.
- Each change keeps the corpus green; add corpus programs for new paths (proved ranges at their
  edges: values that just fit and values that just wrap). Put temporary files in the scratchpad,
  not the repository.

## Deliverables
- Starting clippy total and top lints recorded
- Expression cleanups guided by the measurement
- Range analysis proving the common bounds, with edge-case corpus programs
- Clippy total after, and the README paragraph on idiomatic output updated
