# Target programs: what typical Java looks like

Write a fixed set of typical Java programs that the translator cannot translate yet but should
once the object model (inheritance, interfaces, generics, casts), exceptions, inner classes and
lambdas land. They are the yardstick to measure progress end to end. This brief writes Java
programs only; do not edit anything under `crates/`.

## Scope
- 25 to 30 programs, each 60 to 250 lines, deterministic output (no time, random without a
  seed, hash-order-dependent printing of objects without hashCode), in `targets/` at the
  repository root (outside `corpus/`, which the test runs; one directory per program if it has
  several files, else one file), each with a comment line saying which features it needs.
  Cover what people write: collections (ArrayList, HashMap with String/Integer keys, HashSet,
  TreeMap, ArrayDeque, PriorityQueue with a Comparator, LinkedList, Collections.sort,
  List.of/Map.of), iterating with for-each and iterators, generics in user classes (a generic
  Pair, a generic Stack, a bounded `<T extends Comparable<T>>` max), interfaces with
  default methods, abstract classes and polymorphism (shapes, animals, an expression tree
  evaluator, a visitor), exceptions (custom checked and unchecked, try/catch/finally,
  try-with-resources on a user AutoCloseable), lambdas and method references
  (Comparator.comparing, forEach, removeIf, a Function pipeline), streams (filter/map/collect,
  groupingBy, sum), records, enums with abstract methods and EnumMap, inner and anonymous
  classes (an iterator implemented as an inner class), StringBuilder and String.format, Scanner
  reading standard input (give each such program an input file next to it and say so), Optional,
  interfaces like Comparable and Iterable implemented by user classes, a small bank/inventory/
  library-management domain model, a text adventure, a JSON-like pretty printer, a tiny
  tokenizer and recursive-descent parser, an LRU cache on LinkedHashMap, a simple event system
  with listeners.
- Run each with `java` to check it works and record its expected output next to it
  (`Name.expected`, and the exit code if not 0).
- `targets/README.md`: the list with features per program, and a small script
  `targets/run.sh` that, for each program, tries to translate it (single file, or whole-program
  mode with `--library` pointing at a JDK source tree given as an argument) and reports:
  rejected (with the message), does not compile (first rustc error), wrong output, or passes;
  then a summary count. The JDK's java.base sources are at
  <scratchpad>/jdk-src/java.base
  (never copy them into the repository). Run it once and put the current summary in the README.

## Deliverables
- 25 to 30 target programs with expected outputs
- targets/README.md listing features per program
- targets/run.sh reporting status per program and a summary
- Current results recorded
