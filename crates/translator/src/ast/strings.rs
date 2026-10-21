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
    /// `new String(chars)`, `String.valueOf(chars)` and `String.copyValueOf(chars)`, with an
    /// offset and count or without.
    FromChars,
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
}

impl BuilderOp {
    /// Whether the method changes the builder.
    pub fn mutates(self) -> bool {
        matches!(
            self,
            BuilderOp::Append
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
    /// `Integer.parseInt(s, begin, end, radix)`: the indices of the substring that is read.
    ParseRange,
}
