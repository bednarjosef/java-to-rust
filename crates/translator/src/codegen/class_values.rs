//! Classes as values (see `lower::classes`): a `java::Class`, written out where it is used with
//! its names and a function for its superclass or component type (`java::Class::new("Main$Dog",
//! "Dog", Some(|| ..))`), so that no class needs a declaration of its own. For a value of an enum
//! of classes, the enum's `runtime_class()` and `runtime_type_names()` hold the `match` over its
//! variants (`dispatch::class_value_impls`).

use super::*;

/// `match obj { arms }` over the variants of an enum of classes. With no arms, for an interface
/// that no created class implements, `obj` has no value to match; Rust takes no empty `match` on
/// a reference, so it is run and then the code is unreachable.
pub(super) fn variant_match(obj: &str, arms: &[String]) -> String {
    match arms.is_empty() {
        true => format!("{{ let _ = {obj}; unreachable!() }}"),
        false => format!("match {obj} {{ {} }}", arms.join(", ")),
    }
}

/// `obj.method()`, with `obj` in parentheses unless it is a plain path, field or call.
pub(super) fn method_on(obj: &str, method: &str) -> String {
    let obj = obj.strip_prefix('&').filter(|rest| plain(rest)).unwrap_or(obj);
    match plain(obj) {
        true => format!("{obj}.{method}()"),
        false => format!("({obj}).{method}()"),
    }
}

/// A borrow of `obj`: `&obj`, with `obj` in parentheses unless it is a plain path, field or call.
pub(super) fn borrow_of(obj: &str) -> String {
    match plain(obj) {
        true => format!("&{obj}"),
        false => format!("&({obj})"),
    }
}

/// Whether `text` is a path, a field or a call, which a method call can follow as it is.
fn plain(text: &str) -> bool {
    !text.is_empty()
        && !text.contains(char::is_whitespace)
        && !text.starts_with(['&', '*', '!', '-', '{', '(', '|'])
}

impl<'a> Gen<'a> {
    /// `GetClass(o)`: the class of the value of `o`, from the variant of an enum of classes.
    pub(super) fn get_class(&mut self, object: &'a Expr) -> Code {
        if let Some(t) = self.enum_of(object) {
            let obj = self.object(object);
            self.need(Need::RuntimeClass(t));
            return Code::atom(method_on(&obj, "runtime_class"));
        }
        let class = match object.ty {
            Type::Class(..) => {
                let c = self.struct_class(object).expect("an object has a class");
                self.decl_class(c)
            }
            // An `Object[]` that may be a covariant array, which carries its class.
            ty if self.carries_class(ty) => {
                let exact = self.type_class(ty);
                let handle = self.handle(object);
                return Code::atom(format!("{}({handle}, || {exact})", self.runtime("array_class")));
            }
            ty => self.type_class(ty),
        };
        self.after_object(object, &class)
    }

    /// Whether an array of type `ty` may carry its class: an `Object[]` in a program that makes
    /// covariant arrays (`Class::element_classes`).
    pub(super) fn carries_class(&self, ty: Type) -> bool {
        !self.class.element_classes.is_empty() && ty == Type::array(Type::class(OBJECT))
    }

    /// The Java class of the array that `e` makes, where the translation holds it as an
    /// `Object[]` and the array carries the class (`Class::element_classes`): a new array or a
    /// copy that lowering noted, and the `String[]`s that the runtime makes as `Object[]`s
    /// (`split`, `main`'s arguments).
    pub(super) fn made_class(&self, e: &Expr) -> Option<Type> {
        if self.class.element_classes.is_empty() || e.ty != Type::array(Type::class(OBJECT)) {
            return None;
        }
        match &e.kind {
            ExprKind::NewArray(_)
            | ExprKind::ArrayLit(_)
            | ExprKind::Builtin(Builtin::ArraysCopyOf | Builtin::Arrays(ArraysOp::CopyOfRange), _) => {
                self.class.element_classes.get(&e.id).copied()
            }
            ExprKind::Builtin(Builtin::ObjectArrayOf | Builtin::MainArgs, _) => {
                Some(Type::array(Type::String))
            }
            _ => None,
        }
    }

    /// `code`, a new `Object[]`, as an array of class `class` (`java::tagged`): one that carries
    /// `String[]`, or none for `Object[]`.
    pub(super) fn with_element_class(&mut self, class: Type, code: Code) -> Code {
        let tagged = self.runtime("tagged");
        let object = Type::class(OBJECT);
        if class == Type::array(object) {
            return Code::atom(format!("{tagged}({}, None)", code.text));
        }
        let Some(t) = self.enum_type(object) else { return code };
        self.need(Need::RuntimeTypeNames(t));
        let names = match self.nullable_element(object) {
            true => "|v| v.as_ref().map(|o| o.runtime_type_names())",
            false => "|v| Some(v.runtime_type_names())",
        };
        let element = class.element();
        let name = match self.class.type_of(element) {
            Some(c) => self.decl_names(c).swap_remove(0),
            None => self.type_names(element).swap_remove(0),
        };
        let item = self.element_ty_name(object);
        let made = self.type_class(class);
        let element_class = self.runtime("ElementClass");
        Code::atom(format!(
            "{tagged}({}, Some({element_class}::<{item}>::new(|| {made}, {}, {names})))",
            code.text,
            rust_string(&name)
        ))
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
            Type::Class(name, _) if self.class.annotation_classes.contains(name) => {
                let simple = name.rsplit('.').next().unwrap_or(name);
                format!("java::Class::annotation({}, {})", rust_string(name), rust_string(simple))
            }
            _ => match self.class.type_of(ty) {
                Some(c) => self.decl_class(c),
                None => constant("OBJECT"),
            },
        }
    }

    /// The `java::Class` of class `c` of the program, or of `Object` and the runtime classes.
    pub(super) fn decl_class(&mut self, c: usize) -> String {
        let decl = &self.class.types[c];
        if let Some(ty) = runtime_type(&decl.name) {
            return self.type_class(ty);
        }
        // An array held as an `Object` (`char[]`): `[C`, with its component type.
        if let Some(ty) = self.class.array_of(c) {
            return self.type_class(ty);
        }
        match decl.name.as_str() {
            OBJECT => return "java::Class::OBJECT".to_string(),
            COMPARABLE => return "java::Class::COMPARABLE".to_string(),
            ENUM => return "java::Class::ENUM".to_string(),
            NUMBER => return "java::Class::NUMBER".to_string(),
            CHAR_SEQUENCE => return "java::Class::CHAR_SEQUENCE".to_string(),
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
            ClassOp::ToString => {
                return Code::atom(format!("{}::from({class}.to_string())", self.runtime("JString")));
            }
            ClassOp::HashCode => "hash_code",
            ClassOp::GetCanonicalName => "get_canonical_name",
            ClassOp::GetPackageName => "get_package_name",
            ClassOp::DescriptorString => "descriptor_string",
            ClassOp::IsAnnotation => "is_annotation",
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

    /// `ClassIsInstance(class, x)`, or with `check` the statement `ClassCheck(class, x)`: the
    /// runtime's test of the binary names of the classes and interfaces of `x`'s class
    /// (`instance_names`) against the class's.
    pub(super) fn class_instance(&mut self, check: bool, args: &'a [Expr]) -> Code {
        let class = if self.nullable(&args[0]) {
            format!("{}({})", self.runtime("non_null"), self.expr(&args[0]).text)
        } else {
            self.expr(&args[0]).at(ATOM)
        };
        let names = self.instance_names(&args[1]);
        let method = if check { "check_cast" } else { "is_instance" };
        Code::atom(format!("{class}.{method}({names})"))
    }

    /// The binary names of the class of the value of `x` and of every class and interface above
    /// it, the class's own first, as an `Option<&[&str]>` that is `None` when `x` is null. For a
    /// value of an enum of classes, a `match` on its variant.
    fn instance_names(&mut self, x: &'a Expr) -> String {
        if matches!(x.kind, ExprKind::Null) || x.ty == Type::Null {
            return "None".to_string();
        }
        let list = |names: Vec<String>| {
            let names: Vec<String> = names.iter().map(|n| rust_string(n)).collect();
            format!("&[{}][..]", names.join(", "))
        };
        let nullable = self.nullable(x);
        if let Some(t) = self.enum_of(x) {
            self.need(Need::RuntimeTypeNames(t));
            if nullable {
                // A value of the enum that may be null is an `Option<&..>`.
                let value = self.enum_option(x);
                return format!("{}.map(|o| o.runtime_type_names())", Code::new(value, LOWEST).left_at(ATOM));
            }
            let value = self.expr(x).at(ATOM);
            return format!("Some({})", method_on(&value, "runtime_type_names"));
        }
        let names = match x.ty {
            Type::Class(..) => match self.struct_class(x) {
                Some(c) => self.decl_names(c),
                None => vec!["java.lang.Object".to_string()],
            },
            ty => self.type_names(ty),
        };
        let names = list(names);
        if nullable {
            let option = self.option_ref(x);
            return format!("{option}.map(|_| {names})");
        }
        if has_effects(x) {
            let text = self.expr(x).text;
            return format!("{{ let _ = {text}; Some({names}) }}");
        }
        format!("Some({names})")
    }

    /// `decl_names(c)` as a slice: `&["Main$Dog", .., "java.lang.Object"]`.
    pub(super) fn names_list(&mut self, c: usize) -> String {
        let names: Vec<String> = self.decl_names(c).iter().map(|n| rust_string(n)).collect();
        format!("&[{}]", names.join(", "))
    }

    /// The binary names of class `c` of the program, or of a runtime class, and of the classes
    /// and interfaces above it, its own first.
    fn decl_names(&mut self, c: usize) -> Vec<String> {
        if let Some(ty) = self.runtime_of(c) {
            return self.type_names(ty);
        }
        let mut names: Vec<String> = Vec::new();
        let mut queue = std::collections::VecDeque::from([c]);
        let mut seen = std::collections::HashSet::new();
        while let Some(d) = queue.pop_front() {
            if !seen.insert(d) {
                continue;
            }
            let decl = &self.class.types[d];
            queue.extend(decl.extends);
            queue.extend(decl.implements.iter().copied());
            let name = match decl.name.as_str() {
                OBJECT => "java.lang.Object".to_string(),
                COMPARABLE => "java.lang.Comparable".to_string(),
                ENUM => "java.lang.Enum".to_string(),
                NUMBER => "java.lang.Number".to_string(),
                CHAR_SEQUENCE => "java.lang.CharSequence".to_string(),
                _ => match runtime_type(&decl.name) {
                    Some(ty) => class_name(self, ty),
                    None => self.java_class_name(d, false),
                },
            };
            if !names.contains(&name) {
                names.push(name);
            }
        }
        if self.class.types[c].kind == TypeKind::Enum {
            names.extend(
                [
                    "java.lang.Enum",
                    "java.lang.Comparable",
                    "java.io.Serializable",
                    "java.lang.constant.Constable",
                ]
                .map(String::from),
            );
        }
        names.push("java.lang.Object".to_string());
        let mut unique = Vec::new();
        for name in names {
            if !unique.contains(&name) {
                unique.push(name);
            }
        }
        unique
    }

    /// The binary names of the class of the values of type `ty` (a primitive is its box), and of
    /// the classes and interfaces above it, its own first.
    fn type_names(&mut self, ty: Type) -> Vec<String> {
        let list = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<Vec<_>>();
        const SERIAL: &str = "java.io.Serializable";
        const COMPARE: &str = "java.lang.Comparable";
        const CONSTABLE: &str = "java.lang.constant.Constable";
        const DESC: &str = "java.lang.constant.ConstantDesc";
        const NUM: &str = "java.lang.Number";
        let ty =
            if ty.is_numeric() || matches!(ty, Type::Boolean | Type::Char) { Type::boxed(ty) } else { ty };
        let mut names = match ty {
            Type::String => {
                list(&["java.lang.String", SERIAL, COMPARE, "java.lang.CharSequence", CONSTABLE, DESC])
            }
            Type::StringBuilder => list(&[
                "java.lang.StringBuilder",
                "java.lang.AbstractStringBuilder",
                SERIAL,
                COMPARE,
                "java.lang.CharSequence",
                "java.lang.Appendable",
            ]),
            Type::JavaClass => list(&[
                "java.lang.Class",
                SERIAL,
                "java.lang.reflect.GenericDeclaration",
                "java.lang.reflect.Type",
                "java.lang.reflect.AnnotatedElement",
                "java.lang.invoke.TypeDescriptor$OfField",
                "java.lang.invoke.TypeDescriptor",
                CONSTABLE,
            ]),
            Type::Boxed(primitive) => {
                let own = class_name(self, ty);
                match *primitive {
                    Type::Int | Type::Long | Type::Float | Type::Double => {
                        list(&[&own, NUM, SERIAL, COMPARE, CONSTABLE, DESC])
                    }
                    Type::Short | Type::Byte => list(&[&own, NUM, SERIAL, COMPARE, CONSTABLE]),
                    _ => list(&[&own, SERIAL, COMPARE, CONSTABLE]),
                }
            }
            // An array is of its type, and of the arrays of the types above its elements'.
            Type::Array(element) => {
                let mut names = vec![array_name(self, *element)];
                if element.is_reference() || matches!(*element, Type::Enum(_)) {
                    for name in self.type_names(*element) {
                        let name =
                            if name.starts_with('[') { format!("[{name}") } else { format!("[L{name};") };
                        if !names.contains(&name) {
                            names.push(name);
                        }
                    }
                }
                names.extend(list(&["java.lang.Cloneable", SERIAL]));
                names
            }
            _ => match self.class.type_of(ty) {
                Some(c) => return self.decl_names(c),
                None => Vec::new(),
            },
        };
        names.push("java.lang.Object".to_string());
        names
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
pub(super) fn array_name(g: &Gen, element: Type) -> String {
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
fn class_name(g: &Gen, ty: Type) -> String {
    match ty {
        Type::String => "java.lang.String".to_string(),
        Type::StringBuilder => "java.lang.StringBuilder".to_string(),
        Type::JavaClass => "java.lang.Class".to_string(),
        Type::Boxed(_) => runtime_name(ty).unwrap_or("java.lang.Object").to_string(),
        _ => match g.class.type_of(ty) {
            Some(c) if g.class.types[c].name == OBJECT => "java.lang.Object".to_string(),
            Some(c) if g.class.types[c].name == COMPARABLE => "java.lang.Comparable".to_string(),
            Some(c) if g.class.types[c].name == ENUM => "java.lang.Enum".to_string(),
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
