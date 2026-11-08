//! Expressions: assignments, arithmetic (plain where the analyses allow, Java's otherwise),
//! conversions, shifts and the built-in methods.

use super::*;

pub(super) struct Operand {
    pub(super) code: Code,
    pub(super) ty: Type,
}

pub(super) fn in_shift_range(e: &Expr, ty: Type) -> bool {
    let bits = if ty == Type::Long { 64 } else { 32 };
    match e.kind {
        ExprKind::Int(v) => (0..bits).contains(&(v as i64)),
        ExprKind::Long(v) => (0..bits).contains(&v),
        _ => false,
    }
}

/// Whether `e` assigns or steps a variable, field or array element. Rust evaluates the right side
/// of a primitive `x op= e` before reading `x`, where Java reads `x` first, so such an `e` cannot
/// go in a Rust compound assignment.
pub(super) fn writes_inside(e: &Expr) -> bool {
    let mut found = false;
    visit_expr(e, &mut |x| found |= matches!(x.kind, ExprKind::Assign { .. } | ExprKind::IncDec { .. }));
    found
}

impl<'a> Gen<'a> {
    /// `target = value` or `target op= value` as a Rust statement, without the semicolon.
    /// The Rust compound assignment operator (`+=`) that does what Java's `x op= value` does on a
    /// variable of type `ty`, if there is one: no promotion to a wider type, and no overflow,
    /// division by zero or shift out of range to handle.
    pub(super) fn in_place_op(&self, op: BinOp, ty: Type, value: &Expr, id: ExprId) -> Option<&'static str> {
        if writes_inside(value) {
            return None;
        }
        let symbol = match op {
            BinOp::Add => "+=",
            BinOp::Sub => "-=",
            BinOp::Mul => "*=",
            BinOp::Div => "/=",
            BinOp::Rem => "%=",
            BinOp::BitAnd => "&=",
            BinOp::BitOr => "|=",
            BinOp::BitXor => "^=",
            BinOp::Shl => "<<=",
            _ => ">>=",
        };
        let result = crate::types::binary_type(op, ty, value.ty.unboxed()).expect("checked");
        let simple = match op {
            // Floating arithmetic has no traps; a float is computed as a double when the other
            // operand is one, so only an operation in the variable's own type is in place.
            _ if ty.is_floating() => {
                result == ty && !op.is_shift() && !matches!(op, BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor)
            }
            BinOp::Add | BinOp::Sub | BinOp::Mul => result == ty && self.no_overflow(id),
            BinOp::Div | BinOp::Rem => result == ty && self.safe_division(id, value),
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => {
                ty == Type::Boolean
                    || (ty.is_integral() && crate::types::promote(ty, value.ty.unboxed()) == ty)
            }
            // Shifting by a constant in range cannot overflow.
            BinOp::Shl | BinOp::Shr => ty.is_integral() && in_shift_range(value, ty),
            _ => false,
        };
        simple.then_some(symbol)
    }

    pub(super) fn assignment(
        &mut self,
        op: Option<BinOp>,
        target: &str,
        ty: Type,
        value: &'a Expr,
        id: ExprId,
    ) -> String {
        let name = self.local(target);
        let Some(op) = op else {
            if let Some(text) = self.self_update(&name, target, ty, value) {
                return text;
            }
            let lends = self.lends_itself(target, ty, value);
            let value = if self.optional_var(self.method, ty, target) {
                self.optional(value)
            } else {
                self.value(value, ty)
            };
            if lends {
                let bound = self.fresh_name(&["value", "item", "new"]);
                return format!("{{ let {bound} = {}; {name} = {bound} }}", value.text);
            }
            return format!("{name} = {}", value.text);
        };
        if ty == Type::String {
            return self.append(&name, target, value);
        }
        if matches!(ty, Type::Boxed(_)) {
            let right = self.operand(value);
            return self.boxed_update(op, target, ty, right, id);
        }
        self.compound(op, &name, ty, value, id)
    }

    /// `place op= value` on a number, where reading `place` has no effect: in place when Rust's
    /// operator does what Java's does, else Java's arithmetic and a write of the result.
    pub(super) fn compound(
        &mut self,
        op: BinOp,
        place: &str,
        ty: Type,
        value: &'a Expr,
        id: ExprId,
    ) -> String {
        if let Some(text) = self.primitive_assignment(op, place, ty, value, id) {
            return text;
        }
        if let Some(text) = literal_shift(op, place, ty, value) {
            return text;
        }
        if let Some(symbol) = self.in_place_op(op, ty, value, id) {
            let v = self.value(value, if op.is_shift() { value.ty } else { ty });
            return format!("{place} {symbol} {}", v.text);
        }
        let result = crate::types::binary_type(op, ty, value.ty.unboxed()).expect("checked");
        // Java's compound assignment casts the result back to the variable's type.
        let current = Code::atom(place.to_string());
        let right = self.operand(value);
        let code = self.binary(op, Operand { code: current, ty }, right, result, id);
        format!("{place} = {}", self.convert(code, result, ty).text)
    }

    /// Whether a division or remainder can neither throw nor overflow.
    pub(super) fn safe_division(&self, id: ExprId, divisor: &Expr) -> bool {
        let constant = match divisor.kind {
            ExprKind::Int(v) => Some(v as i64),
            ExprKind::Long(v) => Some(v),
            _ => None,
        };
        self.facts.safe_division.contains(&id) || constant.is_some_and(|v| v != 0 && v != -1)
    }

    /// `s += value` on a string.
    pub(super) fn append(&mut self, name: &str, target: &str, value: &'a Expr) -> String {
        if self.optional_var(self.method, Type::String, target) {
            return self.append_optional(name, value);
        }
        if !mentions(value, target) && !super::shared_strings() {
            if let ExprKind::Str(s) = &value.kind {
                return format!("{name}.push_str({})", rust_string(s));
            }
            if value.ty == Type::String
                && matches!(value.kind, ExprKind::Str(_) | ExprKind::Var(_))
                && !self.nullable(value)
            {
                return format!("{name}.push_wtf8({})", self.str_ref(value));
            }
            if matches!(value.ty, Type::Class(..))
                && self.enum_of(value).is_none()
                && let Some(index) = self.to_string_index(value)
            {
                return format!("{name}.push_wtf8(&{})", self.method_call(value, index, &[]).text);
            }
            let mut parts = Vec::new();
            self.parts(value, &mut parts);
            let pieces = self.pieces(parts).join(", ");
            return format!("{}({name}, {pieces})", self.runtime("append!"));
        }
        let mut parts = vec![Part::Text(Code::atom(name))];
        self.parts(value, &mut parts);
        format!("{name} = {}", self.concat(parts).text)
    }

    /// `x += 1` or `x -= 1` as a Rust statement.
    pub(super) fn step(&mut self, target: &str, ty: Type, increment: bool, id: ExprId) -> String {
        if matches!(ty, Type::Boxed(_)) {
            let op = if increment { BinOp::Add } else { BinOp::Sub };
            let one = Operand { code: Code::literal(Literal::Int(1)), ty: Type::Int };
            return self.boxed_update(op, target, ty, one, id);
        }
        let name = self.local(target);
        self.step_place(name, ty, increment, id)
    }

    /// `place++` or `place--` as a statement, where reading `place` has no effect.
    pub(super) fn step_place(&mut self, name: String, ty: Type, increment: bool, id: ExprId) -> String {
        if ty.is_floating() || self.no_overflow(id) {
            let one = if ty.is_floating() { "1.0" } else { "1" };
            format!("{name} {} {one}", if increment { "+=" } else { "-=" })
        } else {
            // A method call needs the variable's type written where it is declared.
            let receiver = self.recv(&Code::atom(name.clone()), ty);
            let method = if increment { "wrapping_add" } else { "wrapping_sub" };
            format!("{name} = {receiver}.{method}(1)")
        }
    }

    /// `e` as a value of type `ty`, applying Java's widening conversions.
    pub(super) fn value(&mut self, e: &'a Expr, ty: Type) -> Code {
        // A value given to a Rust generic's type variable is of its own type, which a literal
        // states, since nothing else may.
        if self.is_type_param(ty) && !self.is_type_param(e.ty) {
            let code = self.value(e, e.ty);
            return if code.literal.is_some() { Code::atom(code.typed()) } else { code };
        }
        if matches!(e.ty, Type::Boxed(_)) || matches!(ty, Type::Boxed(_)) {
            return self.boxing_value(e, ty);
        }
        let code = self.expr(e);
        self.convert(code, e.ty, ty)
    }

    pub(super) fn operand(&mut self, e: &'a Expr) -> Operand {
        let ty = e.ty.unboxed();
        Operand { code: self.value(e, ty), ty }
    }

    /// Converts between numeric types, folding conversions of literals.
    pub(super) fn convert(&mut self, code: Code, from: Type, to: Type) -> Code {
        if from == to || !from.is_numeric() || !to.is_numeric() {
            return code;
        }
        if involves_primitive(from, to) {
            return self.convert_primitive(code, from, to);
        }
        if let Some(literal) = code.literal {
            let (i, d) = match literal {
                Literal::Int(v) => (v as i64, v as f64),
                Literal::Long(v) => (v, v as f64),
                Literal::Double(v) => (v as i64, v),
                _ => unreachable!("convert_primitive converts {from} literals"),
            };
            return Code::literal(match to {
                Type::Int => match literal {
                    Literal::Double(v) => Literal::Int(v as i32),
                    _ => Literal::Int(i as i32),
                },
                Type::Long => Literal::Long(i),
                _ => Literal::Double(d),
            });
        }
        if let Some(integer) = self.float_to_integer(&code, from, to) {
            return integer;
        }
        Code::new(format!("{} as {}", code.cast_operand(), rust_type(to)), AS)
    }

    pub(super) fn expr(&mut self, e: &'a Expr) -> Code {
        let code = self.expr_of(e);
        // A string that the expression makes is shared where the program's strings are; a
        // caught expression is written inside its `catching`.
        let outside_catch = self.facts.catching.contains_key(&e.id) && !self.catching_now.contains(&e.id);
        if super::shared_strings() && e.ty == Type::String && !outside_catch && self.discarded != Some(e.id) {
            return self.shared_made(e, code);
        }
        // A covariant array carries its class (`lower::covariant`).
        if !outside_catch && let Some(class) = self.made_class(e) {
            return self.with_element_class(class, code);
        }
        if !outside_catch && let Some(like) = self.class.array_likes.get(&e.id) {
            return self.with_class_of(like, code);
        }
        code
    }

    fn expr_of(&mut self, e: &'a Expr) -> Code {
        if self.facts.catching.contains_key(&e.id) && self.catching_now.insert(e.id) {
            let mark = self.out.len();
            let code = self.expr(e);
            self.catching_now.remove(&e.id);
            let caught = self.caught(e.id, code, mark);
            // The `ClassNotFoundException` of a name not in the translation, and a field
            // updater's `ClassCastException`, always throw: the closure's value is `Infallible`,
            // which ends the code here as Java's throw does.
            if matches!(e.kind, ExprKind::Builtin(Builtin::UnknownClass | Builtin::UpdaterRefused, _)) {
                return self.never_of(format!("match {} {{}}", caught.text), e.ty);
            }
            return caught;
        }
        if let Some(code) = self.shared_array(e) {
            return code;
        }
        match &e.kind {
            ExprKind::Int(v) => Code::literal(Literal::Int(*v)),
            ExprKind::Long(v) => Code::literal(Literal::Long(*v)),
            ExprKind::Double(v) => Code::literal(Literal::Double(*v)),
            ExprKind::Float(v) => Code::literal(Literal::Float(*v)),
            ExprKind::Char(c) => Code::literal(Literal::Char(*c)),
            ExprKind::Bool(b) => Code::atom(b.to_string()),
            ExprKind::Str(s) if super::shared_strings() => Code::atom(self.string_literal(&rust_string(s))),
            ExprKind::Str(s) if s.is_empty() => Code::atom("java::JString::new()"),
            ExprKind::Units(units) => {
                let text = self.units_literal(units);
                Code::atom(self.string_literal(&text))
            }
            ExprKind::Str(s) => Code::atom(format!("java::JString::from({})", rust_string(s))),
            ExprKind::Var(name) => {
                let rust = self.local(name);
                match e.ty {
                    Type::String if self.borrowed.contains(name) && super::shared_strings() => {
                        Code::atom(format!("{rust}.clone()"))
                    }
                    Type::String if self.borrowed.contains(name) => Code::atom(format!("{rust}.to_owned()")),
                    // An object or array narrowed by a null check is a reference to the `Rc`.
                    Type::Class(..) | Type::StringBuilder | Type::Array(_)
                        if self.borrowed.contains(name) =>
                    {
                        Code::atom(format!("{rust}.clone()"))
                    }
                    // So is a possibly-null object parameter that the method borrows.
                    Type::Class(..) if self.borrowed_optional_objects.contains(name) => {
                        Code::atom(format!("{rust}.clone()"))
                    }
                    ty if self.clones(ty) && !self.facts.moves.contains(&e.id) => {
                        Code::atom(format!("{rust}.clone()"))
                    }
                    _ => Code::atom(rust),
                }
            }
            ExprKind::This if matches!(e.ty, Type::Enum(_)) => Code::atom(self.this_name()),
            ExprKind::This => Code::atom(format!("{}.clone()", self.this_name())),
            ExprKind::EnumConstant { class, ordinal } => {
                let path = self.constant_path(*class, *ordinal);
                // Java initializes an enum on the first use of one of its constants.
                match self.init_call(*class) {
                    Some(init) => Code::atom(format!("{{ {init} {path} }}")),
                    None => Code::atom(path),
                }
            }
            ExprKind::Field { .. } if let Some(local) = self.loop_fields.get(&e.id) => {
                Code::atom(local.clone())
            }
            ExprKind::Field { object, name } => self.field_read(object, name, e.ty),
            // A read during its class's initialization, which may see the static still empty.
            ExprKind::Static { class, name } if self.facts.early_static_reads.contains(&e.id) => {
                match self.placeholder_read(*class, name, e.ty) {
                    Some(read) => read,
                    None => self.static_read(*class, name, e.ty),
                }
            }
            ExprKind::Static { class, name } => self.static_read(*class, name, e.ty),
            ExprKind::New { args, ctor, .. } => {
                let ctor = ctor.expect("constructors are resolved by the checker");
                self.new_object(e.id, ctor, args, e.ty)
            }
            ExprKind::Unary(op, operand) => {
                let code = self.value(operand, e.ty);
                if let Some(literal) = code.literal.and_then(|lit| fold_unary(*op, lit)) {
                    return Code::literal(literal);
                }
                let prefix = match (op, e.ty) {
                    (UnOp::Plus, _) => return code,
                    (UnOp::Neg, Type::Double | Type::Float) => "-",
                    (UnOp::Neg, _) if self.no_overflow(e.id) => "-",
                    (UnOp::Neg, _) => {
                        return Code::atom(format!("{}.wrapping_neg()", self.recv(&code, e.ty)));
                    }
                    (UnOp::Not | UnOp::BitNot, _) => "!",
                };
                let mut result = Code::new(format!("{prefix}{}", code.at(UNARY)), UNARY);
                // `-(if c { 1 } else { 2 })` leaves its type to inference too.
                if code.untyped {
                    result.untyped = true;
                    result.typed_text = Some(format!("{prefix}{}", code.typed_at(UNARY)));
                }
                result
            }
            ExprKind::Null => Code::atom("None"),
            ExprKind::Binary(BinOp::Add, ..) if e.ty == Type::String => {
                let mut parts = Vec::new();
                self.parts(e, &mut parts);
                self.concat(parts)
            }
            ExprKind::Binary(op, l, r) if is_null_test(*op, l, r) => self.null_test(*op, l, r),
            ExprKind::Binary(BinOp::And, ..) if let Some(code) = self.bound_and(e) => code,
            ExprKind::Binary(op @ (BinOp::Eq | BinOp::Ne), l, r) if matches!(l.ty, Type::Class(..)) => {
                self.identity(e.id, *op, l, r)
            }
            ExprKind::Binary(op @ (BinOp::Eq | BinOp::Ne), l, r)
                if l.ty == Type::JavaClass || r.ty == Type::JavaClass =>
            {
                self.class_identity(*op, l, r)
            }
            ExprKind::Binary(op @ (BinOp::Eq | BinOp::Ne), l, r)
                if l.ty == Type::Charset || r.ty == Type::Charset =>
            {
                self.charset_identity(*op, l, r)
            }
            ExprKind::Binary(op @ (BinOp::Eq | BinOp::Ne), l, r) if l.ty == Type::StringBuilder => {
                self.builder_identity(*op, l, r)
            }
            ExprKind::Binary(op @ (BinOp::Eq | BinOp::Ne), l, r) if matches!(l.ty, Type::Array(_)) => {
                self.array_identity(*op, l, r)
            }
            ExprKind::Binary(op @ (BinOp::Eq | BinOp::Ne), l, r)
                if self.faithful_box(l.ty) && r.ty == l.ty =>
            {
                self.boxed_identity(*op, l, r)
            }
            // Enums of which one may be null (`analysis::nullness`): `Option`s, equal when both
            // are null or both are the same constant. So are two boxes of a kind whose equal
            // values are one object (`Boolean` and `Byte`, see `analysis::boxing`).
            ExprKind::Binary(op @ (BinOp::Eq | BinOp::Ne), l, r) if self.optional_equality(l, r) => {
                self.optional_equality_code(*op, l, r)
            }
            ExprKind::Binary(op @ (BinOp::Eq | BinOp::Ne), l, r) if bool_test(l, r).is_some() => {
                self.bool_comparison(*op, l, r)
            }
            ExprKind::Binary(op, l, r) => {
                let (l, r) = (self.operand(l), self.operand(r));
                self.binary(*op, l, r, e.ty, e.id)
            }
            // A variable that may be null takes a value that is not: the assignment's value is
            // that value, not the variable's `Option`.
            ExprKind::Assign { op: None, target: Place::Var(target), value }
                if self.optional_var(self.method, e.ty, target) && !self.nullable(value) =>
            {
                let bound = self.fresh_name(&["value", "item", "new"]);
                let v = self.value(value, e.ty).text;
                let name = self.local(target);
                Code::new(format!("{{ let {bound} = {v}; {name} = Some({bound}.clone()); {bound} }}"), LOWEST)
            }
            ExprKind::Assign { op, target: Place::Var(target), value } => {
                let text = self.assignment(*op, target, e.ty, value, e.id);
                Code::new(format!("{{ {text}; {} }}", self.read(target, e.ty)), LOWEST)
            }
            ExprKind::IncDec { target: Place::Var(target), increment, prefix } => {
                let text = self.step(target, e.ty, *increment, e.id);
                let name = self.local(target);
                if *prefix {
                    Code::new(format!("{{ {text}; {name} }}"), LOWEST)
                } else {
                    let old = if name == "old" { "previous" } else { "old" };
                    Code::new(format!("{{ let {old} = {name}; {text}; {old} }}"), LOWEST)
                }
            }
            ExprKind::Switch { selector, cases } => {
                let optional = self.nullable(e);
                self.switch_expr(e.ty, optional, selector, cases)
            }
            ExprKind::Cast(ty, value) if ty.is_small() => self.small_cast(*ty, value, e.id),
            ExprKind::Assign { op: None, target: Place::Index(array, index), value } => {
                Code::atom(self.index_set(e.id, array, index, arrays::element(array.ty), value, true))
            }
            ExprKind::Assign { op: Some(op), target: Place::Index(array, index), value } => {
                Code::atom(self.index_update(*op, array, index, e.ty, value, e.id, true))
            }
            ExprKind::IncDec { target: Place::Index(array, index), increment, prefix } => {
                Code::atom(self.index_step(array, index, e.ty, *increment, *prefix, e.id, true))
            }
            ExprKind::Assign { op, target: target @ (Place::Field { .. } | Place::Static { .. }), value } => {
                let how = match op {
                    None => Update::Assign(value),
                    Some(op) => Update::Compound(*op, value),
                };
                // The write of `Unsafe.putReference` and its kind is a void call (`lower::handles`):
                // its value has the field's type.
                let ty = if e.ty == Type::Void { value.ty } else { e.ty };
                self.update(target, ty, how, e.id, false)
            }
            ExprKind::IncDec {
                target: target @ (Place::Field { .. } | Place::Static { .. }),
                increment,
                prefix,
            } => self.update(target, e.ty, Update::Step(*increment), e.id, *prefix),
            ExprKind::Index(array, index) => self.index(e.id, array, index),
            ExprKind::Length(array) => self.length(array),
            ExprKind::NewArray(sizes) => {
                let code = self.new_array(e.ty, sizes);
                self.frozen_new(e, code)
            }
            // `new int[n][m]` where `int[][]` is an `Object[]` (see `lower::arrays::sized_array`).
            ExprKind::Cast(to @ Type::Array(_), value)
                if let ExprKind::NewArray(sizes) = &value.kind
                    && value.ty != *to =>
            {
                // The `new` is written here rather than by `expr`, which would run it inside
                // `java::catching` where a `catch` sees its exception.
                if self.facts.catching.contains_key(&value.id) && self.catching_now.insert(value.id) {
                    let mark = self.out.len();
                    let code = self.new_object_rows(*to, value, sizes);
                    self.catching_now.remove(&value.id);
                    return self.caught(value.id, code, mark);
                }
                self.new_object_rows(*to, value, sizes)
            }
            ExprKind::ArrayLit(items) => {
                let code = self.array_lit(e.ty, items);
                self.frozen_new(e, code)
            }
            // An upcast or a downcast between the program's types, or a string, a box or a
            // primitive becoming an `Object`.
            ExprKind::Cast(to @ Type::Class(..), value)
                if let Some(t) = self.closure_type(*to)
                    && let ExprKind::New { args, ctor: Some(ctor), .. } = &value.kind =>
            {
                self.closure(t, *ctor, args)
            }
            // A closure is one of the closure type its interface extends too.
            ExprKind::Cast(to @ Type::Class(..), value)
                if self.closure_type(*to).is_some() && self.closure_type(value.ty).is_some() =>
            {
                self.expr(value)
            }
            ExprKind::Cast(to @ Type::Class(..), value) => self.checked_class_cast(e.id, *to, value),
            // A cast to an enum of a value that may be null: an `Option` of the enum, `None` for
            // null (`analysis::maybe_null`).
            ExprKind::Cast(to @ Type::Enum(_), value)
                if matches!(value.ty, Type::Class(..)) && self.nullable(value) =>
            {
                self.checked_class_cast(e.id, *to, value)
            }
            // A downcast from an interface to an enum that implements it, or from `Object` to a
            // string, a builder, a box, an array, a class or a charset.
            ExprKind::Cast(
                to @ (Type::Enum(_)
                | Type::String
                | Type::StringBuilder
                | Type::Boxed(_)
                | Type::Array(_)
                | Type::JavaClass
                | Type::Charset),
                value,
            ) if matches!(value.ty, Type::Class(..)) => self.checked_class_cast(e.id, *to, value),
            ExprKind::InstanceOf { value, ty, .. } => self.instance_test(value, *ty),
            // A cast of a box to its own type is the box, null where it is: what takes it unboxes
            // it, or not.
            ExprKind::Cast(ty @ Type::Boxed(_), value) if value.ty == *ty => self.expr(value),
            ExprKind::Cast(ty, value) => self.value(value, *ty),
            ExprKind::Conditional(cond, a, b) => {
                if let Some(code) = self.enum_conditional(e, cond, a, b) {
                    return code;
                }
                if let Some(code) = self.narrowed_conditional(e, cond, a, b) {
                    return code;
                }
                // The condition of an `if` drops its temporaries before a branch runs. With threads,
                // two locks held at once in the condition itself could wait for each other.
                let guarded = self.facts.threaded && self.conflicts(&[&**cond]);
                let outer = std::mem::replace(&mut self.guarded, guarded);
                let cond = self.condition(cond);
                self.guarded = outer;
                let chain = std::mem::replace(&mut self.chain, false);
                let a = if self.nullable(e) { self.optional(a) } else { self.value(a, e.ty) };
                self.chain = matches!(b.kind, ExprKind::Conditional(..));
                let lines = chain || self.chain;
                let b = if self.nullable(e) { self.optional(b) } else { self.value(b, e.ty) };
                self.chain = chain;
                let text = |a: String, b: String| self.if_else(&cond, &a, &b, lines);
                if a.untyped && b.untyped {
                    // As for literals, inference gives the branches `i32` or `f64` unless the
                    // type is written.
                    let typed = text(a.typed(), b.typed());
                    if !matches!(e.ty, Type::Int | Type::Double) {
                        return Code::new(typed, LOWEST);
                    }
                    let mut code = Code::new(text(a.text, b.text), LOWEST);
                    code.untyped = true;
                    code.typed_text = Some(typed);
                    return code;
                }
                Code::new(text(a.text, b.text), LOWEST)
            }
            ExprKind::Call { args, resolved, .. } => {
                let index = resolved.expect("calls are resolved by the checker");
                let outer_plain = self.plain_call.take();
                let code = match self.call_on_stack(e) {
                    Some(code) => code,
                    None => self.static_call(index, args),
                };
                self.plain_call = outer_plain;
                self.raising(code, index)
            }
            // A getter's call on an object on the stack, whose field a local took from its slot.
            ExprKind::MethodCall { .. } if let Some(local) = self.loop_fields.get(&e.id) => {
                Code::atom(local.clone())
            }
            ExprKind::MethodCall { receiver, args, resolved, is_super, .. } => {
                let index = resolved.expect("calls are resolved by the checker");
                // The calls in the receiver and the arguments are written with their own names.
                let outer_plain = self.plain_call.take();
                let code = self.method_call_expr(e, receiver, args, index, *is_super);
                self.plain_call = outer_plain;
                code
            }
            ExprKind::ThisCall { .. } | ExprKind::SuperInit { .. } => {
                unreachable!("lowering keeps `this(...)` and `super(...)` first in a constructor")
            }
            ExprKind::Builtin(builtin, args) => self.builtin(*builtin, args, e.ty, e.id),
        }
    }

    /// A method call, `receiver.m(args)` of method `index`, as `expr` writes it.
    fn method_call_expr(
        &mut self,
        e: &'a Expr,
        receiver: &'a Expr,
        args: &'a [Expr],
        index: usize,
        is_super: bool,
    ) -> Code {
        let code = if let Some(code) = self.call_on_stack(e) {
            code
        } else if self.closure_type(receiver.ty).is_some() {
            self.closure_call(receiver, index, args)
        } else if self.inlined(receiver, index) {
            self.inline_call(index, args)
        } else if self.class.methods[index].is_static {
            self.static_call(index, args)
        } else if !is_super && let Some(t) = self.enum_of(receiver) {
            self.enum_call(t, e.pos, receiver, index, args)
        } else {
            let target = self.dispatch(receiver, index, is_super);
            if self.is_object_method(target) {
                return self.identity_call(receiver, target, args);
            }
            if self.runs_nowhere(receiver, target) {
                return self.no_object_runs(receiver, target, index);
            }
            // A plain body (`Facts::plain_bodies`) called directly gives its value as it
            // is: there is no `Result` to test.
            if self.written_plain(target) {
                self.plain_call = Some(target);
                let code = self.method_call(receiver, target, args);
                self.plain_call = None;
                return code;
            }
            self.method_call(receiver, target, args)
        };
        self.raising(code, index)
    }

    /// A call of method `target` (`index` as called) that no object runs (`Home::Nowhere`): the
    /// receiver's class has no objects, as a census leaves a class whose constructors cannot run
    /// (JNA's `W32WindowUtils`), so the receiver is null, which Java's call throws for. Its code
    /// was never emitted.
    /// Whether method `target` never runs on `receiver`: no object runs it, or it takes the handle
    /// of another class's objects than the receiver's. A method is emitted on one class when only
    /// that class's objects run it (`hierarchy::home`), so the program creates no object of the
    /// receiver's class.
    pub(super) fn runs_nowhere(&self, receiver: &Expr, target: usize) -> bool {
        match self.homes[target] {
            Home::Nowhere => true,
            Home::Class(c) => {
                !self.facts.self_methods.contains(&target)
                    && self.struct_class(receiver).is_some_and(|s| s != c)
            }
            _ => false,
        }
    }

    pub(super) fn no_object_runs(&mut self, receiver: &'a Expr, target: usize, index: usize) -> Code {
        let m = &self.class.methods[target];
        let message = format!("no object runs {}.{}", self.class.types[m.class].name, m.name);
        let checked = self.receiver_ref(receiver);
        let plain = self.plain_ret_text(target);
        let ret = plain.strip_prefix(" -> ").unwrap_or("()").to_string();
        let ret = if self.facts.throwing.contains(&index) {
            format!("Result<{ret}, {}>", self.throwable_type())
        } else {
            ret
        };
        let never = format!("{{ let _ = {checked}; let value: {ret} = unreachable!({message:?}); value }}");
        self.raising(Code::atom(never), index)
    }

    /// Reads a variable after assigning it, for an assignment used as a value.
    pub(super) fn read(&self, target: &str, ty: Type) -> String {
        let name = self.local(target);
        // The variable keeps its value: a string is copied, and an array's or an object's handle
        // is cloned (`(k = first.key) == key || key.equals(k)` reads `k` again).
        if self.clones(ty) { format!("{name}.clone()") } else { name }
    }

    pub(super) fn binary(&mut self, op: BinOp, l: Operand, r: Operand, result: Type, id: ExprId) -> Code {
        // Comparisons promote their operands; other operators work in the result type, except
        // that a shift distance keeps its own type.
        let operand_type = if op.is_comparison() && l.ty.is_numeric() {
            comparison_type(&l, &r)
        } else if op.is_comparison() {
            l.ty
        } else {
            result
        };
        let left = self.convert(l.code, l.ty, operand_type);
        let right = if op.is_shift() { r.code } else { self.convert(r.code, r.ty, operand_type) };
        // Integer arithmetic that may overflow wraps, as in Java.
        let integral = operand_type.is_integral();
        let wrapping = integral && !self.no_overflow(id);
        let mut method = |name: &str, left: &Code, right: &Code| {
            Code::atom(format!("{}.{name}({})", self.recv(left, operand_type), right.text))
        };
        match op {
            BinOp::Add if wrapping => method("wrapping_add", &left, &right),
            BinOp::Sub if wrapping => method("wrapping_sub", &left, &right),
            BinOp::Mul if wrapping => method("wrapping_mul", &left, &right),
            BinOp::Div | BinOp::Rem if integral => {
                // Dividing by a constant other than 0 and -1 can neither throw nor overflow.
                let constant = match right.literal {
                    Some(Literal::Int(v)) => v != 0 && v != -1,
                    Some(Literal::Long(v)) => v != 0 && v != -1,
                    _ => false,
                };
                if constant || self.facts.safe_division.contains(&id) {
                    infix(if op == BinOp::Div { "/" } else { "%" }, MUL, &left, &right)
                } else {
                    let f = if op == BinOp::Div { "div" } else { "rem" };
                    self.may_fail(id, f, "", &format!("{}, {}", left.typed(), right.text))
                }
            }
            BinOp::Shl | BinOp::Shr | BinOp::UShr => self.shift(op, left, right, result),
            BinOp::Add => infix("+", ADD, &left, &right),
            BinOp::Sub => infix("-", ADD, &left, &right),
            BinOp::Mul => infix("*", MUL, &left, &right),
            BinOp::Div => infix("/", MUL, &left, &right),
            BinOp::Rem => infix("%", MUL, &left, &right),
            BinOp::BitAnd => infix("&", BIT_AND, &left, &right),
            BinOp::BitOr => infix("|", BIT_OR, &left, &right),
            BinOp::BitXor => infix("^", BIT_XOR, &left, &right),
            BinOp::And => infix("&&", AND, &left, &right),
            BinOp::Or => infix("||", OR, &left, &right),
            _ => infix(op.java(), CMP, &left, &right),
        }
    }

    /// Java masks the shift distance to the width of the shifted type; so does Rust's
    /// `wrapping_shl`, and a constant distance is masked here.
    pub(super) fn shift(&mut self, op: BinOp, left: Code, right: Code, ty: Type) -> Code {
        let bits = if ty == Type::Long { 64 } else { 32 };
        let unsigned = if ty == Type::Long { "u64" } else { "u32" };
        let constant = match right.literal {
            Some(Literal::Int(v)) => Some(v as i64 & (bits - 1)),
            Some(Literal::Long(v)) => Some(v & (bits - 1)),
            _ => None,
        };
        match (op, constant) {
            (BinOp::UShr, Some(0)) | (BinOp::Shl | BinOp::Shr, Some(0)) => left,
            (BinOp::Shl, Some(n)) => infix("<<", SHIFT, &left, &Code::literal(Literal::Int(n as i32))),
            (BinOp::Shr, Some(n)) => infix(">>", SHIFT, &left, &Code::literal(Literal::Int(n as i32))),
            (BinOp::UShr, Some(n)) => {
                let shifted = format!("({} as {unsigned}) >> {n}", left.cast_operand());
                Code::new(format!("({shifted}) as {}", rust_type(ty)), AS)
            }
            _ => {
                let distance = format!("{} as u32", right.cast_operand());
                match op {
                    BinOp::Shl => Code::atom(format!("{}.wrapping_shl({distance})", self.recv(&left, ty))),
                    BinOp::Shr => Code::atom(format!("{}.wrapping_shr({distance})", self.recv(&left, ty))),
                    _ => Code::new(
                        format!(
                            "({} as {unsigned}).wrapping_shr({distance}) as {}",
                            left.cast_operand(),
                            rust_type(ty)
                        ),
                        AS,
                    ),
                }
            }
        }
    }

    /// `if head { a } else { b }`; with `lines`, a link of a chain `if .. else if .. else ..`, on
    /// lines of their own.
    pub(super) fn if_else(&self, head: &str, a: &str, b: &str, lines: bool) -> String {
        if !lines {
            return format!("if {head} {{ {a} }} else {{ {b} }}");
        }
        let pad = "    ".repeat(self.indent);
        let otherwise = if b.starts_with("if ") {
            format!("else {b}")
        } else {
            format!(
                "else {{
{pad}    {b}
{pad}}}"
            )
        };
        format!(
            "if {head} {{
{pad}    {a}
{pad}}} {otherwise}"
        )
    }

    /// Code that never gives a value (a throw), whose Rust type `!` has no methods and implements
    /// no trait, as Java's value of type `ty`: a block that declares it so, which the code can call
    /// a method on or cast. A statement's (`void`) stays as it is.
    pub(super) fn never_of(&mut self, text: String, ty: Type) -> Code {
        if ty == Type::Void {
            return Code::never(text);
        }
        let ty_text = self.ty_name(ty);
        Code::atom(format!("{{ let value: {ty_text} = {text}; value }}"))
    }

    pub(super) fn builtin(&mut self, builtin: Builtin, args: &'a [Expr], ty: Type, id: ExprId) -> Code {
        match builtin {
            Builtin::ArraysToString | Builtin::ArraysFill | Builtin::ArraysSort | Builtin::ArraysCopyOf => {
                self.arrays_builtin(builtin, args, id)
            }
            Builtin::Arrays(op) => self.arrays_op(op, args, id),
            Builtin::ElementUpdate(update) => self.element_update_call(update, args),
            Builtin::ByteView { ty, big_endian } => self.byte_view_call(ty, big_endian, args),
            // `printf`: the runtime prints what Java's `Formatter` writes, piece by piece.
            Builtin::Print if matches!(args[0].kind, ExprKind::Builtin(Builtin::Format, _)) => {
                self.printing_format = true;
                // It prints, and makes no string.
                let discarded = self.discarded.replace(args[0].id);
                let code = self.expr(&args[0]);
                self.discarded = discarded;
                self.printing_format = false;
                code
            }
            Builtin::Println | Builtin::Print => {
                let mut parts = Vec::new();
                let outer = std::mem::replace(&mut self.printing, true);
                if let Some(arg) = args.first() {
                    self.parts(arg, &mut parts);
                }
                self.printing = outer;
                let mac = self.runtime(if builtin == Builtin::Println { "println!" } else { "print!" });
                self.format(&mac, parts)
            }
            Builtin::ObjectText => self.object_to_string(&args[0]),
            Builtin::ObjectClone => self.object_clone(&args[0], id),
            Builtin::EnumOrdinal | Builtin::EnumName | Builtin::EnumCompareTo | Builtin::EnumValueOf => {
                self.enum_builtin(builtin, args, ty, id)
            }
            Builtin::RawMemory { ty, put } => self.raw_memory(ty, put, args),
            Builtin::NotEnumClass => {
                let name = self.str_ref(&args[0]);
                let message = format!("format!(\"{{}} is not an enum class\", {name})");
                if self.facts.raising.contains(&id) {
                    let exception = self.runtime("Exception");
                    let result = self.ty_name(ty);
                    let failure = format!(
                        "Err::<{result}, {exception}>({exception}::new(\"java.lang.IllegalArgumentException\", {message}))"
                    );
                    Code::atom(self.passed_on(Code::atom(failure)))
                } else {
                    let throw = self.runtime("throw");
                    Code::atom(format!("{throw}(\"java.lang.IllegalArgumentException\", &{message})"))
                }
            }
            Builtin::CheckFromToIndex | Builtin::CheckIndex => {
                let values: Vec<String> = args.iter().map(|a| self.value(a, Type::Int).text).collect();
                let name = if builtin == Builtin::CheckIndex { "check_index" } else { "check_from_to_index" };
                self.may_fail(id, name, "", &values.join(", "))
            }
            Builtin::Boxed(op, kind) => self.boxed_builtin(op, kind, args, ty, id),
            Builtin::Unsigned(op, kind) => self.unsigned_builtin(op, kind, args, ty, id),
            Builtin::CodePoint(op) => self.code_point_builtin(op, args, id),
            Builtin::Charset(op) => self.charset_builtin(op, args),
            Builtin::CharNumericValue => self.char_numeric_value(args),
            Builtin::CharDigit | Builtin::CharForDigit => self.character_radix(builtin, args),
            Builtin::Exit => {
                let code = self.value(&args[0], Type::Int);
                Code::atom(format!("{}({})", self.runtime("exit"), code.text))
            }
            Builtin::MainArgs => self.main_args(ty),
            Builtin::StringArrayOf | Builtin::ObjectArrayOf => self.array_conversion(builtin, &args[0], ty),
            // A census's stub of rejected code, with the rejection's message (`Builtin::NoObject`).
            Builtin::NoObject { .. } if let [Expr { kind: ExprKind::Str(message), .. }] = args => {
                Code::atom(format!("{}({})", self.runtime("not_translated"), code::rust_string(message)))
            }
            Builtin::NoObject { class } => Code::atom(no_object(&self.class.types[class].name)),
            Builtin::InitializeClass { class } => match self.init_call(class) {
                Some(init) => Code::atom(format!("{{ {init} }}")),
                None => Code::atom("()".to_string()),
            },
            Builtin::UnknownClass => {
                let name = self.value(&args[0], Type::String);
                let message = "{}, which is not in the translation";
                let class = "java.lang.ClassNotFoundException";
                let throw =
                    format!("{}({class:?}, &format!({message:?}, {}))", self.runtime("throw"), name.text);
                if self.catching_now.contains(&id) {
                    return Code::atom(format!("{{ let never: std::convert::Infallible = {throw}; never }}"));
                }
                self.never_of(throw, ty)
            }
            Builtin::UpdaterRefused => {
                let throw = format!("{}()", self.runtime("updater_refused"));
                if self.catching_now.contains(&id) {
                    return Code::atom(format!("{{ let never: std::convert::Infallible = {throw}; never }}"));
                }
                self.never_of(throw, ty)
            }
            Builtin::MissingResource => {
                let (class, name) =
                    (self.value(&args[0], Type::String).text, self.value(&args[1], Type::String).text);
                let message =
                    "java.lang.InternalError: the resource {} of {} is not part of the translated program";
                let format = format!("format!({message:?}, {name}, {class})");
                // Typed, as a call on the protection domain needs (`getCodeSource()`).
                let throw = format!("{}({format})", self.runtime("throw_uncaught"));
                self.never_of(throw, ty)
            }
            Builtin::MonitorEnter => self.null_check(&args[0]),
            Builtin::MemoryAccessWarning(method) => {
                let check = self.null_check(&args[0]).text;
                let caller = self.value(&args[1], Type::JavaClass).text;
                let warn = self.runtime("memory_access_warning");
                Code::atom(format!("{{ {check}; {warn}({caller}, {method:?}); }}"))
            }
            Builtin::ClassName { simple } => self.class_name(&args[0], simple),
            Builtin::SameClass { same } => self.same_class(same, args),
            Builtin::GetClass => self.get_class(&args[0]),
            Builtin::ClassLiteral(ty) => Code::borrow_or_atom(self.type_class(ty)),
            Builtin::ClassMethod(op) => self.class_method(op, args),
            Builtin::ClassIsInstance | Builtin::ClassCheck => {
                self.class_instance(builtin == Builtin::ClassCheck, args)
            }
            Builtin::ComparableClassFor => self.comparable_class_for(&args[0]),
            Builtin::ClassLiteralName { class, simple } => Code::atom(format!(
                "java::JString::from({})",
                rust_string(&self.java_class_name(class, simple))
            )),
            Builtin::MathAbs => {
                let x = self.value(&args[0], ty);
                // Math.abs(Integer.MIN_VALUE) is Integer.MIN_VALUE.
                let method = if ty.is_floating() || self.no_overflow(id) { "abs" } else { "wrapping_abs" };
                Code::atom(format!("{}.{method}()", self.recv(&x, ty)))
            }
            Builtin::MathMax | Builtin::MathMin => {
                let a = self.value(&args[0], ty);
                let b = self.value(&args[1], ty);
                let name = if builtin == Builtin::MathMax { "max" } else { "min" };
                if ty.is_floating() {
                    // f64::max ignores NaN; Java's Math.max returns it.
                    let name = if ty == Type::Float { format!("{name}_f32") } else { name.to_string() };
                    Code::atom(format!("{}({}, {})", self.runtime(&name), a.text, b.text))
                } else {
                    Code::atom(format!("{}.{name}({})", self.recv(&a, ty), b.text))
                }
            }
            Builtin::MathSqrt => {
                let x = self.value(&args[0], Type::Double);
                Code::atom(format!("{}.sqrt()", self.recv(&x, Type::Double)))
            }
            Builtin::MathPow => {
                let a = self.value(&args[0], Type::Double);
                let b = self.value(&args[1], Type::Double);
                Code::atom(format!("{}.powf({})", self.recv(&a, Type::Double), b.text))
            }
            // UTF-16 length and UTF-8 length agree on ASCII text.
            Builtin::StringLength if self.facts.ascii_length.contains(&id) => {
                let s = match &args[0].kind {
                    _ if self.nullable(&args[0]) => self.deref(&args[0]),
                    ExprKind::Str(_) | ExprKind::Var(_) => self.str_value(&args[0]),
                    ExprKind::Field { object, name } if self.field_in_place(object, name) => {
                        self.object_place(&args[0])
                    }
                    ExprKind::Static { .. }
                        if let Some(place) =
                            self.final_place(&args[0]).or_else(|| self.lent_static(&args[0])) =>
                    {
                        place
                    }
                    _ => self.expr(&args[0]).at(ATOM),
                };
                Code::new(format!("{s}.len() as i32"), AS)
            }
            Builtin::StringLength => {
                let s = if self.nullable(&args[0]) { self.deref(&args[0]) } else { self.str_ref(&args[0]) };
                let s = self.text_owner(s);
                Code::atom(format!("{}({})", self.runtime("string_length"), s))
            }
            Builtin::StringEquals => self.equals_optional(args),
            Builtin::StringCharAt
            | Builtin::CharIsDigit
            | Builtin::CharIsLetter
            | Builtin::CharIsUpperCase
            | Builtin::CharIsLowerCase
            | Builtin::CharToUpperCase
            | Builtin::CharToLowerCase
            | Builtin::CharIsWhitespace
            | Builtin::CharIsSpaceChar
            | Builtin::CharIsLetterOrDigit
            | Builtin::CharIsAlphabetic
            | Builtin::CharIsSurrogate
            | Builtin::CharIsHighSurrogate
            | Builtin::CharIsLowSurrogate => self.char_builtin(builtin, args, id),
            Builtin::Native(native) => self.native(native, args, ty, id),
            Builtin::String(op) => self.string_builtin(op, args, id),
            // A chained call gives `&Self`; as a value, the builder is another handle to it.
            Builtin::Builder(op) if op.chains() => {
                let code = self.builder_call(id, op, args);
                Code::atom(format!("{}.clone()", code.text))
            }
            Builtin::Builder(op) => self.builder_call(id, op, args),
            Builtin::Format => self.format_builtin(args),
            Builtin::ObjectsEquals => self.objects_equals(args),
            Builtin::StringSame => self.string_same(args),
            Builtin::Kept(op) => self.kept(op, args),
            Builtin::ValueOf => self.value_of(args),
            Builtin::NullChecked(_) => self.null_checked(args, ty),
            Builtin::Unmatched => self.unmatched(args),
            Builtin::ObjectsHashCode => self.objects_hash_code(args),
            Builtin::Bits(op, kind) => self.bits_builtin(op, kind, args, id),
        }
    }
}
