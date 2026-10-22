//! The methods of `String`, `StringBuilder` and the bit and radix methods of `Integer` and
//! `Long`, which the runtime implements by hand (`java_runtime::string`, `builder`, `regex`,
//! `format`, `integer`). Each is a `Builtin` whose arguments are the call's, with the receiver
//! first for an instance method; the overload is told apart by the number and types of the
//! arguments.

/// A method of `String`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StringOp {
    /// `s.substring(begin)` and `s.substring(begin, end)`.
    Substring,
    /// `s.indexOf(x)` and `s.indexOf(x, from)`, of a `char` (or code point) or a string.
    IndexOf,
    LastIndexOf,
    Contains,
    /// `s.startsWith(prefix)` and `s.startsWith(prefix, offset)`.
    StartsWith,
    EndsWith,
    IsEmpty,
    IsBlank,
    Trim,
    Strip,
    StripLeading,
    StripTrailing,
    ToUpperCase,
    ToLowerCase,
    /// `s.replace(a, b)` of two `char`s or two strings.
    Replace,
    ReplaceAll,
    ReplaceFirst,
    Matches,
    /// `s.split(regex)` and `s.split(regex, limit)`.
    Split,
    /// `String.join(delimiter, elements...)`, the elements as separate arguments or one array.
    Join,
    Repeat,
    CompareTo,
    CompareToIgnoreCase,
    EqualsIgnoreCase,
    Concat,
    ToCharArray,
    /// `s.getChars(srcBegin, srcEnd, dst, dstBegin)`: the receiver, then the arguments.
    GetChars,
    /// The UTF-16 code units of a string as an `int[]`, which `s.chars()` streams (see
    /// `program::reach::STRING_STREAMS`).
    CharCodes,
    /// `JavaLangAccess.join(prefix, suffix, delimiter, elements, size)`: the first `size`
    /// strings of `elements` joined by `delimiter`, between `prefix` and `suffix`.
    JoinRange,
    /// The lines of a string as an `Object[]` of strings, which `s.lines()` streams: ended by
    /// `\n`, `\r` or `\r\n`, without an empty last line.
    Lines,
    /// `new String(chars)`, `String.valueOf(chars)` and `String.copyValueOf(chars)`, with an
    /// offset and count or without.
    FromChars,
    /// `new String(codePoints, offset, count)`.
    FromCodePoints,
    /// `new String(ascii, hibyte, offset, count)`: each `char` is `hibyte` over a byte.
    FromHighByte,
    HashCode,
    /// `s.toString()`, `s.intern()` and `new String(s)`: the string itself (or an equal copy),
    /// which throws when `s` is null.
    Identity,
    /// `s.equals(x)` where `x` is no string (a box, a builder, an enum, an array, an object): never
    /// equal, after evaluating both and throwing on a null `s`.
    EqualsOther,
    /// `s.getBytes()`, `s.getBytes(charset)` and `s.getBytes(name)`: the receiver, then the
    /// charset (a `Type::Charset`, or a `Type::String` naming one), or none for UTF-8.
    GetBytes,
    /// `new String(bytes)`, `new String(bytes, charset)` and `new String(bytes, offset, length,
    /// charset)`: the array, then the same charset forms as `GetBytes`.
    FromBytes,
    /// `s.getBytes(srcBegin, srcEnd, dst, dstBegin)`, the deprecated form that keeps the low byte
    /// of each `char`.
    GetLowBytes,
    CodePointAt,
    CodePointBefore,
    CodePointCount,
    OffsetByCodePoints,
    /// `s.contentEquals(text)` of a string or a builder: whether the texts are equal.
    ContentEquals,
    /// `s.regionMatches(toffset, other, ooffset, len)` and the form with `ignoreCase` first.
    RegionMatches,
    StripIndent,
    /// `s.translateEscapes()`: the escape sequences of `s` as the characters they stand for,
    /// which throws `IllegalArgumentException` for a malformed one.
    TranslateEscapes,
}

/// A method of `StringBuilder`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuilderOp {
    /// `new StringBuilder()`, `new StringBuilder(capacity)` and `new StringBuilder(text)`.
    New,
    /// `sb.append(x)`, `sb.append(chars, offset, len)` and `sb.append(text, start, end)`.
    Append,
    /// `sb.insert(offset, x)`.
    Insert,
    Reverse,
    SetCharAt,
    CharAt,
    DeleteCharAt,
    Delete,
    Replace,
    IndexOf,
    LastIndexOf,
    Length,
    SetLength,
    ToString,
    Substring,
    IsEmpty,
    CompareTo,
    AppendCodePoint,
    /// `sb.repeat(codePoint, count)` and `sb.repeat(text, count)`.
    Repeat,
    CodePointAt,
    CodePointBefore,
    CodePointCount,
    OffsetByCodePoints,
    /// `sb.getChars(srcBegin, srcEnd, dst, dstBegin)`.
    GetChars,
    /// `sb.ensureCapacity(minimumCapacity)`, which leaves the text as it is.
    EnsureCapacity,
}

impl BuilderOp {
    /// Whether the method changes the builder (or, for `getChars`, an array).
    pub fn mutates(self) -> bool {
        matches!(
            self,
            BuilderOp::Append
                | BuilderOp::GetChars
                | BuilderOp::AppendCodePoint
                | BuilderOp::Repeat
                | BuilderOp::Insert
                | BuilderOp::Reverse
                | BuilderOp::SetCharAt
                | BuilderOp::DeleteCharAt
                | BuilderOp::Delete
                | BuilderOp::Replace
                | BuilderOp::SetLength
        )
    }

    /// Whether the method returns the builder itself, for chaining.
    pub fn chains(self) -> bool {
        matches!(
            self,
            BuilderOp::Append
                | BuilderOp::AppendCodePoint
                | BuilderOp::Repeat
                | BuilderOp::Insert
                | BuilderOp::Reverse
                | BuilderOp::DeleteCharAt
                | BuilderOp::Delete
                | BuilderOp::Replace
        )
    }
}

/// A bit or radix method of `Integer` or `Long`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BitsOp {
    ToHexString,
    ToBinaryString,
    ToOctalString,
    NumberOfLeadingZeros,
    NumberOfTrailingZeros,
    BitCount,
    HighestOneBit,
    LowestOneBit,
    Reverse,
    ReverseBytes,
    RotateLeft,
    RotateRight,
    Signum,
    /// `Integer.toString(v, radix)`.
    ToStringRadix,
    /// `Integer.parseInt(s, radix)`.
    ParseRadix,
    /// `Integer.parseInt(s, begin, end, radix)` and `Long.parseLong`, read from a `CharSequence`.
    ParseRange,
}
