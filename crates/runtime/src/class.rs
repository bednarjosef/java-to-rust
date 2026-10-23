//! `java.lang.Class`, as far as a program uses it without reflection: the class of a value with
//! its names, its superclass and its component type. Two classes are the same when their binary
//! names are, since no two classes of a program share one; the translator writes each class it
//! needs where it is used, so a class names its superclass and component type by a function.

use std::fmt;
use std::hash::{Hash, Hasher};

/// What a class is, which its `toString` shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Class,
    Enum,
    Interface,
    /// An annotation type (`@interface`), an interface.
    Annotation,
    Primitive,
}

/// A `java.lang.Class`.
#[derive(Clone, Copy)]
pub struct Class {
    /// The binary name, as `getName` gives it: `java.lang.String`, `Main$Point`, `[I`.
    name: &'static str,
    /// The name in the source, as `getSimpleName` gives it: `String`, `Point`, `int[]`.
    simple: &'static str,
    kind: Kind,
    superclass: Option<fn() -> Class>,
    component: Option<fn() -> Class>,
}

impl Class {
    /// A class, with its superclass (`None` only for `Object`).
    pub const fn new(name: &'static str, simple: &'static str, superclass: Option<fn() -> Class>) -> Class {
        Class { name, simple, kind: Kind::Class, superclass, component: None }
    }

    /// An enum, whose superclass is `Enum`.
    pub const fn enumeration(name: &'static str, simple: &'static str) -> Class {
        Class { name, simple, kind: Kind::Enum, superclass: Some(|| Class::ENUM), component: None }
    }

    /// An interface, which has no superclass.
    pub const fn interface(name: &'static str, simple: &'static str) -> Class {
        Class { name, simple, kind: Kind::Interface, superclass: None, component: None }
    }

    /// An annotation type, an interface that `isAnnotation()`.
    pub const fn annotation(name: &'static str, simple: &'static str) -> Class {
        Class { name, simple, kind: Kind::Annotation, superclass: None, component: None }
    }

    /// An array class, `[Ljava.lang.String;`, whose superclass is `Object`.
    pub const fn array(name: &'static str, simple: &'static str, component: fn() -> Class) -> Class {
        Class {
            name,
            simple,
            kind: Kind::Class,
            superclass: Some(|| Class::OBJECT),
            component: Some(component),
        }
    }

    const fn primitive(name: &'static str) -> Class {
        Class { name, simple: name, kind: Kind::Primitive, superclass: None, component: None }
    }

    const fn standard(name: &'static str, simple: &'static str) -> Class {
        Class::new(name, simple, Some(|| Class::OBJECT))
    }

    pub const OBJECT: Class = Class::new("java.lang.Object", "Object", None);
    pub const STRING: Class = Class::standard("java.lang.String", "String");
    pub const STRING_BUILDER: Class = Class::standard("java.lang.StringBuilder", "StringBuilder");
    pub const CLASS: Class = Class::standard("java.lang.Class", "Class");
    pub const ENUM: Class = Class::standard("java.lang.Enum", "Enum");
    pub const COMPARABLE: Class = Class::interface("java.lang.Comparable", "Comparable");
    pub const CHAR_SEQUENCE: Class = Class::interface("java.lang.CharSequence", "CharSequence");
    pub const INTEGER: Class = Class::new("java.lang.Integer", "Integer", Some(|| Class::NUMBER));
    pub const LONG: Class = Class::new("java.lang.Long", "Long", Some(|| Class::NUMBER));
    pub const DOUBLE: Class = Class::new("java.lang.Double", "Double", Some(|| Class::NUMBER));
    pub const FLOAT: Class = Class::new("java.lang.Float", "Float", Some(|| Class::NUMBER));
    pub const SHORT: Class = Class::new("java.lang.Short", "Short", Some(|| Class::NUMBER));
    pub const BYTE: Class = Class::new("java.lang.Byte", "Byte", Some(|| Class::NUMBER));
    pub const CHARACTER: Class = Class::standard("java.lang.Character", "Character");
    pub const BOOLEAN: Class = Class::standard("java.lang.Boolean", "Boolean");
    pub const NUMBER: Class = Class::standard("java.lang.Number", "Number");
    pub const INT: Class = Class::primitive("int");
    pub const LONG_PRIMITIVE: Class = Class::primitive("long");
    pub const DOUBLE_PRIMITIVE: Class = Class::primitive("double");
    pub const FLOAT_PRIMITIVE: Class = Class::primitive("float");
    pub const SHORT_PRIMITIVE: Class = Class::primitive("short");
    pub const BYTE_PRIMITIVE: Class = Class::primitive("byte");
    pub const CHAR: Class = Class::primitive("char");
    pub const BOOLEAN_PRIMITIVE: Class = Class::primitive("boolean");
    pub const VOID: Class = Class::primitive("void");

    /// `getName()`.
    pub fn get_name(&self) -> crate::JString {
        self.name.into()
    }

    /// `getSimpleName()`.
    pub fn get_simple_name(&self) -> crate::JString {
        self.simple.into()
    }

    /// `getCanonicalName()`: the name in the source with its outer classes (`a.Outer.Inner`,
    /// `int[]`), `null` for a local or anonymous class, a class inside one, and an array of one.
    /// javac names a local class `Outer$1Local` and an anonymous one `Outer$1`, with an empty
    /// simple name, which tells them apart from member classes.
    pub fn get_canonical_name(&self) -> Option<crate::JString> {
        self.canonical_name().map(crate::JString::from)
    }

    fn canonical_name(&self) -> Option<String> {
        if let Some(component) = self.get_component_type() {
            return component.canonical_name().map(|name| format!("{name}[]"));
        }
        if self.kind == Kind::Primitive {
            return Some(self.name.to_string());
        }
        let mut segments = self.name.split('$');
        let first = segments.next().unwrap_or_default();
        let nested: Vec<&str> = segments.collect();
        let local = nested.iter().any(|s| s.is_empty() || s.starts_with(|c: char| c.is_ascii_digit()));
        if self.simple.is_empty() || local || nested.last().is_some_and(|last| *last != self.simple) {
            return None;
        }
        Some(std::iter::once(first).chain(nested).collect::<Vec<_>>().join("."))
    }

    /// `descriptorString()`: the class's type descriptor, `I`, `[I` or `Ljava/lang/String;`.
    pub fn descriptor_string(&self) -> crate::JString {
        let text = if self.kind == Kind::Primitive {
            match self.name {
                "boolean" => "Z".to_string(),
                "long" => "J".to_string(),
                name => name[..1].to_uppercase(),
            }
        } else if self.component.is_some() {
            self.name.replace('.', "/")
        } else {
            format!("L{};", self.name.replace('.', "/"))
        };
        text.into()
    }

    /// `getPackageName()`: the package of the class, of an array's element class, `java.lang`
    /// for a primitive type and `""` for the unnamed package.
    pub fn get_package_name(&self) -> crate::JString {
        if let Some(component) = self.get_component_type() {
            return component.get_package_name();
        }
        if self.kind == Kind::Primitive {
            return "java.lang".into();
        }
        self.name.rsplit_once('.').map_or("", |(package, _)| package).into()
    }

    /// `getSuperclass()`: `null` for `Object`, an interface and a primitive type.
    pub fn get_superclass(&self) -> Option<Class> {
        self.superclass.map(|f| f())
    }

    /// `getComponentType()`: the type of an array's elements, `null` for any other class.
    pub fn get_component_type(&self) -> Option<Class> {
        self.component.map(|f| f())
    }

    /// `isArray()`.
    pub fn is_array(&self) -> bool {
        self.component.is_some()
    }

    /// `isInterface()`.
    pub fn is_interface(&self) -> bool {
        matches!(self.kind, Kind::Interface | Kind::Annotation)
    }

    /// `isAnnotation()`.
    pub fn is_annotation(&self) -> bool {
        self.kind == Kind::Annotation
    }

    /// `isEnum()`.
    pub fn is_enum(&self) -> bool {
        self.kind == Kind::Enum
    }

    /// `isPrimitive()`.
    pub fn is_primitive(&self) -> bool {
        self.kind == Kind::Primitive
    }

    /// `isInstance(x)`, given the binary names of the class of `x` and of every class and
    /// interface above it (`None` for a null `x`), which the translation knows.
    pub fn is_instance(&self, names: Option<&[&str]>) -> bool {
        names.is_some_and(|names| names.contains(&self.name))
    }

    /// The check of `cast(x)`, given the names as for `is_instance`, the class of `x` first: a
    /// `ClassCastException` unless `x` is null or an instance.
    pub fn check_cast(&self, names: Option<&[&str]>) {
        if let Some(names) = names
            && !names.contains(&self.name)
        {
            let from = names.first().copied().unwrap_or("java.lang.Object");
            crate::throw("java.lang.ClassCastException", &format!("Cannot cast {from} to {}", self.name));
        }
    }

    /// `hashCode()`: Java's is the class object's identity hash, which nothing can depend on; this
    /// one is the hash of its name.
    pub fn hash_code(&self) -> i32 {
        crate::string_hash(crate::Wtf8::new(self.name))
    }
}

impl PartialEq for Class {
    fn eq(&self, other: &Class) -> bool {
        self.name == other.name
    }
}

impl Eq for Class {}

impl Hash for Class {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

/// `toString()`: `class java.lang.String`, `interface java.lang.Comparable`, `int`.
impl fmt::Display for Class {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.kind {
            Kind::Class | Kind::Enum => write!(f, "class {}", self.name),
            Kind::Interface | Kind::Annotation => write!(f, "interface {}", self.name),
            Kind::Primitive => f.write_str(self.name),
        }
    }
}

impl fmt::Debug for Class {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_class_shows_and_compares_as_java_does() {
        let point = Class::new("Main$Point", "Point", Some(|| Class::OBJECT));
        let points = Class::array("[LMain$Point;", "Point[]", || Class::new("Main$Point", "Point", None));
        assert_eq!(point.to_string(), "class Main$Point");
        assert_eq!(Class::COMPARABLE.to_string(), "interface java.lang.Comparable");
        assert_eq!(Class::INT.to_string(), "int");
        assert_eq!(points.get_component_type(), Some(point));
        assert_eq!(points.get_superclass(), Some(Class::OBJECT));
        assert_eq!(Class::OBJECT.get_superclass(), None);
        assert!(points.is_array() && !point.is_array());
        assert_ne!(point, Class::STRING);
    }

    #[test]
    fn canonical_and_package_names_are_javas() {
        let inner = Class::new("a.b.Outer$Inner", "Inner", Some(|| Class::OBJECT));
        let local = Class::new("a.b.Outer$1Local", "Local", Some(|| Class::OBJECT));
        let anonymous = Class::new("a.b.Outer$1", "", Some(|| Class::OBJECT));
        let in_local = Class::new("Outer$1Local$Inner", "Inner", Some(|| Class::OBJECT));
        let inners = Class::array("[La.b.Outer$Inner;", "Inner[]", || {
            Class::new("a.b.Outer$Inner", "Inner", Some(|| Class::OBJECT))
        });
        assert_eq!(inner.get_canonical_name().as_deref(), Some(crate::Wtf8::new("a.b.Outer.Inner")));
        assert_eq!(inners.get_canonical_name().as_deref(), Some(crate::Wtf8::new("a.b.Outer.Inner[]")));
        assert_eq!(local.get_canonical_name(), None);
        assert_eq!(anonymous.get_canonical_name(), None);
        assert_eq!(in_local.get_canonical_name(), None);
        assert_eq!(Class::INT.get_canonical_name().as_deref(), Some(crate::Wtf8::new("int")));
        assert_eq!(inner.get_package_name(), "a.b");
        assert_eq!(inners.get_package_name(), "a.b");
        assert_eq!(in_local.get_package_name(), "");
        assert_eq!(Class::INT.get_package_name(), "java.lang");
        assert_eq!(Class::STRING.get_package_name(), "java.lang");
        assert_eq!(inner.descriptor_string(), "La/b/Outer$Inner;");
        assert_eq!(inners.descriptor_string(), "[La/b/Outer$Inner;");
        assert_eq!(Class::INT.descriptor_string(), "I");
        assert_eq!(Class::BOOLEAN_PRIMITIVE.descriptor_string(), "Z");
        assert_eq!(Class::LONG_PRIMITIVE.descriptor_string(), "J");
    }
}
