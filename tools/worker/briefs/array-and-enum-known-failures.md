# Four known failures in arrays, field initializers and enums

`known-failures/` holds minimal Java programs that the translator gets wrong inside the
supported subset, each described in `known-failures/README.md`. Fix these four:
`ArrayStoreFromMethodCall.java` (E0502: `java::set(&self.memory, slot, self.pop())` borrows
`self` twice: evaluate the value into a local first when it may change `self`, keeping Java's
evaluation order: array, index, then value), `CopyRowOfNewArray.java` (E0308: rows built by
`new Cell[2][2]` are plain arrays but the array's type makes rows nullable),
`StringArrayFieldInitializer.java` (E0308 on a `String[]` field initializer; the constructor form
works), and `NullEnumVariable.java` (a local of an enum type set to `null` is rejected: make a
nullable enum an `Option` of the enum, consistent with how other nullable values work, and
print `null` for it).

## Scope
- Read README.md and the code paths involved (`codegen/arrays.rs`, `codegen/java_arrays.rs`,
  `analysis/arrays.rs`, `codegen/enums.rs`, `lower/enums.rs`, `codegen/owned.rs` for field
  initializers). For each failure, find the root cause and fix it generally, not just for the
  repro. When a fix needs `analysis/nullness.rs`, keep the change small and local: an
  object-model agent is rewriting object code (`lower/objects.rs`, `codegen/objects.rs`,
  `analysis/borrowing.rs`, `analysis/nullness.rs`) right now; if a fix would need large changes
  there, stop and record that in your notes instead.
- When a failure translates correctly, move its file into `corpus/features/` (add a few more
  cases of the same kind to it) and delete its line from `known-failures/README.md`.

## Deliverables
- ArrayStoreFromMethodCall fixed and moved to the corpus
- CopyRowOfNewArray fixed and moved to the corpus
- StringArrayFieldInitializer fixed and moved to the corpus
- NullEnumVariable fixed and moved to the corpus
