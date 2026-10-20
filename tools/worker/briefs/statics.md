# Static initializers, constants and enums

Today static fields work (they live in `thread_local!` cells, see `codegen/objects.rs`), but
`static final` constants are not treated specially, `static { ... }` blocks are rejected, and
enums are rejected.

## Scope
- `static final` fields whose initializer is a compile-time constant (javac gives the value; a
  constant expression of primitives and strings): a Rust `const` (or `&str` constant for a
  `String`), read without any cell. javac already folds uses of constants of the same
  compilation unit in many places; handle both forms.
- Other `static final` fields (initialized by a method call or `new`): read-only after
  initialization; still lazily initialized, but no `RefCell` writes are needed after.
- `static { ... }` initializer blocks: Java runs a class's static initializers (field
  initializers and blocks, in source order) once, the first time the class is used (a static
  method call, a static field access other than a constant, or `new`). Output order matters:
  a `println` in a static block must print at the same point as in Java. The class with `main`
  is initialized before `main` runs.
- Enums: `enum Color { RED, GREEN }`, with `values()`, `ordinal()`, `name()`, `toString()`,
  `==`, `valueOf(String)` (an unknown name throws `IllegalArgumentException`, exit 1),
  `compareTo`, use as a field, local, parameter and return type, printing; and enums with
  fields, a constructor and methods (`PLANET(3.0, 4.0)`, `double mass() { ... }`).
  A `switch` on an enum is a separate worker's feature; leave it rejected.

## Translation
- An enum without fields becomes
  `#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)] enum Color { Red, Green }`
  with an `impl` for `name()`, `ordinal()`, `values()`, and `Display`. Variant names in Rust
  style (`Red`), but `name()` returns Java's `"RED"`.
- An enum with fields: the same Rust enum, with each field a method that matches on `self`
  (`fn mass(self) -> f64 { match self { Planet::Earth => 5.97e24, ... } }`) when every
  constructor argument is a constant; otherwise a static table of structs. Pick and note why.
- Enum values can be `null` in Java; until the null feature covers objects, reject a `null`
  enum value ("null enums not supported yet").
- Static initialization order: a generated `fn init_<class>()` guarded by a `thread_local`
  `Cell<bool>`, called at the start of every static method and constructor of that class and
  before every static field access from another class, is the faithful form. Where the class has
  no static block and every static initializer has no side effects (no call, no `new` of a class
  with side effects, no printing), skip all of that: lazy initialization is unobservable.
- Keep each file under 1000 lines; `codegen/objects.rs` is already 550 lines, so put statics
  and enums in `codegen/statics.rs` and `codegen/enums.rs` (and `lower/enums.rs`).

## Deliverables
- `static final` compile-time constants as Rust `const`
- `static { }` blocks with Java's initialization order and timing
- Simple enums as Rust enums with name, ordinal, values, valueOf, compareTo, printing
- Enums with fields, constructors and methods
- Corpus programs: `corpus/features/Constants.java`, `corpus/features/StaticInit.java` (prints
  from static blocks of two classes to show the order), `corpus/features/Enums.java`, and
  `corpus/programs/Planets.java` (an enum with fields and methods)
- Shape tests in `tests/translation.rs` for `const`, the Rust enum and its methods
