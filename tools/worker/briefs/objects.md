# Objects

Translate Java classes with state: today `lower.rs` accepts one class of static methods and
rejects fields, constructors, instance methods and `new` with "X not supported yet".

## Scope
- Several top-level classes in one file (one has `main`), and static nested classes.
- Instance fields and static fields of every supported type, with initializers and Java's
  default values (`0`, `false`, `0.0`; `String` fields default to `null`: you may reject those
  without an initializer or constructor assignment as "null not supported yet" until the null
  feature lands).
- Constructors (several, overloaded, `this(...)` calls), `new C(...)`, instance methods
  (overloaded), `this`, field reads and writes `p.x`, `this.x`, `p.x += 1`, `p.x++`, static
  field access `C.count`, calls `p.m()`, `C.m()`, and calls on expressions (`make().m()`).
- Objects passed to and returned from methods, stored in other objects' fields and in locals.
- A user-defined `public String toString()` used by `println(obj)` and string concatenation;
  `equals`/`hashCode` and printing an object without `toString` (Java prints a hash) can stay
  unsupported.
- No inheritance, interfaces, abstract classes or generics yet (reject `extends` clearly).

## Translation
- Each Java class becomes a Rust struct with its fields and an `impl` with its constructors
  (`fn new(...) -> ...`, overloads named like methods are today) and methods; static fields
  become `thread_local!`/`static` cells or a struct of statics (pick and note why); static
  methods stay associated functions or free functions.
- Faithful form: objects are shared and mutable, so a reference is `Rc<RefCell<C>>`, methods
  take the reference and borrow inside (keep `RefCell` borrows short so that a method that
  calls another method on the same object does not panic with a double borrow).
- Idiomatic forms, by analysis (`src/analysis/objects.rs`):
  - A class whose fields are never written after construction (all effectively final) needs no
    `RefCell`: `Rc<C>`, or a plain `C` value with `#[derive(Clone)]` if it is small and never
    compared by identity.
  - An object that never has two names at once (not copied into a second variable or field while
    the first is still used) can be an owned `C` value, with `&C`/`&mut C` parameters and `&self`
    / `&mut self` methods.
  - Getters and setters stay methods; no need to inline them.

## Deliverables
- Several classes per file, fields, static fields, constructors, `this`, `new`, instance methods
- Field reads, writes, compound assignment and `++` through `Place` (add `Place::Field` or similar)
- Faithful `Rc<RefCell<C>>` translation, safe against double borrows
- User-defined `toString` in `println` and string concatenation
- Analysis: immutable classes without `RefCell`, unaliased objects as owned values with `&self` / `&mut self`
- Corpus programs: `corpus/features/Objects.java`, `corpus/features/ObjectAliasing.java`, `corpus/features/StaticFields.java`, and `corpus/programs/BankAccounts.java` (a few accounts, transfers, a report)
- Shape tests in `tests/translation.rs` for the idiomatic forms
- README "Current state" updated
