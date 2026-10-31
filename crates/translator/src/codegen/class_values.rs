//! Classes as values (see `lower::classes`): a `java::Class`, with its names, a reference to its
//! superclass (or a function for an array's component type) and the binary names of the classes
//! and interfaces above it (`java::Class::new("Main$Dog", "Dog", Some(&CLASS__MAIN_ANIMAL),
//! &["Main$Dog", "Main$Animal", "java.lang.Object"])`). The class of the program that code names
//! is a static at the crate root (`static CLASS__MAIN_DOG: java::Class = ..;`), written once
//! however many places name it, as the reflection tables name a class in each of its methods; the
//! runtime's classes and arrays are written out where they are used. A static rather than a
//! constant, so that classes can name each other in a loop and no function is written to name
//! one: rustc checks and compiles each function body, and the Minecraft server has 40,000 classes. For a value of an enum of classes, the enum's
//! `runtime_class()` and `runtime_type_names()` read tables by class number, with a `match` for
//! the variants that carry their class
//! (`dispatch::class_value_impls`).

use super::*;

/// The static at the crate root that holds the binary names above each class by its class
/// number (`Gen::type_names_table`).
pub(super) const TYPE_NAMES: &str = "TYPE_NAMES_BY_CLASS_NUMBER";

/// The static at the crate root that holds each class by its class number (`Gen::classes_table`).
pub(super) const CLASSES: &str = "CLASSES_BY_CLASS_NUMBER";

/// `match obj.view() { arms }` over the variants of an enum of classes, whose arms match the
/// view's variants (`dispatch::view_pattern`). With no arms, for an interface that no created class
/// implements, `obj` has no value to match, so it is run and then the code is unreachable.
pub(super) fn variant_match(obj: &str, arms: &[String]) -> String {
    match arms.is_empty() {
        true => format!("{{ let _ = {obj}; unreachable!() }}"),
        false => format!("match {} {{ {} }}", method_on(obj, "view"), arms.join(", ")),
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
        self.class.makes_covariant_arrays() && ty == Type::array(Type::class(OBJECT))
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

    /// `code`, a new `Object[]`, as an array of the class of the array `like` holds, a variable or
    /// a field (`Class::array_likes`, `java::tagged_like`).
    pub(super) fn with_class_of(&mut self, like: &'a Expr, code: Code) -> Code {
        let like = match self.nullable(like) {
            true => format!("{}.as_ref()", self.object_place(like)),
            false => format!("Some({})", self.handle(like)),
        };
        Code::atom(format!("{}({}, {like})", self.runtime("tagged_like"), code.text))
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
        let module = self.type_module(class);
        let element_class = self.runtime("ElementClass");
        let own: Vec<String> = self.type_names(class).iter().map(|n| rust_string(n)).collect();
        Code::atom(format!(
            "{tagged}({}, Some({element_class}::<{item}>::new(|| {made}, {}, {module}, {names}, &[{}])))",
            code.text,
            rust_string(&name),
            own.join(", ")
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
        let mut value = value;
        match self.enum_of(x) {
            Some(t) => {
                // The class of a value of an enum is its tag.
                value = if nullable {
                    format!("{value}.map(|o| o.0.tag())")
                } else {
                    format!("{}.0.tag()", super::dispatch::operand(&value))
                };
                for c in self.enum_variants(t) {
                    if comparable(self, c) {
                        let pattern = some(self.class_tag(c).to_string());
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
            Type::Class("java.lang.Runtime", _) => constant("RUNTIME"),
            Type::Class("java.lang.System", _) => constant("SYSTEM"),
            Type::Class(name, _) if self.class.annotation_classes.contains_key(name) => {
                let binary = &self.class.annotation_classes[name];
                let simple = name.rsplit('.').next().unwrap_or(name);
                let names = format!(
                    "&[{}, \"java.lang.annotation.Annotation\", \"java.lang.Object\"]",
                    rust_string(binary)
                );
                format!("java::Class::annotation({}, {}, {names})", rust_string(binary), rust_string(simple))
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
            RECORD => return "java::Class::RECORD".to_string(),
            _ => {}
        }
        self.class_values.insert(c);
        format!("crate::{}", self.class_value_name(c))
    }

    /// The name of the static at the crate root that holds the `java::Class` of class `c` of the
    /// program: `CLASS__` and its name in capitals, with a number after it where two names are
    /// the same, made for every class at once so that each keeps its name.
    fn class_value_name(&mut self, c: usize) -> String {
        if self.class_value_names.is_empty() {
            let mut taken = HashSet::new();
            self.class_value_names = (self.class.types.iter())
                .map(|t| {
                    let upper: String = (t.name.chars())
                        .map(|ch| if ch.is_ascii_alphanumeric() { ch.to_ascii_uppercase() } else { '_' })
                        .collect();
                    let base = format!("CLASS__{upper}");
                    let mut name = base.clone();
                    for n in 2.. {
                        if taken.insert(name.clone()) {
                            break;
                        }
                        name = format!("{base}_{n}");
                    }
                    name
                })
                .collect();
        }
        self.class_value_names[c].clone()
    }

    /// The statics of the classes that the program's code names (`decl_class`), and of their
    /// superclasses, in the order of their names: the items of the crate root that hold them.
    pub(super) fn class_value_constants(&mut self) -> (String, std::collections::BTreeSet<String>) {
        let mut constants = std::collections::BTreeMap::new();
        let mut done = std::collections::BTreeSet::new();
        loop {
            let named: Vec<usize> = self.class_values.difference(&done).copied().collect();
            if named.is_empty() {
                break;
            }
            for c in named {
                done.insert(c);
                let value = self.class_value(c);
                constants.insert(self.class_value_name(c), value);
            }
        }
        let text = constants
            .iter()
            .map(|(name, value)| format!("static {name}: java::Class = {value};\n"))
            .collect();
        (text, done.iter().map(|&c| self.class.types[c].name.clone()).collect())
    }

    /// The `java::Class` of class `c` of the program, written out.
    fn class_value(&mut self, c: usize) -> String {
        let decl = &self.class.types[c];
        let (kind, extends) = (decl.kind, decl.extends);
        // A record's superclass is `Record`, which the program's records implement where it is a
        // type.
        let record = decl.implements.iter().any(|&i| self.class.types[i].name == RECORD);
        let name = rust_string(&self.java_class_name(c, false));
        let simple = rust_string(&self.java_class_name(c, true));
        let names = self.names_list(c);
        let value = match kind {
            TypeKind::Interface => format!("java::Class::interface({name}, {simple}, {names})"),
            TypeKind::Enum => format!("java::Class::enumeration({name}, {simple}, {names})"),
            _ => {
                let superclass = match extends {
                    Some(s) => self.decl_class(s),
                    None if record => "java::Class::RECORD".to_string(),
                    None => "java::Class::OBJECT".to_string(),
                };
                format!("java::Class::new({name}, {simple}, Some(&{superclass}), {names})")
            }
        };
        let value = match self.class_interfaces {
            true => format!("{value}.implementing(&[{}])", self.interface_values(c)),
            false => value,
        };
        // A class that no class of the program extends has instances of exactly itself, which
        // `isInstance` and `cast` test by one name. Not a `Throwable`: the runtime makes
        // exceptions by name, of classes the translation may have no type of.
        let throwable = self.class.type_index("java.lang.Throwable");
        let leaf = matches!(kind, TypeKind::Class | TypeKind::Enum)
            && !self.class.hierarchy.has_subtypes(c)
            && throwable.is_none_or(|t| t != c && !self.class.hierarchy.ancestors(c).contains(&t));
        let value = match leaf {
            true => format!("{value}.numbered({})", self.class_tag(c)),
            false => value,
        };
        if !self.class_details {
            return value;
        }
        // Its modifiers and nesting, which only a program that asks for them carries.
        let decl = &self.class.types[c];
        let (modifiers, enclosing) = (decl.modifiers, decl.enclosing);
        let nest = match (&decl.origin, enclosing) {
            (Origin::Anonymous { .. }, _) => "Anonymous",
            (Origin::Local { .. }, _) => "Local",
            (Origin::Member, Some(_)) => "Member",
            _ => "TopLevel",
        };
        let enclosing = match enclosing {
            Some(e) => format!("Some(&{})", self.decl_class(e)),
            None => "None".to_string(),
        };
        format!("{value}.declared({modifiers:#x}, java::Nest::{nest}, {enclosing})")
    }

    /// References to the class values of the interfaces that class `c` implements directly, which
    /// only a program that asks for them carries: an interface the translation has no type of is a
    /// class value of its name that extends nothing.
    fn interface_values(&mut self, c: usize) -> String {
        let mut values = Vec::new();
        for interface in self.class.types[c].interfaces.clone() {
            values.push(match interface {
                Interface::Type(i) => self.decl_class(i),
                Interface::Named(name) if name == COMPARABLE => "java::Class::COMPARABLE".to_string(),
                Interface::Named(name) if name == CHAR_SEQUENCE => "java::Class::CHAR_SEQUENCE".to_string(),
                Interface::Named(name) => {
                    let simple = name.rsplit('.').next().unwrap_or(&name);
                    let (name, simple) = (rust_string(&name), rust_string(simple));
                    format!("java::Class::interface({name}, {simple}, &[{name}, {}])", rust_string(OBJECT))
                }
            });
        }
        values.iter().map(|value| format!("&{value}")).collect::<Vec<_>>().join(", ")
    }

    /// `ClassMethod(op)(class, args..)`.
    pub(super) fn class_method(&mut self, op: ClassOp, args: &'a [Expr]) -> Code {
        if op == ClassOp::IsAssignableFrom {
            return self.class_assignable(&args[0], &args[1]);
        }
        // Of a null class, the checked form, which a `catch` sees, as `getInterfaces()` is a
        // call of the program's code.
        if op == ClassOp::InterfaceCount && self.nullable(&args[0]) {
            self.class_interfaces = true;
            let class = self.class_option(&args[0]);
            let checked = self.dereference(&[args[0].id], "class_");
            return Code::new(
                format!(
                    "{{ let class_ = {class}; {}.get_interface_count() }}",
                    Code::new(checked, LOWEST).at(ATOM)
                ),
                LOWEST,
            );
        }
        let class = if self.nullable(&args[0]) {
            format!("{}({})", self.runtime("non_null"), self.expr(&args[0]).text)
        } else {
            self.expr(&args[0]).at(ATOM)
        };
        let method = match op {
            ClassOp::GetName => "get_name",
            ClassOp::GetSimpleName => "get_simple_name",
            ClassOp::GetTypeName if super::shared_strings() => "shared_type_name",
            ClassOp::GetTypeName => "get_type_name",
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
            ClassOp::GetModifiers => {
                self.class_details = true;
                "get_modifiers"
            }
            ClassOp::IsAnonymousClass | ClassOp::IsLocalClass | ClassOp::IsMemberClass => {
                self.class_details = true;
                match op {
                    ClassOp::IsAnonymousClass => "is_anonymous_class",
                    ClassOp::IsLocalClass => "is_local_class",
                    _ => "is_member_class",
                }
            }
            ClassOp::InterfaceCount => {
                self.class_interfaces = true;
                "get_interface_count"
            }
            ClassOp::InterfaceAt => {
                self.class_interfaces = true;
                let index = self.value(&args[1], Type::Int).text;
                return Code::atom(format!("{class}.get_interface({index})"));
            }
            ClassOp::GetEnclosingClass | ClassOp::GetDeclaringClass => {
                self.class_details = true;
                if op == ClassOp::GetEnclosingClass { "get_enclosing_class" } else { "get_declaring_class" }
            }
            ClassOp::DesiredAssertionStatus => {
                return Code::new(format!("{{ let _ = {class}; false }}"), LOWEST);
            }
            ClassOp::IsAssignableFrom => unreachable!("`class_assignable` writes it"),
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

    /// `ClassMethod(IsAssignableFrom)(class, other)`: both evaluated, then a null class and then
    /// a null `other` throw `NullPointerException`, as Java's call does, in the checked form
    /// where a `catch` sees it (`dereference`).
    fn class_assignable(&mut self, class: &'a Expr, other: &'a Expr) -> Code {
        let optional = |g: &Self, e: &Expr| g.nullable(e) || matches!(e.kind, ExprKind::Null);
        let (class_optional, other_optional) = (optional(self, class), optional(self, other));
        if !class_optional && !other_optional {
            let (class, other) = (self.expr(class).at(ATOM), self.expr(other).text);
            return Code::atom(format!("{class}.is_assignable_from({other})"));
        }
        let mut value = |e: &'a Expr, optional: bool| match optional {
            true => self.class_option(e),
            false => self.expr(e).text,
        };
        let (class_value, other_value) = (value(class, class_optional), value(other, other_optional));
        let checked = |g: &mut Self, e: &Expr, optional: bool, name: &str| match optional {
            true => g.dereference(&[e.id], name),
            false => name.to_string(),
        };
        let class_text = checked(self, class, class_optional, "class_");
        let other_text = checked(self, other, other_optional, "other_");
        Code::new(
            format!(
                "{{ let (class_, other_) = ({class_value}, {other_value}); {}.is_assignable_from({other_text}) }}",
                Code::new(class_text, LOWEST).at(ATOM)
            ),
            LOWEST,
        )
    }

    /// `ClassIsInstance(class, x)`, or with `check` the statement `ClassCheck(class, x)`: the
    /// runtime's test of the binary names of the classes and interfaces of `x`'s class
    /// (`instance_names`) against the class's, or of the class numbers where `x`'s value has one.
    pub(super) fn class_instance(&mut self, check: bool, args: &'a [Expr]) -> Code {
        let method = if check { "check_cast" } else { "is_instance" };
        // A value of an enum of classes that is not a variable, such as an array's element, is
        // evaluated once, after the class and before a null class throws, as in Java, and both
        // its number and its names are read from it.
        let x = &args[1];
        if let Some(t) = self.enum_of(x)
            && !matches!(x.kind, ExprKind::Var(_) | ExprKind::This | ExprKind::Null)
            && x.ty != Type::Null
        {
            let checked = match self.nullable(&args[0]) {
                true => format!("{}(class_)", self.runtime("non_null")),
                false => "class_".to_string(),
            };
            let class = self.expr(&args[0]).text;
            self.need(Need::RuntimeTypeNames(t));
            let value = self.enum_option(x);
            return Code::new(
                format!(
                    "match ({class}, {value}) {{ (class_, object_) => {checked}.{method}_numbered(object_.map(|o| \
                     o.0.tag()), || object_.map(|o| o.runtime_type_names())) }}"
                ),
                LOWEST,
            );
        }
        let class = if self.nullable(&args[0]) {
            format!("{}({})", self.runtime("non_null"), self.expr(&args[0]).text)
        } else {
            self.expr(&args[0]).at(ATOM)
        };
        let names = self.instance_names(x);
        match self.instance_number(x) {
            Some(number) => Code::atom(format!("{class}.{method}_numbered({number}, || {names})")),
            None => Code::atom(format!("{class}.{method}({names})")),
        }
    }

    /// The class number of the value of `x`, as an `Option<u16>` that is `None` when `x` is null,
    /// where the value carries it (a value of an enum of classes) or its class has no subclass;
    /// `None` for any other `x`, and for an `x` that is not a variable, which the names read again.
    fn instance_number(&mut self, x: &'a Expr) -> Option<String> {
        if !matches!(x.kind, ExprKind::Var(_) | ExprKind::This) || x.ty == Type::Null {
            return None;
        }
        let nullable = self.nullable(x);
        if self.enum_of(x).is_some() {
            return Some(match nullable {
                true => {
                    format!("{}.map(|o| o.0.tag())", Code::new(self.enum_option(x), LOWEST).left_at(ATOM))
                }
                false => format!("Some({}.0.tag())", super::dispatch::operand(&self.expr(x).at(ATOM))),
            });
        }
        let c = self.class.type_of(x.ty).filter(|_| matches!(x.ty, Type::Class(..) | Type::Enum(_)))?;
        let decl = &self.class.types[c];
        if !matches!(decl.kind, TypeKind::Class | TypeKind::Enum) || self.class.hierarchy.has_subtypes(c) {
            return None;
        }
        let tag = self.class_tag(c);
        Some(match nullable {
            true => format!("{}.as_ref().map(|_| {tag}u16)", self.expr(x).at(ATOM)),
            false => format!("Some({tag}u16)"),
        })
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

    /// Whether an object of class `c` carries the names of its class, as an `Object[]` that may be
    /// a covariant array does (`java::array_type_names`), which a store of it into an array of
    /// arrays tests: they are not those of `c`.
    pub(super) fn names_carried(&self, c: usize) -> bool {
        self.class.array_of(c).is_some_and(|ty| self.carries_class(ty))
    }

    /// The static `TYPE_NAMES` at the crate root, if an enum of classes reads it: each class's
    /// `names_list` at its class number, for the classes of those enums, whose
    /// `runtime_type_names()` reads it (`dispatch::class_value_impls`). One read by number, where a
    /// `match` over the thousand classes that implement `Comparable` is a thousand compares
    /// without optimizations and a search with them.
    pub(super) fn type_names_table(&mut self) -> String {
        let enums: Vec<usize> = self
            .wanted
            .iter()
            .filter_map(|need| match need {
                Need::RuntimeTypeNames(t) => Some(*t),
                _ => None,
            })
            .collect();
        let mut by_number = std::collections::BTreeMap::new();
        for t in enums {
            for c in self.enum_variants(t) {
                if !self.names_carried(c) {
                    by_number.insert(self.class_tag(c), c);
                }
            }
        }
        let Some((&last, _)) = by_number.last_key_value() else { return String::new() };
        let mut entries = vec!["&[]".to_string(); usize::from(last) + 1];
        for (number, c) in by_number {
            entries[usize::from(number)] = self.names_list(c);
        }
        format!(
            "static {TYPE_NAMES}: [&[&str]; {}] = [\n{}];\n",
            entries.len(),
            entries.iter().map(|e| format!("    {e},\n")).collect::<String>()
        )
    }

    /// The static `CLASSES` at the crate root, if an enum of classes reads it: a reference to each
    /// class at its class number, for the classes of those enums, whose `runtime_class()` reads it
    /// (`dispatch::class_value_impls`). One read by number, where a `match` over the 20,000
    /// classes of the Minecraft server's `Object` was cut into parts, two calls in another crate.
    pub(super) fn classes_table(&mut self) -> String {
        let enums: Vec<usize> = self
            .wanted
            .iter()
            .filter_map(|need| match need {
                Need::RuntimeClass(t) => Some(*t),
                _ => None,
            })
            .collect();
        let mut by_number = std::collections::BTreeMap::new();
        for t in enums {
            for c in self.enum_variants(t) {
                if !self.class_carried(c) {
                    by_number.insert(self.class_tag(c), c);
                }
            }
        }
        let Some((&last, _)) = by_number.last_key_value() else { return String::new() };
        let mut entries = vec!["&java::Class::OBJECT".to_string(); usize::from(last) + 1];
        for (number, c) in by_number {
            entries[usize::from(number)] = format!("&{}", self.decl_class(c));
        }
        format!(
            "static {CLASSES}: [&java::Class; {}] = [\n{}];\n",
            entries.len(),
            entries.iter().map(|e| format!("    {e},\n")).collect::<String>()
        )
    }

    /// Whether an object of class `c` carries its class, as an `Object[]` that may be a covariant
    /// array does (`java::array_class`), which `runtime_class()` then asks it for.
    pub(super) fn class_carried(&self, c: usize) -> bool {
        self.class.array_of(c).is_some_and(|ty| self.carries_class(ty))
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
                RECORD => "java.lang.Record".to_string(),
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
            // A charset of an `Object` is of the runtime's one class of charsets, not of the
            // JDK's class of that charset (`sun.nio.cs.UTF_8`).
            Type::Charset => list(&["java.nio.charset.Charset", COMPARE]),
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
            Some(c) if g.class.types[c].name == RECORD => "java.lang.Record".to_string(),
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
