# java-to-rust

A tool that converts Java source code into Rust source code, without LLMs: a compiler-style
pipeline of parsing, type resolution, program analysis and code generation.

The goal is Rust that behaves exactly like the original program and, wherever the tool can
prove it safe, is written the way a person would write Rust: owned values instead of shared
garbage-collected objects, enums instead of class hierarchies, no null checks where nothing can
be null.

## How it works

```
Java source -> typed syntax tree -> intermediate representation -> analyses
            -> representation table -> Rust code generator -> Rust crate
```

The representation table holds one decision for every class, field, variable and allocation
site: how it appears in Rust (a shared garbage-collected object, an owned value, a `Box`, an
`enum`, an `Option` or a plain value). The code generator only follows the table, so better
analyses improve the output without touching the generator, and owned and shared values can be
mixed in one program.

### Level 1: faithful translation

Every table entry starts at the safe default: each object is a shared, garbage-collected value
(`Gc<RefCell<...>>`), interfaces become traits, exceptions become `Result` or panics, and `null`
becomes `Option`. A small runtime crate provides Java's strings, collections, garbage collection
and exact number semantics (wrapping integer overflow, Java's float rules, `HashMap` iteration
order). The output compiles and behaves like the Java program, but still reads like Java.

### Level 2: analysis-driven Rust

Analyses prove facts about the program and move table entries to better choices, one step at a
time:

1. Immutable value classes (records, all-`final` fields) become plain structs.
2. Non-null inference removes `Option`s and null checks.
3. `final` classes lose dynamic dispatch; sealed hierarchies become `enum`s.
4. Escape analysis turns objects that never leave a method into stack values.
5. Ownership inference turns objects held in exactly one place into owned fields, `Box`es and
   `Vec` elements.
6. Dynamic profiling: running the program's tests records real aliasing, deciding what static
   analysis cannot (with a runtime check where no proof exists).

Whatever no analysis can prove stays at the level 1 default, so the output is always correct.

## Testing

Every test program runs on Java and, translated, as Rust; the two outputs (printed text, results,
exceptions) must match exactly. The Java run is the expected result, so no expectations are
written by hand.

Beyond correctness, each translation is measured: the share of values that became owned, plain
or `enum` instead of garbage-collected, the remaining `Option`s, null checks and dynamic calls,
`clippy` warnings and `unsafe`, and speed and memory against the Java program.

Test programs come from four sources: small programs per language feature, larger realistic
programs (algorithms, data structures, parsers, interpreters, simulations), open-source Java
projects together with their own unit tests, and randomly generated programs that cover the
corner cases nobody writes by hand. The whole corpus runs on every change, and a table per
program shows what a change improved or broke.

## First milestone

Level 1 end to end for a subset of Java: classes, interfaces, inheritance, generics, the common
collections, strings and exceptions; no reflection and no threads. Done when the feature corpus
translates, compiles and matches Java's output.

## Current state

A first end-to-end pipeline for a small subset: several classes with locals of every primitive
type (`byte`, `short`, `char`, `int`, `long`, `float`, `double`, `boolean`) and `String`,
arithmetic, `if`, `while`, `do`, `for`, `break`, `continue`, the conditional operator,
overloading, static and instance methods, fields with initializers, static fields, constructors
(including `this(...)`), `new`, arrays, `System.out.print`/`println`, `System.exit`, a few `Math`
methods (`floorDiv`, `floorMod`, the `*Exact` integer methods that throw on overflow, `round`,
`rint`, `clamp`, `copySign`, `hypot`, `fma`, `ulp`, `nextUp`/`nextDown`, `max`/`min` on every
type, and the rest of the common ones; `StrictMath` the same), `Double`/`Float` and `Boolean`
methods (`isNaN`, `isInfinite`, `isFinite`, `compare`, `sum`, `max`/`min`, `parseDouble`,
`parseBoolean`, `logicalAnd`/`logicalOr`/`logicalXor`), `String.length`/`equals`/`charAt`, the
`Character` tests (`isWhitespace`, `isLetterOrDigit`, `isAlphabetic`, `isSpaceChar`, `isSurrogate`,
`digit`, `forDigit`, `getNumericValue`), and case conversions on a `char`, the `MIN_VALUE`/`MAX_VALUE`
and `MIN_NORMAL` constants of the wrapper classes, `null` for `String`, and a user `toString` in
`println` and string concatenation, object-typed fields and `null` objects. Inheritance, interfaces
and generics are rejected with an error that names the construct and its position. So are
`Float.parseFloat` and `Float.valueOf(String)` (Rust's `f32` parsing differs from Java's grammar),
`Character.toChars` (it returns a `char[]`), and the `int` code-point overloads of the `Character`
methods, except `isAlphabetic` on a `char`.

The pieces: javac compiles the source (parse and attribute only) through a small Java program,
`crates/translator/java/JavaTree.java`, that prints javac's syntax tree as JSON with the type of
every expression and the declaration every name and call refers to. `lower.rs` maps that tree onto
the supported subset (`ast.rs`), the analyses prove facts about the program (`analysis.rs`), and the
generator writes Rust (`codegen.rs`) that uses the runtime crate (`crates/runtime`) for what Rust
lacks: Java's double and float printing, integer division that throws, `Math.max`/`min` on NaN,
printing a `char`, `String.charAt` by UTF-16 unit and the `Character` methods.

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

The faithful translation of `a + b` on ints is `a.wrapping_add(b)` and of `a % b` is
`java::rem(a, b)`. The first analyses decide where plain Rust means the same:

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
  see, and those become a plain `Vec<T>`. Indexing a `Vec` is `a[i as usize]` where a counting
  loop over `a.length` proves the index in bounds, and `java::at`/`java::put` otherwise. Bounds
  and size errors go through `java::throw`, so they exit with code 1 as Java does. A parameter
  becomes `&[T]` or `&mut [T]` when every call passes a distinct plain array, and a counting loop
  that only reads `a[i]` becomes `for x in a.iter().cloned()`. Arrays that are returned, copied,
  or given to `java.util.Arrays` stay shared for now. Arrays of strings, objects or arrays hold
  `Option`s when some array of that element type may hold null (`new String[n]`,
  `new int[n][]`, or a stored value that may be null), and plain values otherwise.
- `java.util.Arrays` on arrays of primitives and strings (`lower/java_arrays.rs`,
  `codegen/java_arrays.rs`, `runtime/src/java_arrays.rs`): `equals` and `hashCode` (Java's exact
  formula, bit equality for floats, null arrays and null elements), `sort` for every primitive
  type and `String` (Java's order: `-0.0` before `0.0`, NaN last, UTF-16 order for strings), whole
  or by range with Java's range checks, `fill` by range, `copyOfRange` on primitives,
  `binarySearch` in both forms (`-(insertion point) - 1` when absent), and `deepToString` on
  arrays of primitive or `String` rows. `a.clone()` is a shallow copy; rows of a 2-D array are
  shared, as in Java. Sorting an array with a null `String` throws `NullPointerException`.
  `==` on arrays is identity (`Rc::ptr_eq`, or `java::same` where either may be null). Not yet:
  `copyOf` and `copyOfRange` on `String` arrays (the padding would need nulls that the
  nullness analysis does not see), `equals(null, b)`, `asList` and `stream`.
- Objects follow the same pattern (`analysis/objects.rs`, `analysis/owned.rs`, `analysis/borrowing.rs`). The faithful
  form of an object is `Rc<RefCell<C>>`, because Java references are shared and mutable. A class
  whose objects never change after construction is shared as `Rc<C>`. A class whose objects are
  never shared (no class-typed field, parameter or return value, and each object is created by
  `new` into a local that only calls or prints it) is a plain value: its methods take `&self` or
  `&mut self`, and a call on it uses method syntax so that `p.set(p.x + 1)` borrows as Rust allows.
  Methods of shared classes borrow their object too (`analysis/borrowing.rs`) when they use
  `this` only for its fields and for such calls, and nothing they run reaches an object of their
  class another way; a call is then `p.borrow_mut().set(2)`, with arguments that could reach the
  object bound first. Constructors build a plain value, starting from a struct literal that takes
  in the body's leading field assignments (`P { x, y }`; values with effects are computed into
  locals first where the literal's field order would change Java's order), and `new` shares it
  (`Rc::new(RefCell::new(P::new(1, 2)))`). A `toString` becomes `impl fmt::Display`, and an object
  is printed as `{p}` when its `toString` can neither print nor throw. An object parameter that
  the method only uses through (fields, calls, comparison, printing, or passing it on to such a
  parameter) is a `&Rc<..>` that the caller lends.

Whole programs (`program.rs`) spread over files and packages translate too: javac compiles all
of them in one run, with the source roots as its source path, and a reachability pass over its
trees (`program/reach.rs`) starts at `main` and follows every resolved call, `new`, field access
and type, so only the classes, methods, constructors and fields that `main` can reach are lowered
and translated. Library roots (`--library`, meant for the JDK's own sources) are only on the
source path: javac reads a file from them when the program uses one of its classes. Packages
become nested modules in one output file (`com.example.util` is `mod com { pub(crate) mod example
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
`comparableClassFor` and parallel streams reach threads: without the boundary, a program that puts
one entry into a `HashMap` reaches 3,268 classes of java.base; with it, 59. Annotations reach
nothing.

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
`Math.log10` is computed correctly rounded, as HotSpot's is. `System.out` is buffered
(`java::println!`), several times faster than `println!` for programs that print a lot.

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
are opened by path. Translated
programs cannot reach these natives yet: the JDK classes that call them need the object model to
give each stream its descriptor.

Translation never runs the program; it only compiles it. Starting javac costs about half a
second per run; building the translator needs `javac` (JDK 21 or newer) on PATH.

```
cargo run -p java-to-rust -- Program.java                 # print the Rust translation
cargo run -p java-to-rust -- Program.java --crate out/    # write a Cargo project
cargo run -p java-to-rust -- --main com.example.Main --source src/ [--library jdk-src/] [-o out.rs | --crate out/]
cargo run -p java-to-rust -- --main Main A.java B.java    # a program given as files
cargo run -p java-to-rust -- --survey --main Main --source src/ --library jdk-src/java.base
cargo test                                                # needs a JDK: runs corpus/ on both
```

The corpus test runs every program in `corpus/` with `java`, translates and builds it with
overflow checks on (so a wrong "cannot overflow" fact panics), and compares standard output and
exit code. A multi-file program is a directory of `corpus/projects/` with a source root `src/`
and a file `main` that names the main class; it runs on Java after `javac`.
`crates/translator/tests/translation.rs` pins down what the output looks like.
