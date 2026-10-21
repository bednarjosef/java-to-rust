# java-to-rust

A tool that converts Java source code into Rust source code, without LLMs: a compiler-style
pipeline of parsing, type resolution, program analysis and code generation.

The goal is Rust that behaves exactly like the original program and, wherever the tool can
prove it safe, is written the way a person would write Rust: owned values instead of shared
objects, enums instead of class hierarchies, no null checks where nothing can be null.

## How it works

```
Java source -> typed syntax tree -> intermediate representation -> analyses
            -> facts about the program -> Rust code generator -> Rust crate
```

The analyses prove facts and store them in a table keyed by expression id (`analysis.rs`). The
code generator writes the plain form of a construct only where a fact allows it, and the faithful
form otherwise. A better analysis therefore improves the output without touching the generator.
Object representations are decided per class (`analysis/objects.rs`, `analysis/owned.rs`), and
array representations per local (`analysis/arrays.rs`).

### Level 1: faithful translation

Every value starts at the safe default. An object is `Rc<RefCell<C>>`, because Java references are
shared and mutable, and a reference that may be null is an `Option` of it. An array is a shared
`java::Array<T>`, a string is a Rust `String`, and `null` is `None`. Objects are freed by reference
counting, not by a garbage collector, so a cycle of objects is never freed. The runtime crate
(`crates/runtime`) supplies what Rust lacks: Java's exact number semantics (wrapping integer
overflow, Java's float rules and printing), strings, `StringBuilder`, `String.format`, regular
expressions and charsets with Java's results. Interfaces, exceptions and generics are not
translated yet (see "Not yet" below).

### Level 2: analysis-driven Rust

Analyses prove facts and move a value to a plainer form, one step at a time. Whatever no analysis
can prove stays at the level 1 form, so the output is always correct. Done on main:

1. Classes whose objects never change after construction are `Rc<C>`, shared without a `RefCell`
   (`analysis/objects.rs`).
2. Classes whose objects are never shared are plain structs, and their methods take `&self` or
   `&mut self` (`analysis/owned.rs`, `analysis/borrowing.rs`).
3. Nullness analysis removes `Option`s and null checks where nothing can be null
   (`analysis/nullness.rs`).
4. Range analysis makes arithmetic plain where it cannot overflow (`analysis/ranges.rs`).
5. Arrays that no other name can see are plain `Vec`s (`analysis/arrays.rs`).

Not done: `final` classes and sealed hierarchies losing dynamic dispatch and becoming `enum`s
(there is no inheritance yet, see docs/thorough-mode.md), escape analysis (objects that never
leave a method as stack values), ownership of objects held in one place as owned fields, `Box`es
and `Vec` elements, and dynamic profiling from the program's tests.

### Idiomatic output

Statements have plainer forms, chosen only where the meaning is the same. A counting loop
`for (int i = a; i < b; i++)` becomes `for i in a..b` when nothing in the body or the bound can
change while it runs. In a function body, a final `return x;` becomes the tail expression `x`, and
`T x = e; return x;` becomes `e`.

Expressions read the same way. Parentheses appear only where precedence needs them, and a
compound assignment is written in Rust's `op=` form where it does what Java's does. A range proof
lets `+ - *` stand in for `wrapping_*`, for example when a counter is bounded by a constant.
Operations that could overflow in Java stay wrapping.

`scripts/clippy-corpus.sh` counts clippy and rustc warnings in the generated code of all 196
corpus programs: 213 in total. The largest groups are `eq_op` (33), `approx_constant` (20),
`unnecessary_to_owned` (20), `let_and_return` (17) and `dead_code` (12). `needless_return`,
`single_match`, `never_loop` and `unused_parens` are at zero. Most of what remains is code Java
itself writes (`eq_op`, `approx_constant`, `zero_divided_by_zero`), object and array code
(`unnecessary_to_owned`), and reference-typed locals (`let_and_return`), which need the nullness
and aliasing work first.

## Testing

Every test program runs on Java and, translated, as Rust; printed output and exit code must match
exactly. The Java run is the expected result, so no expectations are written by hand.

The test programs come from:

- `corpus/features`: 161 small programs, one or more per language feature;
- `corpus/programs`: 32 larger realistic programs (algorithms, data structures, parsers,
  interpreters, simulations);
- `corpus/projects`: 3 multi-file programs (`imports`, `pruning`, `statics`), each with a source
  root `src/` and a file `main` that names the main class. The corpus has no open-source projects
  yet;
- `tools/fuzz`: random programs generated from a seed (`fuzz.py`, with `fuzz_strings.py` for
  strings and builders under `--strings`), run and compared by `run.py`, and shrunk to a minimal
  case by `reduce.py`. A minimal case that still fails is kept in `known-failures/` (10 programs)
  and stays out of the corpus until it is fixed.

`targets/` holds 28 typical Java programs that the translator does not translate yet, run one by
one and as a whole program (`targets/run.sh`); `targets/README.md` records what rejects each one.

Measured: the corpus test (pass or fail per program, with the first differing lines of output),
the clippy and rustc warnings of the generated code, and the speed of the benchmarks against Java.
Not measured yet: the share of values that became owned or plain, the remaining `Option`s and
null checks, dynamic calls, `unsafe` code, and memory.

## Performance

`benchmarks/` holds 11 programs: sieve, Collatz, n-body, Mandelbrot, matrix multiply, sorting a
million ints, binary trees, a linked list (`NodeList`), string building, statics and a bytecode
interpreter. `python3 scripts/benchmark.py [--runs N]` checks that each translation prints what
Java prints, then times both sides as the median of N wall-clock runs (default 5, JVM start-up
included). A program that warms up also reports the time of its timed kernel. Memory is not
measured.

`docs/performance.md` has the table from one run on the `array-ownership` branch, and the causes
of the slow cases. `NodeList` does not compile yet.

## First milestone

Level 1 end to end for a subset of Java: classes, interfaces, inheritance, generics, the common
collections, strings and exceptions; no reflection and no threads. Done when the feature corpus
translates, compiles and matches Java's output. The corpus meets the string, array and expression
parts of this; the object parts, interfaces, generics, collections and exceptions are not reached
yet (see "Not yet").

## Current state

### What translates

A small subset, end to end:

- **Types and statements**: `byte`, `short`, `char`, `int`, `long`, `float`, `double`, `boolean`
  and `String` locals; arithmetic; `if`, `while`, `do`, `for`, `break` and `continue` (labelled
  too); the conditional operator; `switch` statements and expressions on integers, chars, strings
  and enums (case labels may be constant expressions, which are folded as javac folds them);
  `System.exit`.
- **Classes**: static and instance methods, overloading, varargs, fields with initializers,
  instance initializer blocks, static fields, constructors (including `this(...)`), `new`,
  object-typed fields and `null` objects, and a user `toString` in `println` and concatenation.
  `synchronized (x)` checks `x` for null and runs the block: a program has one thread, so the lock
  does nothing.
- **Output**: `System.out.print` and `println` are buffered (`java::println!`).
- **Library**: a set of `Math`, `StrictMath`, `Double`, `Float`, `Boolean`, `Integer`, `Long`,
  `Byte`, `Short` and `Character` methods, `String`, `StringBuilder` and `StringBuffer`, and
  `String.format`, all hand-written in the runtime with Java's exact results:
  - `Math` and `StrictMath`: `floorDiv`, `floorMod`, the `*Exact` integer methods (they throw on
    overflow), `round`, `rint`, `clamp`, `copySign`, `hypot`, `fma`, `ulp`, `nextUp`/`nextDown`,
    `max`/`min` on every type, and the rest of the common ones.
  - `Double`, `Float` and `Boolean`: `isNaN`, `isInfinite`, `isFinite`, `compare`, `sum`,
    `max`/`min`, `parseDouble`, `parseFloat`, `parseBoolean`, `logicalAnd`/`logicalOr`/
    `logicalXor`, and the `valueOf` of both with Java's grammar and rounding (whitespace,
    suffixes, hexadecimal floats).
  - `Integer` and `Long`: parsing with a radix, the bit and radix methods (`toHexString`,
    `bitCount`, `numberOfLeadingZeros`, `toString(v, radix)`, ...), and the unsigned methods
    (`divideUnsigned`, `remainderUnsigned`, `compareUnsigned`, `toUnsignedString`,
    `parseUnsignedInt`, `toUnsignedLong`, `toUnsignedInt`), which `Byte` and `Short` share.
  - `Character`: the code point methods (`toChars`, `toString(int)`, `charCount`,
    `toCodePoint`, `highSurrogate`/`lowSurrogate`, `isValidCodePoint`, ...), the tests
    `isWhitespace`, `isLetterOrDigit`, `isAlphabetic`, `isSpaceChar`, `isSurrogate`, `digit`,
    `forDigit` and `getNumericValue`, case conversion, and the `MIN_VALUE`, `MAX_VALUE` and
    `MIN_NORMAL` constants of the wrapper classes.
  - `String`: searching, slicing, case, trimming, replacing, comparing, `split`, `replaceAll`,
    `matches`, `join`, `repeat`, `hashCode`, `toCharArray`, `valueOf` and `new String(char[])`.

The pieces: javac compiles the source (parse and attribute only) through a small Java program,
`crates/translator/java/JavaTree.java`, that prints javac's syntax tree as JSON with the type of
every expression and the declaration every name and call refers to. `lower.rs` maps that tree onto
the supported subset (`ast.rs`), the analyses prove facts about the program (`analysis.rs`), and
the generator writes Rust (`codegen.rs`) that uses the runtime crate (`crates/runtime`) for what
Rust lacks: Java's double and float printing, integer division that throws, `Math.max`/`min` on
NaN, printing a `char`, `String.charAt` by UTF-16 unit and the `Character` methods.

#### Numbers, characters and strings

`byte`, `short`, `char` and `float` are `i8`, `i16`, `u16` (Java's `char` is a UTF-16 code unit,
not a Unicode scalar value like Rust's `char`) and `f32`. Java promotes the small types to `int`
for arithmetic, so `c - 'a'` is `c as i32 - 'a' as i32`, and narrows with casts that keep the low
bits, as Rust's `as` does; floating values reach the small types through `int`
(`d as i32 as u16`). `b += x` and `b++` narrow implicitly: wrapping arithmetic in the small type
keeps the same bits, so they become `b = b.wrapping_add(x as i8)` and `b = b.wrapping_add(1)`.
Strings are Rust strings, so a lone surrogate `char` turns into `?` when it is printed or
concatenated (as Java's UTF-8 output prints it), even where Java would later pair it up. The
`Character` methods are exact on Latin-1, ASCII and fullwidth letters and digits, and
`isWhitespace` excludes no-break spaces as Java does. Beyond Latin-1, `isLetter`, `isAlphabetic`
and `isDigit` use Rust's Unicode tables, which are not Java's general categories, so they can
differ from Java there; `Character.digit` recognises only ASCII and fullwidth letters and digits,
so it can differ from Java on other decimal digits.

The `String` methods count in UTF-16 units as Java does (`indexOf`, `substring`, `charAt` agree on
`"né😀e"`), and throw what Java throws (`StringIndexOutOfBoundsException` with Java's message).
`toUpperCase` and `toLowerCase` are the root-locale mappings. A `StringBuilder` is a
`java::StringBuilder`, a shared handle to UTF-16 units like an array: every variable, field and
parameter that holds the builder sees it change, `reverse` keeps surrogate pairs in order, and a
chain is Rust's chain (`sb.append_str("a").append(1)`), the handle cloned only where the builder
is used as a value.

`split`, `replaceAll`, `replaceFirst` and `matches` run a small backtracking regular expression
engine with a literal fast path: literals and escapes, `.`, `\d \s \w` and their negations, sets
and ranges, groups, alternation, `^ $` and greedy and lazy quantifiers. What it lacks (lookaround,
flags, backreferences, `\b`, `\p{..}`, possessive quantifiers, set intersections) is rejected when
the pattern is a constant and throws `UnsupportedOperationException` when it is not; a malformed
pattern throws `PatternSyntaxException` as in Java.

`String.format` is Java's `Formatter` in the US locale for `%b %h %s %c %d %o %x %e %f %g %n %%`
with argument indexes, flags, width and precision, and rounds floating point half up from the
shortest decimal as Java does (`%.1f` of `0.15` is `0.2`, where C's `printf` gives `0.1`); `%t`
and `%a` are rejected in a constant format. A format argument keeps its Java type
(`java::Arg::Long(n)`), so a mismatch throws `IllegalFormatConversionException` as in Java.

Strings and bytes meet through `getBytes()`, `getBytes(charset)` and `getBytes("name")`, and
`new String(bytes)`, `new String(bytes, charset)`, `new String(bytes, offset, length, charset)`
and the name forms, for UTF-8, US-ASCII, ISO-8859-1, UTF-16 (with its byte order mark), UTF-16BE
and UTF-16LE. The `StandardCharsets` constants and `Charset.forName` are charsets only in those
arguments: a `Charset` kept in a variable or passed elsewhere is rejected. Encoding replaces
unmappable characters as Java does, and decoding replaces malformed input as Java's `String` does
(`crates/runtime/src/charset.rs`).

#### Enums

An enum is a `Copy` Rust `enum` with a variant per constant. Its fields hold one value per
constant, given by its field initializers and constructor with constant arguments, so each field
is an accessor that matches on `self` (named `mass_field` when the enum has a method `mass`); a
field that is not final is accepted as long as only the constructor sets it. An enum's own
`toString` is its `Display`, while `name()` stays the constant's name. A constant with a body
(`PLUS { int apply(int a, int b) { return a + b; } }`) overrides the enum's methods, so each
overridden method becomes a `match self` with an arm per overriding constant and the enum's own
body (if it is not abstract) for the rest.

### Analyses

The faithful translation of `a + b` on ints is `a.wrapping_add(b)` and of `a % b` is
`java::rem(a, b)`. The analyses decide where plain Rust means the same:

- Range analysis (`analysis/ranges.rs`) tracks the values every integer variable (`byte`,
  `short`, `char`, `int`, `long`) can hold, by abstract interpretation over intervals: conditions
  narrow ranges, loops are solved with widening to the program's constants and narrowing,
  counters in counting loops are bounded by the trip count, and argument and return ranges flow
  between methods. Arithmetic that provably stays in range uses `+`, `-`, `*`, and division by a
  provably nonzero divisor uses `/` and `%`. On the small types it also shows when a compound
  assignment or a narrowing cast keeps its value: `b += 5` stays `b += 5`, and
  `(char) (c + 1)` becomes `c + 1` in `u16` (or `c.wrapping_add(1)` when it may wrap).
- Strings that are only ever ASCII use `len()` for `length()` and index bytes for `charAt`
  (`java::ascii_char_at`); inside `for (int i = 0; i < s.length(); i++)` the index is also in
  bounds (`analysis/indexing.rs`), so `s.charAt(i)` is `s.as_bytes()[i as usize] as u16`. String
  parameters that are never assigned are borrowed as `&str`.
- Nullness analysis (`analysis/nullness.rs`) finds the strings and objects that may be null
  (locals, parameters across calls, return values, fields and static fields). Only those are
  `Option<String>` or `Option<Rc<..>>`; the others stay `String`, `&str` or `Rc<..>`. A field is
  not null when every constructor sets it, before anything can read it, to a value that is not
  null. A null check narrows the value in the branch where it is not null
  (`if let Some(s) = s.as_deref()`, `if let Some(n) = n.as_ref()`, or
  `let Some(s) = s.as_deref() else { .. }` after an early return), so no unwrap is repeated.
  Dereferencing null throws `NullPointerException` through `java::non_null`, and printing or
  concatenating it gives `null` (`java::or_null`). `==` on objects compares identity:
  `Rc::ptr_eq(&a, &b)`, or `java::same(a.as_ref(), b.as_ref())` where either may be null.
- Arrays (`int[]`, `long[]`, `double[]`, `boolean[]`, `String[]`, and arrays of arrays) are shared
  references in the faithful form, `java::Array<T>`: a shared cell whose clones see each other's
  writes. The aliasing analysis (`analysis/arrays.rs`) finds local arrays that no other name can
  see, and those become a plain `Vec<T>`, or `Vec<Vec<T>>` (at any depth) for arrays of arrays
  of primitives or strings whose rows are only reached through the array. A plain local may take
  a new array, a literal, another plain local that is dead afterwards (`a = b; b = c;` moves the
  `Vec`), or the result of a method that returns only arrays it made, which then returns the
  `Vec`. A plain array that meets code needing a shared one is converted once with `java::share`.
  A second name that is used later (`int[] b = a;`, a row kept in a variable, a field, `==`,
  `java.util.Arrays`) keeps the array shared, so writes through one name show through the other.
  Indexing a `Vec` is `a[i as usize]` where the bounds analysis (`analysis/arrays/bounds.rs`)
  proves the index in range (loop and `if` guards, lengths of rows, declared sizes), and
  `java::at`/`java::put`/`java::modify` otherwise, whose failing path is cold and out of line.
  Bounds and size errors go through `java::throw`, so they exit with code 1 as Java does. A
  parameter becomes `&[T]`, `&mut [T]`, `&[Vec<T>]` or `&mut [Vec<T>]` when every call can lend
  it a plain array or a row of one, a counting loop that only reads `a[i]` becomes
  `for x in a.iter().cloned()`, and a for-each over the rows of a plain array borrows them
  (`for row in m.iter()`). An innermost counting loop that reaches a plain array only at its
  index checks once, before it starts, that the whole range is in bounds, and then runs over
  slices with no checks (`let b_row = &b[k][..n];`); otherwise it runs with Java's checks
  (`analysis/arrays/slices.rs`). Arrays of strings, objects or arrays hold `Option`s when some
  array of that element type may hold null (`new String[n]`, `new int[n][]`, or a stored value
  that may be null), and plain values otherwise.
- `java.util.Arrays` on arrays of primitives and strings (`lower/java_arrays.rs`,
  `codegen/java_arrays.rs`, `runtime/src/java_arrays.rs`): `equals` and `hashCode` (Java's exact
  formula, bit equality for floats, null arrays and null elements), `sort` for every primitive
  type and `String` (Java's order: `-0.0` before `0.0`, NaN last, UTF-16 order for strings), whole
  or by range with Java's range checks, `fill` by range, `copyOfRange` on primitives,
  `binarySearch` in both forms (`-(insertion point) - 1` when absent), and `deepToString` on
  arrays of primitive or `String` rows. `a.clone()` is a shallow copy; rows of a 2-D array are
  shared, as in Java. Sorting an array with a null `String` throws `NullPointerException`. Not
  yet: `copyOf` and `copyOfRange` on `String` arrays (the padding would need nulls that the
  nullness analysis does not see), `equals(null, b)`, `asList` and `stream`.
- Objects (`analysis/objects.rs`, `analysis/owned.rs`, `analysis/borrowing.rs`). The faithful form
  of an object is `Rc<RefCell<C>>`, because Java references are shared and mutable. A class whose
  objects never change after construction is shared as `Rc<C>`: its fields are written only by its
  constructors, through `this`, and `this` never escapes. A class whose objects are never shared
  (no class-typed field, parameter or return value, and each object is created by `new` into a
  local that only calls or prints it) is a plain value: its methods take `&self` or `&mut self`,
  and a call on it uses method syntax so that `p.set(p.x + 1)` borrows as Rust allows. Methods of
  shared classes borrow their object too (`analysis/borrowing.rs`) when they use `this` only for
  its fields and for such calls, and nothing they run reaches an object of their class another
  way; a call is then `p.borrow_mut().set(2)`, with arguments that could reach the object bound
  first. Constructors build a plain value, starting from a struct literal that takes in the body's
  leading field assignments (`P { x, y }`; values with effects are computed into locals first
  where the literal's field order would change Java's order), and `new` shares it
  (`Rc::new(RefCell::new(P::new(1, 2)))`). A `toString` becomes `impl fmt::Display`, and an object
  is printed as `{p}` when its `toString` can neither print nor throw. An object parameter that
  the method only uses through (fields, calls, comparison, printing, or passing it on to such a
  parameter) is a `&Rc<..>` that the caller lends.

### Whole programs

Whole programs (`program.rs`) spread over files and packages translate too: javac compiles all of
them in one run, with the source roots as its source path, and a reachability pass over its trees
(`program/reach.rs`) starts at `main` and follows every resolved call, `new`, field access and
type, so only the classes, methods, constructors and fields that `main` can reach are lowered and
translated. Library roots (`--library`, meant for the JDK's own sources) are only on the source
path: javac reads a file from them when the program uses one of its classes. Packages become
nested modules in one output file (`com.example.util` is `mod com { pub(crate) mod example
{ pub(crate) mod util { .. } } }`); a class of another package is named by its path from the crate
root, such as `crate::com::example::util::Counter::new(..)`, and items in packages are
`pub(crate)`. A program without packages is written like a single file.

Reachability dispatches calls as Java does: javac lists the methods each method overrides, and a
call of an instance method also reaches its overriders in every class that an instantiated class
is, extends or implements. It stops at the classes that the runtime provides (`String`,
`StringBuilder`, `System`, `Math`, the boxes, `Object`, `Class`, `Thread`, the `java.lang.ref`
references; `RUNTIME_CLASSES`), whose sources reach deep into the JVM, and at what is not
translated: reflection, method handles, serialization and security (`UNTRANSLATED`). Library code
reaches those on paths that only its data rules out, as `HashMap` reaches reflection through
`comparableClassFor` and parallel streams reach threads, so the boundary is needed. A program that
puts one entry into a `HashMap` reaches 55 classes this way (`--survey` prints the count).
Annotations reach nothing.

`--survey` prints what the code reachable from `main` uses that is not supported yet instead of
translating it: for the program's own code and for its library code, separately, per feature
(interfaces, inheritance, generic classes, exceptions, lambdas, ...) the classes and pieces of code
(methods, field initializers, initializer blocks) that use it, those it alone blocks, and up to
three example places (`file:line`, relative to the source or library root). It also prints the
order of features that frees the most code at each step, the methods of the runtime's classes
that are called, and the native methods reached (`program/survey.rs`). `--survey-json <file>`
writes the same census as JSON for scripts: the pieces with their class, kind, name, location and
features, the runtime calls and the natives. It is a syntactic census, so it misses the narrower
rejections of the lowering.

### Native layer and runtime

The native layer, the JDK methods that bottom out in the JVM, is hand-written in the runtime
(`natives.rs`, `math.rs`, `fdlibm/`, `out.rs`) and recognised by owner and name
(`lower/natives.rs`): `System.arraycopy` (between shared or plain arrays, overlap as through a
temporary, bounds checked first), the `Double` and `Float` bit conversions, `Math` and
`StrictMath`, `System.identityHashCode` and an inherited `hashCode()` (derived from the `Rc`'s
address: stable for the object's life, but like Java's different on every run), the clocks,
`availableProcessors`, the line separator, `System.err` and `System.out.write`/`flush`.
`StrictMath` is a port of fdlibm 5.3 and matches Java bit for bit; so does `Math` where HotSpot
delegates to fdlibm. HotSpot replaces `Math.sin`, `cos`, `tan`, `exp`, `log`, `pow`, `cbrt`,
`sinh` and `tanh` with its own code, which cannot be ported; they become Rust's methods (the
platform libm), which agree with it on all but about 0.1 percent of arguments, and
`Math.log10` is computed correctly rounded, as HotSpot's is.

`System.getProperty` is answered at run time for the keys whose values do not depend on the JVM:
`line.separator`, `file.separator`, `path.separator`, `user.dir`, `user.home`, `user.name`,
`java.io.tmpdir`, `os.name` and `os.arch`. Any other key gives null, or the default given; a key
the JVM sets itself (`java.version`, `java.vendor`, ...) is rejected at translation. The memory
figures of `Runtime` (`totalMemory`, `freeMemory`, `maxMemory`) are fixed plausible values, not the
JVM's, and `gc()` does nothing. `Runtime.halt` flushes the output and exits without shutdown
hooks. `Thread.sleep` (both forms), `onSpinWait` and `yield` are the platform's. The runtime's
descriptor table (`fd.rs`) backs the `FileInputStream`, `FileOutputStream` and `FileDescriptor`
natives: stdin has its own buffered reader, stdout writes through the buffered `System.out` so
its order is kept with `println`, stderr is written as it is (unbuffered, like Java's), and files
are opened by path. Translated programs cannot reach these natives yet: the JDK classes that call
them need the object model to give each stream its descriptor.

### Not yet

Each construct below is rejected at translation with an error that names it:

- exceptions (`try`, `catch`, `finally`, `throw`, `throws`);
- inheritance (`extends`), interfaces, generic classes and generic methods;
- records, inner, local and anonymous classes, and `instanceof` with patterns;
- lambdas and method references, and for-each over an `Iterable`;
- `Object` as a type, and the `java.util` collections and interfaces (`Map`, `List`, `HashMap`,
  `ArrayList`), `asList` and streams;
- threads: a program runs on one thread, and `Thread` is only the runtime's class for the calls
  above.

Also rejected: the `int` code-point overloads of the other `Character` tests (`isLetter(int)` and
the like), `Character.toChars(int, char[], int)`, `Charset` values kept in variables, and the
unsigned and parse methods with other argument counts.

Checked by `--survey` on the JDK's sources: the library code that `HashMap` reaches uses
interfaces, generic classes, inheritance, exceptions, inner classes and reflection, so none of it
translates yet.

### Build and test

Translation never runs the program; it only compiles it with javac, which takes a few seconds per
run. Building the translator needs `javac` (JDK 21 or newer) on PATH.

```
cargo run -p java-to-rust -- Program.java                 # print the Rust translation
cargo run -p java-to-rust -- Program.java --crate out/    # write a Cargo project
cargo run -p java-to-rust -- --main com.example.Main --source src/ [--library jdk-src/] [-o out.rs | --crate out/]
cargo run -p java-to-rust -- --survey --main Main --source src/ --library jdk-src/java.base
cargo test                                                # needs a JDK: runs corpus/ on both
```

The corpus test runs every program in `corpus/` with `java`, translates and builds it with
overflow checks on (so a wrong "cannot overflow" fact panics), and compares standard output and
exit code. Java's runs, translation and the Cargo build run in parallel, one thread per CPU. All
the Rust programs are one Cargo project under `target/tmp/corpus`, kept between runs, so a program
is rebuilt only when its translation changes: a warm run takes about as long as translating the
corpus. The report prints a table with one row per program and lists each failing program with the
first differing lines of output. `CORPUS_FILTER=text cargo test --test corpus` runs only the
programs whose name contains `text`, and `CORPUS_THREADS=n` limits it to n threads (one per CPU by
default). `crates/translator/tests/translation.rs` pins down what the output looks like.

## Plans

- [docs/lists-and-streams.md](docs/lists-and-streams.md): `ArrayList` as a `Vec` and sequential
  streams as Rust iterators, after the object model, lambdas and for-each over `Iterable` land.
- [docs/thorough-mode.md](docs/thorough-mode.md): a slower whole-program mode with points-to
  analysis, ownership checked by rustc, enums for class trees, and typed errors.
- [docs/multi-threading.md](docs/multi-threading.md): real threads, shared objects as `Arc` and
  `java::Locked`, monitors, and parallel execution, after the object model and exceptions.
- [docs/performance.md](docs/performance.md): the benchmark table from one run, and the causes of
  the slow cases.
