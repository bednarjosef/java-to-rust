/// The unsigned methods, see `Builtin::Unsigned`. The kind of the operation is the primitive of
/// its operands: `Type::Int` for `Integer`, `Type::Long` for `Long`, and for `Widen` the narrow
/// type it widens from (`Type::Byte`, `Type::Short` or `Type::Int`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsignedOp {
    /// `Integer.divideUnsigned(a, b)` and `Long.divideUnsigned`: throws on a zero divisor.
    Divide,
    /// `Integer.remainderUnsigned(a, b)` and `Long.remainderUnsigned`.
    Remainder,
    /// `Integer.compareUnsigned(a, b)` and `Long.compareUnsigned`: -1, 0 or 1.
    Compare,
    /// `Integer.toUnsignedString(i)` and the radix form, `Long.toUnsignedString` too.
    ToString,
    /// `Integer.parseUnsignedInt(s)` and the radix form, `Long.parseUnsignedLong` too.
    Parse,
    /// `Integer.toUnsignedLong(i)`, `Byte.toUnsignedInt(b)` and the other unsigned views: the
    /// operand's bits read as unsigned, widened to the result type.
    Widen,
}
