//! The Java syntax tree the translator works on: a typed subset of Java, lowered from javac's
//! attributed tree (`lower.rs`), so every expression has its Java type and calls are resolved.

use std::collections::HashSet;
use std::fmt;
use std::sync::{LazyLock, Mutex};

mod charsets;
mod code_point;
mod counting;
mod hierarchy;
mod java_arrays;
mod natives;
mod strings;
mod unsigned;
pub use charsets::*;
pub use code_point::*;
pub use counting::*;
pub use hierarchy::*;
pub use java_arrays::*;
pub use natives::*;
pub use strings::*;
pub use unsigned::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Type {
    Int,
    Long,
    Double,
    Byte,
    Short,
    /// An unsigned 16-bit UTF-16 code unit.
    Char,
    Float,
    Boolean,
    String,
    Void,
    /// An array type such as `int[]` or `String[][]`.
    Array(&'static Type),
    /// A class declared in the translated program, by name, with the type arguments of a generic
    /// class that is a Rust generic (see `lower::generics`): `Box<Integer>` is
    /// `Class("Box", [Integer])`. An erased generic class has none.
    Class(&'static str, &'static [Type]),
    /// An enum declared in the translated program, by name. Its values are plain Rust enums.
    Enum(&'static str),
    /// The type of `null`.
    Null,
    /// A boxed primitive such as `Integer` (`Type::boxed(Type::Int)`), which holds that primitive
    /// in an object of its own.
    Boxed(&'static Type),
    /// `java.lang.StringBuilder`, a shared and mutable text (`java::StringBuilder`).
    StringBuilder,
    /// `java.lang.Class<?>`, the class of a value (`java::Class`; see `lower::classes`).
    JavaClass,
    /// `java.nio.charset.Charset`, a value (`java::Charset`) that is never null; its operations
    /// are `Builtin::Charset`.
    Charset,
}

impl Type {
    pub fn is_numeric(self) -> bool {
        self.is_integral() || self.is_floating()
    }

    pub fn is_integral(self) -> bool {
        matches!(self, Type::Int | Type::Long | Type::Byte | Type::Short | Type::Char)
    }

    pub fn is_floating(self) -> bool {
        matches!(self, Type::Double | Type::Float)
    }

    /// `byte`, `short` and `char`, which arithmetic promotes to `int`.
    pub fn is_small(self) -> bool {
        matches!(self, Type::Byte | Type::Short | Type::Char)
    }

    /// `element[]`. Types are small and copied everywhere, so compound ones are interned for the
    /// life of the process (a few bytes per distinct type).
    pub fn array(element: Type) -> Type {
        Type::Array(intern(element))
    }

    pub fn class(name: &str) -> Type {
        Type::Class(interned_name(name), &[])
    }

    /// A generic class with its type arguments: `Box<Integer>`.
    pub fn generic(name: &str, args: &[Type]) -> Type {
        Type::Class(interned_name(name), intern_types(args))
    }

    /// The type arguments of a generic class's type; none for any other type.
    pub fn args(self) -> &'static [Type] {
        match self {
            Type::Class(_, args) => args,
            _ => &[],
        }
    }

    /// The type of an enum declared in the file.
    pub fn enum_type(name: &str) -> Type {
        Type::Enum(interned_name(name))
    }

    /// The boxed type of a primitive, such as `Integer` for `int`.
    pub fn boxed(primitive: Type) -> Type {
        Type::Boxed(intern(primitive))
    }

    /// The primitive that a boxed type holds, and the type itself for any other type.
    pub fn unboxed(self) -> Type {
        match self {
            Type::Boxed(primitive) => *primitive,
            _ => self,
        }
    }

    /// The element type of an array type.
    pub fn element(self) -> Type {
        match self {
            Type::Array(elem) => *elem,
            _ => unreachable!("an element of a type that is not an array: {self}"),
        }
    }

    /// The type of `levels` nested elements: `int[]` for `int[][][]` at 2.
    pub fn element_at(self, levels: usize) -> Type {
        (0..levels).fold(self, |ty, _| ty.element())
    }

    /// Whether values of this type are references (and so can be `null`). An enum value is not:
    /// Rust's enum is a plain value, and `null` enums are rejected.
    pub fn is_reference(self) -> bool {
        matches!(
            self,
            Type::String
                | Type::Array(_)
                | Type::Class(..)
                | Type::Null
                | Type::Boxed(_)
                | Type::StringBuilder
                | Type::JavaClass
        )
    }

    /// The wider of two integer types.
    pub fn max_integral(self, other: Type) -> Type {
        if self == other {
            self
        } else if self == Type::Long || other == Type::Long {
            Type::Long
        } else {
            Type::Int
        }
    }
}

/// A class or enum name, leaked once so that types stay `Copy`.
fn interned_name(name: &str) -> &'static str {
    static NAMES: LazyLock<Mutex<HashSet<&'static str>>> = LazyLock::new(Default::default);
    let mut names = NAMES.lock().unwrap();
    match names.get(name) {
        Some(&n) => n,
        None => {
            let n: &'static str = Box::leak(name.to_string().into_boxed_str());
            names.insert(n);
            n
        }
    }
}

fn intern_types(types: &[Type]) -> &'static [Type] {
    if types.is_empty() {
        return &[];
    }
    static LISTS: LazyLock<Mutex<HashSet<&'static [Type]>>> = LazyLock::new(Default::default);
    let mut lists = LISTS.lock().unwrap();
    match lists.get(types) {
        Some(&l) => l,
        None => {
            let l: &'static [Type] = Box::leak(types.to_vec().into_boxed_slice());
            lists.insert(l);
            l
        }
    }
}

fn intern(ty: Type) -> &'static Type {
    static TYPES: LazyLock<Mutex<HashSet<&'static Type>>> = LazyLock::new(Default::default);
    let mut types = TYPES.lock().unwrap();
    match types.get(&ty) {
        Some(&t) => t,
        None => {
            let t: &'static Type = Box::leak(Box::new(ty));
            types.insert(t);
            t
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Type::Int => f.write_str("int"),
            Type::Long => f.write_str("long"),
            Type::Double => f.write_str("double"),
            Type::Byte => f.write_str("byte"),
            Type::Short => f.write_str("short"),
            Type::Char => f.write_str("char"),
            Type::Float => f.write_str("float"),
            Type::Boolean => f.write_str("boolean"),
            Type::String => f.write_str("String"),
            Type::Void => f.write_str("void"),
            Type::Array(element) => write!(f, "{element}[]"),
            Type::Class(name, []) | Type::Enum(name) => f.write_str(name),
            Type::Class(name, args) => {
                let args: Vec<String> = args.iter().map(Type::to_string).collect();
                write!(f, "{name}<{}>", args.join(", "))
            }
            Type::Null => f.write_str("null"),
            Type::StringBuilder => f.write_str("StringBuilder"),
            Type::Charset => f.write_str("Charset"),
            Type::JavaClass => f.write_str("Class"),
            Type::Boxed(primitive) => f.write_str(match primitive {
                Type::Int => "Integer",
                Type::Char => "Character",
                Type::Boolean => "Boolean",
                Type::Long => "Long",
                Type::Double => "Double",
                Type::Byte => "Byte",
                Type::Short => "Short",
                _ => "Float",
            }),
        }
    }
}

/// A position in the Java source, 1-based, for error messages.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Pos {
    pub line: usize,
    pub column: usize,
}

/// The translated file: its classes, and every method and constructor of all of them in one list,
/// so that analyses can refer to any of them by index.
pub struct Class {
    /// The class that declares `main`, which names the program.
    pub name: String,
    pub types: Vec<TypeDecl>,
    pub methods: Vec<Method>,
    /// Subtypes and overriders, computed from the types and methods (see `hierarchy`).
    pub hierarchy: Hierarchy,
    /// `new C(message)` for each standard class `C` whose objects the runtime may raise where
    /// the program could catch them, `message` a `String` variable: how a failure of the runtime
    /// becomes the program's exception (see `lower/exceptions.rs`).
    pub raised: Vec<Expr>,
}

/// What kind of type a declaration is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeKind {
    Class,
    Interface,
    Enum,
    /// A class of the runtime whose values can be `Object`s: `String` and the boxed classes. The
    /// program never declares one: lowering adds them with `Object` (see `OBJECT`).
    Runtime,
    /// A type variable of a generic class or method that is a Rust generic (see
    /// `lower::generics`). Its values are of the Rust type parameter, which nothing implements.
    Param,
}

/// The name of the type of `java.lang.Object`'s values. Lowering adds it to a program that uses
/// `Object`, as an interface that every other type implements, with an abstract `equals` and
/// `hashCode` that the program's own override, and the `Runtime` types beside it.
pub const OBJECT: &str = "Object";

/// The name of the type of `java.lang.Comparable`'s values, which lowering adds to a program that
/// uses it as a type, beside `OBJECT`.
pub const COMPARABLE: &str = "Comparable";

/// The name of the type of `java.lang.Enum`'s values, which lowering adds to a program that uses
/// it as a type (as the JDK's `EnumMap` and `EnumSet` do), beside `OBJECT`: an interface that the
/// program's enums implement.
pub const ENUM: &str = "Enum";
/// The name of the type of `java.lang.Number`'s values, which lowering adds to a program that uses
/// it as a type, beside `OBJECT`: the numeric boxes are its values.
pub const NUMBER: &str = "Number";
/// The name of the type of `java.lang.CharSequence`'s values, which lowering adds to a program
/// that uses it as a type, beside `OBJECT`: strings and builders are its values.
pub const CHAR_SEQUENCE: &str = "CharSequence";

/// The name of the function that builds a constant of the enum `name` with tables
/// (`lower/enum_tables.rs`): its constructor, `initShapeConstant` for `Main.Shape`.
pub fn constant_builder(name: &str) -> String {
    format!("init{}Constant", simple_enum_name(name))
}

/// The name of the function that runs the field initializers of the enum `name` with tables.
pub fn fields_builder(name: &str) -> String {
    format!("init{}Fields", simple_enum_name(name))
}

fn simple_enum_name(name: &str) -> &str {
    name.rsplit(['.', '$']).next().unwrap_or(name)
}

/// The runtime classes whose values an `Object` can hold, with the Java type of those values.
pub fn runtime_values() -> [(&'static str, Type); 9] {
    [
        ("java.lang.String", Type::String),
        ("java.lang.Integer", Type::boxed(Type::Int)),
        ("java.lang.Long", Type::boxed(Type::Long)),
        ("java.lang.Double", Type::boxed(Type::Double)),
        ("java.lang.Float", Type::boxed(Type::Float)),
        ("java.lang.Short", Type::boxed(Type::Short)),
        ("java.lang.Byte", Type::boxed(Type::Byte)),
        ("java.lang.Character", Type::boxed(Type::Char)),
        ("java.lang.Boolean", Type::boxed(Type::Boolean)),
    ]
}

/// The runtime class of the `StringBuilder`s that become `Object`s.
pub const STRING_BUILDER: &str = "java.lang.StringBuilder";

/// The runtime class of the classes (`Class<?>` values) that become `Object`s.
pub const CLASS_VALUE: &str = "java.lang.Class";

/// The runtime class whose values are of type `ty`, a primitive standing for its box.
pub fn runtime_name(ty: Type) -> Option<&'static str> {
    let ty = if ty.is_numeric() || ty == Type::Boolean { Type::boxed(ty) } else { ty };
    runtime_values().into_iter().find(|(_, t)| *t == ty).map(|(name, _)| name)
}

/// The Java type of the values of runtime class `name`.
pub fn runtime_type(name: &str) -> Option<Type> {
    runtime_values().into_iter().find(|(n, _)| *n == name).map(|(_, t)| t)
}

/// A class of the file: its fields. Methods and constructors are in `Class::methods`.
pub struct TypeDecl {
    pub name: String,
    /// The class's package, such as `com.example`; empty for the unnamed package.
    pub package: String,
    pub kind: TypeKind,
    /// An abstract class or an interface, which has no objects of its own.
    pub is_abstract: bool,
    /// The superclass, an index into `Class::types`; `None` for a class that extends `Object`,
    /// interfaces and enums.
    pub extends: Option<usize>,
    /// The interfaces a class implements or an interface extends, indexes into `Class::types`.
    pub implements: Vec<usize>,
    /// Instance fields, in declaration order.
    pub fields: Vec<Field>,
    /// Static fields, in declaration order.
    pub statics: Vec<Field>,
    /// The constants of an enum, in declaration order; `None` for a class.
    pub enum_constants: Option<Vec<EnumConstant>>,
    /// The type parameters of a generic class that is a Rust generic, as types of kind `Param`.
    pub type_params: Vec<Type>,
    /// The enclosing class whose instance objects of this class hold in the field `outer`: an
    /// inner class, or a local or anonymous class created in an instance context, whose code uses
    /// it; else `None`. Every constructor takes it as its first parameter (see `lower/inner.rs`).
    pub outer: Option<usize>,
    /// The locals of the enclosing code that the class's code reads, each held in a field and
    /// passed to every constructor as a parameter after the declared ones. Empty for a member
    /// class.
    pub captures: Vec<Capture>,
    pub origin: Origin,
    /// The class or a superclass implements `Cloneable`, directly or through an interface, so
    /// that `Object`'s `clone` copies its objects rather than throwing.
    pub cloneable: bool,
}

/// A local variable that a local or anonymous class captures.
pub struct Capture {
    /// The local's name in the enclosing code.
    pub name: String,
    /// The field that holds it, declared by the class or by a superclass that captures it too.
    pub field: String,
    pub ty: Type,
}

/// Where a class is declared.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    /// A class of a package, or a member of a class.
    Member,
    /// A local class, declared in a block of `method` (`None`: of an initializer); `binary` is
    /// javac's binary name, such as `Main$1Labeler`.
    Local { method: Option<usize>, binary: String },
    /// An anonymous class, `new T(..) { .. }`, in `method` (`None`: in an initializer), which
    /// implements or extends `T`; `binary` is javac's binary name, such as `Main$1`.
    Anonymous { method: Option<usize>, binary: String },
    /// The class of a lambda or method reference in `method` (`None`: in an initializer);
    /// `binary` is the name the JVM gives such a class, up to the part that varies between runs
    /// (`Main$$Lambda`).
    Lambda { method: Option<usize>, binary: String },
    /// The class of the objects that `new Object()` creates, which Java names `java.lang.Object`.
    Object,
}

impl Origin {
    /// The names that `Class.getName` and `Class.getSimpleName` give a local or anonymous class
    /// or a lambda's: its binary name, and its simple name (empty for an anonymous class).
    pub fn java_names(&self) -> Option<(&str, &str)> {
        match self {
            Origin::Member => None,
            Origin::Local { binary, .. } => {
                let last = binary.rsplit('$').next().unwrap_or_default();
                Some((binary, last.trim_start_matches(|c: char| c.is_ascii_digit())))
            }
            Origin::Anonymous { binary, .. } => Some((binary, "")),
            Origin::Lambda { binary, .. } => Some((binary, binary.rsplit('.').next().unwrap_or_default())),
            Origin::Object => Some(("java.lang.Object", "Object")),
        }
    }
}

/// One constant of an enum: `PLANET(3.0, 4.0)`.
pub struct EnumConstant {
    pub name: String,
    /// The value of each instance field, in declaration order: the constructor's assignment with
    /// its parameters replaced by the constant's arguments (see `lower/enums.rs`).
    pub values: Vec<Expr>,
}

pub struct Field {
    pub name: String,
    pub ty: Type,
    pub init: Option<Expr>,
    /// A `static final` compile-time constant: a Rust `const`, with no cell.
    pub constant: bool,
    pub pos: Pos,
}

pub struct Method {
    /// `<init>` for a constructor.
    pub name: String,
    pub params: Vec<Param>,
    pub ret: Type,
    pub body: Vec<Stmt>,
    /// `public static void main(String[] args)`, the program's entry point.
    pub is_main: bool,
    pub is_static: bool,
    pub is_ctor: bool,
    /// The index of the declaring class in `Class::types`.
    pub class: usize,
    pub pos: Pos,
    /// A method without a body: abstract, or an interface method that is not a default method.
    pub is_abstract: bool,
    /// A `synchronized` method, which holds the monitor of its object, or of its class when
    /// static, for the call.
    pub is_synchronized: bool,
    /// The methods of the program that this one overrides, as javac lists them (indexes into
    /// `Class::methods`).
    pub overrides: Vec<usize>,
    /// The type parameters of a generic method that is a Rust generic, as types of kind `Param`.
    pub type_params: Vec<Type>,
    /// The code is the JDK's, though the class is the program's: `CharSequence`'s default
    /// `getChars` in a class without one of its own (`lower::object`). Its class file is the
    /// JDK's, which has the names of its locals.
    pub jdk_code: bool,
}

pub struct Param {
    pub name: String,
    pub ty: Type,
}

pub enum Stmt {
    Local {
        ty: Type,
        name: String,
        init: Option<Expr>,
        pos: Pos,
    },
    Expr(Expr),
    If {
        cond: Expr,
        then: Box<Stmt>,
        otherwise: Option<Box<Stmt>>,
    },
    While {
        cond: Expr,
        body: Box<Stmt>,
    },
    DoWhile {
        body: Box<Stmt>,
        cond: Expr,
    },
    For {
        init: Vec<Stmt>,
        cond: Option<Expr>,
        update: Vec<Expr>,
        body: Box<Stmt>,
    },
    /// `for (ty name : array) body`, over the elements of an array.
    ForEach {
        ty: Type,
        name: String,
        array: Expr,
        body: Box<Stmt>,
    },
    Return(Option<Expr>, Pos),
    Break,
    Continue,
    Block(Vec<Stmt>),
    /// A `switch` statement on an integer, `char` or `String` selector.
    Switch {
        selector: Expr,
        cases: Vec<Case>,
    },
    /// `label: body`: a loop or a block that `break label` or `continue label` names.
    Labeled {
        label: String,
        body: Box<Stmt>,
    },
    /// `break label;`, leaving the labeled statement.
    BreakLabel(String),
    /// `continue label;`, continuing the labeled loop.
    ContinueLabel(String),
    /// `yield value;`, the value of the innermost switch expression.
    Yield(Expr),
    /// `throw value;`. The value has been converted to `java.lang.Throwable`'s type.
    Throw(Expr),
    /// `try body catch (..) .. finally ..`.
    Try {
        body: Vec<Stmt>,
        catches: Vec<Catch>,
        finally: Option<Vec<Stmt>>,
    },
}

/// One `catch` clause: the exception classes it catches (indexes into `Class::types`, several
/// for a multi-catch `catch (A | B e)`), its parameter, and its statements.
pub struct Catch {
    pub types: Vec<usize>,
    pub name: String,
    /// The parameter's type: the one class it catches, or for a multi-catch the closest class
    /// above all of them.
    pub ty: Type,
    pub body: Vec<Stmt>,
    pub pos: Pos,
}

/// One `case` group of a switch: the labels that select it and its statements. A group falls
/// through into the next one unless its statements end by leaving (`break`, `return`, ...). An
/// arrow case ends with a `Break`, so it never falls through.
pub struct Case {
    pub labels: Vec<CaseLabel>,
    pub body: Vec<Stmt>,
}

pub enum CaseLabel {
    /// A constant of the selector's type.
    Value(Expr),
    Default,
}

/// Identifies an expression, for facts that analyses attach to it.
pub type ExprId = usize;

pub struct Expr {
    pub kind: ExprKind,
    pub pos: Pos,
    /// Filled in by the checker.
    pub ty: Type,
    /// Unique within the class.
    pub id: ExprId,
}

pub enum ExprKind {
    Int(i32),
    Long(i64),
    Double(f64),
    /// A `char` literal, as its UTF-16 code unit.
    Char(u16),
    Float(f32),
    Bool(bool),
    Str(String),
    /// The `null` literal; its type is `Type::Null` until it meets a reference type.
    Null,
    Var(String),
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// `target = value`, or `target op= value` when `op` is set.
    Assign {
        op: Option<BinOp>,
        target: Place,
        value: Box<Expr>,
    },
    /// `++x`, `x++`, `--x`, `x--`.
    IncDec {
        target: Place,
        increment: bool,
        prefix: bool,
    },
    Cast(Type, Box<Expr>),
    /// `value instanceof T`, or with a pattern, `value instanceof T binding`, which binds the
    /// variable `binding` where the test is true.
    InstanceOf {
        value: Box<Expr>,
        ty: Type,
        binding: Option<String>,
    },
    /// `new T[d1][d2]...` with the given sizes; `ty` is the full array type.
    NewArray(Vec<Expr>),
    /// `{a, b, c}` or `new T[] {a, b, c}`; `ty` is the array type.
    ArrayLit(Vec<Expr>),
    /// `a[i]`.
    Index(Box<Expr>, Box<Expr>),
    /// `a.length`.
    Length(Box<Expr>),
    Conditional(Box<Expr>, Box<Expr>, Box<Expr>),
    /// A call of one of the class's static methods; `resolved` is the index of the chosen
    /// overload in `Class::methods`, set by the checker.
    Call {
        name: String,
        args: Vec<Expr>,
        resolved: Option<usize>,
    },
    /// A call of a library method such as `System.out.println` or `Math.max`.
    Builtin(Builtin, Vec<Expr>),
    /// `this`: the object an instance method or constructor runs on.
    This,
    /// `object.name`, an instance field.
    Field {
        object: Box<Expr>,
        name: String,
    },
    /// `Class.name`, a static field; `class` indexes `Class::types`.
    Static {
        class: usize,
        name: String,
    },
    /// `new C(args)`; `ctor` is the chosen constructor, an index into `Class::methods`.
    New {
        class: usize,
        args: Vec<Expr>,
        ctor: Option<usize>,
    },
    /// `receiver.name(args)`, a call of an instance method; `resolved` as for `Call`. A call
    /// runs the implementation of the receiver's class (see `Hierarchy::targets`), except
    /// `super.name(args)` (`is_super`), whose receiver is `this` and which runs `resolved`.
    MethodCall {
        receiver: Box<Expr>,
        name: String,
        args: Vec<Expr>,
        resolved: Option<usize>,
        is_super: bool,
    },
    /// `this(args)`, a constructor calling another constructor of its class.
    ThisCall {
        args: Vec<Expr>,
        resolved: Option<usize>,
    },
    /// `super(args)`, the first statement of a constructor of a class whose superclass is in
    /// the program: it builds the superclass's part of the object.
    SuperInit {
        args: Vec<Expr>,
        resolved: Option<usize>,
    },
    /// `E.CONSTANT` of an enum; `class` indexes `Class::types` and `ordinal` the constant.
    EnumConstant {
        class: usize,
        ordinal: usize,
    },
    /// A `switch` expression; its value is the `yield` (or arrow expression) that runs.
    Switch {
        selector: Box<Expr>,
        cases: Vec<Case>,
    },
}

/// What an assignment, `++` or `--` writes to.
pub enum Place {
    /// A local variable or parameter.
    Var(String),
    /// An array element `a[i]`.
    Index(Box<Expr>, Box<Expr>),
    /// `object.name`.
    Field { object: Box<Expr>, name: String },
    /// `Class.name`; `class` indexes `Class::types`.
    Static { class: usize, name: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Builtin {
    Println,
    Print,
    Exit,
    /// The `String[]` that `main` gets as `args`: the command-line arguments, each a new string.
    /// Built fresh where it is read (`lower::arrays::args_decl`), so it is an owned array.
    MainArgs,
    /// A `String[]` that is an `Object[]` (see `lower::covariant`) as the `String[]` that a
    /// builtin of strings reads (`String.join`, `Arrays.toString`): a copy, each element cast back.
    StringArrayOf,
    /// The new `String[]` that a builtin makes (`split`) as the `Object[]` that a `String[]` is
    /// where it is covariant: a copy, which no other name sees.
    ObjectArrayOf,
    /// The body of an instance method of a class that has no objects (`Reach::dormant`): it never
    /// runs.
    NoObject {
        class: usize,
    },
    /// The default of a creation from a class name (`lower::by_name`): the name, its argument,
    /// is of no class of the translated program, which throws a `ClassNotFoundException` that
    /// says so, through `java::catching`.
    UnknownClass,
    /// `[class, name]`: `Class.getResourceAsStream(name)`. A translated program has no resources
    /// (the JDK's data files are not in its source), so the program ends with an `InternalError`
    /// that nothing catches. `class` is the name of the class, a string literal.
    MissingResource,
    /// `[]`: runs the static initialization of the class, as its first use would, for
    /// `Class.forName` with a name that the code spells out (`lower::by_name`).
    InitializeClass {
        class: usize,
    },
    /// The lock of a `synchronized` block, the statement before the block: evaluated, and a
    /// `NullPointerException` when it is null. Where the program can tell (`codegen::sync`), the
    /// block holds the lock's monitor.
    MonitorEnter,
    MathAbs,
    MathMax,
    MathMin,
    MathSqrt,
    MathPow,
    /// `s.length()`; the receiver is the first argument.
    StringLength,
    /// `s.equals(t)`; the receiver is the first argument.
    StringEquals,
    /// `Arrays.toString(a)`.
    ArraysToString,
    /// `Arrays.fill(a, v)`.
    ArraysFill,
    /// `Arrays.sort(a)` for an array of a primitive type.
    ArraysSort,
    /// `Arrays.copyOf(a, n)`.
    ArraysCopyOf,
    /// The other `java.util.Arrays` methods and `clone()`, see `ArraysOp`.
    Arrays(ArraysOp),
    /// `s.charAt(i)`; the receiver is the first argument.
    StringCharAt,
    /// `[from, to, length]`: `Objects.checkFromToIndex`, an `IndexOutOfBoundsException` when
    /// `from..to` is not a range of `0..length`.
    CheckFromToIndex,
    /// `[index, length]`: `Objects.checkIndex`, an `IndexOutOfBoundsException` when `index` is
    /// outside `0..length`.
    CheckIndex,
    /// `Character.isDigit(char)` and the other `Character` methods on a `char`.
    CharIsDigit,
    CharIsLetter,
    CharIsUpperCase,
    CharIsLowerCase,
    CharToUpperCase,
    CharToLowerCase,
    CharIsWhitespace,
    CharIsSpaceChar,
    CharIsLetterOrDigit,
    CharIsAlphabetic,
    CharIsSurrogate,
    CharIsHighSurrogate,
    CharIsLowSurrogate,
    /// `Character.digit(char, int)`, and `Character.forDigit(int, int)`, which makes a `char`.
    CharDigit,
    CharForDigit,
    /// A method of the native layer: `System.arraycopy`, `Math.sin` and the like.
    Native(Native),
    /// `x.toString()` of `Object` on an object (the receiver is the argument): the text that
    /// printing it shows, its class's `toString` or `Object`'s `Name@hash`.
    ObjectText,
    /// `super.clone()` of `Object` in a `Cloneable` class (the receiver is the argument): a new
    /// object of the same class with a copy of each of its fields.
    ObjectClone,
    /// `e.ordinal()`; the receiver is the first argument.
    EnumOrdinal,
    /// `e.name()` and `e.toString()`, which the enum's default `toString` returns as `name()`.
    EnumName,
    /// `a.compareTo(b)` on enums, the difference of the ordinals.
    EnumCompareTo,
    /// `E.valueOf(s)` for the enum type of the call's result.
    EnumValueOf,
    /// The failure of `Enum.valueOf(c, s)` for a class `c` that is no enum of the program: an
    /// `IllegalArgumentException`, `<name> is not an enum class`, of the class's name.
    NotEnumClass,
    /// `Unsafe`'s read (or with `put`, write) of a primitive of type `ty` at an offset of a base
    /// that holds an array of primitives, by the array's bytes (`java::get_memory`): the
    /// arguments are the base as an `Object`, the offset and, for a write, the value. An offset
    /// outside the array, or a base that is null or no such array, ends the program.
    RawMemory {
        ty: Type,
        put: bool,
    },
    /// `Character.getNumericValue(c)`: the digit value of a letter or digit, or -1.
    CharNumericValue,
    /// A method of a boxed class (`Integer.parseInt`, `x.intValue()`), see `BoxedOp`, with the
    /// primitive of the boxed class it belongs to (`Type::Int` for `Integer`).
    Boxed(BoxedOp, Type),
    /// A method of `String`, see `StringOp`.
    String(StringOp),
    /// A method of `StringBuilder`, see `BuilderOp`.
    Builder(BuilderOp),
    /// A charset, see `CharsetOp`: the value of a `StandardCharsets` constant or `forName`.
    Charset(CharsetOp),
    /// `String.format(format, args...)` and `format.formatted(args...)`: the format, then the
    /// arguments.
    Format,
    /// A bit or radix method of `Integer` or `Long` (see `BitsOp`), with the primitive of the
    /// class.
    Bits(BitsOp, Type),
    /// An unsigned method of `Integer`, `Long`, `Byte` or `Short`, see `UnsignedOp`.
    Unsigned(UnsignedOp, Type),
    /// A code point method of `Character` (`toChars`, `charCount` and the like), see `CodePointOp`.
    CodePoint(CodePointOp),
    /// `Objects.equals(a, b)` on two strings, boxes of one kind, or objects; for objects a third
    /// argument is the call `a.equals(b)` that runs when `a` is not null.
    ObjectsEquals,
    /// `Objects.hashCode(o)` on a string, a box, an enum or an object; for an object a second
    /// argument is the call `o.hashCode()` that runs when `o` is not null.
    ObjectsHashCode,
    /// `[selector, value]`: `value`, once a pattern switch has checked its selector (a variable)
    /// for null (see `lower::patterns`); `[selector]`: the check alone.
    NullChecked,
    /// `[selector]`: the end of an exhaustive pattern switch that no case matched, which only a
    /// null selector reaches: it throws `NullPointerException`.
    Unmatched,
    /// `o.getClass().getName()`: Java's binary name of the class of the object `o`, such as
    /// `java.lang.IllegalStateException` or `Bank$Overdrawn`; with `simple`,
    /// `o.getClass().getSimpleName()`, its name in its source: `IllegalStateException`, `Overdrawn`.
    ClassName {
        simple: bool,
    },
    /// `Point.class.getName()`: the binary name of class `class` (an index into
    /// `Class::types`); with `simple`, `Point.class.getSimpleName()`. A class literal compared
    /// with a class is its binary name too (see `lower::classes`).
    ClassLiteralName {
        class: usize,
        simple: bool,
    },
    /// `[a, b]`: whether two classes are the same, `a.getClass() == b.getClass()` (`!same`:
    /// `!=`), where each is the binary name of an object's class (`ClassName`) or of a class
    /// literal (`ClassLiteralName`).
    SameClass {
        same: bool,
    },
    /// `[o]`: `o.getClass()`, a `java::Class`.
    GetClass,
    /// `[]`: the class literal `T.class` of the type, a `java::Class`.
    ClassLiteral(Type),
    /// `[class, args...]`: a method of `java.lang.Class`.
    ClassMethod(ClassOp),
    /// `[class, x]`: `class.isInstance(x)`, whether `x` is not null and of `class` or a subclass,
    /// told by the binary names of the classes and interfaces of `x`'s class.
    ClassIsInstance,
    /// `[class, x]`, a statement: the check of `class.cast(x)`, a `ClassCastException` (`Cannot
    /// cast A to B`) unless `x` is null or `class.isInstance(x)`. `lower::classes` writes the cast
    /// that gives the value after it.
    ClassCheck,
    /// `[x]`: `HashMap.comparableClassFor(x)` (and `ConcurrentHashMap`'s, the same), the class of `x` if it says `implements
    /// Comparable` itself, else `null`; the JDK's reads the type argument through reflection.
    ComparableClassFor,
    /// `[expected, a[i] = x]` (the compare forms) or `[a[i] = x]` (`a[i] += delta` for
    /// get-and-add): an atomic update of an array element through `Unsafe` at an element offset
    /// (`lower::handles`), made under one borrow of the array. The operands are the store it
    /// makes, as an assignment, for the analyses to see; its value is the old element, or for
    /// compare-and-set whether it was `expected`.
    ElementUpdate(crate::program::FieldUpdate),
    /// `[a[i]]` (a read) or `[a[i] = (byte) v]` (a write): the value of type `ty` (`short`,
    /// `char`, `int`, `long`, `float` or `double`) whose bytes are those of the `byte[]` `a` from
    /// index `i` on, in big-endian order or not, as a `VarHandle` of
    /// `MethodHandles.byteArrayViewVarHandle` reads and writes it (`lower::handles`). The operand
    /// is the element access at `i`, for the analyses to see the array read or written there.
    ByteView {
        ty: Type,
        big_endian: bool,
    },
}

/// The methods of `java.lang.Class` that a program can use without reflection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClassOp {
    GetName,
    GetSimpleName,
    /// `null` for `Object`, an interface or a primitive type.
    GetSuperclass,
    /// `null` for a class that is not an array.
    GetComponentType,
    IsArray,
    IsInterface,
    IsPrimitive,
    IsEnum,
    ToString,
    HashCode,
    /// `[class, other]`: `equals`, of two classes.
    Equals,
    /// `desiredAssertionStatus()`: assertions are off, as in a JVM started without `-ea`.
    DesiredAssertionStatus,
    /// `getCanonicalName()`, `null` for a local or anonymous class.
    GetCanonicalName,
    GetPackageName,
    /// `isAnnotation()`: false, as a translation has no class values of annotation types (their
    /// class literals are rejected, and no object's class is one).
    IsAnnotation,
}

impl ClassOp {
    /// Whether the result can be `null`.
    pub fn nullable(self) -> bool {
        matches!(self, ClassOp::GetSuperclass | ClassOp::GetComponentType | ClassOp::GetCanonicalName)
    }
}

/// The methods of the boxed classes. The kind of the boxes comes from the types of the operands
/// and of the result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxedOp {
    /// `Integer.parseInt(s)` and the other `parseX`: the primitive of the result's kind, read from
    /// a string. `Integer.valueOf(s)` is the same, boxed.
    Parse,
    /// `Integer.parseInt(s, radix)`, `Long.parseLong(s, radix)` and `Integer.valueOf(s, radix)`:
    /// the same, read in the radix given as the second argument.
    ParseRadix,
    /// `Integer.decode(s)`, `Long.decode`, `Short.decode` and `Byte.decode`: the boxed kind of
    /// the result, read from a string with a sign and a prefix of its radix (`0x`, `#`, `0`).
    Decode,
    /// `Integer.valueOf(i)`: the boxed kind of the result, from its primitive argument.
    Box,
    /// `x.intValue()` and the other `xValue()`: the result's primitive, from the receiver.
    Unbox,
    /// `Integer.toString(i)` or `x.toString()`.
    ToString,
    /// `Double.toHexString(d)` or `Float.toHexString(f)`: the hexadecimal form of the value.
    HexString,
    /// `Integer.compare(a, b)` or `x.compareTo(y)`.
    Compare,
    /// `Integer.hashCode(i)` or `x.hashCode()`.
    HashCode,
    /// `x.equals(y)`.
    Equals,
    /// `Integer.sum(a, b)`.
    Sum,
    /// `Double.isNaN(d)`, `isInfinite` and `isFinite`, static or on a box: the Rust method of the
    /// same test on the primitive, such as `is_nan`.
    FloatTest(&'static str),
    /// `Boolean.logicalAnd(a, b)`, `logicalOr` and `logicalXor`: `&`, `|` or `^` on both operands,
    /// neither short-circuited.
    Logical(BinOp),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Plus,
    Not,
    BitNot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Shl,
    Shr,
    UShr,
    BitAnd,
    BitOr,
    BitXor,
    And,
    Or,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl BinOp {
    pub fn java(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Rem => "%",
            BinOp::Shl => "<<",
            BinOp::Shr => ">>",
            BinOp::UShr => ">>>",
            BinOp::BitAnd => "&",
            BinOp::BitOr => "|",
            BinOp::BitXor => "^",
            BinOp::And => "&&",
            BinOp::Or => "||",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
        }
    }

    pub fn is_comparison(self) -> bool {
        matches!(self, BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge)
    }

    pub fn is_shift(self) -> bool {
        matches!(self, BinOp::Shl | BinOp::Shr | BinOp::UShr)
    }
}

/// A translation error: a syntax error, a type error or a Java feature not supported yet.
#[derive(Debug)]
pub struct Error {
    pub pos: Pos,
    pub message: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}: {}", self.pos.line, self.pos.column, self.message)
    }
}

impl std::error::Error for Error {}

pub fn error<T>(pos: Pos, message: impl Into<String>) -> Result<T, Error> {
    Err(Error { pos, message: message.into() })
}

// Queries over the syntax tree, shared by the analyses and the code generator.

/// Whether `e` evaluates to the same value on every iteration of a loop with this body.
pub fn stable(e: &Expr, loop_var: &str, body: &Stmt) -> bool {
    match &e.kind {
        ExprKind::Int(_) | ExprKind::Long(_) | ExprKind::Char(_) => true,
        ExprKind::Var(v) => v != loop_var && !assigns_stmt(body, v),
        // Strings are immutable, so the length of a variable that is not assigned is stable.
        ExprKind::Builtin(Builtin::StringLength, args) => stable(&args[0], loop_var, body),
        // An array's length never changes, so it is stable while the variable is.
        ExprKind::Unary(_, x) | ExprKind::Cast(_, x) | ExprKind::Length(x) => stable(x, loop_var, body),
        ExprKind::Binary(_, l, r) => stable(l, loop_var, body) && stable(r, loop_var, body),
        _ => false,
    }
}

/// Whether `e` assigns the local variable `name`.
pub fn assigns(e: &Expr, name: &str) -> bool {
    let mut found = false;
    visit_expr(e, &mut |x| match &x.kind {
        ExprKind::Assign { target: Place::Var(target), .. }
        | ExprKind::IncDec { target: Place::Var(target), .. } => found |= target == name,
        _ => {}
    });
    found
}

pub fn assigns_any(stmts: &[Stmt], name: &str) -> bool {
    stmts.iter().any(|s| assigns_stmt(s, name))
}

pub fn assigns_stmt(stmt: &Stmt, name: &str) -> bool {
    let mut found = false;
    visit_stmt(stmt, &mut |e| found |= assigns(e, name), &mut |_| {});
    found
}

/// Whether `e` reads or writes the variable `name`.
/// Whether evaluating `e` can do something observable: a call, an assignment, `++` or `--`, a
/// print, or a method of the native layer.
pub fn has_effects(e: &Expr) -> bool {
    let mut found = false;
    visit_expr(e, &mut |x| {
        found |= matches!(
            x.kind,
            ExprKind::Call { .. }
                | ExprKind::MethodCall { .. }
                | ExprKind::New { .. }
                | ExprKind::ThisCall { .. }
                | ExprKind::SuperInit { .. }
                | ExprKind::Assign { .. }
                | ExprKind::IncDec { .. }
                | ExprKind::Builtin(
                    Builtin::Print
                        | Builtin::Println
                        | Builtin::Exit
                        | Builtin::NoObject { .. }
                        | Builtin::UnknownClass
                        | Builtin::MissingResource
                        | Builtin::InitializeClass { .. }
                        | Builtin::Native(_),
                    _
                )
                | ExprKind::Builtin(
                    Builtin::Unsigned(
                        UnsignedOp::Divide
                            | UnsignedOp::Remainder
                            | UnsignedOp::Parse
                            | UnsignedOp::ParseRange,
                        _
                    ),
                    _
                )
                | ExprKind::Builtin(
                    Builtin::CodePoint(
                        CodePointOp::ToChars | CodePointOp::ToCharsInto | CodePointOp::ToString,
                    ),
                    _
                )
        ) || matches!(x.kind, ExprKind::Builtin(Builtin::Builder(op), _) if op.mutates())
    });
    found
}

/// Whether an expression reads or uses `this`.
pub fn mentions_this(e: &Expr) -> bool {
    let mut found = false;
    visit_expr(e, &mut |x| found |= matches!(x.kind, ExprKind::This));
    found
}

/// The index in a constructor's `body` of its `this(..)` or `super(..)`. Java 25 lets statements
/// that do not use `this` come before it (see `lower::prologues`); in most constructors it is
/// first.
pub fn explicit_call(body: &[Stmt]) -> Option<usize> {
    body.iter().position(|s| {
        matches!(s, Stmt::Expr(Expr { kind: ExprKind::ThisCall { .. } | ExprKind::SuperInit { .. }, .. }))
    })
}

/// Whether a constructor's `body` calls another constructor of its class with `this(..)`.
pub fn delegates(body: &[Stmt]) -> bool {
    explicit_call(body)
        .is_some_and(|i| matches!(&body[i], Stmt::Expr(Expr { kind: ExprKind::ThisCall { .. }, .. })))
}

/// A constructor's `body` split at its `this(..)` or `super(..)`: the statements before it, the
/// call, and the statements after it.
pub fn split_explicit_call(body: &[Stmt]) -> (&[Stmt], Option<&Expr>, &[Stmt]) {
    match explicit_call(body) {
        Some(i) => match &body[i] {
            Stmt::Expr(call) => (&body[..i], Some(call), &body[i + 1..]),
            _ => unreachable!("an explicit constructor call is an expression statement"),
        },
        None => (&[], None, body),
    }
}

/// The local that holds the value a constructor's statements before `super(..)` assign to field
/// `field`, until the object exists (see `lower::prologues`).
pub fn early_field_local(field: &str) -> String {
    format!("early${field}")
}

/// Whether `stmt` is `this.f = early$f`, the assignment that sets a field assigned before
/// `super(..)` once the object exists.
pub fn is_early_field_assignment(stmt: &Stmt) -> bool {
    matches!(stmt, Stmt::Expr(Expr {
        kind: ExprKind::Assign { op: None, target: Place::Field { object, name }, value },
        ..
    }) if matches!(object.kind, ExprKind::This)
        && matches!(&value.kind, ExprKind::Var(v) if *v == early_field_local(name)))
}

pub fn mentions(e: &Expr, name: &str) -> bool {
    let mut found = false;
    visit_expr(e, &mut |x| match &x.kind {
        ExprKind::Var(v)
        | ExprKind::Assign { target: Place::Var(v), .. }
        | ExprKind::IncDec { target: Place::Var(v), .. } => found |= v == name,
        _ => {}
    });
    found
}

/// Calls `on_expr` on every top-level expression and `on_stmt` on every statement in `stmt`, and on
/// the statements inside the switch expressions of those expressions, at any depth.
pub fn visit_stmt(stmt: &Stmt, on_expr: &mut dyn FnMut(&Expr), on_stmt: &mut dyn FnMut(&Stmt)) {
    walk_stmt(stmt, on_expr, on_stmt, true);
}

/// Calls `on_stmt` on every statement in `stmt` except those inside its switch expressions. For
/// callers that count what belongs to one switch expression or loop: a `yield` or `continue` in a
/// nested switch expression belongs to that expression, and Java does not let one leave it.
pub fn visit_stmt_outside_switches(stmt: &Stmt, on_stmt: &mut dyn FnMut(&Stmt)) {
    walk_stmt(stmt, &mut |_| {}, on_stmt, false);
}

/// `visit_stmt`, where `switches` says whether the statements of switch expressions are reported.
/// `visit_expr` walks those itself, so its own walk turns this off to report each one once.
fn walk_stmt(stmt: &Stmt, on_expr: &mut dyn FnMut(&Expr), on_stmt: &mut dyn FnMut(&Stmt), switches: bool) {
    on_stmt(stmt);
    match stmt {
        Stmt::Local { init: Some(e), .. } => top_expr(e, on_expr, on_stmt, switches),
        Stmt::Local { init: None, .. } => {}
        Stmt::Expr(e) | Stmt::Return(Some(e), _) => top_expr(e, on_expr, on_stmt, switches),
        Stmt::If { cond, then, otherwise } => {
            top_expr(cond, on_expr, on_stmt, switches);
            walk_stmt(then, on_expr, on_stmt, switches);
            if let Some(o) = otherwise {
                walk_stmt(o, on_expr, on_stmt, switches);
            }
        }
        Stmt::While { cond, body } | Stmt::DoWhile { body, cond } => {
            top_expr(cond, on_expr, on_stmt, switches);
            walk_stmt(body, on_expr, on_stmt, switches);
        }
        Stmt::For { init, cond, update, body } => {
            init.iter().for_each(|s| walk_stmt(s, on_expr, on_stmt, switches));
            if let Some(cond) = cond {
                top_expr(cond, on_expr, on_stmt, switches);
            }
            for e in update {
                top_expr(e, on_expr, on_stmt, switches);
            }
            walk_stmt(body, on_expr, on_stmt, switches);
        }
        Stmt::ForEach { array, body, .. } => {
            top_expr(array, on_expr, on_stmt, switches);
            walk_stmt(body, on_expr, on_stmt, switches);
        }
        Stmt::Block(stmts) => stmts.iter().for_each(|s| walk_stmt(s, on_expr, on_stmt, switches)),
        Stmt::Switch { selector, cases } => {
            top_expr(selector, on_expr, on_stmt, switches);
            for case in cases {
                for label in &case.labels {
                    if let CaseLabel::Value(e) = label {
                        top_expr(e, on_expr, on_stmt, switches);
                    }
                }
                case.body.iter().for_each(|s| walk_stmt(s, on_expr, on_stmt, switches));
            }
        }
        Stmt::Labeled { body, .. } => walk_stmt(body, on_expr, on_stmt, switches),
        Stmt::Yield(e) | Stmt::Throw(e) => top_expr(e, on_expr, on_stmt, switches),
        Stmt::Try { body, catches, finally } => {
            body.iter().for_each(|s| walk_stmt(s, on_expr, on_stmt, switches));
            for catch in catches {
                catch.body.iter().for_each(|s| walk_stmt(s, on_expr, on_stmt, switches));
            }
            finally.iter().flatten().for_each(|s| walk_stmt(s, on_expr, on_stmt, switches));
        }
        Stmt::Return(None, _)
        | Stmt::Break
        | Stmt::Continue
        | Stmt::BreakLabel(_)
        | Stmt::ContinueLabel(_) => {}
    }
}

/// Calls `on_expr` on a top-level expression of a statement, then `on_stmt` on the statements of
/// the switch expressions inside it (see `switch_stmts`) when `switches` is set.
fn top_expr(e: &Expr, on_expr: &mut dyn FnMut(&Expr), on_stmt: &mut dyn FnMut(&Stmt), switches: bool) {
    on_expr(e);
    if switches {
        switch_stmts(e, on_stmt);
    }
}

/// Calls `on_stmt` on the statements of every switch expression in `e`, at any depth. Each case
/// body is walked once, by the switch expression that owns it; `walk_stmt` does not descend into
/// the expressions of those statements, which `visit_expr` already reaches.
fn switch_stmts(e: &Expr, on_stmt: &mut dyn FnMut(&Stmt)) {
    visit_expr(e, &mut |x| {
        if let ExprKind::Switch { cases, .. } = &x.kind {
            for case in cases {
                for stmt in &case.body {
                    walk_stmt(stmt, &mut |_| {}, on_stmt, false);
                }
            }
        }
    });
}

/// Calls `f` on `e` and every expression inside it.
pub fn visit_expr(e: &Expr, f: &mut dyn FnMut(&Expr)) {
    f(e);
    match &e.kind {
        ExprKind::Unary(_, x)
        | ExprKind::Cast(_, x)
        | ExprKind::InstanceOf { value: x, .. }
        | ExprKind::Length(x)
        | ExprKind::Field { object: x, .. } => visit_expr(x, f),
        ExprKind::Assign { target, value, .. } => {
            visit_place(target, f);
            visit_expr(value, f);
        }
        ExprKind::IncDec { target, .. } => visit_place(target, f),
        ExprKind::Binary(_, l, r) | ExprKind::Index(l, r) => {
            visit_expr(l, f);
            visit_expr(r, f);
        }
        ExprKind::NewArray(items) | ExprKind::ArrayLit(items) => items.iter().for_each(|a| visit_expr(a, f)),
        ExprKind::Conditional(c, a, b) => {
            visit_expr(c, f);
            visit_expr(a, f);
            visit_expr(b, f);
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            visit_expr(receiver, f);
            args.iter().for_each(|a| visit_expr(a, f))
        }
        ExprKind::Call { args, .. }
        | ExprKind::Builtin(_, args)
        | ExprKind::New { args, .. }
        | ExprKind::ThisCall { args, .. }
        | ExprKind::SuperInit { args, .. } => args.iter().for_each(|a| visit_expr(a, f)),
        ExprKind::Switch { selector, cases } => {
            visit_expr(selector, f);
            for case in cases {
                for label in &case.labels {
                    if let CaseLabel::Value(e) = label {
                        visit_expr(e, f);
                    }
                }
                for stmt in &case.body {
                    walk_stmt(stmt, &mut |e| visit_expr(e, f), &mut |_| {}, false);
                }
            }
        }
        _ => {}
    }
}

/// Calls `f` on the expressions inside a place: `a` and `i` in `a[i]`, the object of a field.
pub fn visit_place(place: &Place, f: &mut dyn FnMut(&Expr)) {
    match place {
        Place::Index(array, index) => {
            visit_expr(array, f);
            visit_expr(index, f);
        }
        Place::Field { object, .. } => visit_expr(object, f),
        Place::Var(_) | Place::Static { .. } => {}
    }
}

/// Calls `f` on every expression inside `e` and then on `e`, innermost first, so that `f` may
/// replace an expression after its parts.
pub fn rewrite_expr(e: &mut Expr, f: &mut dyn FnMut(&mut Expr)) {
    match &mut e.kind {
        ExprKind::Unary(_, x)
        | ExprKind::Cast(_, x)
        | ExprKind::InstanceOf { value: x, .. }
        | ExprKind::Length(x)
        | ExprKind::Field { object: x, .. } => rewrite_expr(x, f),
        ExprKind::Assign { target, value, .. } => {
            rewrite_place(target, f);
            rewrite_expr(value, f);
        }
        ExprKind::IncDec { target, .. } => rewrite_place(target, f),
        ExprKind::Binary(_, l, r) | ExprKind::Index(l, r) => {
            rewrite_expr(l, f);
            rewrite_expr(r, f);
        }
        ExprKind::NewArray(items) | ExprKind::ArrayLit(items) => {
            items.iter_mut().for_each(|a| rewrite_expr(a, f))
        }
        ExprKind::Conditional(c, a, b) => {
            rewrite_expr(c, f);
            rewrite_expr(a, f);
            rewrite_expr(b, f);
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            rewrite_expr(receiver, f);
            args.iter_mut().for_each(|a| rewrite_expr(a, f))
        }
        ExprKind::Call { args, .. }
        | ExprKind::Builtin(_, args)
        | ExprKind::New { args, .. }
        | ExprKind::ThisCall { args, .. }
        | ExprKind::SuperInit { args, .. } => args.iter_mut().for_each(|a| rewrite_expr(a, f)),
        ExprKind::Switch { selector, cases } => {
            rewrite_expr(selector, f);
            rewrite_cases(cases, f);
        }
        _ => {}
    }
    f(e);
}

fn rewrite_place(place: &mut Place, f: &mut dyn FnMut(&mut Expr)) {
    match place {
        Place::Index(array, index) => {
            rewrite_expr(array, f);
            rewrite_expr(index, f);
        }
        Place::Field { object, .. } => rewrite_expr(object, f),
        Place::Var(_) | Place::Static { .. } => {}
    }
}

fn rewrite_cases(cases: &mut [Case], f: &mut dyn FnMut(&mut Expr)) {
    for case in cases {
        for label in &mut case.labels {
            if let CaseLabel::Value(e) = label {
                rewrite_expr(e, f);
            }
        }
        case.body.iter_mut().for_each(|s| rewrite_stmt(s, f));
    }
}

/// `rewrite_expr` on every expression of `stmt`, at any depth.
pub fn rewrite_stmt(stmt: &mut Stmt, f: &mut dyn FnMut(&mut Expr)) {
    match stmt {
        Stmt::Local { init: Some(e), .. }
        | Stmt::Expr(e)
        | Stmt::Return(Some(e), _)
        | Stmt::Yield(e)
        | Stmt::Throw(e) => rewrite_expr(e, f),
        Stmt::If { cond, then, otherwise } => {
            rewrite_expr(cond, f);
            rewrite_stmt(then, f);
            if let Some(o) = otherwise {
                rewrite_stmt(o, f);
            }
        }
        Stmt::While { cond, body } | Stmt::DoWhile { body, cond } => {
            rewrite_expr(cond, f);
            rewrite_stmt(body, f);
        }
        Stmt::For { init, cond, update, body } => {
            init.iter_mut().for_each(|s| rewrite_stmt(s, f));
            if let Some(cond) = cond {
                rewrite_expr(cond, f);
            }
            update.iter_mut().for_each(|e| rewrite_expr(e, f));
            rewrite_stmt(body, f);
        }
        Stmt::ForEach { array, body, .. } => {
            rewrite_expr(array, f);
            rewrite_stmt(body, f);
        }
        Stmt::Block(stmts) => stmts.iter_mut().for_each(|s| rewrite_stmt(s, f)),
        Stmt::Switch { selector, cases } => {
            rewrite_expr(selector, f);
            rewrite_cases(cases, f);
        }
        Stmt::Labeled { body, .. } => rewrite_stmt(body, f),
        Stmt::Try { body, catches, finally } => {
            body.iter_mut().for_each(|s| rewrite_stmt(s, f));
            for catch in catches {
                catch.body.iter_mut().for_each(|s| rewrite_stmt(s, f));
            }
            finally.iter_mut().flatten().for_each(|s| rewrite_stmt(s, f));
        }
        Stmt::Local { init: None, .. }
        | Stmt::Return(None, _)
        | Stmt::Break
        | Stmt::Continue
        | Stmt::BreakLabel(_)
        | Stmt::ContinueLabel(_) => {}
    }
}
