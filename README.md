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

A first end-to-end pipeline for a small subset: one class of static methods with `int`, `long`,
`double`, `boolean` and `String` locals, arithmetic, `if`, `while`, `do`, `for`, `break`,
`continue`, the conditional operator, overloading, `System.out.print`/`println`, `System.exit`,
a few `Math` methods and `String.length`/`equals`, and `null` for `String`. Anything else is
rejected with an error that names the construct and its position.

The pieces: javac compiles the source (parse and attribute only) through a small Java program,
`crates/translator/java/JavaTree.java`, that prints javac's syntax tree as JSON with the type of
every expression and the declaration every name and call refers to. `lower.rs` maps that tree onto
the supported subset (`ast.rs`), the analyses prove facts about the program (`analysis.rs`), and the
generator writes Rust (`codegen.rs`) that uses the runtime crate (`crates/runtime`) for what Rust
lacks: Java's double printing, integer division that throws, and `Math.max`/`min` on NaN.

The faithful translation of `a + b` on ints is `a.wrapping_add(b)` and of `a % b` is
`java::rem(a, b)`. The first analyses decide where plain Rust means the same:

- Range analysis (`analysis/ranges.rs`) tracks the values every `int` and `long` can hold, by
  abstract interpretation over intervals: conditions narrow ranges, loops are solved with
  widening to the program's constants and narrowing, counters in counting loops are bounded by
  the trip count, and argument and return ranges flow between methods. Arithmetic that provably
  stays in range uses `+`, `-`, `*`, and division by a provably nonzero divisor uses `/` and `%`.
- Strings that are only ever ASCII use `len()` for `length()`; string parameters that are never
  assigned are borrowed as `&str`.
- Nullness analysis (`analysis/nullness.rs`) finds the strings that may be null (locals,
  parameters across calls, return values). Only those are `Option<String>`; the others stay
  `String` or `&str`. A null check narrows the string in the branch where it is not null
  (`if let Some(s) = s.as_deref()`, or `let Some(s) = s.as_deref() else { .. }` after an early
  return), so no unwrap is repeated. Dereferencing a null string throws `NullPointerException`
  through `java::non_null`, and printing or concatenating it gives `null`.
- Arrays (`int[]`, `long[]`, `double[]`, `boolean[]`, `String[]`, and arrays of arrays) are shared
  references in the faithful form, `java::Array<T>`: a shared cell whose clones see each other's
  writes. The aliasing analysis (`analysis/arrays.rs`) finds local arrays that no other name can
  see, and those become a plain `Vec<T>`. Indexing a `Vec` is `a[i as usize]` where a counting
  loop over `a.length` proves the index in bounds, and `java::at`/`java::put` otherwise. Bounds
  and size errors go through `java::throw`, so they exit with code 1 as Java does. A parameter
  becomes `&[T]` or `&mut [T]` when every call passes a distinct plain array, and a counting loop
  that only reads `a[i]` becomes `for x in a.iter().cloned()`. Arrays that are returned, copied,
  or given to `java.util.Arrays` stay shared for now.

Translation never runs the program; it only compiles it. Starting javac costs about half a
second per file; building the translator needs `javac` (JDK 21 or newer) on PATH.

```
cargo run -p java-to-rust -- Program.java                 # print the Rust translation
cargo run -p java-to-rust -- Program.java --crate out/    # write a Cargo project
cargo test                                                # needs a JDK: runs corpus/ on both
```

The corpus test runs every program in `corpus/` with `java`, translates and builds it with
overflow checks on (so a wrong "cannot overflow" fact panics), and compares standard output and
exit code. `crates/translator/tests/translation.rs` pins down what the output looks like.
