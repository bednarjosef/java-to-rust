# Appending a range of a program's CharSequence, and Matcher's StringBuilder overloads

`StringBuilder.append(CharSequence s, int start, int end)` works when `s` is a string or a
builder, but is rejected when `s` is a program class (or a JDK class translated from source) that
implements CharSequence: "appending a range of a `CharSequence` that a program class implements".
javac's census shows it 3 times, in library code. The CharSequence model now has `charAt`,
`length`, `subSequence` and `getChars` for such classes (commit 097ca59, CharSequence.getChars),
so the range append can be built from them, as the JDK's `AbstractStringBuilder.append(CharSequence,
int, int)` does: bounds checks first, then one `charAt` per character.

Second part: `Matcher.appendReplacement(StringBuilder, String)` and `Matcher.appendTail(StringBuilder)`
stayed rejected after an earlier worker (brief jdk-overloads-and-constructors.md) added the range
append. Find out why they are still rejected on current main, and make them work if they can be
done exactly as Java, including `appendReplacement`'s `$1`/`${name}`/`\` handling and its
exceptions.

First search `crates/translator/src` for the rejection message above and for how
`append(CharSequence, int, int)` is handled for strings and builders, and read
`$JAVA_BASE_SOURCES/java/lang/AbstractStringBuilder.java` for the bounds checks and their messages.

## Deliverables
- The range append for program CharSequences, with Java's `IndexOutOfBoundsException` and
  message for each bad range.
- The two Matcher overloads, or a short note in your report on exactly what stops them.
- One corpus program corpus/features/ProgramTextRanges.java: a program class implementing
  CharSequence appended in ranges, caught bad ranges, and (if done) Matcher replacement into a
  StringBuilder with groups, compared with Java.

## How to test
- Build and test through the build-slot limiter, which keeps the machine from running out of
  memory: prefix every cargo command with
  `<scratchpad>/build-slot.sh`.
  `CORPUS_FILTER=programtextranges` runs only that corpus program.
- Whole programs: `JAVA_BASE_SOURCES=<scratchpad>/jdk-src/java.base`
  with `cargo run -q -p java-to-rust -- --main Name --source DIR --library $JAVA_BASE_SOURCES --crate OUT`
  (DIR holds only your program), through the limiter.
- Test only what you touch while working; run `scripts/check.sh` once at the end, through the
  limiter.
- Do not touch Unsafe, VarHandle, Charset, reflection, ClassLoader, liveness or exception
  analysis code: other agents are changing it now.
