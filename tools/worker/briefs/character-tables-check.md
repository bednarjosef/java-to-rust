# Check every Character method against Java for every char

The runtime implements `Character` methods (`crates/runtime/src/character.rs`,
`character_classes.rs`): `isDigit`, `isLetter`, `isLetterOrDigit`, `isAlphabetic`,
`isUpperCase`, `isLowerCase`, `isWhitespace`, `isSpaceChar`, `toUpperCase`, `toLowerCase`,
`getNumericValue`, `digit`, `forDigit`, the surrogate tests, and whatever else is there now.
Java's answers come from its own Unicode tables (the JDK's Unicode version), which may differ
from Rust's `char` methods. Prove they match for all 65,536 `char` values (and for the int
code point overloads that exist), and fix them where they do not.

## Scope
- A differential check outside the test suite, e.g. `tools/character-check/`: a Java program
  printing, for every char 0..=0xFFFF, the result of every supported method (one line per char,
  compact), a Rust program printing the same through the runtime, and a script that diffs and
  summarises mismatches per method. Note the JDK version and its Unicode version.
- Fix every mismatch in the runtime. If Rust's tables are a different Unicode version than the
  JDK's, generate exact tables from Java's output instead (a generator script in the tool's
  directory; the generated Rust table is committed in the runtime with a header saying how to
  regenerate it, and must stay small: ranges, not 65,536 entries).
- Methods on `String` that use these (e.g. `toUpperCase` of a string) belong to another agent;
  only fix the char methods. Do not touch translator code beyond what a char method needs, and
  nothing in object, nullness, borrowing or String code.
- Add a corpus program that prints, for a few thousand interesting chars (every category
  boundary you found), the results of every method.

## Deliverables
- Differential Character check tool with a README
- All supported Character methods compared for all 65,536 chars
- Every mismatch fixed in the runtime, with generated tables where Rust's differ
- Corpus program corpus/features/CharacterTablesSweep.java
