# JDK method overloads and constructors that javac reaches

A whole-program translation of javac rejects a few overloads and constructors of JDK types that
the translator writes by hand (StringBuilder, String, the exception classes). Each is a small
addition next to the forms that already work. First read how an existing overload of the same
method is recognised and generated: search `crates/translator/src` for the rejection messages
"this overload of" and "this constructor of", and read the code around them.

## Scope
- `StringBuilder.append(CharSequence s, int start, int end)` (and the same on `StringBuffer` if
  it has the same gap): appends `s.subSequence(start, end)`, throwing
  `IndexOutOfBoundsException` when `start < 0`, `start > end` or `end > s.length()`, as Java does.
  javac reaches it from `java/util/regex/Matcher.java:1039` and `:1183` (`appendReplacement` and
  `appendTail` with a StringBuilder) and `java/text/StringBufFactory.java:102`.
- Any other "this overload of" rejection you meet on the way in `Matcher` or `StringBuilder`
  (for example `Matcher.appendReplacement(StringBuilder, String)` and
  `Matcher.appendTail(StringBuilder)` themselves); list the ones you leave in your report.
- `LinkageError()` with no arguments, reached as `super()` from
  `java/lang/IncompatibleClassChangeError.java:44`: no message, no cause, as Java.
- `MissingResourceException(String message, String className, String key, Throwable cause)`
  (the package-private constructor `java/util/ResourceBundle.java:2026` uses): like the
  three-argument one, plus the cause.

Each must behave exactly as Java (messages, `getCause()`, exceptions). Anything you cannot do
correctly stays rejected; say so in your report rather than guessing.

Leave out: `new String(byte[], …, Charset)` (another agent is changing Charset now) and the
`Thread` constructors.

## Deliverables
- StringBuilder.append(CharSequence, int, int) with its bounds exception
- The Matcher StringBuilder overloads, or a list of what stays rejected and why
- LinkageError() and the four-argument MissingResourceException constructor
- One corpus program corpus/features/BuilderRangesAndErrors.java exercising all of them,
  including a caught IndexOutOfBoundsException and printed getCause()/getMessage() values

## How to test
- Build and test through the build-slot limiter, which keeps the machine from running out of
  memory: prefix every cargo command with
  `<scratchpad>/build-slot.sh`,
  for example `.../build-slot.sh cargo test -p java-to-rust --test corpus`.
  `CORPUS_FILTER=builderranges` runs only programs whose lowercase name contains it.
- Whole programs: `JAVA_BASE_SOURCES=<scratchpad>/jdk-src/java.base`
  with `cargo run -q -p java-to-rust -- --main Name --source DIR --library $JAVA_BASE_SOURCES --crate OUT`
  (DIR holds only your program), through the limiter.
- Test only what you touch while working; run `scripts/check.sh` once at the end, through the
  limiter.
- Do not touch Unsafe, VarHandle, Charset, file system, reflection or ClassLoader code: other
  agents are changing it now.
