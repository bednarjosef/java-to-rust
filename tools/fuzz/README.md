# Differential fuzzer

Generates random Java programs from seeds, runs each on Java and as translated Rust, and
reports the programs whose output or exit code differ. A reducer shrinks one failing program
while the same failure persists. Python 3 standard library only.

## Running

    cargo build --release -p java-to-rust          # run.py builds it too, unless --no-build
    python3 tools/fuzz/run.py --count 300 --start 1 --jobs 4

    python3 tools/fuzz/reduce.py target/fuzz/runs/s17/Prog.java --out target/fuzz/reduced/s17.java

`run.py` options: `--count` (seeds, default 200), `--start` (first seed, default 1), `--size`
(statements in `main`, default 30), `--jobs` (programs in parallel, default 4), `--no-build`.
It prints one status per program, the rejection messages with their counts, and the first
failures of each kind. Every result is in `target/fuzz/results.jsonl`.

`fuzz.py SEED [SIZE]` prints the program for one seed, for inspection.

Statuses:

- `match`: the translation prints the same stdout and exits with the same code as Java
- `mismatch`: they differ; `target/fuzz/runs/<seed>/` keeps `Prog.java`, `java.out`, `rust.out`
- `rustc`: the translation does not compile; the detail is the first rustc error
- `translator-crash`: the translator panics or exits without a rejection message
- `rust-timeout`: the translation runs past 20 seconds
- `rejected`: the translator stops with a message such as "Object not supported yet"
- `invalid`, `java-timeout`, `harness-error`: a bug in the fuzzer itself, not the translator

`reduce.py` keeps a candidate only when it has the same status and key as the original (for
`rustc`, the error code; for a rejection, the message). It deletes statements (a line, or a block
from a line ending in `{` to its matching `}`) and then replaces parenthesised groups with `0`,
`1`, `true`, `false` or one operand of the top-level operator. Candidates run in parallel
batches, the first kept candidate in order is accepted, and `--max-tests` (default 400) caps the
work. A candidate that javac rejects gets the status `invalid`, so it is never kept.

Work files go to `target/fuzz/` (git-ignored): `crates/<slot>/` are Cargo projects reused across
programs, so each build only recompiles `main.rs`.

## What it covers

- Primitive locals and static fields of every numeric type and `boolean`, `char`, `byte`, `short`
- Every binary, unary, shift and compound-assignment operator; `=`, compound assignments and
  `++`/`--` in expressions, on variables and on array elements
- Casts between primitives, ternaries (some with branches of two types), `Math.abs`, `Math.min`,
  `Math.max`, `Math.floorMod`
- String concatenation of mixed types printed with `System.out.println`, and a `String` accumulator
- `if`, `while`, `do`, `for`, `switch` with fall-through and `break`, labeled `break`/`continue`
- Methods with parameters and return values, recursion with a depth bound
- Primitive arrays: `new T[n]`, array initializers, indexing with `Math.floorMod`, `length`,
  arrays as method parameters, arrays in static fields
- Deliberate exceptions: an out-of-bounds index or division by zero at a chosen statement (about
  15% of programs), so exit code 1 must match

Every program terminates: loops have constant bounds, and recursion counts a depth parameter
down from at most 4.

## What it does not cover

- Classes, objects, fields of instance type, `null`, `instanceof`, inheritance, interfaces
- Enums, generics, lambdas, streams, inner or anonymous classes, multiple top-level classes
- Two-dimensional or array-of-array values, `Arrays.*`, `String` methods other than `+`
- `try`/`catch`/`finally`, exceptions other than the two above, `System.exit`
- Input (`Scanner`, `args`), threads, time, random numbers
- Programs much larger than `--size` statements, or deeper nesting than the generator's limits

Programs are random and short, so a run finds the failures that many seeds reach. A clean run
over a few hundred seeds is evidence, not proof: check the feature list above before reading one
as coverage of the whole subset.
