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

/// Whether the binary name `name` is among `names`, as the instance tests ask. Names of one
/// package differ near their ends (`net.minecraft.world.level.block.state.properties.Half`), and
/// many are of one length, so each is compared from its last eight bytes first, and then whole:
/// a name that is not there costs a compare or two, not a `memcmp` of its whole length.
pub(crate) fn names_contain(names: &[&str], name: &str) -> bool {
    let (len, end) = (name.len(), tail(name));
    names.iter().any(|n| n.len() == len && tail(n) == end && (n.as_ptr() == name.as_ptr() || *n == name))
}

/// The last eight bytes of `name` as a number, or its bytes where it is shorter.
#[inline(always)]
fn tail(name: &str) -> u64 {
    let bytes = name.as_bytes();
    match bytes.len().checked_sub(8) {
        Some(start) => u64::from_le_bytes(bytes[start..].try_into().unwrap_or_default()),
        None => bytes.iter().fold(0, |t, &b| t << 8 | u64::from(b)),
    }
}

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

/// Where a class is declared, as `isMemberClass`, `isLocalClass` and `isAnonymousClass` tell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nest {
    TopLevel,
    Member,
    Local,
    Anonymous,
}

const PUBLIC: u16 = 0x1;
const FINAL: u16 = 0x10;
const INTERFACE: u16 = 0x200;
const ABSTRACT: u16 = 0x400;
const ANNOTATION: u16 = 0x2000;
const ENUM: u16 = 0x4000;

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
    /// What `getModifiers` gives: the translator's for a class of the program (`declared`),
    /// public and the like for the runtime's.
    modifiers: u16,
    nest: Nest,
    /// The class that declares this one (`getEnclosingClass`), `None` for a top-level class.
    enclosing: Option<fn() -> Class>,
    /// The interfaces that the class implements, or that an interface extends, directly and in
    /// the order the source names them (`getInterfaces`); an array's are `Cloneable` and
    /// `Serializable`, which `get_interface` answers by its kind.
    interfaces: fn() -> &'static [Class],
}

impl Class {
    /// A class, with its superclass (`None` only for `Object`) and the names above it (`names`).
    pub const fn new(
        name: &'static str,
        simple: &'static str,
        superclass: Option<fn() -> Class>,
        names: Names,
    ) -> Class {
        Class::made(name, simple, Kind::Class, superclass, None, names, PUBLIC)
    }

    /// An enum, whose superclass is `Enum`.
    pub const fn enumeration(name: &'static str, simple: &'static str, names: Names) -> Class {
        Class::made(name, simple, Kind::Enum, Some(|| Class::ENUM), None, names, PUBLIC | FINAL | ENUM)
    }

    /// An interface, which has no superclass.
    pub const fn interface(name: &'static str, simple: &'static str, names: Names) -> Class {
        Class::made(name, simple, Kind::Interface, None, None, names, PUBLIC | INTERFACE | ABSTRACT)
    }

    /// An annotation type, an interface that `isAnnotation()`.
    pub const fn annotation(name: &'static str, simple: &'static str, names: Names) -> Class {
        Class::made(
            name,
            simple,
            Kind::Annotation,
            None,
            None,
            names,
            PUBLIC | INTERFACE | ABSTRACT | ANNOTATION,
        )
    }

    /// An array class, `[Ljava.lang.String;`, whose superclass is `Object`.
    pub const fn array(name: &'static str, simple: &'static str, component: fn() -> Class) -> Class {
        // `getModifiers` answers from the component's.
        Class::made(name, simple, Kind::Class, Some(|| Class::OBJECT), Some(component), &[], 0)
    }

    const fn primitive(name: &'static str) -> Class {
        Class::made(name, name, Kind::Primitive, None, None, &[], PUBLIC | FINAL | ABSTRACT)
    }

    /// A class of the runtime that is final, as most of the standard classes are.
    const fn standard(name: &'static str, simple: &'static str, names: Names) -> Class {
        Class::made(name, simple, Kind::Class, Some(|| Class::OBJECT), None, names, PUBLIC | FINAL)
    }

    /// One of the runtime's abstract classes.
    const fn abstract_standard(name: &'static str, simple: &'static str, names: Names) -> Class {
        Class::made(name, simple, Kind::Class, Some(|| Class::OBJECT), None, names, PUBLIC | ABSTRACT)
    }

    const fn made(
        name: &'static str,
        simple: &'static str,
        kind: Kind,
        superclass: Option<fn() -> Class>,
        component: Option<fn() -> Class>,
        names: Names,
        modifiers: u16,
    ) -> Class {
        Class {
            name,
            simple,
            kind,
            superclass,
            component,
            names,
            modifiers,
            nest: Nest::TopLevel,
            enclosing: None,
            interfaces: || &[],
        }
    }

    /// The class as the program declares it: what `getModifiers` gives, where it is declared and
    /// the class that declares it.
    pub const fn declared(self, modifiers: u16, nest: Nest, enclosing: Option<fn() -> Class>) -> Class {
        Class { modifiers, nest, enclosing, ..self }
    }

    /// The class with the interfaces it implements, or extends, directly (`getInterfaces`).
    pub const fn implementing(self, interfaces: fn() -> &'static [Class]) -> Class {
        Class { interfaces, ..self }
    }

    /// A box of a number, whose superclass is `Number`.
    const fn number(name: &'static str, simple: &'static str, names: Names) -> Class {
        Class::made(name, simple, Kind::Class, Some(|| Class::NUMBER), None, names, PUBLIC | FINAL)
    }

    const SERIALIZABLE: Class = Class::interface(SERIAL, "Serializable", &[SERIAL, OBJECT]);
    const CLONEABLE: Class =
        Class::interface("java.lang.Cloneable", "Cloneable", &["java.lang.Cloneable", OBJECT]);
    const CONSTABLE: Class = Class::interface(CONSTABLE, "Constable", &[CONSTABLE, OBJECT]);
    const CONSTANT_DESC: Class = Class::interface(DESC, "ConstantDesc", &[DESC, OBJECT]);

    pub const OBJECT: Class = Class::new(OBJECT, "Object", None, &[OBJECT]);
    pub const STRING: Class = Class::standard(
        "java.lang.String",
        "String",
        &["java.lang.String", SERIAL, COMPARE, "java.lang.CharSequence", CONSTABLE, DESC, OBJECT],
    )
    .implementing(|| {
        &[
            Class::SERIALIZABLE,
            Class::COMPARABLE,
            Class::CHAR_SEQUENCE,
            Class::CONSTABLE,
            Class::CONSTANT_DESC,
        ]
    });
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
    )
    .implementing(|| &[Class::SERIALIZABLE, Class::COMPARABLE, Class::CHAR_SEQUENCE]);
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
    pub const ENUM: Class = Class::abstract_standard(
        "java.lang.Enum",
        "Enum",
        &["java.lang.Enum", COMPARE, SERIAL, CONSTABLE, OBJECT],
    )
    .implementing(|| &[Class::CONSTABLE, Class::COMPARABLE, Class::SERIALIZABLE]);
    pub const COMPARABLE: Class = Class::interface(COMPARE, "Comparable", &[COMPARE, OBJECT]);
    pub const RECORD: Class =
        Class::abstract_standard("java.lang.Record", "Record", &["java.lang.Record", OBJECT]);
    /// `Runtime.class` and `System.class`, classes without objects in a translation.
    pub const RUNTIME: Class =
        Class::new("java.lang.Runtime", "Runtime", Some(|| Class::OBJECT), &["java.lang.Runtime", OBJECT]);
    pub const SYSTEM: Class = Class::standard("java.lang.System", "System", &["java.lang.System", OBJECT]);
    pub const CHAR_SEQUENCE: Class =
        Class::interface("java.lang.CharSequence", "CharSequence", &["java.lang.CharSequence", OBJECT]);
    pub const INTEGER: Class = Class::number(
        "java.lang.Integer",
        "Integer",
        &["java.lang.Integer", NUMBER, SERIAL, COMPARE, CONSTABLE, DESC, OBJECT],
    )
    .implementing(|| &[Class::COMPARABLE, Class::CONSTABLE, Class::CONSTANT_DESC]);
    pub const LONG: Class = Class::number(
        "java.lang.Long",
        "Long",
        &["java.lang.Long", NUMBER, SERIAL, COMPARE, CONSTABLE, DESC, OBJECT],
    )
    .implementing(|| &[Class::COMPARABLE, Class::CONSTABLE, Class::CONSTANT_DESC]);
    pub const DOUBLE: Class = Class::number(
        "java.lang.Double",
        "Double",
        &["java.lang.Double", NUMBER, SERIAL, COMPARE, CONSTABLE, DESC, OBJECT],
    )
    .implementing(|| &[Class::COMPARABLE, Class::CONSTABLE, Class::CONSTANT_DESC]);
    pub const FLOAT: Class = Class::number(
        "java.lang.Float",
        "Float",
        &["java.lang.Float", NUMBER, SERIAL, COMPARE, CONSTABLE, DESC, OBJECT],
    )
    .implementing(|| &[Class::COMPARABLE, Class::CONSTABLE, Class::CONSTANT_DESC]);
    pub const SHORT: Class = Class::number(
        "java.lang.Short",
        "Short",
        &["java.lang.Short", NUMBER, SERIAL, COMPARE, CONSTABLE, OBJECT],
    )
    .implementing(|| &[Class::COMPARABLE, Class::CONSTABLE]);
    pub const BYTE: Class = Class::number(
        "java.lang.Byte",
        "Byte",
        &["java.lang.Byte", NUMBER, SERIAL, COMPARE, CONSTABLE, OBJECT],
    )
    .implementing(|| &[Class::COMPARABLE, Class::CONSTABLE]);
    pub const CHARACTER: Class = Class::standard(
        "java.lang.Character",
        "Character",
        &["java.lang.Character", SERIAL, COMPARE, CONSTABLE, OBJECT],
    )
    .implementing(|| &[Class::SERIALIZABLE, Class::COMPARABLE, Class::CONSTABLE]);
    pub const BOOLEAN: Class = Class::standard(
        "java.lang.Boolean",
        "Boolean",
        &["java.lang.Boolean", SERIAL, COMPARE, CONSTABLE, OBJECT],
    )
    .implementing(|| &[Class::SERIALIZABLE, Class::COMPARABLE, Class::CONSTABLE]);
    pub const NUMBER: Class = Class::abstract_standard(NUMBER, "Number", &[NUMBER, SERIAL, OBJECT])
        .implementing(|| &[Class::SERIALIZABLE]);
    pub const INT: Class = Class::primitive("int");
    pub const LONG_PRIMITIVE: Class = Class::primitive("long");
    pub const DOUBLE_PRIMITIVE: Class = Class::primitive("double");
    pub const FLOAT_PRIMITIVE: Class = Class::primitive("float");
    pub const SHORT_PRIMITIVE: Class = Class::primitive("short");
    pub const BYTE_PRIMITIVE: Class = Class::primitive("byte");
    pub const CHAR: Class = Class::primitive("char");
    pub const BOOLEAN_PRIMITIVE: Class = Class::primitive("boolean");
    pub const VOID: Class = Class::primitive("void");

    /// `getModifiers()`: an array's are its component's access, abstract and final.
    pub fn get_modifiers(&self) -> i32 {
        match self.get_component_type() {
            Some(component) => (component.get_modifiers() & 0x7) | i32::from(ABSTRACT | FINAL),
            None => i32::from(self.modifiers),
        }
    }

    /// `isAnonymousClass()`.
    pub fn is_anonymous_class(&self) -> bool {
        self.nest == Nest::Anonymous
    }

    /// `isLocalClass()`.
    pub fn is_local_class(&self) -> bool {
        self.nest == Nest::Local
    }

    /// `isMemberClass()`.
    pub fn is_member_class(&self) -> bool {
        self.nest == Nest::Member
    }

    /// How many interfaces `getInterfaces()` gives.
    pub fn get_interface_count(&self) -> i32 {
        if self.is_array() { 2 } else { (self.interfaces)().len() as i32 }
    }

    /// The `index`th of the interfaces that `getInterfaces()` gives.
    pub fn get_interface(&self, index: i32) -> Class {
        match self.is_array() {
            true => [Class::CLONEABLE, Class::SERIALIZABLE][index as usize],
            false => (self.interfaces)()[index as usize],
        }
    }

    /// `getEnclosingClass()`.
    pub fn get_enclosing_class(&self) -> Option<Class> {
        self.enclosing.map(|f| f())
    }

    /// `getDeclaringClass()`: a member class's enclosing class, `null` for any other.
    pub fn get_declaring_class(&self) -> Option<Class> {
        self.enclosing.filter(|_| self.nest == Nest::Member).map(|f| f())
    }

    /// `getName()`.
    pub fn get_name(&self) -> crate::JString {
        self.shown().into()
    }

    /// The binary name as `getName` gives it: a proxy class's is the one it was given when it
    /// was defined (`proxy_classes`).
    fn shown(&self) -> &'static str {
        crate::proxy_classes::proxy_name(self.name).unwrap_or(self.name)
    }

    /// `getSimpleName()`.
    pub fn get_simple_name(&self) -> crate::JString {
        match crate::proxy_classes::proxy_name(self.name) {
            Some(name) => name.rsplit('.').next().unwrap_or(name).into(),
            None => self.simple.into(),
        }
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
        if let Some(name) = crate::proxy_classes::proxy_name(self.name) {
            return Some(name.to_string());
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
            format!("L{};", self.shown().replace('.', "/"))
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
        self.shown().rsplit_once('.').map_or("", |(package, _)| package).into()
    }

    /// `getTypeName()`: for an array, the name of its innermost component type with a `[]` for
    /// each dimension (`int[][]`, `java.lang.String[]`); for any other class, its binary name.
    pub fn get_type_name(&self) -> crate::JString {
        self.type_name().into()
    }

    /// `getTypeName()` where the program's strings are shared: a class's name is the interned
    /// string, an array's a new one, as the JDK makes them.
    pub fn shared_type_name(&self) -> crate::JStr {
        if self.is_array() { crate::JStr::new(self.type_name()) } else { crate::interned(self.shown()) }
    }

    fn type_name(&self) -> String {
        let mut class = *self;
        let mut dimensions = 0;
        while let Some(component) = class.get_component_type() {
            class = component;
            dimensions += 1;
        }
        format!("{}{}", class.shown(), "[]".repeat(dimensions))
    }

    /// The binary name, as `getName` gives it, without a copy.
    pub fn binary_name(&self) -> &'static str {
        self.name
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
        names.is_some_and(|names| names_contain(names, self.name))
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
            (None, None) => self.name == OBJECT || names_contain(other.names, self.name),
        }
    }

    /// The check of `cast(x)`, given the names as for `is_instance`, the class of `x` first: a
    /// `ClassCastException` unless `x` is null or an instance.
    pub fn check_cast(&self, names: Option<&[&str]>) {
        if let Some(names) = names
            && !names_contain(names, self.name)
        {
            let from = names.first().copied().unwrap_or("java.lang.Object");
            crate::throw("java.lang.ClassCastException", &format!("Cannot cast {from} to {}", self.shown()));
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
            Kind::Class | Kind::Enum => write!(f, "class {}", self.shown()),
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

    #[test]
    fn a_name_is_found_among_names_of_its_length_and_end() {
        // Built at run time, so that no name is the same text as the one looked for.
        let half = format!("a.b.c.Half{}", "");
        let names = ["a.b.c.Hal", "a.b.c.Halt", "x.y.z.Half", "a.b.c.Half", "Half"];
        assert!(names_contain(&names, &half));
        assert!(!names_contain(&names[..3], &half));
        assert!(names_contain(&["Ab", "Abc"], &format!("Ab{}", "")));
        assert!(!names_contain(&["Ac", "aB"], "Ab"));
        assert!(!names_contain(&[], "Ab"));
        assert!(names_contain(&[""], ""));
    }
}
