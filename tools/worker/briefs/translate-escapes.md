# String.translateEscapes

javac calls `String.translateEscapes()` (it turns the escape sequences of a text block or string
literal, such as `\n`, `\t`, `\'`, octal `\0`–`\377`, `\s` and a backslash before a line end, into
the characters they stand for). The translator writes String's methods by hand, and this one is
rejected: "method `String.translateEscapes` not supported yet".

First read how an existing hand-written String method goes from recognition to runtime function
to test: search `crates/translator/src` for an existing String method that takes no arguments
and returns a String (for example `strip` or `stripIndent`), and follow it into the runtime.
Then read the JDK's own `String.translateEscapes` in
`$JAVA_BASE_SOURCES/java/lang/String.java` (path below). It is the definition: match it
exactly, including the `IllegalArgumentException` and its message for a bad escape
("Invalid escape sequence: \%c \\u%04X") and for a backslash at the end.

## Scope
- `String.translateEscapes()` in the runtime, with Java's exact results and exceptions.
- The exception must be catchable in the translation, as other String methods' failures are.

## Deliverables
- The method, with runtime unit tests for every escape kind, octal of 1–3 digits (`\7`, `\77`,
  `\377`, and `\400`, which is `\40` followed by `0`), `\s`, a backslash before `\n` and before
  `\r\n`, and each failure.
- One corpus program corpus/features/TranslatedEscapes.java that prints the translated results
  (as code points, so invisible characters show) and a caught failure's message, compared with
  Java.

## How to test
- Build and test through the build-slot limiter, which keeps the machine from running out of
  memory: prefix every cargo command with
  `<scratchpad>/build-slot.sh`,
  for example `.../build-slot.sh cargo test -p java-runtime`.
  `CORPUS_FILTER=translatedescapes` runs only that corpus program.
- `JAVA_BASE_SOURCES=<scratchpad>/jdk-src/java.base`
- Test only what you touch while working; run `scripts/check.sh` once at the end, through the
  limiter.
- Do not touch Unsafe, VarHandle, Charset, reflection, ClassLoader, liveness or exception
  analysis code: other agents are changing it now.
