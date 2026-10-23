//! `byte`, `short`, `char` and `float` (`i8`, `i16`, `u16`, `f32`): their literals, conversions
//! with Java's narrowing rules, assignments that narrow implicitly, printing, `charAt` and the
//! `Character` methods.
//!
//! Java promotes `byte`, `short` and `char` to `int` before any arithmetic, so the faithful
//! translation of `(char) (c + 1)` is `(c as i32 + 1) as u16`. Wrapping arithmetic in the small
//! type keeps the same low bits, so that is `c.wrapping_add(1)`, and `c + 1` where the range
//! analysis shows the result fits.

use super::expr::{Operand, writes_inside};
use super::*;
use crate::types::{integral_bounds, narrow, promote};

/// Whether a conversion from `from` to `to` involves a type handled here.
pub(super) fn involves_primitive(from: Type, to: Type) -> bool {
    from != to
        && from.is_numeric()
        && to.is_numeric()
        && [from, to].iter().any(|t| t.is_small() || *t == Type::Float)
}

/// A Rust char literal for a code unit, if it is a Unicode scalar value: `'a'`, `'\n'`.
pub(super) fn rust_char(c: u16) -> Option<String> {
    char::from_u32(c as u32).map(|c| format!("{c:?}"))
}

/// The code of the literals that `Code::literal` leaves to this module.
pub(super) fn small_literal(literal: Literal) -> Code {
    let (text, prec, untyped) = match literal {
        Literal::Float(v) if v.is_nan() => ("f32::NAN".to_string(), ATOM, false),
        Literal::Float(v) if v.is_infinite() => {
            (if v > 0.0 { "f32::INFINITY" } else { "f32::NEG_INFINITY" }.to_string(), ATOM, false)
        }
        Literal::Double(v) if v.is_nan() => ("f64::NAN".to_string(), ATOM, false),
        Literal::Double(v) if v.is_infinite() => {
            (if v > 0.0 { "f64::INFINITY" } else { "f64::NEG_INFINITY" }.to_string(), ATOM, false)
        }
        Literal::Float(v) => (format!("{v:?}"), if v.is_sign_negative() { UNARY } else { ATOM }, true),
        Literal::Char(c) => match rust_char(c) {
            Some(ch) => (format!("{ch} as u16"), AS, false),
            None => (format!("{c:#x}u16"), ATOM, false),
        },
        Literal::Small(v, _) => (v.to_string(), if v < 0 { UNARY } else { ATOM }, true),
        Literal::Int(_) | Literal::Long(_) | Literal::Double(_) => {
            unreachable!("Code::literal handles these")
        }
    };
    Code { text, prec, literal: Some(literal), untyped, typed_text: None }
}

/// The text that string concatenation appends for a constant.
pub(super) fn literal_text(literal: Literal) -> String {
    match literal {
        Literal::Int(v) => v.to_string(),
        Literal::Long(v) => v.to_string(),
        Literal::Double(v) => java_runtime::double_to_string(v),
        Literal::Float(v) => java_runtime::float_to_string(v),
        Literal::Char(_) | Literal::Small(_, Type::Char) => {
            java_runtime::char_to_string(literal_value(literal).0 as u16).to_string_lossy().into_owned()
        }
        Literal::Small(v, _) => v.to_string(),
    }
}

/// A literal's value: as an integer if it is one, else as a double.
pub(super) fn literal_value(literal: Literal) -> (i64, Option<f64>) {
    match literal {
        Literal::Int(v) | Literal::Small(v, _) => (v as i64, None),
        Literal::Long(v) => (v, None),
        Literal::Char(c) => (c as i64, None),
        Literal::Double(v) => (0, Some(v)),
        Literal::Float(v) => (0, Some(v as f64)),
    }
}

/// Whether `e` is an integer constant that `ty`, a small type, can hold.
fn fits(e: &Expr, ty: Type) -> bool {
    let (min, max) = integral_bounds(ty);
    let v = match e.kind {
        ExprKind::Int(v) => v as i64,
        ExprKind::Char(c) => c as i64,
        _ => return false,
    };
    (min..=max).contains(&v)
}

/// Whether `e` can take part in arithmetic done in the small type `ty`.
fn of_type(e: &Expr, ty: Type) -> bool {
    e.ty == ty || fits(e, ty)
}

/// The type in which two operands are compared: their own type when they share it or one is a
/// constant that the other's type can hold (`c == 'a'`, `b < 10`, `f == 0.5`), else the
/// promoted type.
pub(super) fn comparison_type(l: &Operand, r: &Operand) -> Type {
    let constant_fits = |o: &Operand, ty: Type| {
        let (min, max) = integral_bounds(ty);
        ty.is_small()
            && o.ty.is_integral()
            && o.code.literal.is_some_and(|lit| match literal_value(lit) {
                (v, None) => (min..=max).contains(&v),
                _ => false,
            })
    };
    // A float widens exactly, so comparing it with a double that a float holds exactly is the
    // same as comparing it with that float.
    let exact_float = |o: &Operand, ty: Type| {
        ty == Type::Float && matches!(o.code.literal, Some(Literal::Double(v)) if v as f32 as f64 == v)
    };
    if l.ty == r.ty || constant_fits(r, l.ty) || exact_float(r, l.ty) {
        l.ty
    } else if constant_fits(l, r.ty) || exact_float(l, r.ty) {
        r.ty
    } else {
        promote(l.ty, r.ty)
    }
}

fn wrapping(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "wrapping_add",
        BinOp::Sub => "wrapping_sub",
        _ => "wrapping_mul",
    }
}

impl<'a> Gen<'a> {
    /// Converts between numeric types where one is `byte`, `short`, `char` or `float`, folding
    /// conversions of literals.
    pub(super) fn convert_primitive(&mut self, code: Code, from: Type, to: Type) -> Code {
        if let Some(literal) = code.literal {
            return fold(literal, to);
        }
        // Java narrows a floating value to `int` (saturating, NaN to 0) and then to the small
        // type; Rust's `as` saturates to the small type directly.
        let via_int = if from.is_floating() && to.is_small() { " as i32" } else { "" };
        Code::new(format!("{}{via_int} as {}", code.cast_operand(), rust_type(to)), AS)
    }

    /// `(ty) value` where `ty` is a small type. Addition, subtraction and multiplication of
    /// values of that type are done in it, wrapping, or plainly when the result fits, and so is
    /// a division that fits and cannot throw; bitwise operations on them need no cast at all.
    pub(super) fn small_cast(&mut self, ty: Type, value: &'a Expr, id: ExprId) -> Code {
        let ExprKind::Binary(op, l, r) = &value.kind else { return self.value(value, ty) };
        let fits = self.no_overflow(id);
        let applies = match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => true,
            // A quotient that fits cannot be `MIN / -1`.
            BinOp::Div | BinOp::Rem => fits && self.safe_division(value.id, r),
            _ => false,
        };
        if !applies || value.ty != Type::Int || !of_type(l, ty) || !of_type(r, ty) {
            return self.value(value, ty);
        }
        let (a, b) = (self.value(l, ty), self.value(r, ty));
        match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul if !fits => {
                Code::atom(format!("{}.{}({})", self.recv(&a, ty), wrapping(*op), b.text))
            }
            BinOp::Add | BinOp::Sub => infix(op.java(), ADD, &a, &b),
            BinOp::Mul | BinOp::Div | BinOp::Rem => infix(op.java(), MUL, &a, &b),
            BinOp::BitAnd => infix("&", BIT_AND, &a, &b),
            BinOp::BitOr => infix("|", BIT_OR, &a, &b),
            _ => infix("^", BIT_XOR, &a, &b),
        }
    }

    /// `target op= value` on a `byte`, `short`, `char` or `float` variable, which Java narrows
    /// back to the variable's type; `None` where the general translation is right.
    pub(super) fn primitive_assignment(
        &mut self,
        op: BinOp,
        name: &str,
        ty: Type,
        value: &'a Expr,
        id: ExprId,
    ) -> Option<String> {
        let compound = format!("{}=", op.java());
        if writes_inside(value) {
            return None;
        }
        if ty == Type::Float {
            // `f += d` on a double rounds once, after the addition.
            if value.ty == Type::Double {
                return None;
            }
            return Some(format!("{name} {compound} {}", self.value(value, ty).text));
        }
        if !ty.is_small() {
            return None;
        }
        let integral = value.ty.is_integral();
        let plain = of_type(value, ty) && self.no_overflow(id);
        match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul if integral && plain => {
                return Some(format!("{name} {compound} {}", self.value(value, ty).text));
            }
            BinOp::Add | BinOp::Sub | BinOp::Mul if integral => {
                let v = self.value(value, ty);
                return Some(format!("{name} = {name}.{}({})", wrapping(op), v.text));
            }
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor if integral => {
                return Some(format!("{name} {compound} {}", self.value(value, ty).text));
            }
            BinOp::Div | BinOp::Rem if plain && self.safe_division(id, value) => {
                return Some(format!("{name} {compound} {}", self.value(value, ty).text));
            }
            _ => {}
        }
        // Computed in the promoted type, then narrowed.
        let result = crate::types::binary_type(op, ty, value.ty).expect("checked");
        let current = Operand { code: Code::atom(name), ty };
        let right = self.operand(value);
        let code = self.binary(op, current, right, result, id);
        Some(format!("{name} = {}", self.convert(code, result, ty).text))
    }

    /// The text that printing or concatenating a value of type `ty` shows: doubles, floats and
    /// chars go through the runtime's `Display` types.
    pub(super) fn shown(&mut self, ty: Type, text: String) -> String {
        let wrapper = match ty {
            Type::Double => "Double",
            Type::Float => "Float",
            Type::Char => "Char",
            _ => return text,
        };
        format!("{}({text})", self.runtime(wrapper))
    }

    /// `s.charAt(i)` and the `Character` methods.
    pub(super) fn char_builtin(&mut self, builtin: Builtin, args: &'a [Expr], id: ExprId) -> Code {
        if builtin == Builtin::StringCharAt {
            return self.char_at(args, id);
        }
        let name = match builtin {
            Builtin::CharIsDigit => "is_digit",
            Builtin::CharIsLetter => "is_letter",
            Builtin::CharIsUpperCase => "is_upper_case",
            Builtin::CharIsLowerCase => "is_lower_case",
            Builtin::CharToUpperCase => "to_upper_case",
            Builtin::CharIsWhitespace => "is_whitespace",
            Builtin::CharIsSpaceChar => "is_space_char",
            Builtin::CharIsLetterOrDigit => "is_letter_or_digit",
            Builtin::CharIsAlphabetic => "is_alphabetic",
            Builtin::CharIsSurrogate => "is_surrogate",
            Builtin::CharIsHighSurrogate => "is_high_surrogate",
            Builtin::CharIsLowSurrogate => "is_low_surrogate",
            _ => "to_lower_case",
        };
        let c = self.value(&args[0], Type::Char);
        Code::atom(format!("{}({})", self.runtime(name), c.text))
    }

    /// `s.charAt(i)`: by UTF-16 code unit, by byte on an ASCII string, and by plain indexing
    /// where the index is also known to be in bounds.
    fn char_at(&mut self, args: &'a [Expr], id: ExprId) -> Code {
        let index = self.value(&args[1], Type::Int);
        let ascii = self.facts.ascii_length.contains(&id);
        if ascii && self.facts.index_in_bounds.contains(&id) && !self.nullable(&args[0]) {
            let s = match &args[0].kind {
                ExprKind::Str(_) | ExprKind::Var(_) => self.str_value(&args[0]),
                _ => self.expr(&args[0]).at(ATOM),
            };
            let i = match index.literal {
                Some(Literal::Int(v)) => v.to_string(),
                _ => format!("{} as usize", index.cast_operand()),
            };
            return Code::new(format!("{s}.as_bytes()[{i}] as u16"), AS);
        }
        let s = if self.nullable(&args[0]) { self.deref(&args[0]) } else { self.str_ref(&args[0]) };
        let f = if ascii { "ascii_char_at" } else { "char_at" };
        self.may_fail(id, f, "", &format!("{s}, {}", index.text))
    }
}

/// Converts a literal to `to` the way Java converts its value (JLS 5.1.3 for narrowing).
fn fold(literal: Literal, to: Type) -> Code {
    let (int, float) = literal_value(literal);
    // A floating value converts to an integer type through `int` (or `long`), saturating.
    let integer = |wide: Type| match float {
        Some(f) if wide == Type::Long => f as i64,
        Some(f) => f as i32 as i64,
        None => int,
    };
    let folded = match to {
        Type::Double => Literal::Double(float.unwrap_or(int as f64)),
        Type::Float => Literal::Float(float.map_or(int as f32, |f| f as f32)),
        Type::Long => Literal::Long(integer(Type::Long)),
        Type::Int => Literal::Int(narrow(integer(Type::Int), Type::Int) as i32),
        _ => Literal::Small(narrow(integer(Type::Int), to) as i32, to),
    };
    // `'a'` used as a number reads better as `'a' as i32` than as `97`.
    if let (Literal::Char(c), Type::Int | Type::Long) = (literal, to)
        && let Some(ch) = rust_char(c)
    {
        return Code {
            text: format!("{ch} as {}", rust_type(to)),
            prec: AS,
            literal: Some(folded),
            untyped: false,
            typed_text: None,
        };
    }
    Code::literal(folded)
}

/// `-x` or `~x` of a literal as a literal, the way Java computes it: `-(3)` is the literal `-3`,
/// whose type a cast such as `-3 as u16` would otherwise take, and `-(4294967296L)` keeps the
/// type of its operand.
pub(super) fn fold_unary(op: UnOp, literal: Literal) -> Option<Literal> {
    Some(match (op, literal) {
        (UnOp::Plus, _) => literal,
        (UnOp::Neg, Literal::Int(v)) => Literal::Int(v.wrapping_neg()),
        (UnOp::Neg, Literal::Long(v)) => Literal::Long(v.wrapping_neg()),
        (UnOp::Neg, Literal::Double(v)) => Literal::Double(-v),
        (UnOp::Neg, Literal::Float(v)) => Literal::Float(-v),
        (UnOp::BitNot, Literal::Int(v)) => Literal::Int(!v),
        (UnOp::BitNot, Literal::Long(v)) => Literal::Long(!v),
        _ => return None,
    })
}
