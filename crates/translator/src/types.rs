//! The Java typing rules that the analyses and the code generator need: conversions and the
//! result types of operators (JLS chapters 5 and 15).

use crate::ast::*;

/// Whether a value of type `from` can be passed or assigned where `to` is expected: identity or
/// widening primitive conversion.
pub fn assignable(from: Type, to: Type) -> bool {
    from == to
        || matches!(
            (from, to),
            (Type::Int, Type::Long) | (Type::Int, Type::Double) | (Type::Long, Type::Double)
        )
}

/// Binary numeric promotion of two numeric types.
pub fn promote(a: Type, b: Type) -> Type {
    if a == Type::Double || b == Type::Double {
        Type::Double
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
        BinOp::Shl | BinOp::Shr | BinOp::UShr if integral => Some(promote(left, Type::Int)),
        BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor if integral => Some(promote(left, right)),
        BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::And | BinOp::Or if boolean => {
            Some(Type::Boolean)
        }
        BinOp::Eq | BinOp::Ne if numeric || boolean => Some(Type::Boolean),
        BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge if numeric => Some(Type::Boolean),
        _ => None,
    }
}
