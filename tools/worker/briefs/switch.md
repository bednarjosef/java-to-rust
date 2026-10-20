# Switch and labeled jumps

Translate `switch` statements and expressions and labeled `break`/`continue`; today `lower.rs`
rejects them.

## Scope
- Classic `switch` statements on `int`, `char`, `byte`, `short` and `String`, with `case`
  labels, several labels per case (`case 1: case 2:` and `case 1, 2:`), `default`, `break`,
  and fall-through from one case into the next.
- Arrow `switch` (`case 1 -> ...;`, `case 1, 2 -> { ... }`) as statements and as expressions,
  with `yield` in block arms. A switch expression must cover every value (javac checks this:
  for `int` that means a `default`).
- Labeled statements: `outer: for (...) { ... break outer; ... continue outer; }`, also on
  `while`, `do` and plain blocks (`label: { ... break label; }`).
- Java semantics: a `String` switch on a `null` string throws `NullPointerException` (use the
  null feature's `java::non_null`); evaluation of the selector happens once; fall-through runs
  the following cases' statements in order until a `break`.

## Translation
- Idiomatic form first where it is equivalent: a switch without fall-through (every case ends in
  `break`, `return`, `continue` or `throw`, or uses arrows) becomes a Rust `match` on the
  selector, `1 | 2 => { ... }`, `_ => {}` for a missing default. A `String` switch matches on
  `s.as_str()` with string literal patterns. A `char` case is a `'a' as u16` value: match on
  `c` with guards, or better, `match char::from_u32(c as u32)` only if you prove it is the same;
  the plainest correct form wins.
- Fall-through: the faithful form. One way: a labeled block per case run by a small state
  machine (`let mut case = <index of first matching case>; loop { match case { 0 => { ...;
  case = 1; } 1 => { ...; break; } ... } }`). Pick a form, say why in your notes, and keep it
  readable.
- `break` inside a `switch` that is inside a loop ends only the switch, and `continue` continues
  the loop: get Rust's `break` targets right (labels like `'switch_1:` where needed).
- Labeled Java jumps become Rust loop labels: `'outer: for ...` and `break 'outer;`,
  `continue 'outer;`. A labeled block that is not a loop becomes `'label: { ... break 'label; }`
  (labeled block expressions are stable Rust).
- Reuse the existing counting-loop translation (`for i in 0..n`) where it already applies.

## Deliverables
- Classic switch without fall-through as `match`, on int, char, byte, short and String
- Fall-through switch, faithful form
- Arrow switch statements and switch expressions with `yield`
- `break`/`continue` inside switch inside loops, with correct targets
- Labeled break and continue on loops and blocks
- Null String selector throws NullPointerException
- Corpus programs: `corpus/features/Switch.java`, `corpus/features/SwitchFallThrough.java`,
  `corpus/features/Labels.java`, and `corpus/programs/Calculator.java` (a small RPN or
  expression calculator over a string that uses switches)
- Shape tests in `tests/translation.rs` for `match`, arrow switch expressions and labels
