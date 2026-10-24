//! `java.lang.Class`, as far as a program uses it without reflection: the class of a value with
//! its names, its superclass, its component type and the binary names of the classes and
//! interfaces above it. Two classes are the same when their binary names are, since no two
//! classes of a program share one; the translator writes each class it needs where it is used, so
//! a class names its superclass and component type by a function.

use std::fmt;
use std::hash::{Hash, Hasher};

const OBJECT: &str = "java.lang.Object";
const SERIAL: &str = "java.io.Serializable";
const COMPARE: &str = "java.lang.Comparable";
const CONSTABLE: &str = "java.lang.constant.Constable";
const DESC: &str = "java.lang.constant.ConstantDesc";
const NUMBER: &str = "java.lang.Number";

/// The binary names of a class and of every class and interface above it, its own first.
type Names = &'static [&'static str];

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
    /// The binary names of the class and of every class and interface above it, its own first,
    /// as `is_instance` takes them; empty for an array and a primitive type, which
    /// `is_assignable_from` answers by their kind.
    names: Names,
}

impl Class {
    /// A class, with its superclass (`None` only for `Object`) and the names above it (`names`).
    pub const fn new(
        name: &'static str,
        simple: &'static str,
        superclass: Option<fn() -> Class>,
        names: Names,
    ) -> Class {
        Class { name, simple, kind: Kind::Class, superclass, component: None, names }
    }

    /// An enum, whose superclass is `Enum`.
    pub const fn enumeration(name: &'static str, simple: &'static str, names: Names) -> Class {
        Class { name, simple, kind: Kind::Enum, superclass: Some(|| Class::ENUM), component: None, names }
    }

    /// An interface, which has no superclass.
    pub const fn interface(name: &'static str, simple: &'static str, names: Names) -> Class {
        Class { name, simple, kind: Kind::Interface, superclass: None, component: None, names }
    }

    /// An annotation type, an interface that `isAnnotation()`.
    pub const fn annotation(name: &'static str, simple: &'static str, names: Names) -> Class {
        Class { name, simple, kind: Kind::Annotation, superclass: None, component: None, names }
    }

    /// An array class, `[Ljava.lang.String;`, whose superclass is `Object`.
    pub const fn array(name: &'static str, simple: &'static str, component: fn() -> Class) -> Class {
        Class {
            name,
            simple,
            kind: Kind::Class,
            superclass: Some(|| Class::OBJECT),
            component: Some(component),
            names: &[],
        }
    }

    const fn primitive(name: &'static str) -> Class {
        Class { name, simple: name, kind: Kind::Primitive, superclass: None, component: None, names: &[] }
    }

    const fn standard(name: &'static str, simple: &'static str, names: Names) -> Class {
        Class::new(name, simple, Some(|| Class::OBJECT), names)
    }

    /// A box of a number, whose superclass is `Number`.
    const fn number(name: &'static str, simple: &'static str, names: Names) -> Class {
        Class::new(name, simple, Some(|| Class::NUMBER), names)
    }

    pub const OBJECT: Class = Class::new(OBJECT, "Object", None, &[OBJECT]);
    pub const STRING: Class = Class::standard(
        "java.lang.String",
        "String",
        &["java.lang.String", SERIAL, COMPARE, "java.lang.CharSequence", CONSTABLE, DESC, OBJECT],
    );
    pub const STRING_BUILDER: Class = Class::standard(
        "java.lang.StringBuilder",
        "StringBuilder",
        &[
            "java.lang.StringBuilder",
            "java.lang.AbstractStringBuilder",
            SERIAL,
            COMPARE,
            "java.lang.CharSequence",
            "java.lang.Appendable",
            OBJECT,
        ],
    );
    pub const CLASS: Class = Class::standard(
        "java.lang.Class",
        "Class",
        &[
            "java.lang.Class",
            SERIAL,
            "java.lang.reflect.GenericDeclaration",
            "java.lang.reflect.Type",
            "java.lang.reflect.AnnotatedElement",
            "java.lang.invoke.TypeDescriptor$OfField",
            "java.lang.invoke.TypeDescriptor",
            CONSTABLE,
            OBJECT,
        ],
    );
    pub const ENUM: Class =
        Class::standard("java.lang.Enum", "Enum", &["java.lang.Enum", COMPARE, SERIAL, CONSTABLE, OBJECT]);
    pub const COMPARABLE: Class = Class::interface(COMPARE, "Comparable", &[COMPARE, OBJECT]);
    pub const CHAR_SEQUENCE: Class =
        Class::interface("java.lang.CharSequence", "CharSequence", &["java.lang.CharSequence", OBJECT]);
    pub const INTEGER: Class = Class::number(
        "java.lang.Integer",
        "Integer",
        &["java.lang.Integer", NUMBER, SERIAL, COMPARE, CONSTABLE, DESC, OBJECT],
    );
    pub const LONG: Class = Class::number(
        "java.lang.Long",
        "Long",
        &["java.lang.Long", NUMBER, SERIAL, COMPARE, CONSTABLE, DESC, OBJECT],
    );
    pub const DOUBLE: Class = Class::number(
        "java.lang.Double",
        "Double",
        &["java.lang.Double", NUMBER, SERIAL, COMPARE, CONSTABLE, DESC, OBJECT],
    );
    pub const FLOAT: Class = Class::number(
        "java.lang.Float",
        "Float",
        &["java.lang.Float", NUMBER, SERIAL, COMPARE, CONSTABLE, DESC, OBJECT],
    );
    pub const SHORT: Class = Class::number(
        "java.lang.Short",
        "Short",
        &["java.lang.Short", NUMBER, SERIAL, COMPARE, CONSTABLE, OBJECT],
    );
    pub const BYTE: Class = Class::number(
        "java.lang.Byte",
        "Byte",
        &["java.lang.Byte", NUMBER, SERIAL, COMPARE, CONSTABLE, OBJECT],
    );
    pub const CHARACTER: Class = Class::standard(
        "java.lang.Character",
        "Character",
        &["java.lang.Character", SERIAL, COMPARE, CONSTABLE, OBJECT],
    );
    pub const BOOLEAN: Class = Class::standard(
        "java.lang.Boolean",
        "Boolean",
        &["java.lang.Boolean", SERIAL, COMPARE, CONSTABLE, OBJECT],
    );
    pub const NUMBER: Class = Class::standard(NUMBER, "Number", &[NUMBER, SERIAL, OBJECT]);
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

    /// `getTypeName()`: for an array, the name of its innermost component type with a `[]` for
    /// each dimension (`int[][]`, `java.lang.String[]`); for any other class, its binary name.
    pub fn get_type_name(&self) -> crate::JString {
        self.type_name().into()
    }

    /// `getTypeName()` where the program's strings are shared: a class's name is the interned
    /// string, an array's a new one, as the JDK makes them.
    pub fn shared_type_name(&self) -> crate::JStr {
        if self.is_array() { crate::JStr::new(self.type_name()) } else { crate::interned(self.name) }
    }

    fn type_name(&self) -> String {
        let mut class = *self;
        let mut dimensions = 0;
        while let Some(component) = class.get_component_type() {
            class = component;
            dimensions += 1;
        }
        format!("{}{}", class.name, "[]".repeat(dimensions))
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

    /// `isAssignableFrom(other)`: whether `other` is this class or a class or interface below it,
    /// by the names above `other`. A primitive type is only itself; every other class is below
    /// `Object`; an array is below `Cloneable` and `Serializable`, and below the arrays of the
    /// classes above its component type where that is not primitive.
    pub fn is_assignable_from(&self, other: Class) -> bool {
        if self.name == other.name {
            return true;
        }
        if self.kind == Kind::Primitive || other.kind == Kind::Primitive {
            return false;
        }
        match (self.get_component_type(), other.get_component_type()) {
            (Some(mine), Some(theirs)) => {
                !mine.is_primitive() && !theirs.is_primitive() && mine.is_assignable_from(theirs)
            }
            (None, Some(_)) => matches!(self.name, OBJECT | "java.lang.Cloneable" | SERIAL),
            (Some(_), None) => false,
            (None, None) => self.name == OBJECT || other.names.contains(&self.name),
        }
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
        let point = Class::new("Main$Point", "Point", Some(|| Class::OBJECT), &["Main$Point", OBJECT]);
        let points =
            Class::array("[LMain$Point;", "Point[]", || Class::new("Main$Point", "Point", None, &[]));
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
    fn assignability_is_javas() {
        let shape = Class::interface("Shape", "Shape", &["Shape", OBJECT]);
        let point = Class::new("Point", "Point", Some(|| Class::OBJECT), &["Point", "Shape", OBJECT]);
        let points = Class::array("[LPoint;", "Point[]", || {
            Class::new("Point", "Point", Some(|| Class::OBJECT), &["Point", "Shape", OBJECT])
        });
        let shapes =
            Class::array("[LShape;", "Shape[]", || Class::interface("Shape", "Shape", &["Shape", OBJECT]));
        let ints = Class::array("[I", "int[]", || Class::INT);
        let longs = Class::array("[J", "long[]", || Class::LONG_PRIMITIVE);
        assert!(shape.is_assignable_from(point) && !point.is_assignable_from(shape));
        assert!(Class::OBJECT.is_assignable_from(shape) && !Class::OBJECT.is_assignable_from(Class::INT));
        assert!(shapes.is_assignable_from(points) && !points.is_assignable_from(shapes));
        assert!(ints.is_assignable_from(ints) && !ints.is_assignable_from(longs));
        assert!(Class::OBJECT.is_assignable_from(ints) && !shape.is_assignable_from(ints));
        assert!(Class::INT.is_assignable_from(Class::INT) && !Class::INTEGER.is_assignable_from(Class::INT));
        assert!(
            Class::NUMBER.is_assignable_from(Class::INTEGER)
                && Class::COMPARABLE.is_assignable_from(Class::STRING)
        );
    }

    #[test]
    fn type_names_are_javas() {
        let ints = Class::array("[[I", "int[][]", || Class::array("[I", "int[]", || Class::INT));
        assert_eq!(&*ints.get_type_name(), crate::Wtf8::new("int[][]"));
        assert_eq!(&*Class::STRING.get_type_name(), crate::Wtf8::new("java.lang.String"));
        let inner = Class::new("a.Outer$Inner", "Inner", Some(|| Class::OBJECT), &["a.Outer$Inner", OBJECT]);
        assert_eq!(&*inner.get_type_name(), crate::Wtf8::new("a.Outer$Inner"));
    }

    #[test]
    fn canonical_and_package_names_are_javas() {
        let inner = Class::new("a.b.Outer$Inner", "Inner", Some(|| Class::OBJECT), &[]);
        let local = Class::new("a.b.Outer$1Local", "Local", Some(|| Class::OBJECT), &[]);
        let anonymous = Class::new("a.b.Outer$1", "", Some(|| Class::OBJECT), &[]);
        let in_local = Class::new("Outer$1Local$Inner", "Inner", Some(|| Class::OBJECT), &[]);
        let inners = Class::array("[La.b.Outer$Inner;", "Inner[]", || {
            Class::new("a.b.Outer$Inner", "Inner", Some(|| Class::OBJECT), &[])
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
