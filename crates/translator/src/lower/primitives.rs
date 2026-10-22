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
        // The separators of `File`, which are the platform's, as the runtime's `file.separator`
        // and `path.separator` are: a translated program runs on Unix.
        match (symbol.owner, symbol.name) {
            ("java.io.File", "separator") => return Some(ExprKind::Str("/".to_string())),
            ("java.io.File", "pathSeparator") => return Some(ExprKind::Str(":".to_string())),
            ("java.io.File", "separatorChar") => return Some(ExprKind::Char(u16::from(b'/'))),
            ("java.io.File", "pathSeparatorChar") => return Some(ExprKind::Char(u16::from(b':'))),
            _ => {}
        }
        let integer = |min: i64, max: i64| match symbol.name {
            "MIN_VALUE" => Some(min),
            "MAX_VALUE" => Some(max),
            _ => None,
        };
        let floating = |max: f64, min: f64, normal: f64| match symbol.name {
            "MAX_VALUE" => Some(max),
            "MIN_VALUE" => Some(min),
            "MIN_NORMAL" => Some(normal),
            "NaN" => Some(f64::NAN),
            "POSITIVE_INFINITY" => Some(f64::INFINITY),
            "NEGATIVE_INFINITY" => Some(f64::NEG_INFINITY),
            _ => None,
        };
        // `Integer.SIZE` and `Integer.BYTES`: the width in bits and bytes, an `int` for each class.
        let bits = match symbol.owner {
            "java.lang.Byte" => Some(8),
            "java.lang.Short" | "java.lang.Character" => Some(16),
            "java.lang.Integer" | "java.lang.Float" => Some(32),
            "java.lang.Long" | "java.lang.Double" => Some(64),
            _ => None,
        };
        match (bits, symbol.name) {
            (Some(bits), "SIZE") => return Some(ExprKind::Int(bits)),
            (Some(bits), "BYTES") => return Some(ExprKind::Int(bits / 8)),
            _ => {}
        }
        Some(match symbol.owner {
            "java.lang.Integer" => ExprKind::Int(integer(i32::MIN as i64, i32::MAX as i64)? as i32),
            "java.lang.Long" => ExprKind::Long(integer(i64::MIN, i64::MAX)?),
            "java.lang.Character" => ExprKind::Char(integer(0, u16::MAX as i64)? as u16),
            // There are no byte and short literals: `(byte) -128`.
            "java.lang.Byte" => self.small_constant(Type::Byte, integer(i8::MIN as i64, i8::MAX as i64)?),
            "java.lang.Short" => self.small_constant(Type::Short, integer(i16::MIN as i64, i16::MAX as i64)?),
            "java.lang.Float" => {
                let normal = f32::MIN_POSITIVE as f64;
                ExprKind::Float(floating(f32::MAX as f64, f32::from_bits(1) as f64, normal)? as f32)
            }
            "java.lang.Double" => ExprKind::Double(floating(f64::MAX, f64::from_bits(1), f64::MIN_POSITIVE)?),
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
    /// (an `int`) are not supported. `isAlphabetic` has only the code point overload, which a
    /// `char` argument selects by widening, so it takes that form for a `char` only.
    pub(super) fn character_method(
        &self,
        node: Node,
        name: &str,
        signature: &str,
        char_arg: bool,
    ) -> Result<Builtin, Error> {
        let Some((builtin, params)) = character_builtin(name) else {
            return unsupported(node, &format!("method `Character.{name}`"));
        };
        if signature.starts_with(params) && (params != "(int)" || char_arg) {
            return Ok(builtin);
        }
        // The `int` overload of the same method, which works on a code point (see `lower/code_point.rs`).
        match super::code_point::character_op(name) {
            Some(op) if signature.starts_with("(int") => Ok(Builtin::CodePoint(op)),
            _ => unsupported(node, &format!("`Character.{name}` on a code point")),
        }
    }
}

/// Whether the runtime does the method `name` of `Character` with javac's `signature`: an
/// instance method or constructor of a box, a method that the boxes share, or one of the `char`
/// and code point methods that the runtime's tables answer (`character_method`). The JDK's
/// `Character` is translated for its other static methods and its nested classes
/// (`Character.getType`, `UnicodeScript`), whose data are the JDK's own `CharacterData` classes.
pub fn runtime_character_method(name: &str, signature: &str) -> bool {
    if name == "<init>" || super::boxing::is_boxed_method(name) || super::code_point::is_code_point_name(name)
    {
        return true;
    }
    match character_builtin(name) {
        Some((_, params)) if signature.starts_with(params) => true,
        Some(_) => super::code_point::character_op(name).is_some() && signature.starts_with("(int"),
        None => false,
    }
}

/// The builtin of a `Character` method on a `char`, with the parameters it takes.
fn character_builtin(name: &str) -> Option<(Builtin, &'static str)> {
    Some(match name {
        "isDigit" => (Builtin::CharIsDigit, "(char)"),
        "isLetter" => (Builtin::CharIsLetter, "(char)"),
        "isUpperCase" => (Builtin::CharIsUpperCase, "(char)"),
        "isLowerCase" => (Builtin::CharIsLowerCase, "(char)"),
        "toUpperCase" => (Builtin::CharToUpperCase, "(char)"),
        "toLowerCase" => (Builtin::CharToLowerCase, "(char)"),
        "getNumericValue" => (Builtin::CharNumericValue, "(char)"),
        "isWhitespace" => (Builtin::CharIsWhitespace, "(char)"),
        "isSpaceChar" => (Builtin::CharIsSpaceChar, "(char)"),
        "isLetterOrDigit" => (Builtin::CharIsLetterOrDigit, "(char)"),
        "isAlphabetic" => (Builtin::CharIsAlphabetic, "(int)"),
        "isSurrogate" => (Builtin::CharIsSurrogate, "(char)"),
        "isHighSurrogate" => (Builtin::CharIsHighSurrogate, "(char)"),
        "isLowSurrogate" => (Builtin::CharIsLowSurrogate, "(char)"),
        "digit" => (Builtin::CharDigit, "(char,int)"),
        "forDigit" => (Builtin::CharForDigit, "(int,int)"),
        _ => return None,
    })
}
