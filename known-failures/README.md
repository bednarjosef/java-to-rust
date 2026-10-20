# Known failures

Minimal programs that the translator gets wrong, though the feature is in the supported subset
(see the "Current state" section of the README). Each one is kept out of `corpus/`, which must
pass. Java's output is what `java File.java` prints. When a failure is fixed, move its file into
`corpus/features/` with the fixed behaviour.

- `ArrayStoreFromMethodCall.java`: `memory[slot] = pop();` in a method of a nested class, where
  `pop` changes a field of the same object. Java prints `1`; rustc fails with E0502 "cannot
  borrow `*self` as mutable because it is also borrowed as immutable" (the generated
  `java::set(&self.memory, slot, self.pop())` borrows `self` twice). Storing the value in a local
  first avoids it.
- `CopyRowOfNewArray.java`: `new Cell[2][]` followed by `copy[0] = grid[0];` where `grid` is a
  `Cell[][]`. Java prints `4`; rustc fails with E0308 "mismatched types" at the `new Cell[2][2]`
  line: the rows it builds are plain arrays, but the generated type of `grid` makes its rows
  `Option`s, because `copy[0] = grid[0]` stores a row in a nullable slot.
- `NullEnumVariable.java`: a local `Level best = null;` of an enum type. Java prints `none` then
  `HIGH`; the translator rejects it with "null enums not supported yet" (null objects are in the
  supported subset).
- `NullObjectConcat.java`: `"slot " + slots[0]` where `slots` is a `Node[]` and `Node` has no
  `toString`. Java prints `slot null`; the translator rejects it with "printing an object without
  toString not supported yet". The value is always null, so the class's own printing is never
  needed.
