//! `Objects.equals` and `Objects.hashCode` (see `lower::java_objects`): strings and boxes compare
//! and hash as values, null equal only to null and hashing to 0; an object runs the call of its
//! class's `equals` or `hashCode` that the builtin carries, once it is known not to be null.

use super::*;

impl<'a> Gen<'a> {
    /// `Objects.equals(a, b)`.
    /// `a == b` of two strings that only their identity decides (`analysis::string_identity`):
    /// whether they are the same `java::JStr`.
    pub(super) fn string_same(&mut self, args: &'a [Expr]) -> Code {
        let (a, b) = (&args[0], &args[1]);
        let nullable = self.nullable(a) || self.nullable(b);
        let side = |g: &mut Self, e: &'a Expr| {
            let code = g.expr(e).at(ATOM);
            match (nullable, g.nullable(e)) {
                (true, true) => format!("{code}.as_ref()"),
                (true, false) => format!("Some(&{code})"),
                _ => format!("&{code}"),
            }
        };
        let (a, b) = (side(self, a), side(self, b));
        let f = if nullable { "same_nullable_str" } else { "same_str" };
        Code::atom(format!("{}({a}, {b})", self.runtime(f)))
    }

    pub(super) fn objects_equals(&mut self, args: &'a [Expr]) -> Code {
        let (a, b) = (&args[0], &args[1]);
        if let [_, _, call] = args {
            if !self.nullable(a) {
                return self.expr(call);
            }
            let (a_null, b_null) = (self.is_none(a), self.is_none(b));
            let call = self.expr(call);
            return Code::new(format!("if {a_null} {{ {b_null} }} else {{ {} }}", call.text), LOWEST);
        }
        let ty = if a.ty == Type::Null { b.ty } else { a.ty };
        match ty {
            Type::Boxed(kind) if self.nullable(a) || self.nullable(b) => {
                let (a, b) = (self.optional_to(a, ty), self.optional_to(b, ty));
                let (x, y) = (
                    self.unbox_code(Code::atom("a_"), *kind, false),
                    self.unbox_code(Code::atom("b_"), *kind, false),
                );
                let equal = self.equal(*kind, x, y);
                Code::new(
                    format!(
                        "match ({}, {}) {{ (Some(a_), Some(b_)) => {}, (a_, b_) => a_.is_none() && b_.is_none() }}",
                        a.text, b.text, equal.text
                    ),
                    LOWEST,
                )
            }
            Type::Boxed(kind) => {
                let (a, b) = (self.value(a, *kind), self.value(b, *kind));
                self.equal(*kind, a, b)
            }
            _ if self.nullable(a) || self.nullable(b) => {
                let (a, b) = (self.option_arg(a), self.option_arg(b));
                infix("==", CMP, &Code::atom(a), &Code::atom(b))
            }
            _ => {
                let (a, b) = (self.str_value(a), self.str_value(b));
                infix("==", CMP, &Code::atom(a), &Code::atom(b))
            }
        }
    }

    /// `Objects.hashCode(o)`.
    pub(super) fn objects_hash_code(&mut self, args: &'a [Expr]) -> Code {
        let o = &args[0];
        let nullable = self.nullable(o);
        if let [_, call] = args {
            let call = self.expr(call);
            if !nullable {
                return call;
            }
            let null = self.is_none(o);
            return Code::new(format!("if {null} {{ 0 }} else {{ {} }}", call.text), LOWEST);
        }
        match o.ty {
            Type::String if nullable => {
                let option = self.string_option_kept(o);
                Code::atom(format!("{option}.map_or(0, {})", self.runtime("string_hash")))
            }
            Type::String => {
                let s = self.str_ref(o);
                Code::atom(format!("{}({s})", self.runtime("string_hash")))
            }
            Type::Enum(_) if nullable => {
                let option = self.expr(o);
                Code::atom(format!("{}.map_or(0, |e_| e_ as i32)", option.at(ATOM)))
            }
            Type::Enum(_) => {
                let code = self.expr(o);
                Code::new(format!("{} as i32", code.cast_operand()), AS)
            }
            Type::Boxed(kind) if nullable => {
                let option = self.expr(o);
                let value = self.unbox_code(Code::atom("v_"), *kind, false);
                let hash = self.primitive_hash(*kind, value);
                Code::new(format!("match {} {{ Some(v_) => {}, None => 0 }}", option.text, hash.text), LOWEST)
            }
            Type::Boxed(kind) => {
                let value = self.value(o, *kind);
                self.primitive_hash(*kind, value)
            }
            _ => unreachable!("`Objects.hashCode` of a {:?}", o.ty),
        }
    }

    /// `x == null` for a pure value that may be null.
    fn is_none(&mut self, e: &'a Expr) -> String {
        match &e.kind {
            ExprKind::Null => "true".to_string(),
            _ if !self.nullable(e) => "false".to_string(),
            ExprKind::Var(_) | ExprKind::Field { .. } | ExprKind::Index(..) => {
                format!("{}.is_none()", self.object_place(e))
            }
            _ => format!("{}.is_none()", self.expr(e).at(ATOM)),
        }
    }

    /// The hash code of the box of the primitive `code` of `kind`.
    fn primitive_hash(&mut self, kind: Type, code: Code) -> Code {
        let function = match kind {
            Type::Int => return code,
            Type::Long => "long_hash",
            Type::Double => "double_hash",
            Type::Float => "float_hash",
            Type::Boolean => "bool_hash",
            _ => return Code::new(format!("{} as i32", code.cast_operand()), AS),
        };
        Code::atom(format!("{}({})", self.runtime(function), code.text))
    }
}
