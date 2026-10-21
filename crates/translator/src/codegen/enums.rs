//! Code generation for enums. An enum is a Rust `enum` with one variant per constant, which is a
//! `Copy` value. Each field is an accessor method that matches on `self` and returns the value the
//! constant has (lowering computed it, see `lower/enums.rs`). Instance methods take `self` by value,
//! and `Display` prints the name, as Java's default `toString` does, or what the enum's own
//! `toString` gives.

use super::*;

/// The Rust variant for an enum constant: `RED` is `Red`, and `VERY_BIG` is `VeryBig`.
pub(super) fn variant_name(constant: &str) -> String {
    constant
        .split('_')
        .map(|word| {
            let lower = word.to_lowercase();
            let mut chars = lower.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect(),
                None => String::new(),
            }
        })
        .collect()
}

impl<'a> Gen<'a> {
    /// The Rust path of an enum constant, such as `Color::Red`.
    pub(super) fn constant_path(&self, class: usize, ordinal: usize) -> String {
        let decl = &self.class.types[class];
        let constants = decl.enum_constants.as_deref().unwrap_or_default();
        format!("{}::{}", struct_name(&decl.name), variant_name(&constants[ordinal].name))
    }

    /// The value a field or cell of type `ty` holds before its initializer runs. An enum has no
    /// Java default that is not `null`, so its placeholder is its first constant.
    pub(super) fn placeholder(&self, ty: Type, nullable: bool) -> String {
        if nullable {
            return "None".to_string();
        }
        let Type::Enum(name) = ty else { return default_value(ty).to_string() };
        let class = self.class.types.iter().position(|t| t.name == name).expect("an enum type is declared");
        if self.class.types[class].enum_constants.as_deref().unwrap_or_default().is_empty() {
            unreachable!("an enum with no constants has no value: {name}");
        }
        self.constant_path(class, 0)
    }

    /// The value of an enum's field, read from `self`: `planet.mass()`.
    pub(super) fn enum_field(&mut self, object: &'a Expr, name: &str) -> Code {
        let Type::Enum(enum_name) = object.ty else { unreachable!("an enum field of an enum") };
        let class = self.class.types.iter().position(|t| t.name == enum_name).expect("a declared enum");
        let accessor = self.accessor(class, name);
        let obj = self.object(object);
        Code::atom(format!("{obj}.{accessor}()"))
    }

    /// The name of the accessor of field `name` of enum `class`: the field's name, unless a method
    /// of the enum has it (`double mass() { return mass; }`).
    fn accessor(&self, class: usize, name: &str) -> String {
        let accessor = snake_case(name);
        let taken = (0..self.class.methods.len())
            .any(|i| self.class.methods[i].class == class && self.fn_names[i] == accessor);
        if taken { format!("{accessor}_field") } else { accessor }
    }

    /// The method of enum `class` that is its own `toString`, if it declares one.
    fn enum_to_string(&self, class: usize) -> Option<usize> {
        self.class.methods.iter().position(|m| {
            m.class == class
                && m.name == "toString"
                && m.params.is_empty()
                && !m.is_static
                && m.ret == Type::String
        })
    }

    /// The `enum` and its `impl` for an enum class: the variants, an accessor for each field,
    /// `enum_name` and `enum_value_of`, the instance methods, and `Display`.
    pub(super) fn enum_items(&mut self, index: usize) {
        let class = self.class;
        let decl = &class.types[index];
        let Some(constants) = &decl.enum_constants else { return };
        let name = struct_name(&decl.name);
        self.gap();
        self.line("#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]");
        self.line(&format!("{}enum {name} {{", self.vis()));
        self.indent += 1;
        for c in constants {
            self.line(&format!("{},", variant_name(&c.name)));
        }
        self.indent -= 1;
        self.line("}");
        self.gap();
        self.line(&format!("impl {name} {{"));
        self.indent += 1;
        for (f, field) in decl.fields.iter().enumerate() {
            let ty = self.ty_name(field.ty);
            self.gap();
            let accessor = self.accessor(index, &field.name);
            self.line(&format!("{}fn {accessor}(self) -> {ty} {{", self.vis()));
            self.indent += 1;
            self.line("match self {");
            self.indent += 1;
            for c in constants {
                let value = self.value(&c.values[f], field.ty).text;
                self.line(&format!("{name}::{} => {value},", variant_name(&c.name)));
            }
            self.indent -= 1;
            self.line("}");
            self.indent -= 1;
            self.line("}");
        }
        self.gap();
        self.line(&format!("{}fn enum_name(self) -> String {{", self.vis()));
        self.indent += 1;
        self.line("let name: &str = match self {");
        self.indent += 1;
        for c in constants {
            self.line(&format!("{name}::{} => {},", variant_name(&c.name), rust_string(&c.name)));
        }
        self.indent -= 1;
        self.line("};");
        self.line("name.to_string()");
        self.indent -= 1;
        self.line("}");
        self.gap();
        self.line(&format!("{}fn enum_value_of(name: &str) -> {name} {{", self.vis()));
        self.indent += 1;
        self.line("match name {");
        self.indent += 1;
        for c in constants {
            self.line(&format!("{} => {name}::{},", rust_string(&c.name), variant_name(&c.name)));
        }
        let throw = self.runtime("throw");
        self.line(&format!(
            "_ => {throw}(\"java.lang.IllegalArgumentException\", &format!(\"No enum constant {}.{{name}}\")),",
            decl.name
        ));
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.line("}");
        if self.checked_value_of(index) {
            self.try_value_of(index);
        }
        for i in 0..class.methods.len() {
            let m = &class.methods[i];
            // Its own methods, and the default methods of its interfaces that live here.
            if (m.class == index || self.homes[i] == Home::Class(index)) && !m.is_static && !m.is_ctor {
                self.gap();
                self.method(i);
            }
        }
        self.indent -= 1;
        self.line("}");
        self.gap();
        self.line(&format!("impl fmt::Display for {name} {{"));
        self.indent += 1;
        self.line("fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {");
        self.indent += 1;
        match self.enum_to_string(index) {
            // The path names the enum's own method: on `&self`, `ToString::to_string` would be
            // found first, which is this `Display`.
            Some(m) => self.line(&format!("f.write_str(&Self::{}(*self))", self.fn_names[m])),
            None => self.line("write!(f, \"{}\", self.enum_name())"),
        }
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.line("}");
    }

    /// Whether a `valueOf` of the enum `index` is somewhere a `catch` or `finally` sees its failure.
    fn checked_value_of(&self, index: usize) -> bool {
        let mut found = false;
        for m in &self.class.methods {
            for s in &m.body {
                visit_stmt(
                    s,
                    &mut |e| {
                        visit_expr(e, &mut |x| {
                            found |= matches!(x.kind, ExprKind::Builtin(Builtin::EnumValueOf, _))
                                && matches!(x.ty, Type::Enum(n) if *n == self.class.types[index].name)
                                && self.facts.raising.contains(&x.id)
                        })
                    },
                    &mut |_| {},
                );
            }
        }
        found
    }

    /// `try_enum_value_of`, the checked form of `enum_value_of`, whose failure is an exception the
    /// program sees.
    fn try_value_of(&mut self, index: usize) {
        let decl = &self.class.types[index];
        let Some(constants) = &decl.enum_constants else { return };
        let name = struct_name(&decl.name);
        let exception = self.runtime("Exception");
        self.gap();
        self.line(&format!(
            "{}fn try_enum_value_of(name: &str) -> Result<{name}, {exception}> {{",
            self.vis()
        ));
        self.indent += 1;
        self.line("Ok(match name {");
        self.indent += 1;
        for c in constants {
            self.line(&format!("{} => {name}::{},", rust_string(&c.name), variant_name(&c.name)));
        }
        self.line(&format!(
            "_ => return Err({exception}::new(\"java.lang.IllegalArgumentException\", format!(\"No enum constant {}.{{name}}\"))),",
            decl.name
        ));
        self.indent -= 1;
        self.line("})");
        self.indent -= 1;
        self.line("}");
    }

    /// The builtins of enums: `ordinal()`, `name()`, `compareTo` and `valueOf`.
    pub(super) fn enum_builtin(&mut self, builtin: Builtin, args: &'a [Expr], ty: Type, id: ExprId) -> Code {
        if let Some(t) = self.enum_of(&args[0]) {
            return self.erased_enum_builtin(builtin, args, t);
        }
        match builtin {
            Builtin::EnumOrdinal => {
                let obj = self.expr(&args[0]);
                Code::new(format!("{} as i32", obj.cast_operand()), AS)
            }
            Builtin::EnumName => {
                let obj = self.expr(&args[0]);
                Code::atom(format!("{}.enum_name()", self.recv(&obj, args[0].ty)))
            }
            Builtin::EnumCompareTo => {
                let (a, b) = (self.expr(&args[0]), self.expr(&args[1]));
                Code::atom(format!("({} as i32 - {} as i32)", a.cast_operand(), b.cast_operand()))
            }
            Builtin::EnumValueOf => {
                let Type::Enum(name) = ty else { unreachable!("valueOf returns an enum") };
                let text = self.str_ref(&args[0]);
                if self.facts.raising.contains(&id) {
                    let call = Code::atom(format!("{}::try_enum_value_of({text})", struct_name(name)));
                    return Code::atom(self.passed_on(call));
                }
                Code::atom(format!("{}::enum_value_of({text})", struct_name(name)))
            }
            _ => unreachable!("not an enum builtin: {builtin:?}"),
        }
    }

    /// An enum's builtin on an `Enum` that is an `Object` (of the enum of type `t`): a `match`
    /// on the enums it can be.
    fn erased_enum_builtin(&mut self, builtin: Builtin, args: &'a [Expr], t: usize) -> Code {
        let enums: Vec<usize> =
            self.enum_variants(t).into_iter().filter(|&c| self.class.types[c].kind == TypeKind::Enum).collect();
        let ordinal = |g: &mut Self, e: &'a Expr| {
            let obj = g.object(e);
            let mut arms: Vec<String> = enums.iter().map(|&c| format!("{}(e) => e as i32", g.variant(t, c))).collect();
            arms.push(format!("_ => {}(\"java.lang.Enum\")", g.runtime("cast_failed")));
            format!("(match {obj} {{ {} }})", arms.join(", "))
        };
        match builtin {
            Builtin::EnumOrdinal => Code::atom(ordinal(self, &args[0])),
            Builtin::EnumCompareTo => {
                let (a, b) = (ordinal(self, &args[0]), ordinal(self, &args[1]));
                Code::new(format!("{a} - {b}"), ADD)
            }
            Builtin::EnumName => {
                let obj = self.object(&args[0]);
                let mut arms: Vec<String> =
                    enums.iter().map(|&c| format!("{}(e) => e.enum_name()", self.variant(t, c))).collect();
                arms.push(format!("_ => {}(\"java.lang.Enum\")", self.runtime("cast_failed")));
                Code::new(format!("match {obj} {{ {} }}", arms.join(", ")), LOWEST)
            }
            _ => unreachable!("not a builtin of an erased enum: {builtin:?}"),
        }
    }
}
