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
