//! Lowering of what the primitive types `byte`, `short`, `char` and `float` bring: their
//! literals, the constants of the wrapper classes (`Integer.MAX_VALUE`, `Float.NaN`) and the
//! `Character` methods.

use super::*;

impl<'t> Lowering<'t> {
    /// `'a'`, which javac's tree gives as its UTF-16 code unit (see `JavaTree.java`).
    pub(super) fn char_literal(&self, node: Node) -> Result<ExprKind, Error> {
        Ok(ExprKind::Char(self.number(node)?))
    }

    /// `1.5f`, which javac's tree gives as `Float.toString` of its value; that text reads back as
    /// the same `f32`.
    pub(super) fn float_literal(&self, node: Node) -> Result<ExprKind, Error> {
        Ok(ExprKind::Float(self.number(node)?))
    }

    /// A field of the class library that is a primitive constant, such as `Integer.MAX_VALUE`;
    /// `None` for any other field.
    pub(super) fn constant_field(&self, node: Node) -> Option<ExprKind> {
        let symbol = node.symbol().filter(|s| s.kind == "FIELD" && s.is_static)?;
        let integer = |min: i64, max: i64| match symbol.name {
            "MIN_VALUE" => Some(min),
            "MAX_VALUE" => Some(max),
            _ => None,
        };
        let floating = |max: f64, min: f64| match symbol.name {
            "MAX_VALUE" => Some(max),
            "MIN_VALUE" => Some(min),
            "NaN" => Some(f64::NAN),
            "POSITIVE_INFINITY" => Some(f64::INFINITY),
            "NEGATIVE_INFINITY" => Some(f64::NEG_INFINITY),
            _ => None,
        };
        Some(match symbol.owner {
            "java.lang.Integer" => ExprKind::Int(integer(i32::MIN as i64, i32::MAX as i64)? as i32),
            "java.lang.Long" => ExprKind::Long(integer(i64::MIN, i64::MAX)?),
            "java.lang.Character" => ExprKind::Char(integer(0, u16::MAX as i64)? as u16),
            // There are no byte and short literals: `(byte) -128`.
            "java.lang.Byte" => self.small_constant(Type::Byte, integer(i8::MIN as i64, i8::MAX as i64)?),
            "java.lang.Short" => self.small_constant(Type::Short, integer(i16::MIN as i64, i16::MAX as i64)?),
            "java.lang.Float" => ExprKind::Float(floating(f32::MAX as f64, f32::from_bits(1) as f64)? as f32),
            "java.lang.Double" => ExprKind::Double(floating(f64::MAX, f64::from_bits(1))?),
            "java.lang.Math" | "java.lang.StrictMath" => ExprKind::Double(match symbol.name {
                "PI" => std::f64::consts::PI,
                "E" => std::f64::consts::E,
                "TAU" => std::f64::consts::TAU,
                _ => return None,
            }),
            _ => return None,
        })
    }

    fn small_constant(&self, ty: Type, value: i64) -> ExprKind {
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        let int = Expr { kind: ExprKind::Int(value as i32), pos: Pos::default(), ty: Type::Int, id };
        ExprKind::Cast(ty, Box::new(int))
    }

    /// `Character.isDigit(c)` and the like, on a `char`; the overloads that take a code point
    /// (an `int`) are not supported.
    pub(super) fn character_method(&self, node: Node, name: &str, signature: &str) -> Result<Builtin, Error> {
        let builtin = match name {
            "isDigit" => Builtin::CharIsDigit,
            "isLetter" => Builtin::CharIsLetter,
            "isUpperCase" => Builtin::CharIsUpperCase,
            "isLowerCase" => Builtin::CharIsLowerCase,
            "toUpperCase" => Builtin::CharToUpperCase,
            "toLowerCase" => Builtin::CharToLowerCase,
            "getNumericValue" => Builtin::CharNumericValue,
            _ => return unsupported(node, &format!("method `Character.{name}`")),
        };
        if !signature.starts_with("(char)") {
            return unsupported(node, &format!("`Character.{name}` on a code point"));
        }
        Ok(builtin)
    }
}
