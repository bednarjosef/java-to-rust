/// The code point methods of `Character`, see `Builtin::CodePoint`. Each takes an `int` code
/// point, or two `char`s for `ToCodePoint`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodePointOp {
    /// `Character.toChars(cp)`: a `char[]` of one or two units; throws on an invalid code point.
    ToChars,
    /// `Character.toString(cp)` for an `int` code point; throws on an invalid code point.
    ToString,
    /// `Character.charCount(cp)`: 2 for a supplementary code point, else 1.
    CharCount,
    /// `Character.toCodePoint(high, low)`.
    ToCodePoint,
    /// `Character.highSurrogate(cp)` and `Character.lowSurrogate(cp)`, which make a `char`.
    HighSurrogate,
    LowSurrogate,
    /// `Character.isValidCodePoint(cp)`, `isBmpCodePoint(cp)` and `isSupplementaryCodePoint(cp)`.
    IsValid,
    IsBmp,
    IsSupplementary,
}
