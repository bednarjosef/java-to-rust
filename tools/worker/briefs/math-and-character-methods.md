# Math, Double, Float, Character and Boolean methods

Add the static JDK methods that typical programs call on `Math`, `StrictMath`, `Double`,
`Float`, `Character` and `Boolean`, with Java's exact results. First check which exist:
`lower/natives.rs`, `lower/primitives.rs`, `lower/boxing.rs`, `lower.rs` (`fn call`), and the
runtime's `math.rs`, `float.rs`, `character.rs`, `boxed.rs`. Add only what is missing.

## Scope
- `Math`: floorDiv, floorMod (int and long), abs/negateExact/addExact/subtractExact/
  multiplyExact/incrementExact/toIntExact (overflow throws ArithmeticException: exit code 1
  through the runtime's throw), signum, copySign, hypot, cbrt, ceil, floor, rint, round (int and
  long overloads, Java's half-up rule), clamp, fma, ulp, nextUp, nextDown, toRadians, toDegrees,
  log1p, expm1, max/min on all types (NaN and -0.0 rules), random is NOT needed.
- `Double`/`Float`: isNaN, isInfinite, isFinite, compare, MIN_VALUE, MAX_VALUE, MIN_NORMAL,
  POSITIVE/NEGATIVE_INFINITY, NaN, sum/max/min, toString(double) (same text as printing),
  parseDouble/valueOf edge cases if not done (hex floats may stay rejected).
- `Character`: isWhitespace, isLetterOrDigit, isAlphabetic, isSpaceChar, digit(char, radix),
  forDigit, getNumericValue if missing, toChars, isSurrogate, isHighSurrogate, isLowSurrogate,
  MIN_VALUE/MAX_VALUE, compare, toString(char). Use Java's definitions exactly (e.g.
  isWhitespace is not Rust's is_whitespace: no-break spaces are excluded).
- `Boolean`: parseBoolean (case-insensitive "true"), toString, compare, logicalAnd/Or/Xor.
- Do NOT touch `Integer`/`Long` methods or `String`/`StringBuilder` (another agent adds those
  now in `lower.rs` and `codegen/strings.rs`; keep your edits to the `call` match small and
  together so the merge is easy), nor objects, inheritance or generics.
- Each method gets a corpus program exercising edge cases (NaN, ±0.0, infinities, MIN/MAX
  values, overflow), compared against Java by the corpus test.

## Deliverables
- Missing Math and StrictMath methods with exact results
- Missing Double and Float methods and constants
- Missing Character methods with Java's definitions
- Boolean methods
- Corpus programs: corpus/features/MathMethods.java, corpus/features/DoubleAndFloatMethods.java, corpus/features/CharacterMethods.java, corpus/features/BooleanMethods.java
- README "Current state" updated
