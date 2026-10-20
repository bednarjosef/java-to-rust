//! Boxed values: `Integer`, `Double` and the rest of the boxed classes. A boxed value is its
//! primitive in Rust, or an `Option` of it where it may be null (see `null.rs`). Where `==`
//! observes the identity of a kind (`Facts::faithful_boxes`), it is a `java::Boxed<T>` instead,
//! made by the cached `java::box_*` functions, so that `valueOf` returns the same object.

use super::*;

/// The boxed types of a method's parameters and locals, by Java name. A name declared twice with
/// different types is not handled; Java programs that do so are rare, and the last type wins.
pub(super) fn boxed_locals(m: &Method) -> HashMap<String, Type> {
    let mut out = HashMap::new();
    for p in m.params.iter().filter(|p| matches!(p.ty, Type::Boxed(_))) {
        out.insert(p.name.clone(), p.ty);
    }
    for stmt in &m.body {
        visit_stmt(stmt, &mut |_| {}, &mut |s| {
            if let Stmt::Local { ty, name, .. } | Stmt::ForEach { ty, name, .. } = s
                && matches!(ty, Type::Boxed(_))
            {
                out.insert(name.clone(), *ty);
            }
        });
    }
    out
}

impl<'a> Gen<'a> {
    /// Whether the boxed type `ty` is the faithful `java::Boxed<T>`, whose identity is kept.
    pub(super) fn faithful_box(&self, ty: Type) -> bool {
        matches!(ty, Type::Boxed(_)) && self.facts.faithful_boxes.contains(&ty)
    }

    /// Whether reading a value of `ty` copies it in Rust: owned data is cloned, and so is a
    /// faithful box, which is shared. A plain boxed value is `Copy` (or an `Option` of one).
    pub(super) fn clones(&self, ty: Type) -> bool {
        super::objects::needs_clone(ty) || self.faithful_box(ty)
    }

    /// The Rust type of the boxed type `ty`, without the `Option` that a null value adds.
    pub(super) fn boxed_name(&mut self, ty: Type) -> String {
        let Type::Boxed(primitive) = ty else { unreachable!("boxed_name of a type that is not boxed") };
        let rust = rust_type(*primitive);
        if self.faithful_box(ty) { format!("{}<{rust}>", self.runtime("Boxed")) } else { rust.to_string() }
    }

    /// The boxed value of the primitive `code` of `kind`: a cached object where `valueOf` caches,
    /// and the value itself where the identity is not observed.
    pub(super) fn box_code(&mut self, code: Code, kind: Type) -> Code {
        if !self.faithful_box(Type::boxed(kind)) {
            return code;
        }
        let function = match kind {
            Type::Int => "box_int",
            Type::Long => "box_long",
            Type::Short => "box_short",
            Type::Byte => "box_byte",
            Type::Char => "box_char",
            Type::Boolean => "box_bool",
            // `Double.valueOf` and `Float.valueOf` never cache.
            _ => "box_new",
        };
        Code::atom(format!("{}({})", self.runtime(function), code.text))
    }

    /// The primitive of `kind` in the boxed value `code`. A value that may be null (`nullable`)
    /// is an `Option`, and unboxing a null throws `NullPointerException`.
    pub(super) fn unbox_code(&mut self, code: Code, kind: Type, nullable: bool) -> Code {
        let value =
            if nullable { Code::atom(format!("{}({})", self.runtime("non_null"), code.text)) } else { code };
        if self.faithful_box(Type::boxed(kind)) {
            Code::new(format!("*{}", value.at(UNARY)), UNARY)
        } else {
            value
        }
    }

    /// `e` as a value of type `ty` when `e` or `ty` is boxed: unboxing throws on a null, and
    /// boxing makes the object of the kind (cached where `valueOf` caches).
    pub(super) fn boxing_value(&mut self, e: &'a Expr, ty: Type) -> Code {
        let code = self.expr(e);
        if e.ty == ty {
            // A boxed value passes on as it is; a null can only reach here if it is unboxed.
            return if self.nullable(e) {
                Code::atom(format!("{}({})", self.runtime("non_null"), code.text))
            } else {
                code
            };
        }
        let from = e.ty.unboxed();
        let prim = if matches!(e.ty, Type::Boxed(_)) {
            let nullable = self.nullable(e);
            self.unbox_code(code, from, nullable)
        } else {
            code
        };
        let converted = self.convert(prim, from, ty.unboxed());
        if matches!(ty, Type::Boxed(_)) { self.box_code(converted, ty.unboxed()) } else { converted }
    }

    /// `a == b` or `a != b` where both are boxed of a faithful kind: the objects are compared,
    /// so two `valueOf` results are equal only in the cache, and two `new` ones never are.
    pub(super) fn boxed_identity(&mut self, op: BinOp, l: &'a Expr, r: &'a Expr) -> Code {
        let left = self.identity_arg(l);
        let right = self.identity_arg(r);
        let same = format!("{}({left}, {right})", self.runtime("same_optional"));
        if matches!(op, BinOp::Ne) { Code::new(format!("!{same}"), UNARY) } else { Code::atom(same) }
    }

    /// A boxed value as the `Option<&Boxed<T>>` that `same_optional` compares.
    fn identity_arg(&mut self, e: &'a Expr) -> String {
        if self.nullable(e) {
            format!("{}.as_ref()", self.expr(e).at(ATOM))
        } else {
            format!("Some(&{})", self.expr(e).at(UNARY))
        }
    }

    /// A call of a method of a boxed class. `args[0]` is the receiver of an instance method.
    pub(super) fn boxed_builtin(
        &mut self,
        op: BoxedOp,
        kind: Type,
        args: &'a [Expr],
        ty: Type,
        id: ExprId,
    ) -> Code {
        match op {
            BoxedOp::Parse => {
                let code = self.parse(kind, &args[0]);
                self.box_code_if_boxed(code, kind, ty)
            }
            BoxedOp::Box => {
                let code = self.value(&args[0], kind);
                self.box_code(code, kind)
            }
            BoxedOp::Unbox => {
                let code = self.value(&args[0], kind);
                self.convert(code, kind, ty)
            }
            BoxedOp::ToString => {
                let code = self.value(&args[0], kind);
                let text = self.shown(kind, code.at(ATOM));
                Code::atom(format!("{text}.to_string()"))
            }
            BoxedOp::HashCode => {
                let code = self.value(&args[0], kind);
                let function = match kind {
                    Type::Int => return code,
                    Type::Long => "long_hash",
                    Type::Double => "double_hash",
                    Type::Float => "float_hash",
                    Type::Boolean => "bool_hash",
                    // A `Character`, `Short` or `Byte` hash is its value.
                    _ => return Code::new(format!("{} as i32", code.cast_operand()), AS),
                };
                Code::atom(format!("{}({})", self.runtime(function), code.text))
            }
            BoxedOp::Compare => {
                let a = self.value(&args[0], kind);
                let b = self.value(&args[1], kind);
                match kind {
                    Type::Int | Type::Long => {
                        Code::new(format!("{}.cmp(&{}) as i32", a.at(ATOM), b.at(UNARY)), AS)
                    }
                    // `Character.compare`, `Short.compareTo` and `Byte.compareTo` subtract.
                    Type::Char | Type::Short | Type::Byte => {
                        let a = Code::new(format!("{} as i32", a.cast_operand()), AS);
                        let b = Code::new(format!("{} as i32", b.cast_operand()), AS);
                        infix("-", ADD, &a, &b)
                    }
                    Type::Boolean => {
                        Code::atom(format!("{}({}, {})", self.runtime("bool_compare"), a.text, b.text))
                    }
                    _ => {
                        let a = self.convert(a, kind, Type::Double);
                        let b = self.convert(b, kind, Type::Double);
                        Code::atom(format!("{}({}, {})", self.runtime("double_compare"), a.text, b.text))
                    }
                }
            }
            BoxedOp::Equals => {
                let receiver = self.value(&args[0], kind);
                match &args[1].ty {
                    Type::Null => Code::atom(format!("{{ {}; false }}", receiver.text)),
                    _ if self.nullable(&args[1]) => {
                        let other = self.expr(&args[1]).text;
                        let other_value = self.unbox_code(Code::atom("o_"), kind, false);
                        let equal = self.equal(kind, Code::atom("r_"), other_value);
                        Code::atom(format!(
                            "{{ let r_ = {}; {other}.is_some_and(|o_| {}) }}",
                            receiver.text, equal.text
                        ))
                    }
                    _ => {
                        let other = self.value(&args[1], kind);
                        self.equal(kind, receiver, other)
                    }
                }
            }
            BoxedOp::Sum => {
                let a = self.value(&args[0], kind);
                let b = self.value(&args[1], kind);
                let (a, b) = (Operand { code: a, ty: kind }, Operand { code: b, ty: kind });
                self.binary(BinOp::Add, a, b, kind, id)
            }
        }
    }

    /// `Integer.valueOf(s)` and the like: the primitive, boxed when the result is a boxed type.
    fn box_code_if_boxed(&mut self, code: Code, kind: Type, ty: Type) -> Code {
        if matches!(ty, Type::Boxed(_)) { self.box_code(code, kind) } else { code }
    }

    /// `Integer.parseInt(s)` and the like. A null string is `null` to `parseBoolean`, which is
    /// false, and a `NullPointerException` to the others.
    fn parse(&mut self, kind: Type, arg: &'a Expr) -> Code {
        let text = if kind == Type::Boolean && self.nullable(arg) {
            format!("{}.unwrap_or(\"\")", self.option_ref(arg))
        } else if self.nullable(arg) {
            self.deref(arg)
        } else {
            self.str_ref(arg)
        };
        match kind {
            Type::Boolean => Code::atom(format!("{}({text})", self.runtime("parse_boolean"))),
            Type::Double => Code::atom(format!("{}({text})", self.runtime("parse_double"))),
            _ => Code::atom(format!("{}::<{}>({text})", self.runtime("parse_number"), rust_type(kind))),
        }
    }

    /// `a == b` for two primitives of `kind`, for `equals`: `Double.equals` compares bits.
    fn equal(&mut self, kind: Type, a: Code, b: Code) -> Code {
        match kind {
            Type::Double | Type::Float => {
                let a = self.convert(a, kind, Type::Double);
                let b = self.convert(b, kind, Type::Double);
                Code::atom(format!("{}({}, {})", self.runtime("double_equals"), a.text, b.text))
            }
            _ => infix("==", CMP, &a, &b),
        }
    }

    /// `target op= right` or `target++` on a boxed local: see `boxed_result`.
    pub(super) fn boxed_update(
        &mut self,
        op: BinOp,
        target: &str,
        ty: Type,
        right: Operand,
        id: ExprId,
    ) -> String {
        let name = self.local(target);
        let optional = self.optional_var(self.method, ty, target);
        let option = if self.faithful_box(ty) { format!("{name}.clone()") } else { name.clone() };
        let stored = self.boxed_result(op, ty, Code::atom(option), optional, right, id);
        format!("{name} = {}", stored.text)
    }

    /// The value of `current op right` for a boxed `ty`: `current` is unboxed (a null throws)
    /// when `optional`, the operation is done in the primitive type, and the result is boxed
    /// again. An `Option` of it when `optional`, as the variable or field holds it.
    pub(super) fn boxed_result(
        &mut self,
        op: BinOp,
        ty: Type,
        current: Code,
        optional: bool,
        right: Operand,
        id: ExprId,
    ) -> Code {
        let kind = ty.unboxed();
        let current = if optional {
            Code::atom(format!("{}({})", self.runtime("non_null"), current.text))
        } else {
            current
        };
        let current = self.unbox_code(current, kind, false);
        let result = crate::types::binary_type(op, kind, right.ty.unboxed()).expect("checked");
        let code = self.binary(op, Operand { code: current, ty: kind }, right, result, id);
        let code = self.convert(code, result, kind);
        let boxed = self.box_code(code, kind);
        if optional { Code::atom(format!("Some({})", boxed.text)) } else { boxed }
    }

    /// The Rust type of a field: a boxed field is an `Option`, as its default is `null`, and so is
    /// a `nullable` one (see `analysis::nullness`).
    pub(super) fn field_ty_name(&mut self, ty: Type, nullable: bool) -> String {
        if matches!(ty, Type::Boxed(_)) || nullable {
            format!("Option<{}>", self.ty_name(ty))
        } else {
            self.ty_name(ty)
        }
    }

    /// A value as a field of type `ty` stores it: `Some` of the value, or `None` for `null`, when
    /// the field is boxed or `nullable`.
    pub(super) fn field_value(&mut self, e: &'a Expr, ty: Type, nullable: bool) -> String {
        if matches!(ty, Type::Boxed(_)) || nullable {
            self.optional_to(e, ty).text
        } else {
            self.value(e, ty).text
        }
    }

    /// `Character.getNumericValue` and the like; see `java::char_numeric_value`.
    pub(super) fn char_numeric_value(&mut self, args: &'a [Expr]) -> Code {
        let c = self.value(&args[0], Type::Char);
        Code::atom(format!("{}({})", self.runtime("char_numeric_value"), c.text))
    }

    /// A boxed value that may be null, as the text of its string form: `null` for a null.
    pub(super) fn boxed_text(&mut self, e: &'a Expr, kind: Type) -> String {
        let option = self.expr(e).at(ATOM);
        let value = self.unbox_code(Code::atom("v_"), kind, false);
        let shown = self.shown(kind, value.at(ATOM));
        let or_null = self.runtime("or_null");
        match shown.strip_suffix("(v_)") {
            _ if shown == "v_" => format!("{or_null}({option})"),
            _ if shown == "(*v_)" => format!("{or_null}({option}.map(|v_| *v_))"),
            Some(function) if !function.contains(['.', '(']) => {
                format!("{or_null}({option}.map({function}))")
            }
            _ => format!("{or_null}({option}.map(|v_| {shown}))"),
        }
    }
}
