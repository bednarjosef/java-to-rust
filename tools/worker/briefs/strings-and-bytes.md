# Strings and bytes, StringBuffer

Strings are hand-written in the runtime (see README and the runtime's `string.rs`,
`builder.rs`, and `lower/strings.rs`, `codegen/strings.rs`, `codegen/text.rs`). Add the
boundary between strings and bytes, so the JDK's charset code never needs translating, and
`StringBuffer`.

## Scope
- `String.getBytes()` (UTF-8, Java's default charset since 18), `getBytes(StandardCharsets.X)`
  and `getBytes("UTF-8")`-style names for UTF-8, US-ASCII, ISO-8859-1, UTF-16 (with BOM, as
  Java), UTF-16BE, UTF-16LE; unmappable characters replaced as Java does (`?` for ASCII and
  Latin-1, U+FFFD handling of lone surrogates in UTF-8: check Java's output). The result is a
  `byte[]` like other arrays.
- `new String(byte[])`, `new String(byte[], charset)`, `new String(byte[], offset, length,
  charset)`: decoding with Java's replacement of malformed input (check Java's exact output for
  invalid UTF-8 sequences).
- `StandardCharsets.UTF_8` etc. and `Charset.forName("...")` as values only usable in these
  calls (reject other uses with a clear message).
- `StringBuffer`: the same API as `StringBuilder` (synchronization has no effect in a single
  thread), sharing its runtime type.
- `String.chars()` is NOT needed (streams). `CharSequence` as a type is NOT needed yet (it is
  an interface; the object-model work comes first).
- Corpus programs comparing with Java on non-ASCII text, emoji (surrogate pairs) and invalid
  bytes. Do not touch object, nullness or borrowing code, nor number code in codegen/expr.rs.

## Deliverables
- getBytes for the standard charsets with Java's replacement rules
- new String from bytes with Java's malformed-input handling
- StandardCharsets and Charset.forName values
- StringBuffer
- Corpus programs corpus/features/StringBytes.java and corpus/features/StringBuffers.java
- README "Current state" updated
