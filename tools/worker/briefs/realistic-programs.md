# Realistic programs in the supported subset

Write realistic Java programs that stay inside what the translator supports, and find where
the translation goes wrong. This brief only writes tests and records failures; other agents
are changing the translator now, so do not edit anything under `crates/`.

## Scope
- Read README.md for the supported subset: classes without inheritance, interfaces, generics,
  lambdas, exceptions or inner classes; static nested classes; enums; arrays; null; statics;
  strings with `length`, `charAt`, `equals` and concatenation; `Math`; switch.
- Write programs of 80 to 300 lines the way people write them for exercises and contests:
  dynamic programming (knapsack, edit distance, longest common subsequence), graph algorithms
  on adjacency matrices or arrays (Dijkstra with arrays, Floyd–Warshall, union-find, BFS with an
  array queue), sorting algorithms (merge, quick, heap, counting, radix), number theory (sieve,
  gcd/lcm, modular exponentiation, big numbers as digit arrays), matrix operations, simulations
  (game of life, elevator, bank queue with a hand-written ring buffer), text processing with
  `charAt` (run-length encoding, Caesar cipher, palindromes, word counting with arrays), a
  hand-written hash map of int keys, a priority queue on an array, a tiny interpreter for a
  stack language, Sudoku/N-queens backtracking, fixed-point and floating-point numerics.
- Run each on Java and on the translation (`cargo run -q -p java-to-rust -- File.java --crate
  DIR`, then `cargo run --release` in DIR; compare stdout and exit code with `java File.java`).
  Also note when the translated program is much slower than Java.
- Each program that passes goes into `corpus/programs/` under a descriptive name.
- For each failure inside the supported subset (a "not supported yet" rejection of something
  README says works, a rustc error, a wrong output, a panic), reduce it to the smallest program
  that shows it, put it in `known-failures/<DescriptiveName>.java` at the repository root, and
  add a line to `known-failures/README.md` in its existing style: file, what Java prints, what
  happens instead (message, rustc error code and text, or wrong output). Check first that the
  failure is not already listed there.
- Failures caused by features outside the subset are not known failures; list the three most
  common ones in your final report instead.

## Deliverables
- At least 15 realistic programs in corpus/programs/, all passing
- Every failure inside the subset reduced and recorded in known-failures/
- No changes under crates/
