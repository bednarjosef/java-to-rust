# Differential fuzzer

Generates random Java programs from seeds, runs each on Java and as translated Rust, and
reports the programs whose output or exit code differ. A reducer shrinks one failing program
while the same failure persists. Python 3 standard library only.

## Running

    cargo build --release -p java-to-rust          # run.py builds it too, unless --no-build
    python3 tools/fuzz/run.py --count 300 --start 1 --jobs 4
    python3 tools/fuzz/run.py --count 300 --start 1 --jobs 4 --strings
    python3 tools/fuzz/run.py --count 300 --start 1 --jobs 4 --arrays

    python3 tools/fuzz/reduce.py target/fuzz/runs/s17/Prog.java --out target/fuzz/reduced/s17.java

`run.py` options: `--count` (seeds, default 200), `--start` (first seed, default 1), `--size`
(statements in `main`, default 30), `--jobs` (programs in parallel, default 4), `--strings`
(also generate the String, StringBuilder, `String.format` and `Arrays` code described below),
`--arrays` (also generate the array code described below; it combines with `--strings`),
`--no-build`. It prints one status per program, the rejection messages with their counts, and
the first failures of each kind. Every result is in `target/fuzz/results.jsonl`.

Two runs at once, of two builds of the translator say, need their own work directories: the
environment variables `FUZZ_WORK` (instead of `target/fuzz`) and `FUZZ_TRANSLATOR` (instead of
`target/release/java-to-rust`, with `--no-build`) apply to `run.py` and `reduce.py`.

`fuzz.py SEED [SIZE] [--strings] [--arrays]` prints the program for one seed, for inspection.

Text is read and written as UTF-8 whatever the locale, because the string features print
non-ASCII characters. Java runs with `-Dstdout.encoding=UTF-8`. `String.format` uses the default
locale, so run the fuzzer with a fixed locale (`LANG=C.UTF-8`) when comparing runs.

Statuses:

- `match`: the translation prints the same stdout and exits with the same code as Java, and an
  uncaught exception has the same class and message (the first line of standard error; for a
  `NullPointerException` only the class, as Java's message describes the null expression)
- `mismatch`: they differ; `target/fuzz/runs/<seed>/` keeps `Prog.java`, `java.out`, `rust.out`
- `rustc`: the translation does not compile; the detail is the first rustc error
- `translator-crash`: the translator panics or exits without a rejection message
- `rust-timeout`: the translation runs past 20 seconds
- `rejected`: the translator stops with a message such as "Object not supported yet"
- `invalid`, `java-timeout`, `harness-error`: a bug in the fuzzer itself, not the translator

`reduce.py PROGRAM [--out FILE] [--jobs 4] [--max-tests 400] [--no-build]` shrinks a failing
program. A failure's class is its status and key: for `rustc`, the error code (or the first 60
characters of a message without one); for `rejected`, the message; for `mismatch`, `translator-crash`
and `rust-timeout`, the status alone. A candidate is kept only when it has the same class as the
original, so the reduced program fails for the same reason.

- Deletion first: runs of consecutive statements or members, bottom up, from the largest runs
  (halving, as in delta debugging) to single lines. A block counts from a line ending in `{` to
  its matching `}`. The class header and the `main` method are never deleted.
- Then simplification: a parenthesised group is replaced by `0`, `1`, `true`, `false` or one
  operand of its top-level operator. Groups that follow a name (calls) are left alone.
- The two passes repeat until neither shrinks the program.

Each candidate goes through the translator first. The translator runs javac, so a candidate that
javac rejects gets the status `invalid` and is never kept, and no Rust is built for it. Rust is
built only for programs the translator accepts, in a Cargo crate that is reused, so only
`src/main.rs` is recompiled. Java runs only when a mismatch needs its output. Candidates run in
parallel batches (`--jobs`); the first kept candidate in order is accepted. Results are cached by
text, so no candidate runs twice, and `--max-tests` caps the total. A reduction that stops at the
cap is still a valid program: it fails the same way, just not minimal.

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

With `--strings`, also:

- `String` locals and a `String` static field, assigned, appended with `+=`, and capped in length
- Literals with non-ASCII text (`é`, `日本語`, `ß`), escapes, a surrogate pair written as escapes,
  and an emoji literal; `char` literals with non-ASCII and escaped values
- Concatenation of a string with every primitive type, and `String.valueOf` of each
- `length`, `charAt`, `indexOf`, `compareTo`, `equals`, `startsWith`, `contains`, `replace` (char
  and string forms), `toUpperCase`, `trim`, `substring` with one and two indices, `split` with a
  literal separator, indexed by `Math.floorMod`
- `String.format` with `%d`, `%x`, `%08d`, `%c`, `%5.2f`, `%s`, `%-8s`, one to three per call
- `StringBuilder`: `new` with and without a string, `append`, `insert`, `reverse`, `setCharAt`,
  `deleteCharAt`, `toString`; its length is capped
- `switch` on a string, with `case`, fall-through, `default` and `break`
- `Arrays.toString`, `Arrays.sort` (not `boolean[]`, which Java does not have), `Arrays.fill` and
  `Arrays.equals` on primitive arrays

Every index is reduced into range with `Math.floorMod`, so string programs do not throw (apart
from the deliberate exceptions above).

With `--arrays` (`fuzz_arrays.py`), also:

- Arrays of one, two and three dimensions of every primitive type and `String`: `new` with all
  or some sizes, literals, jagged rows made by a loop, `null` rows and `null` strings
- Second names: `b = a`, `x = c ? a : b`, a row in a local (`int[] r = m[i]`), a row loop
  variable, a parameter, a static, an array returned by the method it was given to; writes
  through one name and reads through the others
- Moves and swaps: `a = b; b = new ..`, `t = a; a = b; b = t;`, `a = c ? b : a`, `a = a.clone()`
- Rows replaced by new rows, by other rows (`m[i] = m[j]`), by a local array or a clone, and
  swapped
- For-each over arrays and over rows, with writes to the array and to the row in the body
- Counting loops bounded by `length`, by a variable (`Math.min` of two lengths), by a guard in
  the body, downward, by twos, from 1, `<=` to `length - 1`, as a `while`, over the rows of a
  grid and product-like over two grids; each with or without other statements in its body
- Helper methods that read, write, return the array they were given, return a new one, keep it
  in a static, pick one of two, copy one into another (also into itself), replace their
  parameter, recurse over it, take a grid and a row of it
- `a.clone()`, `==` on arrays and rows, `Arrays.equals`, `hashCode`, `sort` (whole and by range),
  `fill` (whole and by range), `toString`, `deepToString`, `binarySearch`, `copyOf`,
  `copyOfRange`, `System.arraycopy` (also within one array), `new String(char[])`
- Stores whose value replaces the array: `a[a.length - 1] = (a = b)[0]`
- The deliberate exceptions, at the chosen statement, are array failures in Java: an index past
  the end or below zero in a loop that prints first, a bound taken before the array was
  replaced, a size known at the declaration and then changed, a null row, a negative size,
  a jagged row, `System.arraycopy`, `Arrays.fill` and `copyOfRange` out of range, a for-each over
  a null row, a null string's `length()`

An array is tracked with a tag: plain (not null, not empty, rows likewise, strings not null),
so any index reduced with `Math.floorMod` is in range, or loose (may be empty, rows may be null),
whose every access is guarded. A plain array may also be private: no statement gives it a
second name (no alias, row local, `==`, `Arrays` call, clone or call that keeps or returns it),
so the translator keeps it as a `Vec` and exercises borrowed parameters, moves and loops over
slices. The arrays a counting loop walks cannot be assigned in its body,
so only the deliberate exceptions throw. Half of these programs have no array statics, since
a static array keeps the translator from lending shared arrays to calls.

Every program terminates: loops have constant bounds, and recursion counts a depth parameter
down from at most 4.

## What it does not cover

- Classes, objects, fields of instance type, `null`, `instanceof`, inheritance, interfaces
- Enums, generics, lambdas, streams, inner or anonymous classes, multiple top-level classes
- Without `--arrays`: two-dimensional or array-of-array values, arrays of `String`, `Arrays`
  methods other than the four listed above; with it, arrays of objects and arrays in fields
- String methods other than those listed, `String` values with `null`, `Character` and `Integer`
  static methods, `String.join`, regular expressions, `StringBuilder` methods other than the
  listed ones
- `try`/`catch`/`finally`, exceptions other than the two above, `System.exit`
- Input (`Scanner`, `args`), threads, time, random numbers
- Programs much larger than `--size` statements, or deeper nesting than the generator's limits

Programs are random and short, so a run finds the failures that many seeds reach. A clean run
over a few hundred seeds is evidence, not proof: check the feature list above before reading one
as coverage of the whole subset.
