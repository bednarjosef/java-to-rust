//! The Java typing rules that the analyses and the code generator need: conversions and the
//! result types of operators (JLS chapters 5 and 15).

use crate::ast::*;

/// Whether a value of type `from` can be passed or assigned where `to` is expected: identity or
/// widening primitive conversion (JLS 5.1.2).
pub fn assignable(from: Type, to: Type) -> bool {
    from == to || (from.is_numeric() && to.is_numeric() && rank(from) < rank(to) && from_fits(from, to))
}

/// The order of the widening conversions: byte, short, int, long, float, double, with char
/// beside short.
fn rank(ty: Type) -> u8 {
    match ty {
        Type::Byte => 0,
        Type::Short | Type::Char => 1,
        Type::Int => 2,
        Type::Long => 3,
        Type::Float => 4,
        _ => 5,
    }
}

/// `char` widens to `int` and beyond but not to `short`, and nothing widens to `char`.
fn from_fits(from: Type, to: Type) -> bool {
    to != Type::Char && !(from == Type::Char && to == Type::Short)
}

/// Unary numeric promotion (JLS 5.6), as for the operand of `-`, `~` or a shift: `byte`, `short`
/// and `char` become `int`.
pub fn promote_unary(ty: Type) -> Type {
    if ty.is_small() { Type::Int } else { ty }
}

/// Binary numeric promotion of two numeric types (JLS 5.6): `double` if either is, then
/// `float`, then `long`, and `int` otherwise (so `byte`, `short` and `char` become `int`).
pub fn promote(a: Type, b: Type) -> Type {
    if a == Type::Double || b == Type::Double {
        Type::Double
    } else if a == Type::Float || b == Type::Float {
        Type::Float
    } else if a == Type::Long || b == Type::Long {
        Type::Long
    } else {
        Type::Int
    }
}

/// The type of `left op right`, or `None` when the operator does not apply to these types.
pub fn binary_type(op: BinOp, left: Type, right: Type) -> Option<Type> {
    let numeric = left.is_numeric() && right.is_numeric();
    let integral = left.is_integral() && right.is_integral();
    let boolean = left == Type::Boolean && right == Type::Boolean;
    match op {
        BinOp::Add
            if (left == Type::String || right == Type::String)
                && left != Type::Void
                && right != Type::Void =>
        {
            Some(Type::String)
        }
        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem if numeric => {
            Some(promote(left, right))
        }
        BinOp::Shl | BinOp::Shr | BinOp::UShr if integral => Some(promote_unary(left)),
        BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor if integral => Some(promote(left, right)),
        BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::And | BinOp::Or if boolean => {
            Some(Type::Boolean)
        }
        BinOp::Eq | BinOp::Ne if numeric || boolean => Some(Type::Boolean),
        BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge if numeric => Some(Type::Boolean),
        _ => None,
    }
}

/// The values of an integer type, as (minimum, maximum).
pub fn integral_bounds(ty: Type) -> (i64, i64) {
    match ty {
        Type::Byte => (i8::MIN as i64, i8::MAX as i64),
        Type::Short => (i16::MIN as i64, i16::MAX as i64),
        Type::Char => (0, u16::MAX as i64),
        Type::Int => (i32::MIN as i64, i32::MAX as i64),
        _ => (i64::MIN, i64::MAX),
    }
}

/// Narrows an integer to an integer type, as Java's casts do: the low bits, read in that type.
pub fn narrow(v: i64, to: Type) -> i64 {
    match to {
        Type::Byte => v as i8 as i64,
        Type::Short => v as i16 as i64,
        Type::Char => v as u16 as i64,
        Type::Int => v as i32 as i64,
        _ => v,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn promotion_follows_the_jls() {
        assert_eq!(promote(Type::Byte, Type::Short), Type::Int);
        assert_eq!(promote(Type::Char, Type::Char), Type::Int);
        assert_eq!(promote(Type::Float, Type::Long), Type::Float);
        assert_eq!(promote(Type::Float, Type::Double), Type::Double);
        assert_eq!(binary_type(BinOp::Shl, Type::Char, Type::Long), Some(Type::Int));
        assert_eq!(promote_unary(Type::Byte), Type::Int);
    }

    #[test]
    fn widening_skips_char_and_short() {
        assert!(assignable(Type::Byte, Type::Short));
        assert!(assignable(Type::Char, Type::Int));
        assert!(assignable(Type::Long, Type::Float));
        assert!(!assignable(Type::Char, Type::Short));
        assert!(!assignable(Type::Short, Type::Char));
        assert!(!assignable(Type::Byte, Type::Char));
        assert!(!assignable(Type::Float, Type::Long));
    }

    #[test]
    fn narrowing_keeps_the_low_bits() {
        assert_eq!(narrow(200, Type::Byte), -56);
        assert_eq!(narrow(-1, Type::Char), 65535);
        assert_eq!(narrow(70000, Type::Short), 4464);
    }
}
