//! `java.lang.Class`, as far as a program uses it without reflection: the class of a value with
//! its names, its superclass, its component type and the binary names of the classes and
//! interfaces above it. Two classes are the same when their binary names are, since no two
//! classes of a program share one. A class of the program is a static at the translation's crate
//! root, and names its superclass, its interfaces and the class that declares it by references,
//! which may go round in a loop (`class Outer implements Outer.Inner`); a class the translator
//! writes where it is used, an array's, names its component type by a function. A class value is
//! a `&'static Class`, so the methods that give a class give a reference.

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

/// A set of class numbers, a bit for each from the word of the first to that of the last: the
/// test of a type whose classes are many runs of numbers (`as_x()` of an interface), which a
/// `match` makes a tree of compares.
pub struct ClassSet<const N: usize> {
    /// The word of the first number, the number divided by 64.
    first: usize,
    words: [u64; N],
}

impl<const N: usize> ClassSet<N> {
    /// The set of the numbers of `runs`, `(first, last)` in order, in `N` words.
    pub const fn of(runs: &[(u16, u16)]) -> ClassSet<N> {
        let first = if runs.is_empty() { 0 } else { runs[0].0 as usize / 64 };
        let mut words = [0u64; N];
        let mut r = 0;
        while r < runs.len() {
            let mut n = runs[r].0 as usize;
            while n <= runs[r].1 as usize {
                words[n / 64 - first] |= 1 << (n % 64);
                n += 1;
            }
            r += 1;
        }
        ClassSet { first, words }
    }

    /// Whether class number `n` is in the set.
    #[inline(always)]
    pub fn contains(&self, n: u16) -> bool {
        let w = (n as usize / 64).wrapping_sub(self.first);
        w < N && self.words[w] >> (n % 64) & 1 != 0
    }
}

/// What a class is, which its `toString` shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClassKind {
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
    kind: ClassKind,
    superclass: Option<&'static Class>,
    component: Option<fn() -> &'static Class>,
    /// The binary names of the class and of every class and interface above it, its own first,
    /// as `is_instance` takes them; empty for an array and a primitive type, which
    /// `is_assignable_from` answers by their kind.
    names: Names,
    /// What `getModifiers` gives: the translator's for a class of the program (`declared`),
    /// public and the like for the runtime's.
    modifiers: u16,
    nest: Nest,
    /// The class that declares this one (`getEnclosingClass`), `None` for a top-level class.
    enclosing: Option<&'static Class>,
    /// The interfaces that the class implements, or that an interface extends, directly and in
    /// the order the source names them (`getInterfaces`); an array's are `Cloneable` and
    /// `Serializable`, which `get_interface` answers by its kind.
    interfaces: &'static [&'static Class],
    /// Whether no class is below this one, as for a final class or a class of the program that
    /// no other class of the program extends: an instance is then exactly of this class, which
    /// `is_instance` and `check_cast` test by the first of its names alone.
    leaf: bool,
    /// The class numbers of the program's classes at or below this one (`numbered`, `numbers`),
    /// from its own to its last subclass's, which their values carry: a test of a value whose
    /// number the translation has is one subtract and one compare; 0 when unknown.
    number: u16,
    last: u16,
    /// The class numbers of the program's classes below an interface (`members`), as the words
    /// of a `ClassSet` from the word of the first: a test of a value whose number the translation
    /// has is a load and a shift. Empty when unknown.
    members: (usize, &'static [u64]),
    /// The hash of the name as a string's (`hash_code`), computed when the class value is made.
    hash: i32,
}

impl Class {
    /// A class, with its superclass (`None` only for `Object`) and the names above it (`names`).
    pub const fn new(
        name: &'static str,
        simple: &'static str,
        superclass: Option<&'static Class>,
        names: Names,
    ) -> Class {
        Class::made(name, simple, ClassKind::Class, superclass, None, names, PUBLIC)
    }

    /// An enum, whose superclass is `Enum`.
    pub const fn enumeration(name: &'static str, simple: &'static str, names: Names) -> Class {
        Class::made(name, simple, ClassKind::Enum, Some(&Class::ENUM), None, names, PUBLIC | FINAL | ENUM)
    }

    /// An interface, which has no superclass.
    pub const fn interface(name: &'static str, simple: &'static str, names: Names) -> Class {
        Class::made(name, simple, ClassKind::Interface, None, None, names, PUBLIC | INTERFACE | ABSTRACT)
    }

    /// An annotation type, an interface that `isAnnotation()`.
    pub const fn annotation(name: &'static str, simple: &'static str, names: Names) -> Class {
        Class::made(
            name,
            simple,
            ClassKind::Annotation,
            None,
            None,
            names,
            PUBLIC | INTERFACE | ABSTRACT | ANNOTATION,
        )
    }

    /// An array class, `[Ljava.lang.String;`, whose superclass is `Object`.
    pub const fn array(name: &'static str, simple: &'static str, component: fn() -> &'static Class) -> Class {
        // `getModifiers` answers from the component's.
        Class::made(name, simple, ClassKind::Class, Some(&Class::OBJECT), Some(component), &[], 0)
    }

    const fn primitive(name: &'static str) -> Class {
        Class::made(name, name, ClassKind::Primitive, None, None, &[], PUBLIC | FINAL | ABSTRACT)
    }

    /// A class of the runtime that is final, as most of the standard classes are.
    const fn standard(name: &'static str, simple: &'static str, names: Names) -> Class {
        Class::made(name, simple, ClassKind::Class, Some(&Class::OBJECT), None, names, PUBLIC | FINAL).leaf()
    }

    /// One of the runtime's abstract classes.
    const fn abstract_standard(name: &'static str, simple: &'static str, names: Names) -> Class {
        Class::made(name, simple, ClassKind::Class, Some(&Class::OBJECT), None, names, PUBLIC | ABSTRACT)
    }

    const fn made(
        name: &'static str,
        simple: &'static str,
        kind: ClassKind,
        superclass: Option<&'static Class>,
        component: Option<fn() -> &'static Class>,
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
            interfaces: &[],
            leaf: false,
            number: 0,
            last: 0,
            members: (0, &[]),
            hash: name_hash(name),
        }
    }

    /// A class of the program, as the translator writes it out: every field given, the hash of
    /// its name included, in one call. rustc evaluates each class value of a program when it
    /// compiles it, and a chain of builder calls (`new(..).implementing(..).numbered(..)`) with
    /// the hash's loop over the name cost it about three times as much: 13.3 s against 5.0 s for
    /// 20,000 classes (the Minecraft server has 44,000). `leaf`, `number` and `last` are as
    /// `numbered` and `numbers` set them (0 and 0 where unknown); an interface's `members` follow
    /// the call. `hash` is the name's `String.hashCode`.
    #[allow(clippy::too_many_arguments)]
    pub const fn of_program(
        name: &'static str,
        simple: &'static str,
        kind: ClassKind,
        superclass: Option<&'static Class>,
        names: Names,
        interfaces: &'static [&'static Class],
        leaf: bool,
        number: u16,
        last: u16,
        modifiers: u16,
        nest: Nest,
        enclosing: Option<&'static Class>,
        hash: i32,
    ) -> Class {
        Class {
            name,
            simple,
            kind,
            superclass,
            component: None,
            names,
            modifiers,
            nest,
            enclosing,
            interfaces,
            leaf,
            number,
            last,
            members: (0, &[]),
            hash,
        }
    }

    /// The class, which no class is below (`leaf`).
    pub const fn leaf(self) -> Class {
        Class { leaf: true, ..self }
    }

    /// The `leaf` of the program whose values carry class number `number`.
    pub const fn numbered(self, number: u16) -> Class {
        Class { leaf: true, number, last: number, ..self }
    }

    /// The class of the program whose values, and those of its subclasses, carry the class
    /// numbers `number..=last`: the classes are numbered each before its subclasses, so those at
    /// or below a class are one range.
    pub const fn numbers(self, number: u16, last: u16) -> Class {
        Class { number, last, ..self }
    }

    /// The interface of the program whose values' classes have the numbers of `set`.
    pub const fn members<const N: usize>(self, set: &'static ClassSet<N>) -> Class {
        Class { members: (set.first, &set.words), ..self }
    }

    /// Whether class number `n` is at or below the class, by its numbers or its members, or
    /// `None` where it has neither.
    #[inline(always)]
    fn holds(&self, n: u16) -> Option<bool> {
        if self.number != 0 {
            return Some(self.holds_number(n));
        }
        let (first, words) = self.members;
        if words.is_empty() {
            return None;
        }
        let w = (n as usize / 64).wrapping_sub(first);
        Some(w < words.len() && words[w] >> (n % 64) & 1 != 0)
    }

    /// Whether a value of class number `n` is at or below the class; only for a class with
    /// numbers.
    #[inline(always)]
    fn holds_number(&self, n: u16) -> bool {
        n.wrapping_sub(self.number) <= self.last.wrapping_sub(self.number)
    }

    /// The class as the program declares it: what `getModifiers` gives, where it is declared and
    /// the class that declares it.
    pub const fn declared(self, modifiers: u16, nest: Nest, enclosing: Option<&'static Class>) -> Class {
        Class { modifiers, nest, enclosing, ..self }
    }

    /// The class with the interfaces it implements, or extends, directly (`getInterfaces`).
    pub const fn implementing(self, interfaces: &'static [&'static Class]) -> Class {
        Class { interfaces, ..self }
    }

    /// A box of a number, whose superclass is `Number`.
    const fn number(name: &'static str, simple: &'static str, names: Names) -> Class {
        Class::made(name, simple, ClassKind::Class, Some(&Class::NUMBER), None, names, PUBLIC | FINAL).leaf()
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
    .implementing(&[
        &Class::SERIALIZABLE,
        &Class::COMPARABLE,
        &Class::CHAR_SEQUENCE,
        &Class::CONSTABLE,
        &Class::CONSTANT_DESC,
    ]);
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
    .implementing(&[&Class::SERIALIZABLE, &Class::COMPARABLE, &Class::CHAR_SEQUENCE]);
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
    .implementing(&[&Class::CONSTABLE, &Class::COMPARABLE, &Class::SERIALIZABLE]);
    pub const COMPARABLE: Class = Class::interface(COMPARE, "Comparable", &[COMPARE, OBJECT]);
    pub const RECORD: Class =
        Class::abstract_standard("java.lang.Record", "Record", &["java.lang.Record", OBJECT]);
    /// `Runtime.class` and `System.class`, classes without objects in a translation.
    pub const RUNTIME: Class =
        Class::new("java.lang.Runtime", "Runtime", Some(&Class::OBJECT), &["java.lang.Runtime", OBJECT]);
    pub const SYSTEM: Class = Class::standard("java.lang.System", "System", &["java.lang.System", OBJECT]);
    pub const CHAR_SEQUENCE: Class =
        Class::interface("java.lang.CharSequence", "CharSequence", &["java.lang.CharSequence", OBJECT]);
    pub const INTEGER: Class = Class::number(
        "java.lang.Integer",
        "Integer",
        &["java.lang.Integer", NUMBER, SERIAL, COMPARE, CONSTABLE, DESC, OBJECT],
    )
    .implementing(&[&Class::COMPARABLE, &Class::CONSTABLE, &Class::CONSTANT_DESC]);
    pub const LONG: Class = Class::number(
        "java.lang.Long",
        "Long",
        &["java.lang.Long", NUMBER, SERIAL, COMPARE, CONSTABLE, DESC, OBJECT],
    )
    .implementing(&[&Class::COMPARABLE, &Class::CONSTABLE, &Class::CONSTANT_DESC]);
    pub const DOUBLE: Class = Class::number(
        "java.lang.Double",
        "Double",
        &["java.lang.Double", NUMBER, SERIAL, COMPARE, CONSTABLE, DESC, OBJECT],
    )
    .implementing(&[&Class::COMPARABLE, &Class::CONSTABLE, &Class::CONSTANT_DESC]);
    pub const FLOAT: Class = Class::number(
        "java.lang.Float",
        "Float",
        &["java.lang.Float", NUMBER, SERIAL, COMPARE, CONSTABLE, DESC, OBJECT],
    )
    .implementing(&[&Class::COMPARABLE, &Class::CONSTABLE, &Class::CONSTANT_DESC]);
    pub const SHORT: Class = Class::number(
        "java.lang.Short",
        "Short",
        &["java.lang.Short", NUMBER, SERIAL, COMPARE, CONSTABLE, OBJECT],
    )
    .implementing(&[&Class::COMPARABLE, &Class::CONSTABLE]);
    pub const BYTE: Class = Class::number(
        "java.lang.Byte",
        "Byte",
        &["java.lang.Byte", NUMBER, SERIAL, COMPARE, CONSTABLE, OBJECT],
    )
    .implementing(&[&Class::COMPARABLE, &Class::CONSTABLE]);
    pub const CHARACTER: Class = Class::standard(
        "java.lang.Character",
        "Character",
        &["java.lang.Character", SERIAL, COMPARE, CONSTABLE, OBJECT],
    )
    .implementing(&[&Class::SERIALIZABLE, &Class::COMPARABLE, &Class::CONSTABLE]);
    pub const BOOLEAN: Class = Class::standard(
        "java.lang.Boolean",
        "Boolean",
        &["java.lang.Boolean", SERIAL, COMPARE, CONSTABLE, OBJECT],
    )
    .implementing(&[&Class::SERIALIZABLE, &Class::COMPARABLE, &Class::CONSTABLE]);
    pub const NUMBER: Class = Class::abstract_standard(NUMBER, "Number", &[NUMBER, SERIAL, OBJECT])
        .implementing(&[&Class::SERIALIZABLE]);
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
        if self.is_array() { 2 } else { self.interfaces.len() as i32 }
    }

    /// The `index`th of the interfaces that `getInterfaces()` gives.
    pub fn get_interface(&self, index: i32) -> &'static Class {
        match self.is_array() {
            true => [&Class::CLONEABLE, &Class::SERIALIZABLE][index as usize],
            false => self.interfaces[index as usize],
        }
    }

    /// `getEnclosingClass()`.
    pub fn get_enclosing_class(&self) -> Option<&'static Class> {
        self.enclosing
    }

    /// `getDeclaringClass()`: a member class's enclosing class, `null` for any other.
    pub fn get_declaring_class(&self) -> Option<&'static Class> {
        self.enclosing.filter(|_| self.nest == Nest::Member)
    }

    /// `Enum.getDeclaringClass()` of a constant of this class: the enum, which is the superclass
    /// of the class of a constant's body.
    pub fn enum_declaring_class(&'static self) -> &'static Class {
        match self.superclass {
            Some(s) if s.kind == ClassKind::Enum => s,
            _ => self,
        }
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
        if self.kind == ClassKind::Primitive {
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
        let text = if self.kind == ClassKind::Primitive {
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
        if self.kind == ClassKind::Primitive {
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
        let mut class = self;
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
    pub fn get_superclass(&self) -> Option<&'static Class> {
        self.superclass
    }

    /// `getComponentType()`: the type of an array's elements, `null` for any other class.
    pub fn get_component_type(&self) -> Option<&'static Class> {
        self.component.map(|f| f())
    }

    /// `isArray()`.
    pub fn is_array(&self) -> bool {
        self.component.is_some()
    }

    /// `isInterface()`.
    pub fn is_interface(&self) -> bool {
        matches!(self.kind, ClassKind::Interface | ClassKind::Annotation)
    }

    /// `isAnnotation()`.
    pub fn is_annotation(&self) -> bool {
        self.kind == ClassKind::Annotation
    }

    /// `isEnum()`.
    pub fn is_enum(&self) -> bool {
        self.kind == ClassKind::Enum
    }

    /// `isPrimitive()`.
    pub fn is_primitive(&self) -> bool {
        self.kind == ClassKind::Primitive
    }

    /// `isInstance(x)`, given the binary names of the class of `x` and of every class and
    /// interface above it (`None` for a null `x`), which the translation knows.
    pub fn is_instance(&self, names: Option<&[&str]>) -> bool {
        names.is_some_and(|names| self.above(names))
    }

    /// `is_instance`, given the class number of `x` where the translation has it (`None` for a
    /// null `x` or one of unknown class) and the names as a function: for a class with numbers or
    /// members, a test of the number, without asking `x` for its names.
    #[inline(always)]
    pub fn is_instance_numbered<'n>(
        &self,
        number: Option<u16>,
        names: impl FnOnce() -> Option<&'n [&'n str]>,
    ) -> bool {
        match number.and_then(|n| self.holds(n)) {
            Some(holds) => holds,
            None => self.is_instance(names()),
        }
    }

    /// `check_cast` with the class number of `x`, as for `is_instance_numbered`.
    #[inline(always)]
    pub fn check_cast_numbered<'n>(
        &self,
        number: Option<u16>,
        names: impl FnOnce() -> Option<&'n [&'n str]>,
    ) {
        if number.and_then(|n| self.holds(n)) != Some(true) {
            self.check_cast(names());
        }
    }

    /// `check_cast_numbered`, where a `catch` or a `finally` sees its `ClassCastException`: the
    /// failure is returned, to go on as a thrown exception does, with no unwinding to catch.
    #[inline(always)]
    pub fn try_check_cast_numbered<'n>(
        &self,
        number: Option<u16>,
        names: impl FnOnce() -> Option<&'n [&'n str]>,
    ) -> Result<(), crate::Exception> {
        match number.and_then(|n| self.holds(n)) {
            Some(true) => Ok(()),
            _ => self.try_check_cast(names()),
        }
    }

    /// Whether the class is among `names`, those of a value's class and of the classes above it:
    /// for a `leaf`, whether it is the first.
    #[inline(always)]
    fn above(&self, names: &[&str]) -> bool {
        match self.leaf {
            true => names_contain(names.get(..1).unwrap_or_default(), self.name),
            false => names_contain(names, self.name),
        }
    }

    /// `isAssignableFrom(other)`: whether `other` is this class or a class or interface below it,
    /// by the names above `other`. A primitive type is only itself; every other class is below
    /// `Object`; an array is below `Cloneable` and `Serializable`, and below the arrays of the
    /// classes above its component type where that is not primitive.
    pub fn is_assignable_from(&self, other: &Class) -> bool {
        if self.name == other.name {
            return true;
        }
        if self.kind == ClassKind::Primitive || other.kind == ClassKind::Primitive {
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
            && !self.above(names)
        {
            crate::throw("java.lang.ClassCastException", &self.cast_message(names));
        }
    }

    /// `check_cast`, where a `catch` or a `finally` sees its `ClassCastException`, which is
    /// returned.
    pub fn try_check_cast(&self, names: Option<&[&str]>) -> Result<(), crate::Exception> {
        match names {
            Some(names) if !self.above(names) => {
                Err(crate::Exception::new("java.lang.ClassCastException", self.cast_message(names)))
            }
            _ => Ok(()),
        }
    }

    /// The message of `cast`'s `ClassCastException` for a value whose class's names are `names`.
    #[cold]
    fn cast_message(&self, names: &[&str]) -> String {
        let from = names.first().copied().unwrap_or("java.lang.Object");
        format!("Cannot cast {from} to {}", self.shown())
    }

    /// `hashCode()`: Java's is the class object's identity hash, which nothing can depend on; this
    /// one is the hash of its name.
    pub fn hash_code(&self) -> i32 {
        self.hash
    }
}

/// `String.hashCode` of `name` over its UTF-16 units, for a class value made at compile time.
const fn name_hash(name: &str) -> i32 {
    let bytes = name.as_bytes();
    let (mut hash, mut i) = (0i32, 0);
    while i < bytes.len() {
        let b = bytes[i] as u32;
        let (c, n) = match b {
            0..0x80 => (b, 1),
            0xC0..0xE0 => ((b & 0x1F) << 6 | (bytes[i + 1] as u32 & 0x3F), 2),
            0xE0..0xF0 => {
                ((b & 0x0F) << 12 | (bytes[i + 1] as u32 & 0x3F) << 6 | (bytes[i + 2] as u32 & 0x3F), 3)
            }
            _ => {
                let c = (b & 0x07) << 18
                    | (bytes[i + 1] as u32 & 0x3F) << 12
                    | (bytes[i + 2] as u32 & 0x3F) << 6
                    | (bytes[i + 3] as u32 & 0x3F);
                (c, 4)
            }
        };
        if c >= 0x10000 {
            let c = c - 0x10000;
            hash = hash.wrapping_mul(31).wrapping_add((0xD800 + (c >> 10)) as i32);
            hash = hash.wrapping_mul(31).wrapping_add((0xDC00 + (c & 0x3FF)) as i32);
        } else {
            hash = hash.wrapping_mul(31).wrapping_add(c as i32);
        }
        i += n;
    }
    hash
}

impl PartialEq for Class {
    /// The same class: usually the same static, else the same name, which two values of one
    /// class usually hold at one address.
    fn eq(&self, other: &Class) -> bool {
        std::ptr::eq(self, other)
            || std::ptr::eq(self.name, other.name)
            || (self.hash == other.hash && self.name == other.name)
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
            ClassKind::Class | ClassKind::Enum => write!(f, "class {}", self.shown()),
            ClassKind::Interface | ClassKind::Annotation => write!(f, "interface {}", self.name),
            ClassKind::Primitive => f.write_str(self.name),
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
        let point = Class::new("Main$Point", "Point", Some(&Class::OBJECT), &["Main$Point", OBJECT]);
        let points =
            Class::array(
                "[LMain$Point;",
                "Point[]",
                || &const { Class::new("Main$Point", "Point", None, &[]) },
            );
        assert_eq!(point.to_string(), "class Main$Point");
        assert_eq!(Class::COMPARABLE.to_string(), "interface java.lang.Comparable");
        assert_eq!(Class::INT.to_string(), "int");
        assert_eq!(points.get_component_type(), Some(&point));
        assert_eq!(points.get_superclass(), Some(&Class::OBJECT));
        assert_eq!(Class::OBJECT.get_superclass(), None);
        assert!(points.is_array() && !point.is_array());
        assert_ne!(point, Class::STRING);
    }

    #[test]
    fn a_class_hashes_as_its_name_does() {
        for name in ["Main$Point", "java.lang.Object", "[I", "Ünïcode$Klasse", "Main$\u{1F600}"] {
            let class = Class::new(name, name, Some(&Class::OBJECT), &[]);
            assert_eq!(class.hash_code(), crate::string_hash(crate::Wtf8::new(name)), "{name}");
        }
    }

    #[test]
    fn assignability_is_javas() {
        let shape = Class::interface("Shape", "Shape", &["Shape", OBJECT]);
        let point = Class::new("Point", "Point", Some(&Class::OBJECT), &["Point", "Shape", OBJECT]);
        let points = Class::array("[LPoint;", "Point[]", || {
            &const { Class::new("Point", "Point", Some(&Class::OBJECT), &["Point", "Shape", OBJECT]) }
        });
        let shapes = Class::array("[LShape;", "Shape[]", || {
            &const { Class::interface("Shape", "Shape", &["Shape", OBJECT]) }
        });
        let ints = Class::array("[I", "int[]", || &Class::INT);
        let longs = Class::array("[J", "long[]", || &Class::LONG_PRIMITIVE);
        assert!(shape.is_assignable_from(&point) && !point.is_assignable_from(&shape));
        assert!(Class::OBJECT.is_assignable_from(&shape) && !Class::OBJECT.is_assignable_from(&Class::INT));
        assert!(shapes.is_assignable_from(&points) && !points.is_assignable_from(&shapes));
        assert!(ints.is_assignable_from(&ints) && !ints.is_assignable_from(&longs));
        assert!(Class::OBJECT.is_assignable_from(&ints) && !shape.is_assignable_from(&ints));
        assert!(
            Class::INT.is_assignable_from(&Class::INT) && !Class::INTEGER.is_assignable_from(&Class::INT)
        );
        assert!(
            Class::NUMBER.is_assignable_from(&Class::INTEGER)
                && Class::COMPARABLE.is_assignable_from(&Class::STRING)
        );
    }

    #[test]
    fn type_names_are_javas() {
        let ints = Class::array("[[I", "int[][]", || &const { Class::array("[I", "int[]", || &Class::INT) });
        assert_eq!(&*ints.get_type_name(), crate::Wtf8::new("int[][]"));
        assert_eq!(&*Class::STRING.get_type_name(), crate::Wtf8::new("java.lang.String"));
        let inner = Class::new("a.Outer$Inner", "Inner", Some(&Class::OBJECT), &["a.Outer$Inner", OBJECT]);
        assert_eq!(&*inner.get_type_name(), crate::Wtf8::new("a.Outer$Inner"));
    }

    #[test]
    fn canonical_and_package_names_are_javas() {
        let inner = Class::new("a.b.Outer$Inner", "Inner", Some(&Class::OBJECT), &[]);
        let local = Class::new("a.b.Outer$1Local", "Local", Some(&Class::OBJECT), &[]);
        let anonymous = Class::new("a.b.Outer$1", "", Some(&Class::OBJECT), &[]);
        let in_local = Class::new("Outer$1Local$Inner", "Inner", Some(&Class::OBJECT), &[]);
        let inners = Class::array("[La.b.Outer$Inner;", "Inner[]", || {
            &const { Class::new("a.b.Outer$Inner", "Inner", Some(&Class::OBJECT), &[]) }
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

    #[test]
    fn class_sets_hold_their_runs() {
        const RUNS: [(u16, u16); 3] = [(70, 72), (130, 130), (190, 260)];
        static SET: ClassSet<4> = ClassSet::of(&RUNS);
        for n in 0..400u16 {
            let inside = RUNS.iter().any(|&(a, b)| (a..=b).contains(&n));
            assert_eq!(SET.contains(n), inside, "{n}");
        }
        assert!(!SET.contains(u16::MAX));
    }
}
