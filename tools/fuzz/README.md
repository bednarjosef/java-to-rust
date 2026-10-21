# Differential fuzzer

Generates random Java programs from seeds, runs each on Java and as translated Rust, and
reports the programs whose output or exit code differ. A reducer shrinks one failing program
while the same failure persists. Python 3 standard library only.

## Running

    cargo build --release -p java-to-rust          # run.py builds it too, unless --no-build
    python3 tools/fuzz/run.py --count 300 --start 1 --jobs 4
    python3 tools/fuzz/run.py --count 300 --start 1 --jobs 4 --strings
    python3 tools/fuzz/run.py --count 300 --start 1 --jobs 4 --objects

    python3 tools/fuzz/reduce.py target/fuzz/runs/s17/Prog.java --out target/fuzz/reduced/s17.java

`run.py` options: `--count` (seeds, default 200), `--start` (first seed, default 1), `--size`
(statements in `main`, default 30), `--jobs` (programs in parallel, default 4), `--strings`
(also generate the String, StringBuilder, `String.format` and `Arrays` code described below),
`--objects` (also generate the classes, interfaces and generics described below; it combines with
`--strings`), `--no-build`. It prints one status per program, the rejection messages with their counts, and
the first failures of each kind. Every result is in `target/fuzz/results.jsonl`.

`fuzz.py SEED [SIZE] [--strings] [--objects]` prints the program for one seed, for inspection.

Text is read and written as UTF-8 whatever the locale, because the string features print
non-ASCII characters. Java runs with `-Dstdout.encoding=UTF-8`. `String.format` uses the default
locale, so run the fuzzer with a fixed locale (`LANG=C.UTF-8`) when comparing runs.

Statuses:

- `match`: the translation prints the same stdout and exits with the same code as Java
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

With `--objects` (`fuzz_objects.py` for the types and expressions,
`fuzz_object_decls.py` for the declarations, `fuzz_object_stmts.py` for the statements), also:

- One or two class trees of one to three levels as static nested classes: abstract and concrete
  classes, fields of primitive and `String` type with and without initializers (which may
  change statics), a nullable field of the tree's own type, static counters (whose initializers
  may read statics) and static methods, one or two constructors with `super(...)` or `this(...)`,
  printing and sometimes calling an overridable method
- Method groups with ranks: abstract, concrete and overriding methods returning `int` or
  `String`, with no parameters, `int` ones, or another object of the tree; `super.m(..)` calls;
  an `ov` overload family (`int`, `long`, `double`) across subclasses; `touch`, which returns
  `this`. A body calls only groups of a lower rank, so every call chain ends.
- Zero to two interfaces with abstract, default and static methods and a constant, implemented by
  one or more classes (a class may implement two), `I.super.m()` calls
- `toString`, `equals(Object)` (pattern or cast form) and `hashCode` overrides, and the defaults
  where a class has none; `Comparable<Self>` implemented by a root class
- Generic classes `GBox<T>`, `GPair<A, B>` (with `swap`), `GStack<T>` over `GNode<T>`, and
  `GSorted<T extends Comparable<T>>`, with or without `toString` (so both real and erased
  generics); generic methods `pick`, `boxOf` and `maxOf`; type arguments of the program's types,
  `String`, `Integer`, `Object` and `GBox<..>`
- In `main` and Prog's static methods: variables of class, interface, `Object` and generic types,
  assigned new objects, aliases, upcasts, guarded downcasts (`x instanceof C ? (C) x : ..`),
  conditionals, factory calls and container reads; field writes through variables, array
  elements and `GBox.get()`; calls; `instanceof` with and without a pattern, negated patterns,
  chains over an `Object`; `==` between objects, `equals`, `hashCode` where every class it may
  run overrides it; null variables and null tests that narrow; arrays of objects (with null
  elements from `new C[n]`), for and for-each loops over them; overloads of a static `which` on a
  superclass chain, `Object`, `int`, `long` and `String`; an `Object` static field holding an
  object; `describe(Object)` with a chain of patterns
- Exceptions (`fuzz_exceptions.py`, in most programs): the program's own `Fail extends
  RuntimeException` with a field, `Worse extends Fail` and a checked `Checked extends Exception`;
  methods that throw them; `try` blocks that throw behind a condition, call those methods or
  divide by zero, with `catch` clauses from the most specific (`Worse`, `Fail`, `Checked`,
  `ArithmeticException`, `RuntimeException`) printing the message, the exception or its field,
  and sometimes `finally`. Rarely a failing cast or a null dereference inside a `try` that
  catches it, which the translator rejects for now
- Deliberate exceptions: a failing cast (`ClassCastException`), a null dereference
  (`NullPointerException`) or a `Fail`, uncaught, at the statement that `zero0` would use

An object is printed only when every class it may be has a `toString` of the program's, so no
identity hash is printed; values that may be null are only dereferenced behind a null test.

Every program terminates: loops have constant bounds, and recursion counts a depth parameter
down from at most 4.

## What it does not cover

- Without `--objects`: classes, objects, `null`, `instanceof`, inheritance, interfaces, generics.
  With it: fields hidden by subclasses, private members, interfaces extending interfaces,
  generic interfaces, bounded type variables other than `Comparable`, wildcards, records
- Enums, lambdas, streams, inner or anonymous classes, multiple top-level classes
- Two-dimensional or array-of-array values, arrays of `String`, `Arrays` methods other than the four
  listed above
- String methods other than those listed, `String` values with `null`, `Character` and `Integer`
  static methods, `String.join`, regular expressions, `StringBuilder` methods other than the
  listed ones
- `try`/`catch`/`finally`, exceptions other than the two above, `System.exit`
- Input (`Scanner`, `args`), threads, time, random numbers
- Programs much larger than `--size` statements, or deeper nesting than the generator's limits

Programs are random and short, so a run finds the failures that many seeds reach. A clean run
over a few hundred seeds is evidence, not proof: check the feature list above before reading one
as coverage of the whole subset.
