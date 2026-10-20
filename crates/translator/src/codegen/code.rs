//! Pieces of generated Rust: expression text with its precedence, Rust type names, literals.

use super::*;

// Precedence of generated Rust expressions, from loosest to tightest.
pub(super) const LOWEST: u8 = 0;
pub(super) const OR: u8 = 5;
pub(super) const AND: u8 = 10;
pub(super) const CMP: u8 = 20;
pub(super) const BIT_OR: u8 = 30;
pub(super) const BIT_XOR: u8 = 35;
pub(super) const BIT_AND: u8 = 40;
pub(super) const SHIFT: u8 = 50;
pub(super) const ADD: u8 = 60;
pub(super) const MUL: u8 = 70;
pub(super) const AS: u8 = 80;
pub(super) const UNARY: u8 = 90;
pub(super) const ATOM: u8 = 100;

/// A generated Rust expression.
pub(super) struct Code {
    pub(super) text: String,
    pub(super) prec: u8,
    /// Set for numeric literals, whose Rust type comes from context unless suffixed.
    pub(super) literal: Option<Literal>,
    /// Whether the Rust type is left to inference, as for `0` or `if c { 1 } else { 2 }`.
    /// Rust cannot call methods on such values, so their type must be written somewhere.
    pub(super) untyped: bool,
    /// For an untyped expression of literals such as `24 * 60`, the text with its type written.
    pub(super) typed_text: Option<String>,
}

#[derive(Clone, Copy)]
pub(super) enum Literal {
    Int(i32),
    Long(i64),
    Double(f64),
    Float(f32),
    /// A `char` literal as written in the source, such as `'a'`.
    Char(u16),
    /// A `byte`, `short` or `char` constant given as a number.
    Small(i32, Type),
}

impl Code {
    pub(super) fn new(text: impl Into<String>, prec: u8) -> Code {
        Code { text: text.into(), prec, literal: None, untyped: false, typed_text: None }
    }

    pub(super) fn atom(text: impl Into<String>) -> Code {
        Code::new(text, ATOM)
    }

    pub(super) fn literal(literal: Literal) -> Code {
        let (text, negative) = match literal {
            Literal::Int(i32::MIN) => ("i32::MIN".to_string(), false),
            Literal::Long(i64::MIN) => ("i64::MIN".to_string(), false),
            Literal::Int(v) => (v.to_string(), v < 0),
            Literal::Long(v) => (v.to_string(), v < 0),
            Literal::Double(v) if v.is_finite() => (format!("{v:?}"), v.is_sign_negative()),
            _ => return small_literal(literal),
        };
        Code {
            text,
            prec: if negative { UNARY } else { ATOM },
            literal: Some(literal),
            untyped: true,
            typed_text: None,
        }
    }

    /// The text, in parentheses if it binds looser than `prec`.
    pub(super) fn at(&self, prec: u8) -> String {
        if self.prec < prec { format!("({})", self.text) } else { self.text.clone() }
    }

    /// The text with a type suffix on a numeric literal, so that its type does not depend on
    /// inference.
    pub(super) fn typed(&self) -> String {
        if let Some(text) = &self.typed_text {
            return text.clone();
        }
        match self.literal {
            Some(Literal::Int(v)) if v != i32::MIN => format!("{v}i32"),
            Some(Literal::Long(v)) if v != i64::MIN => format!("{v}i64"),
            Some(Literal::Double(v)) if v.is_finite() => format!("{v:?}f64"),
            Some(Literal::Float(v)) if v.is_finite() => format!("{v:?}f32"),
            Some(Literal::Small(v, ty)) => format!("{v}{}", rust_type(ty)),
            _ => self.text.clone(),
        }
    }

    /// The text as the operand of `as`; a literal's type must not come from the cast.
    pub(super) fn cast_operand(&self) -> String {
        if self.literal.is_some() { self.typed() } else { self.at(AS) }
    }

    /// The text as the receiver of a method call.
    pub(super) fn receiver(&self) -> String {
        let text = self.typed();
        if self.prec < ATOM { format!("({text})") } else { text }
    }
}

pub(super) fn rust_type(ty: Type) -> &'static str {
    match ty {
        Type::Int => "i32",
        Type::Long => "i64",
        Type::Double => "f64",
        Type::Byte => "i8",
        Type::Short => "i16",
        Type::Char => "u16",
        Type::Float => "f32",
        Type::Boolean => "bool",
        Type::String => "String",
        Type::StringBuilder => "java::StringBuilder",
        Type::Charset => "java::Charset",
        Type::Void => "()",
        // A boxed type is named by `Gen::ty_name`, which knows whether its identity is observed.
        Type::Array(_) | Type::Class(_) | Type::Enum(_) | Type::Null | Type::Boxed(_) => {
            unreachable!("lowering rejects {ty}")
        }
    }
}

pub(super) fn infix(op: &str, prec: u8, left: &Code, right: &Code) -> Code {
    // `a as i64 < b` would parse `<` as the start of generic arguments.
    let left_cast = matches!(op, "<" | "<<") && ends_with_cast(&left.text) && left.prec >= prec;
    let left_prec = if prec == CMP { CMP + 1 } else { prec };
    let l = if left_cast { format!("({})", left.text) } else { left.at(left_prec) };
    let r = right.at(prec + 1);
    let mut code = Code::new(format!("{l} {op} {r}"), prec);
    if left.untyped && right.untyped && left.literal.is_some() {
        // In `7 / 2` the type is left to inference, which defaults to `i32` and `f64`: right for
        // int and double, but a long needs its type written.
        let typed = left.typed();
        let typed = if left.prec < left_prec { format!("({typed})") } else { typed };
        let typed_text = format!("{typed} {op} {r}");
        if matches!(left.literal, Some(Literal::Long(_))) {
            code.text = typed_text;
        } else if prec != CMP {
            code.untyped = true;
            code.typed_text = Some(typed_text);
        }
    }
    code
}

pub(super) fn ends_with_cast(text: &str) -> bool {
    ["i32", "i64", "f64", "u32", "u64", "i8", "i16", "u16", "f32"]
        .iter()
        .any(|t| text.ends_with(&format!(" as {t}")))
}

pub(super) fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|c| c.is_alphabetic() || c == '_')
        && chars.all(|c| c.is_alphanumeric() || c == '_')
        && !["true", "false"].contains(&text)
}

/// A Rust string literal with the given contents.
pub(super) fn rust_string(s: &str) -> String {
    format!("{s:?}")
}
