//! The values of `static final` integer constants. javac folds each one, and the lowering keeps the
//! folded value as a literal, so the value is known wherever the constant is read. The range
//! analysis uses it as a point, so that a loop bounded by a constant has a known counter range.

use crate::ast::*;

/// The text of a string literal, or of a read of a `static final` string constant.
pub fn text<'c>(class: &'c Class, e: &'c Expr) -> Option<&'c str> {
    match &e.kind {
        ExprKind::Str(s) => Some(s),
        ExprKind::Static { class: owner, name } => {
            let field = class.types.get(*owner)?.statics.iter().find(|f| f.name == *name)?;
            match &field.init.as_ref().filter(|_| field.constant)?.kind {
                ExprKind::Str(s) => Some(s),
                _ => None,
            }
        }
        _ => None,
    }
}

/// The value of the static `name` of class `owner`, when it is an `int` or `long` constant.
pub fn static_value(class: &Class, owner: usize, name: &str) -> Option<i128> {
    let field = class.types.get(owner)?.statics.iter().find(|f| f.name == name)?;
    if !field.constant || !matches!(field.ty, Type::Int | Type::Long) {
        return None;
    }
    match field.init.as_ref()?.kind {
        ExprKind::Int(v) => Some(i128::from(v)),
        ExprKind::Long(v) => Some(i128::from(v)),
        _ => None,
    }
}
