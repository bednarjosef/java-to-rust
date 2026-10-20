# A differential fuzzer for the supported subset

Write a generator of random Java programs in the subset the translator supports, run each on
Java and on the translation, and keep the ones that differ, reduced. This brief adds a tool and
records failures; do not edit anything under `crates/` (other agents are changing the
translator now).

## Scope
- `tools/fuzz/fuzz.py` (Python 3, standard library only): generates programs from a seed. Start
  with expressions and statements over `int`, `long`, `short`, `byte`, `char`, `boolean`,
  `double`, `float` locals and static fields: every operator incl. compound assignment, `++`/`--`
  in expressions, shifts, casts, ternaries, `Math.abs/min/max`, string concatenation of mixed
  types printed with `System.out.println`; then `if`/`while`/`for`/`do`/`switch` (with
  fall-through and `break`), labeled `break`/`continue`, methods with parameters and returns,
  recursion with a depth bound, arrays of primitives (`new int[n]`, literals, indexing with
  in-range indices computed with `Math.floorMod`/masking, `length`), static fields. Every
  generated program must terminate (bounded loops) and must not throw unless that is the point
  (then exit code 1 must match). Print intermediate values often so differences show.
- `tools/fuzz/run.py`: for N seeds, generate, run `java Prog.java`, translate with the
  translator binary (`cargo build --release -p java-to-rust` first; translate to a crate, build
  with `cargo build --release` — reuse one crate directory and only rewrite src/main.rs to keep
  builds fast), compare stdout and exit code. A translator rejection is recorded separately
  (count per message). Run up to 4 programs in parallel. Work directory under `target/fuzz/`.
- `tools/fuzz/reduce.py`: shrinks a differing program by deleting statements and simplifying
  expressions while it still differs.
- Run it for a few hundred seeds. Each distinct mismatch: put the reduced program in
  `known-failures/<DescriptiveName>.java` at the repository root and add a line to
  `known-failures/README.md` in its existing style (check it is not already there).
- A short `tools/fuzz/README.md`: how to run it, what it covers, what it does not.

## Deliverables
- Program generator covering primitives, control flow, methods and primitive arrays
- Runner comparing Java and the translation, with rejection counts
- Reducer for mismatching programs
- A few hundred seeds run; every distinct mismatch reduced into known-failures/
- tools/fuzz/README.md
