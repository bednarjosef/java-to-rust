//! The operations on charsets: the `StandardCharsets` constants, `Charset.forName` and the methods
//! of a `Charset` value. A charset is a value of `Type::Charset`, which the runtime's enum holds;
//! lowering makes each operation a `Builtin::Charset` expression (see `lower/charsets.rs`).

/// A charset, by the constant that names it, or the lookup of a name.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharsetOp {
    /// `StandardCharsets.UTF_8`.
    Utf8,
    /// `StandardCharsets.US_ASCII`.
    UsAscii,
    /// `StandardCharsets.ISO_8859_1`.
    Latin1,
    /// `StandardCharsets.UTF_16`, with a byte order mark when encoding.
    Utf16,
    /// `StandardCharsets.UTF_16BE`.
    Utf16Be,
    /// `StandardCharsets.UTF_16LE`.
    Utf16Le,
    /// `Charset.forName(name)`; the name is the argument.
    ForName,
    /// `Charset.isSupported(name)`; the name is the argument.
    IsSupported,
    /// `Charset.defaultCharset()`, which is UTF-8.
    DefaultCharset,
    /// `charset.name()`; the charset is the first argument.
    Name,
    /// `charset.displayName()`; the charset is the first argument.
    DisplayName,
    /// `TranslatedCoders.historicalName(charset)`, the translator's: the name that `getEncoding`
    /// of a reader or a writer gives (`Cp1252`); the charset is the first argument.
    HistoricalName,
    /// `charset.toString()`; the charset is the first argument.
    ToString,
    /// `charset.hashCode()`; the charset is the first argument.
    HashCode,
    /// `charset.equals(other)`, which compares them as `==` does; they are the arguments, and
    /// `other` may be null.
    Equals,
}
