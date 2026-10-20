//! Code generation for enums. An enum is a Rust `enum` with one variant per constant, which is a
//! `Copy` value. Each field is an accessor method that matches on `self` and returns the value the
//! constant has (lowering computed it, see `lower/enums.rs`). Instance methods take `self` by value,
//! and `Display` prints the name, as Java's default `toString` does.

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
    pub(super) fn placeholder(&self, ty: Type) -> String {
        let Type::Enum(name) = ty else { return default_value(ty).to_string() };
        let class = self.class.types.iter().position(|t| t.name == name).expect("an enum type is declared");
        if self.class.types[class].enum_constants.as_deref().unwrap_or_default().is_empty() {
            unreachable!("an enum with no constants has no value: {name}");
        }
        self.constant_path(class, 0)
    }

    /// The value of an enum's field, read from `self`: `planet.mass()`.
    pub(super) fn enum_field(&mut self, object: &'a Expr, name: &str) -> Code {
        let obj = self.object(object);
        Code::atom(format!("{obj}.{}()", snake_case(name)))
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
        self.line(&format!("enum {name} {{"));
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
            self.line(&format!("fn {}(self) -> {ty} {{", snake_case(&field.name)));
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
        self.line("fn enum_name(self) -> String {");
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
        self.line(&format!("fn enum_value_of(name: &str) -> {name} {{"));
        self.indent += 1;
        self.line("match name {");
        self.indent += 1;
        for c in constants {
            self.line(&format!("{} => {name}::{},", rust_string(&c.name), variant_name(&c.name)));
        }
        let throw = self.runtime("throw");
        self.line(&format!(
            "_ => {throw}(\"IllegalArgumentException\", &format!(\"No enum constant {}.{{name}}\")),",
            decl.name
        ));
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.line("}");
        for i in 0..class.methods.len() {
            let m = &class.methods[i];
            if m.class == index && !m.is_static && !m.is_ctor {
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
        self.line("write!(f, \"{}\", self.enum_name())");
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.line("}");
    }

    /// The builtins of enums: `ordinal()`, `name()`, `compareTo` and `valueOf`.
    pub(super) fn enum_builtin(&mut self, builtin: Builtin, args: &'a [Expr], ty: Type) -> Code {
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
                Code::atom(format!("{}::enum_value_of({text})", struct_name(name)))
            }
            _ => unreachable!("not an enum builtin: {builtin:?}"),
        }
    }
}
