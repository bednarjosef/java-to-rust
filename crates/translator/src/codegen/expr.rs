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
            let value = if self.optional_var(self.method, ty, target) {
                self.optional(value)
            } else {
                self.value(value, ty)
            };
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
        if !mentions(value, target) {
            if value.ty == Type::String
                && matches!(value.kind, ExprKind::Str(_) | ExprKind::Var(_))
                && !self.nullable(value)
            {
                return format!("{name} += {}", self.str_ref(value));
            }
            if matches!(value.ty, Type::Class(_)) && self.enum_of(value).is_none() {
                let index = self.to_string_index(value);
                return format!("{name} += &{}", self.method_call(value, index, &[]).text);
            }
            let mut parts = Vec::new();
            self.parts(value, &mut parts);
            if parts.len() == 1 {
                match parts.pop().expect("one part") {
                    Part::Lit(text) => return format!("{name} += {}", rust_string(&text)),
                    Part::Arg(code) => {
                        let ty = if value.ty == Type::Double { Type::Void } else { value.ty };
                        return format!("{name} += &{}.to_string()", self.recv(&code, ty));
                    }
                }
            }
            return format!("{name} += &{}", self.format("format!", parts).text);
        }
        let mut parts = vec![Part::Arg(Code::atom(name))];
        self.parts(value, &mut parts);
        format!("{name} = {}", self.format("format!", parts).text)
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
        Code::new(format!("{} as {}", code.cast_operand(), rust_type(to)), AS)
    }

    pub(super) fn expr(&mut self, e: &'a Expr) -> Code {
        match &e.kind {
            ExprKind::Int(v) => Code::literal(Literal::Int(*v)),
            ExprKind::Long(v) => Code::literal(Literal::Long(*v)),
            ExprKind::Double(v) => Code::literal(Literal::Double(*v)),
            ExprKind::Float(v) => Code::literal(Literal::Float(*v)),
            ExprKind::Char(c) => Code::literal(Literal::Char(*c)),
            ExprKind::Bool(b) => Code::atom(b.to_string()),
            ExprKind::Str(s) if s.is_empty() => Code::atom("String::new()"),
            ExprKind::Str(s) => Code::atom(format!("String::from({})", rust_string(s))),
            ExprKind::Var(name) => {
                let rust = self.local(name);
                match e.ty {
                    Type::String if self.borrowed.contains(name) => Code::atom(format!("{rust}.to_string()")),
                    // An object narrowed by a null check is a reference to the `Rc`.
                    Type::Class(_) | Type::StringBuilder if self.borrowed.contains(name) => {
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
            ExprKind::EnumConstant { class, ordinal } => Code::atom(self.constant_path(*class, *ordinal)),
            ExprKind::Field { object, name } => self.field_read(object, name, e.ty),
            ExprKind::Static { class, name } => self.static_read(*class, name, e.ty),
            ExprKind::New { args, ctor, .. } => {
                let ctor = ctor.expect("constructors are resolved by the checker");
                self.new_object(ctor, args)
            }
            ExprKind::Unary(op, operand) => {
                let code = self.value(operand, e.ty);
                match (op, e.ty) {
                    (UnOp::Plus, _) => code,
                    (UnOp::Neg, Type::Double | Type::Float) => {
                        Code::new(format!("-{}", code.at(UNARY)), UNARY)
                    }
                    (UnOp::Neg, _) if self.no_overflow(e.id) => {
                        Code::new(format!("-{}", code.at(UNARY)), UNARY)
                    }
                    (UnOp::Neg, _) => Code::atom(format!("{}.wrapping_neg()", self.recv(&code, e.ty))),
                    (UnOp::Not | UnOp::BitNot, _) => Code::new(format!("!{}", code.at(UNARY)), UNARY),
                }
            }
            ExprKind::Null => Code::atom("None"),
            ExprKind::Binary(BinOp::Add, ..) if e.ty == Type::String => {
                let mut parts = Vec::new();
                self.parts(e, &mut parts);
                self.format("format!", parts)
            }
            ExprKind::Binary(op, l, r) if is_null_test(*op, l, r) => self.null_test(*op, l, r),
            ExprKind::Binary(op @ (BinOp::Eq | BinOp::Ne), l, r) if matches!(l.ty, Type::Class(_)) => {
                self.identity(*op, l, r)
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
            ExprKind::Binary(op, l, r) => {
                let (l, r) = (self.operand(l), self.operand(r));
                self.binary(*op, l, r, e.ty, e.id)
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
            ExprKind::Switch { selector, cases } => self.switch_expr(e.ty, selector, cases),
            ExprKind::Cast(ty, value) if ty.is_small() => self.small_cast(*ty, value, e.id),
            ExprKind::Assign { op: None, target: Place::Index(array, index), value } => {
                Code::atom(self.index_set(e.id, array, index, e.ty, value, true))
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
                self.update(target, e.ty, how, e.id, false)
            }
            ExprKind::IncDec {
                target: target @ (Place::Field { .. } | Place::Static { .. }),
                increment,
                prefix,
            } => self.update(target, e.ty, Update::Step(*increment), e.id, *prefix),
            ExprKind::Index(array, index) => self.index(e.id, array, index),
            ExprKind::Length(array) => self.length(array),
            ExprKind::NewArray(sizes) => self.new_array(e.ty, sizes),
            ExprKind::ArrayLit(items) => self.array_lit(e.ty, items),
            ExprKind::Cast(to @ Type::Class(_), value)
                if matches!(value.ty, Type::Class(_) | Type::Enum(_)) =>
            {
                self.class_cast(*to, value)
            }
            ExprKind::Cast(ty, value) => self.value(value, *ty),
            ExprKind::Conditional(cond, a, b) => {
                if let Some(code) = self.narrowed_conditional(e, cond, a, b) {
                    return code;
                }
                // The condition of an `if` drops its temporaries before a branch runs.
                let outer = std::mem::replace(&mut self.guarded, false);
                let cond = self.condition(cond);
                self.guarded = outer;
                let (a, b) = if self.nullable(e) {
                    (self.optional(a), self.optional(b))
                } else {
                    (self.value(a, e.ty), self.value(b, e.ty))
                };
                let mut code = Code::new(format!("if {cond} {{ {} }} else {{ {} }}", a.text, b.text), LOWEST);
                code.untyped = a.untyped && b.untyped;
                code
            }
            ExprKind::Call { args, resolved, .. } => {
                let index = resolved.expect("calls are resolved by the checker");
                let args = self.call_args(index, args).join(", ");
                let code = Code::atom(format!("{}({args})", self.static_fn(index)));
                self.raising(code, index)
            }
            ExprKind::MethodCall { receiver, args, resolved, is_super, .. } => {
                let index = resolved.expect("calls are resolved by the checker");
                let code = if self.class.methods[index].is_static {
                    let args = self.call_args(index, args).join(", ");
                    Code::atom(format!("{}({args})", self.static_fn(index)))
                } else if !*is_super && let Some(t) = self.enum_of(receiver) {
                    self.enum_call(t, receiver, index, args)
                } else {
                    let target = self.dispatch(receiver, index, *is_super);
                    self.method_call(receiver, target, args)
                };
                self.raising(code, index)
            }
            ExprKind::ThisCall { .. } | ExprKind::SuperInit { .. } => {
                unreachable!("lowering keeps `this(...)` and `super(...)` first in a constructor")
            }
            ExprKind::Builtin(builtin, args) => self.builtin(*builtin, args, e.ty, e.id),
        }
    }

    /// Reads a variable after assigning it, for an assignment used as a value.
    pub(super) fn read(&self, target: &str, ty: Type) -> String {
        let name = self.local(target);
        if ty == Type::String { format!("{name}.clone()") } else { name }
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
                    let f = self.runtime(if op == BinOp::Div { "div" } else { "rem" });
                    Code::atom(format!("{f}({}, {})", left.typed(), right.text))
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

    pub(super) fn builtin(&mut self, builtin: Builtin, args: &'a [Expr], ty: Type, id: ExprId) -> Code {
        match builtin {
            Builtin::ArraysToString | Builtin::ArraysFill | Builtin::ArraysSort | Builtin::ArraysCopyOf => {
                self.arrays_builtin(builtin, args)
            }
            Builtin::Arrays(op) => self.arrays_op(op, args),
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
            Builtin::EnumOrdinal | Builtin::EnumName | Builtin::EnumCompareTo | Builtin::EnumValueOf => {
                self.enum_builtin(builtin, args, ty)
            }
            Builtin::Boxed(op, kind) => self.boxed_builtin(op, kind, args, ty, id),
            Builtin::Unsigned(op, kind) => self.unsigned_builtin(op, kind, args, ty),
            Builtin::CodePoint(op) => self.code_point_builtin(op, args),
            Builtin::Charset(op) => self.charset_builtin(op, args),
            Builtin::CharNumericValue => self.char_numeric_value(args),
            Builtin::CharDigit | Builtin::CharForDigit => self.character_radix(builtin, args),
            Builtin::Exit => {
                let code = self.value(&args[0], Type::Int);
                Code::atom(format!("{}({})", self.runtime("exit"), code.text))
            }
            Builtin::NullCheck => self.null_check(&args[0]),
            Builtin::ClassName => self.class_name(&args[0]),
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
                    _ => self.expr(&args[0]).at(ATOM),
                };
                Code::new(format!("{s}.len() as i32"), AS)
            }
            Builtin::StringLength => {
                let s = if self.nullable(&args[0]) { self.deref(&args[0]) } else { self.str_ref(&args[0]) };
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
            Builtin::Native(native) => self.native(native, args, ty),
            Builtin::String(op) => self.string_builtin(op, args),
            // A chained call gives `&Self`; as a value, the builder is another handle to it.
            Builtin::Builder(op) if op.chains() => {
                let code = self.builder_call(op, args);
                Code::atom(format!("{}.clone()", code.text))
            }
            Builtin::Builder(op) => self.builder_call(op, args),
            Builtin::Format => self.format_builtin(args),
            Builtin::Bits(op, kind) => self.bits_builtin(op, kind, args),
        }
    }
}
