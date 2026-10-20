# Number parsing with Java's syntax, unsigned arithmetic, Character.toChars

Add the number methods that map onto Rust's standard library but need Java's exact syntax and
results. First read what exists: `lower/boxing.rs` (`parseInt`, `parseDouble`, `valueOf`, ...),
`lower/primitives.rs`, `lower/natives.rs`, `lower.rs` (`fn call`), and the runtime's `boxed.rs`,
`float.rs`, `character.rs`. Add only what is missing.

## Scope
- `Double.parseDouble`/`valueOf(String)` and `Float.parseFloat`/`valueOf(String)` with Java's
  grammar exactly: leading/trailing whitespace (Java trims chars <= ' '), optional sign,
  `Infinity`, `NaN`, the `d`/`D`/`f`/`F` suffixes, exponents, leading `.5` and trailing `5.`,
  hexadecimal floats (`0x1.8p3`), and correct rounding (for floats: parse straight to f32, not
  through f64, to avoid double rounding). Anything invalid throws NumberFormatException (exit
  code 1 through the runtime's throw). Compare against Java on a few hundred inputs.
- `Integer.parseInt`/`Long.parseLong` with a radix, `Integer.valueOf(s, radix)`, Java's rules
  (`+`/`-` sign, digits of any Unicode decimal digit block that `Character.digit` accepts:
  check what Java really accepts), if not done already.
- Unsigned: `Integer`/`Long` `divideUnsigned`, `remainderUnsigned`, `compareUnsigned`,
  `toUnsignedString` (and with radix), `parseUnsignedInt`/`parseUnsignedLong`,
  `Integer.toUnsignedLong`, `Byte`/`Short` `toUnsignedInt`/`toUnsignedLong`.
- `Character.toChars(int)` (a `char[]` of one or two units) and `Character.toString(int)`,
  `Character.charCount`, `Character.toCodePoint`, `Character.highSurrogate`/`lowSurrogate`,
  `Character.isValidCodePoint`, `isBmpCodePoint`, `isSupplementaryCodePoint`.
- Another agent is adding `String`/`StringBuilder` methods and `Integer`/`Long` `toHexString`,
  `toBinaryString`, `toOctalString`, `numberOfLeadingZeros`/`TrailingZeros`, `bitCount`,
  `highestOneBit`, `reverse` right now: do NOT add those, and keep your edits in `lower.rs`'s
  `call` and in `lower/boxing.rs` small and grouped so merging is easy. Do not touch object,
  class, nullness or borrowing code (another agent is rewriting it).

## Deliverables
- Double and Float parsing with Java's grammar and rounding
- Integer and Long parsing with a radix, Java's rules
- Unsigned Integer, Long, Byte and Short methods
- Character code point methods including toChars
- Corpus programs corpus/features/ParseDoubleAndFloat.java, corpus/features/ParseWithRadix.java, corpus/features/UnsignedArithmetic.java, corpus/features/CodePoints.java
- README "Current state" updated
