# Character check

A differential check of the runtime's `java.lang.Character` methods against the JDK, for every
`char` value (0 to 0xFFFF). It is not part of `scripts/check.sh`: it runs the JDK and takes about
a minute.

## Running it

From the repository root:

    tools/character-check/check.sh

It prints the JDK and Rust versions, then how many chars disagree for each method, and the first
few disagreements per method. Every disagreement goes to `target/character-check/mismatches.txt`
(one line per char and method). A clean run ends with `0 with a disagreement`.

## Files

- `CharacterSweep.java` prints the JDK's answers. `chars` gives one line per char 0 to 0xFFFF:
  the code in hex, then each boolean method as 1 or 0, the two case mappings in hex, the numeric
  value, and `Character.digit` for radixes 2 to 36. `for-digit` gives `Character.forDigit` for
  digits -2 to 37 and radixes -2 to 38.
- `rust/` is a small crate that depends on the runtime (`crates/runtime`). Its `main.rs` prints
  the same lines through the runtime's functions. Its header is the same string as the JDK's, and
  check.sh stops when they differ.
- `summarise.awk` pastes the two outputs side by side and counts the disagreements per method.
- `GenerateTables.java` writes `crates/runtime/src/character_tables.rs` from the JDK. The runtime
  uses the tables for the answers Rust's `char` methods get wrong.

## Versions

- JDK: 26.0.2.1 (`java -version`). The generated tables say which JDK made them.
- Rust: 1.99.0 at the time of the last run. `char::UNICODE_VERSION` is 17.0.0.
- The Unicode version of the JDK's `Character` tables is not checked: the public API does not
  report it, so the JDK's tables are taken as they are.

## Tables

`character_tables.rs` holds these, as sorted ranges of code units:

- `DIGITS`: category Nd, for `isDigit`.
- `LETTERS`: `isLetter`. Rust's `is_alphabetic` also takes Nl and marks, which Java does not.
- `NUMERIC_OFFSETS`: `(first, last, offset)` with the numeric value of `c` being `c + offset`, for
  `getNumericValue`. A run of digits is one entry.
- `FRACTIONS`: the code units whose numeric value is a fraction (`getNumericValue` gives -2).
- `UPPER_CASE_EXCEPTIONS`: `(code unit, uppercase)` for the characters whose full uppercase
  mapping is several characters but whose simple mapping (what Java uses) is one, such as U+1F80.

Regenerate the tables after a JDK change:

    java tools/character-check/GenerateTables.java > crates/runtime/src/character_tables.rs

The generated file is formatted for `cargo fmt`; run `scripts/check.sh` after regenerating.

## Results

At the last run, on JDK 26.0.2.1 and Rust 1.99.0: all 65,536 chars agree for every method, and
all 1,640 `forDigit` pairs agree. The sweep program in `corpus/features/CharacterTablesSweep.java`
prints every method for the 2,945 chars where one of them changes, and it is compared with the
JDK by the corpus test.

## Adding a method

Add it to the header string and to `printChar` in `CharacterSweep.java`, and to the same place in
`rust/src/main.rs`. Keep both headers equal, and keep the order of the fields.
