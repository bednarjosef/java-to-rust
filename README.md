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
expressions and charsets with Java's results. Collections and threads are not translated yet (see
"Not yet" below).

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

- `corpus/features`: 231 small programs, one or more per language feature;
- `corpus/programs`: 32 larger realistic programs (algorithms, data structures, parsers,
  interpreters, simulations);
- `corpus/projects`: 3 multi-file programs (`imports`, `pruning`, `statics`), each with a source
  root `src/` and a file `main` that names the main class. The corpus has no open-source projects
  yet;
- `tools/fuzz`: random programs generated from a seed (`fuzz.py`, with `fuzz_strings.py` for
  strings and builders under `--strings`, `fuzz_arrays.py` for arrays of up to three dimensions,
  their aliases, rows, loops and helper methods under `--arrays`, and `fuzz_objects.py` for
  classes, interfaces and generics under `--objects`), run and compared by `run.py`, and shrunk
  to a minimal case by `reduce.py`. A minimal case that still fails is kept in `known-failures/` (3 programs)
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
translates, compiles and matches Java's output. The corpus meets the string, array, expression,
object, interface, generics and exception parts of this; the collections are not reached yet (see
"Not yet").

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
- **Class trees, interfaces and generics**: class trees of the program's own classes translate,
  abstract or not and of any depth: `extends`, abstract methods, overriding and virtual calls,
  `super(...)` and `super.m(...)`, upcasts, `==` on objects of a class tree, and printing through
  the class's `toString`. So do the program's interfaces: abstract, default, private and static
  methods, constants, interfaces extending interfaces, `Iface.super.m()`, classes and enums
  implementing them, and `Comparable` implemented by a class that calls `compareTo` directly.
  Between the program's own types, `instanceof` is a `matches!` on the type's enum, a pattern `x
  instanceof Circle c` binds `c` with `if let`, a let chain, `let .. else` (for `if (!(x instanceof
  Circle c)) return ..;`) or `is_some_and`, and a downcast or a cast across interfaces is checked,
  throwing `ClassCastException` when it fails. A pattern variable read anywhere else is rejected.
  `Object` is an enum too, with a variant for each of the program's classes and enums whose values
  become `Object`s and one for each string or box (`Object::String(..)`, `Object::Integer(5)`), so
  `Object` variables, fields, parameters, arrays and varargs translate, with `instanceof` and casts
  to the program's types, `String` and the boxes. The program's `equals(Object)` and `hashCode()`
  override `Object`'s; a class without them compares and hashes its identity, and a string or a box
  its value. A class without `toString` prints as Java's `Object.toString` does, its binary name and
  its hash code in hexadecimal (`Point@1b6d3586`). `new Object()` makes an object of a class
  without fields, `PlainObject`, equal only to itself and printed as `java.lang.Object@..`, as the
  JDK's lock objects and placeholder values (`HashSet`'s `PRESENT`) need. An array as an
  `Object`, and `==` between two `Object`s that may both be strings or boxes (whose identity the
  translation does not keep, except in the shortcut `a == b || a.equals(b)`) are rejected. The
  program's generic classes, interfaces and methods translate erased, as javac compiles them: a type
  variable is its bound (`Object` without one), `Box<Integer>` is `Box`, a wildcard is its bound,
  and a use where javac substituted a type argument gets the checked cast back
  (`java::cast(b.get().as_integer(), ..)`). `Comparable` as a type (a bound, mostly) is an enum like
  `Object`, over strings, boxes, the program's enums and its classes that implement it. A method
  implementing one whose erased signature differs (`Integer next()` for `T next()`,
  `compareTo(Point)` for `Comparable`'s) gets the bridge javac writes, a method with the erased
  signature that converts and calls it. A type variable with several bounds (`T extends Object &
  Comparable<? super T>`) erases to the first, and a call of a later bound's method casts to that
  bound first, as javac's erased code does. An array passed as an array of a type variable
  (`first(String[])` for `<T> T first(T[])`) is rejected. A generic class or method that only stores, returns and passes on the values of its type
  variables (a stack, a pair, a node, `<T> T choose(boolean, T, T)`) is a Rust generic instead
  (`struct Stack<T>`, `impl<T: Clone> Stack<T>`, `fn choose<T: Clone>(..)`), and its uses keep the
  substituted types with no casts. One that needs more of them (their text, `equals`, a bound, a
  cast, an array of them), a class with a supertype or a subclass, a raw use, and a method whose
  type parameters its parameters do not all mention stay erased. The JDK's functional interfaces
  (`Runnable`, `Comparator`, `java.util.function`) translate with their abstract methods (see
  lambdas below); its other interfaces are rejected with an error that names the construct and
  its position. The design of the object model, and what is still to come, is in
  [docs/object-model.md](docs/object-model.md).
- **Records, pattern switches and classes**: a record is a class with the members Java gives it
  (`javac/records.rs` writes them into javac's tree): its fields, the canonical constructor (or a
  compact one), accessors, `toString` (`Point[x=1, y=2]`), `equals` by component and `hashCode` as
  `31 * h + hash(component)`. A record that never changes is an `Rc` without a `RefCell`, as any
  such class is. Local records, enums and interfaces in a file without a package move to the top
  level (they are static and see nothing of their method). `java.util.Objects.equals`, `hashCode`,
  `hash`, `isNull` and `nonNull` translate as Java defines them. A `switch` with patterns (`case
  Circle c`, `case Circle c when c.r() > 10`, `case null`, `case Square(double side)`, records
  nested in records, `default`) is an `if let` chain over the selector's enum, and an exhaustive
  one over a sealed hierarchy or an enum needs no `default`: a null selector throws
  `NullPointerException` unless a `case null` takes it. `x instanceof Segment(Point(int fx, var
  fy), Point to)` is the same test in an `if let` chain, its negated form included. `sealed`,
  `permits` and `non-sealed` are accepted. Of `java.lang.Class`, only comparing classes and naming them translate:
  `getClass() != o.getClass()`, `a.getClass().equals(b.getClass())` and `a.getClass() ==
  Dog.class` (a `matches!` of the variant where the class is a variant of an enum), and
  `Dog.class.getName()` and `getSimpleName()`. Any other use of a `Class` is rejected, as are a
  record with an array component (its text would be the array's identity), a generic record in a
  record pattern, and the class literal of a standard class. An accessor written in the record
  runs at each use of a pattern variable that names a component, not once.
- **Exceptions** are values: `throw`, `try`/`catch` with catch clauses in order, subclass matching
  and multi-catch, rethrowing, nested `try`s, the program's own exception classes (with
  `super(message)`, `super(message, cause)`, fields, and overrides of `getMessage` and `toString`)
  and the standard ones (`Exception`, `RuntimeException`, `IllegalArgumentException`,
  `IllegalStateException`, `UnsupportedOperationException` and the rest of
  `crates/translator/java/Standard.java`, written in Java and given to the program by the
  translator), with `getMessage`, `getCause`, `toString`, `printStackTrace` (without stack frames)
  and `getClass().getName()` and `getSimpleName()`. A method returns `Result<T, Throwable>` only
  when some caller catches what it may throw, `?` passes it on, and every other method keeps its
  plain signature: an exception that nothing catches ends the program where it is thrown, with
  `Exception in thread "main" ...` on standard error and exit code 1. A `try` is a labeled block
  that evaluates to a `Result`, and its catch clauses are a `match`. `finally` runs however its
  `try` ends: after an exception (which then goes on), a `return` (whose value is taken first) and a
  `break` or `continue` out of the `try`, through nested `try`s; a `return` or an exception in the
  `finally` replaces how the `try` ended. A failure of the runtime that a `catch` or a `finally`
  sees (a division by zero, an array index out of bounds, `charAt` or `substring` out of range, a
  malformed number) is the standard exception Java throws, with Java's message; elsewhere it ends
  the program as before. try-with-resources closes the program's own resources (a new object whose
  `close` cannot fail) in the reverse order, before its `catch` clauses and `finally`. Rejected for
  now, by name: other try-with-resources, catching or letting a `finally` see a null dereference, a
  negative array size, a failed cast or a failure of the library other than these, a `toString`, a
  field initializer or a constructor of an exception or an enum that throws something a caller
  catches, throwing a value that may be null, and a `yield` or a constructor's `return` through a
  `finally`. A constructor that throws returns a `Result`, as a method does. The design is in
  [docs/exceptions.md](docs/exceptions.md).
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
body (if it is not abstract) for the rest. `valueOf` of a name that is no constant throws
`IllegalArgumentException` with Java's message (`No enum constant Main.Size.HUGE`), which a
`catch` can take (`Size::try_enum_value_of(s)?`).

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
- Class hierarchies (`ast/hierarchy.rs`, `analysis/hierarchy.rs`). A class with subclasses keeps
  its fields in a struct of its own, `ShapeFields`, which the struct of each subclass embeds as
  `base` (`Circle { base: ShapeFields::new("circle"), radius }`), and its constructors build that
  struct. Its methods that use the object only for its fields and for calls of such methods that
  nothing overrides live on that struct (`c.borrow_mut().base.move_()`); the others are emitted for
  the concrete class, where `this` is the whole object, a superclass's method that the subclass
  overrides under a name of its own for `super` calls (`withdraw_in_account`). Where every value of
  a type is an object of one concrete class, a value of the type is that class's object, so an
  upcast changes nothing. A type that objects of several classes can be is an enum named after it
  (`codegen/dispatch.rs`), with a variant per class holding that class's object
  (`Shape::Circle(Rc<RefCell<Circle>>)`): a call on it is a method of the enum that matches the
  variant and calls what that class runs, a field is read through `base()`, which borrows the
  fields struct from whichever object it is (`s.base().name`), `==` is `PartialEq` comparing the
  objects' identity, and `Display` shows each object as its class does. A body inherited from the
  type that needs the whole object is a method of the enum (`to_string_in_shape(&self)`), and an
  upcast wraps the object in its variant (`Shape::Circle(c.clone())`) or converts one enum to
  another (`d.to_animal()`). The enum gets only the methods that the program calls. A class whose
  objects are never used as a value of a supertype stays out of the enum and can stay an owned
  value; the classes of one tree are immutable together or not at all, so `base()` has one type.
  An interface is a type like any other: an enum over the classes that implement it, or that
  class's object when there is one, with its default methods on the enum
  (`fn kind_in_shape(&self)`) or in that class, its static methods free functions and its
  constants `const` items or cells. An enum that implements an interface is a variant holding the
  constant itself (`Shape::Color(Color)`), compared by value. A type's enum has a variant only
  for the classes whose objects become values of the type (`hierarchy::flows`: a `new`, an
  upcast, or running a method that the type declares), so a class whose subclasses' objects are
  only ever used through an interface stays a plain struct. The methods of an override group share what the analyses decide about their parameters and
  results, as a call may run any of them. A superclass's static initialization runs before its
  subclass's.
- Inner, local and anonymous classes (`lower/captures.rs`, `lower/inner.rs`), lowered as javac
  desugars them. An object of an inner class holds its enclosing object in the field `outer`
  (`outer: Rc<RefCell<Names>>`), which every constructor takes first and each `new` passes:
  `this`, or the object of `names.new Cursor()`. An unqualified field or method of the enclosing
  class, and `Names.this`, read through it (`self.outer.borrow().size`), two levels up through
  `self.outer.borrow().outer`; the subclasses of an inner class share its `outer` in `base`. The
  locals that a local class reads from its method are fields of its struct too, set from
  trailing constructor parameters (`MainLabeler::new(scale, &unit)`), and its enclosing object is
  `outer` when its code uses it. A local class that creates another of the same method passes on
  its own captures of it, and a captured array is shared with the method. Field initializers of
  such a class run in its constructors, after `outer` and the captures are set. A local class
  is named after the enclosing class (`MainLabeler`), with the method appended where two local
  classes of a class share a name (`MainBoxInSecond`), and `getName` gives javac's binary name
  (`Main$1Labeler`). An anonymous class is a class like the others, named after its supertype
  and its place (`OrderInMain`, `ReportInMain2`, `TaskInInit` in a field initializer): a variant
  of its interface's enum, or a subclass whose constructor passes the arguments of its `new` to
  the superclass's and its captures after them (`ReportInMain::new("squares", &sep)`); its
  `getName` is javac's (`Main$1`) and its `getSimpleName` empty. Not yet: local interfaces, enums and records, member classes of local
  classes, `Outer.super.m()`, `outer.super()`, and an inner class extending one nested in
  another class.
- For-each over an `Iterable` (`lower/foreach.rs`) is the loop Java runs: `let it =
  Names::iterator(&names); while it.borrow().has_next() { let s = it.borrow_mut().next(); .. }`,
  with the program's or the JDK's `iterator`, `hasNext` and `next`, so a translated iterator
  throws `ConcurrentModificationException` and ends early where Java's does. The element is cast
  from the erased type of `next` to the loop variable's. In a file, `Iterable` and `Iterator`
  are the interfaces lowering provides (`java/Functional.java`), without the default `remove`.
- Lambdas and method references (`lower/lambdas.rs`), lowered as javac desugars them: each is a
  class that implements its functional interface (`IntOpLambdaInMain`), a variant of the
  interface's enum like any other implementor, whose one method is the lambda's body with
  javac's substituted descriptor (`apply_integer(&self, x: i32) -> i32` for a
  `Function<Integer, Integer>`, with the bridge `apply(Object)`). A method reference's body is
  the call it makes (`a.len() as i32` for `String::length`, `Point::new`, `int[]::new`,
  `System.out::println`), lowered as a written call. A lambda's class is nested like a local
  class: the locals it captures are fields set from its constructor
  (`OpLambdaInMain::new(k, &name)`), and `this` and the enclosing class's members are read
  through `outer` (`self.outer.borrow().base`), also from a lambda in a lambda or in a local or
  anonymous class. A bound method reference takes a local, `this` or a final field as its
  receiver. In single-file mode the JDK's functional interfaces come from
  `crates/translator/java/Functional.java` (abstract methods only). Not yet: other receivers,
  `super` in a lambda, the closure forms (`impl FnMut`, `Rc<dyn Fn>`) and the JDK interfaces'
  default and static methods.

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
translated: reflection (but not `java.lang.reflect.Array`, which is array code), method handles,
serialization, security and class loading (`Module`, `ResourceBundle`, `ServiceLoader`,
`jdk.internal.loader`; `UNTRANSLATED`). Library code
reaches those on paths that only its data rules out, as `HashMap` reaches reflection through
`comparableClassFor` and parallel streams reach threads, so the boundary is needed. A program that
puts one entry into a `HashMap` reaches 55 classes this way (`--survey` prints the count).
Annotations reach nothing, and neither does the code that Java Flight Recorder's flags guard
(`jfrTracing` in `Throwable` and the file streams, which only JFR sets, by reflection): it is not
translated. Nor is what `AbstractPipeline.isParallel()` guards, when a check over the code reached
shows that no stream can become parallel (`program/reach/sequential.rs`); otherwise reachability
runs again without that assumption.

`--survey` prints what the code reachable from `main` uses that is not supported yet instead of
translating it: for the program's own code and for its library code, separately, per feature
(generic classes, exceptions, lambdas, ...) the classes and pieces of code
(methods, field initializers, initializer blocks) that use it, those it alone blocks, and up to
three example places (`file:line`, relative to the source or library root). It also prints the
order of features that frees the most code at each step, the methods of the runtime's classes
and of the untranslated ones that are called, and the native methods reached, each with the
shortest chain of calls from `main` that reaches it and whether the runtime implements it
(`program/survey.rs`). `--survey-json <file>`
writes the same census as JSON for scripts: the pieces with their class, kind, name, location and
features, the runtime calls, the natives, the methods outside the sources that are called and the
classes whose objects are created, with their chains. It is a syntactic census, so it misses the narrower rejections of the lowering.
`docs/natives-and-input.md` is the census of the natives that the target programs reach and of
what reading standard input pulls in.

### Native layer and runtime

The native layer, the JDK methods that bottom out in the JVM, is hand-written in the runtime
(`natives.rs`, `math.rs`, `fdlibm/`, `out.rs`) and recognised by owner and name
(`lower/natives.rs`): `System.arraycopy` (between shared or plain arrays, overlap as through a
temporary, bounds checked first), the `Double` and `Float` bit conversions, `Math` and
`StrictMath`, `System.identityHashCode` and an inherited `hashCode()` (derived from the object's
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

A native method of a translated JDK class is never lowered itself (javac marks it `native`); a
call of one becomes the runtime function that `ast/natives.rs` names for its class, name and
signature, and any other is rejected with its name and signature. The runtime has (`vm.rs`,
`fd.rs`): every `registerNatives()` and `initIDs()`, `VM.initialize` and
`CDS.initializeFromArchive` of a class literal (nothing); `CDS.getCDSConfigStatus` and
`getRandomSeedForDumping` (0: no class data sharing); `ContinuationSupport.isSupported0` (false,
as with `-XX:-VMContinuations`); `VM.getNanoTimeAdjustment` (HotSpot's, from the system clock);
`FileDescriptor.getHandle` (-1) and `getAppend` (the descriptor's `O_APPEND`); and
`CRC32.update` and `updateBytes0` (zlib's CRC-32). `Unsafe.arrayBaseOffset` and
`arrayIndexScale` of an array class literal are HotSpot's layout (16, and 1 to 8 by element, 4
for references) and are answered at the call, so their `Class`-taking natives are not reached.
`String.intern()` is the string itself; `==` on two strings stays rejected.

### Not yet

Each construct below is rejected at translation with an error that names it:

- the exceptions and try-with-resources listed under "Exceptions" above;
- records, local interfaces, enums and records, and record patterns;
- method references on a receiver other than a local, `this` or a final field, `super` in a
  lambda, the default and static methods of the JDK's functional interfaces;
- the `java.util` collections and interfaces (`Map`, `List`, `HashMap`, `ArrayList`), `asList`
  and streams;
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
corpus. Java's results are cached the same way, in `target/tmp/corpus/java-cache`, one file per
program: a program runs on `java` again only when its Java sources (for a project, all its files
and their paths), its main class, the JVM arguments or the `java -version` output change, so a
warm run starts no JVM. `CORPUS_JAVA_FRESH=1` runs every program on Java and stores the results
anew, for when the cache is doubted. Only a run that Java finished is stored. The report prints a
table with one row per program and lists each failing program with the first differing lines of
output. `CORPUS_FILTER=text cargo test --test corpus` runs only the programs whose name contains
`text`, and `CORPUS_THREADS=n` limits it to n threads (one per CPU by default). `crates/translator/tests/translation.rs` pins down what the output looks like.

## Plans

- [docs/lists-and-streams.md](docs/lists-and-streams.md): `ArrayList` as a `Vec` and sequential
  streams as Rust iterators, after the object model, lambdas and for-each over `Iterable` land.
- [docs/thorough-mode.md](docs/thorough-mode.md): a slower whole-program mode with points-to
  analysis, ownership checked by rustc, enums for class trees, and typed errors.
- [docs/closures-and-nested-classes.md](docs/closures-and-nested-classes.md): captures, inner,
  local and anonymous classes, lambdas, method references and for-each over `Iterable`, after
  the object model.
- [docs/multi-threading.md](docs/multi-threading.md): real threads, shared objects as `Arc` and
  `java::Locked`, monitors, and parallel execution, after the object model and exceptions.
- [docs/performance.md](docs/performance.md): the benchmark table from one run, and the causes of
  the slow cases.
