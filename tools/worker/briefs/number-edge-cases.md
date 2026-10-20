# Edge cases of numbers, characters and printing

Hunt for wrong translations in what the translator already supports: primitive arithmetic,
casts, characters, string concatenation and printing of every primitive type. Fix the ones
whose cause is in the number code; record the rest.

## Scope
- Write small Java programs that stress supported features and compare Java with the translation
  (`cargo run -q -p java-to-rust -- File.java --crate DIR` then `cargo run` in DIR, against
  `java File.java`; compare stdout and exit code). Areas: int/long overflow in every operator
  and compound assignment, `++`/`--` inside expressions, shifts by negative and >= 64 amounts,
  `>>>` on negative bytes/shorts/chars after promotion, integer division and remainder by zero
  and `Integer.MIN_VALUE / -1`, every narrowing and widening cast (double → int of NaN,
  infinities, huge values; long → float rounding; char ↔ int ↔ short), char arithmetic
  (`c + 1`, `(char)(c + 1)`, `c++` on a char), float vs double printing (`System.out.println`
  of 1.0E10, 1.0E-5, 100.0, -0.0, NaN, Float.MIN_VALUE, 0.1f + 0.2f), string concatenation of
  every type including char + int precedence (`"" + 'a' + 1` vs `'a' + 1 + ""`), compound
  assignment narrowing (`byte b += 300`), ternaries mixing int/char/long/double (binary numeric
  promotion), constant folding of `final` locals, `Math` results printed.
- Every program that passes goes into `corpus/features/` under a descriptive name (several
  focused programs, not one giant one); keep each under a few hundred lines.
- A mismatch whose cause is in `codegen/primitives.rs`, `codegen/code.rs`, `codegen/expr.rs`
  (numeric parts), `analysis/ranges.rs`, `lower/primitives.rs` or the runtime's `float.rs`:
  fix it, with the reproducing program added to the corpus and a commit per fix.
- A mismatch caused elsewhere, or one you cannot fix: do NOT touch object, string, analysis of
  borrowing/nullness or lowering of calls (other agents are rewriting those right now). Put the
  smallest Java program that shows it in `known-failures/<DescriptiveName>.java` at the
  repository root (not under `corpus/`, which the test runs), and add a line to
  `known-failures/README.md`: file, what Java prints, what the translation does.

## Deliverables
- At least 8 focused corpus programs covering the areas above, all passing
- Every number-code mismatch found fixed, each with its corpus program
- Remaining mismatches recorded in known-failures/ with minimal programs and the README list
- README "Current state" updated if behaviour changed
