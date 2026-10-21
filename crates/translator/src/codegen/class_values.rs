//! Classes as values (see `lower::classes`): a `java::Class`, written out where it is used with
//! its names and a function for its superclass or component type (`java::Class::new("Main$Dog",
//! "Dog", Some(|| ..))`), so that no class needs a declaration of its own.

use super::*;

impl<'a> Gen<'a> {
    /// `GetClass(o)`: the class of the value of `o`, from the variant of an enum of classes.
    pub(super) fn get_class(&mut self, object: &'a Expr) -> Code {
        if let Some(t) = self.enum_of(object) {
            let obj = self.object(object);
            let arms: Vec<String> = self
                .enum_variants(t)
                .into_iter()
                .map(|c| format!("{}(_) => {}", self.variant(t, c), self.decl_class(c)))
                .collect();
            return Code::new(format!("match {obj} {{ {} }}", arms.join(", ")), LOWEST);
        }
        let class = match object.ty {
            Type::Class(..) => {
                let c = self.struct_class(object).expect("an object has a class");
                self.decl_class(c)
            }
            ty => self.type_class(ty),
        };
        if self.nullable(object) {
            let check = self.null_check(object).text;
            return Code::new(format!("{{ {check}; {class} }}"), LOWEST);
        }
        if has_effects(object) {
            let text = self.expr(object).text;
            return Code::new(format!("{{ {text}; {class} }}"), LOWEST);
        }
        Code::atom(class)
    }

    /// `ComparableClassFor(x)`: `Some` of the class of `x` where that class says it implements
    /// `Comparable`: a string, a box, or a class of the program (not an enum, whose `Comparable`
    /// is `Enum`'s).
    pub(super) fn comparable_class_for(&mut self, x: &'a Expr) -> Code {
        let comparable = |g: &Self, c: usize| {
            let decl = &g.class.types[c];
            runtime_type(&decl.name).is_some()
                || (decl.kind == TypeKind::Class
                    && decl.implements.iter().any(|&i| g.class.types[i].name == COMPARABLE))
        };
        // A null `x` is no `Comparable`.
        let nullable = self.nullable(x);
        let value = if nullable { self.option_ref(x) } else { self.expr(x).at(ATOM) };
        let some = |pattern: String| if nullable { format!("Some({pattern})") } else { pattern };
        let mut arms = Vec::new();
        match self.enum_of(x) {
            Some(t) => {
                for c in self.enum_variants(t) {
                    if comparable(self, c) {
                        let pattern = some(format!("{}(_)", self.variant(t, c)));
                        arms.push(format!("{pattern} => Some({})", self.decl_class(c)));
                    }
                }
            }
            None => {
                let c = self.value_class(x);
                if comparable(self, c) {
                    arms.push(format!("{} => Some({})", some("_".to_string()), self.decl_class(c)));
                }
            }
        }
        arms.push("_ => None".to_string());
        Code::new(format!("match {value} {{ {} }}", arms.join(", ")), LOWEST)
    }

    /// The `java::Class` of type `ty`, as `T.class` names it.
    pub(super) fn type_class(&mut self, ty: Type) -> String {
        let constant = |name: &str| format!("java::Class::{name}");
        match ty {
            Type::Int => constant("INT"),
            Type::Long => constant("LONG_PRIMITIVE"),
            Type::Double => constant("DOUBLE_PRIMITIVE"),
            Type::Float => constant("FLOAT_PRIMITIVE"),
            Type::Short => constant("SHORT_PRIMITIVE"),
            Type::Byte => constant("BYTE_PRIMITIVE"),
            Type::Char => constant("CHAR"),
            Type::Boolean => constant("BOOLEAN_PRIMITIVE"),
            Type::Void => constant("VOID"),
            Type::String => constant("STRING"),
            Type::StringBuilder => constant("STRING_BUILDER"),
            Type::JavaClass => constant("CLASS"),
            Type::Boxed(primitive) => constant(match *primitive {
                Type::Int => "INTEGER",
                Type::Long => "LONG",
                Type::Double => "DOUBLE",
                Type::Float => "FLOAT",
                Type::Short => "SHORT",
                Type::Byte => "BYTE",
                Type::Char => "CHARACTER",
                _ => "BOOLEAN",
            }),
            Type::Array(element) => {
                let (name, simple) =
                    (array_name(self, *element), format!("{}[]", simple_name(self, *element)));
                let component = self.type_class(*element);
                format!(
                    "java::Class::array({}, {}, || {component})",
                    rust_string(&name),
                    rust_string(&simple)
                )
            }
            _ => match self.class.type_of(ty) {
                Some(c) => self.decl_class(c),
                None => constant("OBJECT"),
            },
        }
    }

    /// The `java::Class` of class `c` of the program, or of `Object` and the runtime classes.
    fn decl_class(&mut self, c: usize) -> String {
        let decl = &self.class.types[c];
        if let Some(ty) = runtime_type(&decl.name) {
            return self.type_class(ty);
        }
        match decl.name.as_str() {
            OBJECT => return "java::Class::OBJECT".to_string(),
            COMPARABLE => return "java::Class::COMPARABLE".to_string(),
            _ => {}
        }
        let (kind, extends) = (decl.kind, decl.extends);
        let name = rust_string(&self.java_class_name(c, false));
        let simple = rust_string(&self.java_class_name(c, true));
        match kind {
            TypeKind::Interface => format!("java::Class::interface({name}, {simple})"),
            TypeKind::Enum => format!("java::Class::enumeration({name}, {simple})"),
            _ => {
                let superclass = match extends {
                    Some(s) => self.decl_class(s),
                    None => "java::Class::OBJECT".to_string(),
                };
                format!("java::Class::new({name}, {simple}, Some(|| {superclass}))")
            }
        }
    }

    /// `ClassMethod(op)(class, args..)`.
    pub(super) fn class_method(&mut self, op: ClassOp, args: &'a [Expr]) -> Code {
        let class = if self.nullable(&args[0]) {
            format!("{}({})", self.runtime("non_null"), self.expr(&args[0]).text)
        } else {
            self.expr(&args[0]).at(ATOM)
        };
        let method = match op {
            ClassOp::GetName => "get_name",
            ClassOp::GetSimpleName => "get_simple_name",
            ClassOp::GetSuperclass => "get_superclass",
            ClassOp::GetComponentType => "get_component_type",
            ClassOp::IsArray => "is_array",
            ClassOp::IsInterface => "is_interface",
            ClassOp::IsPrimitive => "is_primitive",
            ClassOp::IsEnum => "is_enum",
            ClassOp::ToString => "to_string",
            ClassOp::HashCode => "hash_code",
            ClassOp::DesiredAssertionStatus => {
                return Code::new(format!("{{ let _ = {class}; false }}"), LOWEST);
            }
            ClassOp::Equals => {
                let other = &args[1];
                return if self.nullable(other) || matches!(other.kind, ExprKind::Null) {
                    let other = self.class_option(other);
                    infix("==", CMP, &Code::atom(format!("Some({class})")), &Code::atom(other))
                } else {
                    let other = self.expr(other);
                    infix("==", CMP, &Code::atom(class), &other)
                };
            }
        };
        Code::atom(format!("{class}.{method}()"))
    }

    /// `a == b` or `a != b` of two classes, which are the same when their names are.
    pub(super) fn class_identity(&mut self, op: BinOp, l: &'a Expr, r: &'a Expr) -> Code {
        let symbol = if op == BinOp::Ne { "!=" } else { "==" };
        let optional = |g: &Self, e: &Expr| g.nullable(e) || matches!(e.kind, ExprKind::Null);
        if optional(self, l) || optional(self, r) {
            let (a, b) = (self.class_option(l), self.class_option(r));
            return infix(symbol, CMP, &Code::atom(a), &Code::atom(b));
        }
        let (a, b) = (self.expr(l), self.expr(r));
        infix(symbol, CMP, &a, &b)
    }

    /// A class that may be null, as an `Option<java::Class>`.
    fn class_option(&mut self, e: &'a Expr) -> String {
        match &e.kind {
            ExprKind::Null => "None::<java::Class>".to_string(),
            _ if self.nullable(e) => self.expr(e).at(ATOM),
            _ => format!("Some({})", self.expr(e).text),
        }
    }
}

/// The binary name of the class of arrays of `element`: `[I`, `[Ljava.lang.String;`.
fn array_name(g: &mut Gen, element: Type) -> String {
    let code = match element {
        Type::Int => "I".to_string(),
        Type::Long => "J".to_string(),
        Type::Double => "D".to_string(),
        Type::Float => "F".to_string(),
        Type::Short => "S".to_string(),
        Type::Byte => "B".to_string(),
        Type::Char => "C".to_string(),
        Type::Boolean => "Z".to_string(),
        Type::Array(inner) => array_name(g, *inner),
        _ => format!("L{};", class_name(g, element)),
    };
    format!("[{code}")
}

/// The binary name of a class type.
fn class_name(g: &mut Gen, ty: Type) -> String {
    match ty {
        Type::String => "java.lang.String".to_string(),
        Type::StringBuilder => "java.lang.StringBuilder".to_string(),
        Type::JavaClass => "java.lang.Class".to_string(),
        Type::Boxed(_) => runtime_name(ty).unwrap_or("java.lang.Object").to_string(),
        _ => match g.class.type_of(ty) {
            Some(c) if g.class.types[c].name == OBJECT => "java.lang.Object".to_string(),
            Some(c) if g.class.types[c].name == COMPARABLE => "java.lang.Comparable".to_string(),
            Some(c) => g.java_class_name(c, false),
            None => "java.lang.Object".to_string(),
        },
    }
}

/// The simple name of a type, as `getSimpleName` gives it.
fn simple_name(g: &mut Gen, ty: Type) -> String {
    match ty {
        Type::Array(inner) => format!("{}[]", simple_name(g, *inner)),
        _ if !ty.is_reference() && !matches!(ty, Type::Enum(_)) => ty.to_string(),
        _ => class_name(g, ty).rsplit(['.', '$']).next().unwrap_or_default().to_string(),
    }
}
