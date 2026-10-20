//! The charsets of `StandardCharsets` and `Charset.forName`. A charset is never stored in a
//! variable or passed around: lowering accepts one only as the charset argument of `getBytes` and
//! `new String`, and makes it an expression of `Type::Charset` (see `lower/charsets.rs`).

/// A charset, by the constant that names it, or the lookup of a name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
}
