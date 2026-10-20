# Varargs, and Java syntax that changes nothing

## Scope
- Varargs methods: `static int sum(int... xs)`, `static void log(String fmt, Object... args)`
  only for element types the translator supports (primitives, `String`, arrays of those; reject
  `Object...` as "Object not supported yet"). Calls with zero, one or several trailing
  arguments, and with an array passed directly (`sum(arr)`), and `main(String... args)`.
- Modifiers and syntax that do not change a single-threaded program's behaviour, accepted and
  ignored: annotations on classes, methods, fields, parameters and locals (`@Override`,
  `@SuppressWarnings("unchecked")`, `@Deprecated`, `@FunctionalInterface`, custom ones),
  `final` on locals and parameters, `volatile`, `transient`, `synchronized` methods,
  `synchronized (x) { ... }` blocks (the block runs; evaluate `x`, and a null `x` throws
  `NullPointerException`), `strictfp`, `assert` statements (Java runs with assertions disabled
  by default, so the condition is not evaluated: drop them).
- Empty statements and `;` after class members, `var` locals (javac gives the type).

## Translation
- javac gives a varargs call's arguments as written; `symbol.type` of the method has the
  `...`. The faithful form is an array parameter (`java::Array<T>`, see `codegen/arrays.rs`)
  and the call packs the trailing arguments into a new array. The idiomatic form: a varargs
  parameter that the method only reads is `&[T]` and the call passes `&[a, b, c]`, which needs
  no allocation; extend the arrays analysis in `analysis/arrays.rs` so that a packed argument
  is a fresh plain array (it is: nobody else can see it).
- Use the existing array translation for everything inside the method (`xs.length`, `xs[i]`,
  for-each).
- `lower.rs` today rejects `...` types with "varargs not supported yet" (see `fn ty`).
- `synchronized` blocks become plain blocks (after the null check of the lock expression).

## Deliverables
- Varargs methods and calls (zero, one, several arguments, array passed directly)
- Read-only varargs parameters as `&[T]` with `&[a, b]` at the call
- Annotations, final, volatile, transient, strictfp accepted and ignored
- synchronized methods and blocks, assert statements, var locals
- Corpus programs: `corpus/features/Varargs.java` and `corpus/features/IgnoredSyntax.java`
- Shape tests in `tests/translation.rs` for the `&[T]` varargs form and dropped annotations
