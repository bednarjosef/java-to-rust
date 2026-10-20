# Edge cases of classes, arrays, null, statics and enums

Hunt for wrong translations in what the translator already supports for objects, and record
them precisely. Another agent is rewriting the object model right now, so this brief only
writes tests and records failures; it does not change the translator.

## Scope
- Write small Java programs that stress supported features and compare Java with the translation
  (`cargo run -q -p java-to-rust -- File.java --crate DIR` then `cargo run` in DIR, against
  `java File.java`; compare stdout and exit code). Read README.md for what is supported; stay
  inside it (no inheritance, interfaces, generics, lambdas, exceptions, inner classes).
  Areas: objects aliased through fields, arrays and parameters and changed through one name;
  objects passed to methods that change them while the caller holds another reference; objects
  in arrays of arrays; null in fields, arrays, parameters and returns, with null checks in
  `&&`/`||` chains, ternaries and loops (`while (n != null) n = n.next;`); linked structures
  (lists, trees, a doubly linked list, a graph with cycles); static fields and static
  initialization order across classes, statics changed through methods; enums with fields,
  constructors and methods, `values()`, `ordinal()`, `switch` on enums and strings and chars,
  switch expressions with `yield`; `toString` called implicitly in concatenation; recursion that
  passes objects; arrays of `long`, `char`, `boolean`, `String`, `double`, copies with
  `System.arraycopy` and `Arrays.copyOf`, `Arrays.sort`/`fill`/`toString`/`equals`.
- Every program that passes goes into `corpus/features/` (focused programs) or, for a bigger
  realistic one (e.g. an LRU cache with a hand-written linked list and array-backed hash table,
  a bank simulation, a text adventure state machine), `corpus/programs/`.
- A program that the translator rejects with "not supported yet" although README says the
  feature is supported, that fails to compile as Rust, or that prints differently: put the
  smallest Java program that shows it in `known-failures/<DescriptiveName>.java` at the
  repository root (not under `corpus/`, which the test runs) and add a line to
  `known-failures/README.md`: file, the feature, what Java prints, what happens instead
  (rejection message, the rustc error, or the wrong output). Do NOT edit files under
  `crates/translator/src` or `crates/runtime/src`.

## Deliverables
- At least 10 focused corpus programs and 2 realistic programs, all passing
- Every failure found recorded in known-failures/ with a minimal program and a README line
- No changes to the translator or runtime sources
