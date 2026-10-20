# Faster strings and statics in translated programs

`docs/performance.md` (and `scripts/benchmark.py`, `benchmarks/`) show StringBuild 31 times
slower than Java and Statics 3 times slower. Causes found: `charAt` and `length` on a Rust
`String` are O(n) because Java indexes UTF-16 units; static fields live in `thread_local!`
cells read through `with`. Make both fast without changing what programs print.

## Scope
- Read how strings are represented and translated (README, `codegen/strings.rs`,
  `codegen/text.rs`, the runtime's `string.rs`, the ASCII analysis that the String work added:
  find it with `grep -rn ascii crates/translator/src/analysis`). Options: use the ASCII
  analysis more (an index into a string proven ASCII is a byte index); a runtime fast path that
  is O(1) for ASCII strings without scanning each time (e.g. cache per string whether it is
  ASCII, or a small Java-string type that stores the UTF-16 length/ASCII flag); avoid
  re-collecting strings in loops. Pick what keeps generated code readable; measure each.
- Statics: measure `thread_local!` + `with`/`Cell` against alternatives that need no `unsafe`
  (e.g. `const` thread-local initialisers where the value is a constant, which are already used
  in places; reading a static once into a local in a loop when nothing in the loop can change
  it, decided by an analysis; `Cell` instead of `RefCell` for Copy values). Generated code must
  stay safe Rust.
- Every change keeps the corpus green and adds a corpus program for any new path. Re-run
  `python3 scripts/benchmark.py StringBuild Statics` (and the others to make sure nothing got
  slower) and update `docs/performance.md`.
- Do not touch object, array, nullness or borrowing code (other agents are rewriting it now),
  nor `codegen/expr.rs` number code beyond a minimal hook (another agent is fixing it now).

## Deliverables
- O(1) charAt and length for strings in the common case, measured
- Faster static field access in safe Rust, measured
- Corpus programs for the new paths
- docs/performance.md updated with new numbers
