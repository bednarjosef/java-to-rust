//! Field and element offsets: the JDK reaches some of its fields, and the elements of some of its
//! arrays, through `Unsafe` with an offset, as `LazyConstantImpl` and `ConcurrentHashMap` do (see
//! docs/multi-threading.md, 5.3):
//!
//! ```java
//! private static final long CONSTANT_OFFSET = UNSAFE.objectFieldOffset(LazyConstantImpl.class, "constant");
//! ... UNSAFE.getReferenceAcquire(this, CONSTANT_OFFSET) ...
//! private static final long ABASE = U.arrayBaseOffset(Node[].class);
//! ... U.compareAndSetReference(tab, ((long)i << ASHIFT) + ABASE, c, v) ...
//! ```
//!
//! A static final field whose value is such an `objectFieldOffset` is a *field handle*: it names
//! the field, and it is no field of the translation. Its value is its initializer, or the one
//! assignment to it in a static block, which is then left out (`Random` writes
//! `seedOffset = unsafe.objectFieldOffset(Random.class.getDeclaredField("seed"))` in a `try`);
//! the class may be a class literal or a local of the block that holds one, and the call may also
//! be written as the offset argument itself, or made by a static helper of a class and a name
//! (`offset_helper_call`: JCTools' `UnsafeAccess.fieldOffset(Queue.class, "producerIndex")` in
//! netty), whose class `sun.misc.Unsafe`'s warning names. The field may be one of another class, such as the
//! `threadLocalRandomProbe` of `Thread` that `ThreadLocalRandom` reads, as long as the object
//! of the access is of that class. A read or write through the handle is a read or write of the
//! field itself, so the call above lowers to `this.constant`. That is Java's meaning in a program
//! with one thread, and in a program with threads the field is the object's, read and written
//! under its lock, which orders the accesses at least as the plain, opaque, acquire, release and
//! volatile forms ask. Any other use of a handle is rejected. `objectFieldOffset(f)` of a `Field`
//! that is no handle is a number of the field's own, which no access can use
//! (`field_offset_value`).
//!
//! An atomic update through a handle, `U.compareAndSetInt(this, VALUE, expect, update)` and the
//! get-and-add, get-and-bitwise-or (and `And`, `Xor`), get-and-set and compare-and-exchange forms
//! of `int`, `long` and reference fields, and the forms of `float`, `double`, `byte`, `short`,
//! `char` and `boolean` fields that `program::unsafe_field_update` names (the JDK writes them in
//! Java over the `int` and `long` forms, at the same offset or at the word that holds the field,
//! so their bodies are not translated) (`program::FieldUpdate`), is a call of a method that
//! lowering adds to the field's class, one
//! per field and form: `compareAndSetValue(expected, value)` reads the field, compares and
//! writes in its body (`Method::field_update` names the update). On an object that threads share
//! the generator writes the body as one atomic action of the field's `java::SharedField`
//! (`codegen/field_updates.rs`); on an object of one thread no other access can come between the
//! read and the write. So the update is atomic as Java's is. A `float` or `double` compares by
//! its raw bits, as `Unsafe` compares them. The weak forms never fail spuriously, which Java
//! allows.
//!
//! An *element offset* is `base + (index << shift)` or `base + index * scale` (either way round),
//! where `base` is `arrayBaseOffset(T[].class)`, `scale` is `arrayIndexScale(T[].class)` and
//! `shift` is `31 - Integer.numberOfLeadingZeros(scale)`, each written out or read from a static
//! final field (or, in a static block, a local) whose value is one of them, and `index` is an
//! `int` widened to `long`, or a `long`; of a `byte[]` or `boolean[]`, whose elements are a byte
//! apart, it is also `base + index` (`DecimalDigits` writes `ARRAY_BYTE_BASE_OFFSET + charPos`). The offset names element `index` of the array the access
//! is made on, which must be an array of `T`'s kind (references, or the same primitive), as the
//! access must be. The values themselves stay what they are (`Unsafe`'s answers for a class
//! literal, see `natives.rs`), so the static fields and the static block that compute them run as
//! in Java; only the offset arithmetic is never computed. A read or write at an element offset is
//! a read or write of `a[index]`, which in an array that threads share is one atomic action of the
//! element's cell (an acquire load or a release store of a word). A volatile one
//! (`getIntVolatile`, `putReferenceVolatile`, a `VarHandle`'s `getVolatile` and `setVolatile`) is
//! recorded by the index expression of the read or write (`Class::volatile_elements`), which the
//! generator makes a `SeqCst` load or store of the cell. An atomic update of an element
//! is `Builtin::ElementUpdate`, whose operands are the store it makes, `a[index] = x` (or
//! `a[index] += delta`), so that every analysis sees the store, and for the compare forms the
//! expected value first; the runtime's function for it (`java::compare_and_set_element`, ...)
//! reads, compares and writes the element in one atomic action: under one borrow of an array of
//! one thread, and as one compare-and-swap of the element's cell (or under its striped lock, for
//! an element wider than a word) in an array that threads share, so no other access of the
//! element comes between the read and the write.
//!
//! An element offset may also be computed away from the access: by a static method whose body
//! returns one of its parameter (`ForkJoinPool.slotOffset(i)`), whose call is the element at its
//! argument, into a `long` local that every assignment gives an element offset of one kind of
//! array (`U.getReferenceAcquire(a, k = slotOffset(i))`, then `U.compareAndSetReference(a, k, t,
//! null)`), or into a `long` parameter of a private method that every call passes one of one
//! kind (`LazyCollections.Mutexes.acquireMutex(long offset)`): only its outermost class can
//! call it, and no code there may name it otherwise. The local or parameter holds the offset Java
//! computes, from `Unsafe`'s answers for the base and scale, so the element's index is
//! `(k - base) / scale`. Any other offset arithmetic is rejected.
//!
//! The offset of a static field (`staticFieldOffset(f)`, with `staticFieldBase(f)` as the object,
//! of a field `f` that `getDeclaredField` finds, as `ForkJoinPool` finds `poolIds`) is a handle of
//! that field: an access is of the static field, and an atomic update is a static method of its
//! class that is `synchronized`, so it holds the class's lock in a program with threads. The
//! `Field` local is left out with the handles it makes.
//!
//! A `VarHandle` constant (a static final field) is a handle as well: of a field
//! (`MethodHandles.lookup().findVarHandle(C.class, "f", T.class)`, `MhUtil.findVarHandle`, or
//! `ConstantBootstraps.fieldVarHandle`), of a static field (`findStaticVarHandle`, as `SSLContext`
//! keeps its default, or `ConstantBootstraps.staticFieldVarHandle`), whose accesses take no
//! object and are those of `staticFieldOffset`'s handle, of
//! the elements of an array (`arrayElementVarHandle(T[].class)`), or of the values whose bytes a
//! `byte[]` holds (`byteArrayViewVarHandle(int[].class, ByteOrder.BIG_ENDIAN)`, as
//! `DataOutputStream` and `DataInputStream` write and read numbers). Its `get`, `set`,
//! `compareAndSet`, `getAndAdd` and the other accesses, in every memory mode, are those of
//! `Unsafe` at the field or element; a view's `get` and `set` are `Builtin::ByteView`, whose
//! operand is the access of the first byte (`a[i]`, or `a[i] = (byte) v`), so the analyses see
//! it, and which throws Java's `ArrayIndexOutOfBoundsException` when the value's bytes do not fit.
//! A view of the bytes of a `ByteBuffer` (`byteBufferViewVarHandle`) is a handle too: its `get`
//! and `set` in each memory mode are calls of the translator's methods
//! (`java/overlay/java/nio/TranslatedBufferViews.java`), with the handle's byte order last, which
//! check what the handle checks in its order (a null or read-only buffer, the value's bytes before
//! the limit, and for the modes other than the plain one a direct buffer and an aligned address);
//! its atomic updates, and an access at a type the handle would narrow to (which Java answers with
//! `WrongMethodTypeException`), are rejected.
//! A static method that returns such a handle of its parameters (`ByteArray.create`) makes one
//! too, and so does `withInvokeExactBehavior()` of one (`SHA3.asLittleEndian`). A `VarHandle`
//! that the code holds as a value, found at run time and possibly null, is `handle_values`'s; any
//! other use of a `VarHandle` is rejected.
//!
//! A field updater constant (a static final whose value is `AtomicIntegerFieldUpdater`'s,
//! `AtomicLongFieldUpdater`'s or `AtomicReferenceFieldUpdater`'s `newUpdater`, perhaps cast to a
//! raw updater, or a static block's local that holds one, as netty writes them) is a handle of
//! its field when `newUpdater` would succeed for it (`updater_declaration`); its `get`, `set`,
//! `lazySet`, compare-and-set forms, `getAndSet` and `getAndAdd` are `Unsafe`'s at the field's
//! offset, which the updater's methods make, and its increments and `addAndGet` its `getAndAdd`.
//! Its `getAndUpdate`, `updateAndGet`, `getAndAccumulate` and `accumulateAndGet` are a method
//! that lowering adds to the field's class, one per field and form, which loops as the JDK's
//! updater does: it reads the field, applies the function and compares and sets until nothing
//! wrote the field between the read and the write (`update_method`). An object other than
//! `this` is tested for null first, which throws the updater's `ClassCastException`
//! (`updater_access`). Any other use of an updater is rejected.
//!
//! A `MethodHandle` constant of a constructor is a handle too, resolved when translating:
//! `lookup.findConstructor(C.class, MethodType.methodType(void.class, P.class, ..))`, where the
//! lookup is `MethodHandles.lookup()` or `privateLookupIn(D.class, ..)` of one (which only grant
//! access), and the class, the lookup and the type may each be a local of the static block
//! (`ExtendedMapMode` makes `FileChannel.MapMode`'s so). `(C) H.invoke(args)` and
//! `invokeExact` are `new C(args)`, of the constructor that reachability found; the locals
//! that only make it are left out with it. The `MethodHandle`s of the methods that make handles
//! (`findVirtual` of `Lookup.findVarHandle`, as netty finds its handles) are values that
//! `handle_values` follows; any other use of a `MethodHandle` is rejected: the translation has no
//! method handles of the program's methods at run time.

use super::sun_unsafe::SUN_OFFSET_OF_FIELD;
use super::*;
use crate::program::FieldUpdate;
use std::collections::HashMap;
use std::rc::Rc;

/// A method that lowering adds for an atomic update of a field through `Unsafe`.
pub(super) struct FieldUpdateMethod {
    class: usize,
    field: String,
    ty: Type,
    update: FieldUpdate,
    /// Whether the field is static, and so the method, which is synchronized on its class.
    is_static: bool,
    /// The method's name, unique among those added, which its calls are found by until it has
    /// an index.
    name: String,
    /// For a loop of `update`, a compare-and-set, the function that the loop applies.
    looped: Option<UpdateLoop>,
}

/// A loop of compare-and-set that applies a function to a field's value until no other write
/// came between its read and its write, as the JDK's field updaters write `getAndUpdate`,
/// `updateAndGet`, `getAndAccumulate` and `accumulateAndGet`.
#[derive(Clone, PartialEq)]
pub(super) struct UpdateLoop {
    /// Whether the function takes the method's argument `x` after the value (`accumulate`).
    accumulates: bool,
    /// Whether the method gives the new value rather than the old.
    gives_new: bool,
    /// The type of the function, the method's last parameter.
    function: Type,
    /// The function's method (`applyAsInt`, `applyAsLong` or `apply`), its name, the types of
    /// its parameters and the type it returns.
    apply: (usize, &'static str, Vec<Type>, Type),
}

/// `Unsafe.objectFieldOffset(Class, String)`.
const OFFSET_OF: (&str, &str, &str) =
    ("jdk.internal.misc.Unsafe", "objectFieldOffset", "(java.lang.Class<?>,java.lang.String)long");

/// `Unsafe.objectFieldOffset(Field)`, of the field that `Class.getDeclaredField` finds.
pub const OFFSET_OF_FIELD: (&str, &str, &str) =
    ("jdk.internal.misc.Unsafe", "objectFieldOffset", "(java.lang.reflect.Field)long");

/// `Unsafe.staticFieldOffset(Field)` and `staticFieldBase(Field)`, of the static field that
/// `Class.getDeclaredField` finds.
const STATIC_OFFSET_OF: (&str, &str, &str) =
    ("jdk.internal.misc.Unsafe", "staticFieldOffset", "(java.lang.reflect.Field)long");
const STATIC_BASE_OF: (&str, &str, &str) =
    ("jdk.internal.misc.Unsafe", "staticFieldBase", "(java.lang.reflect.Field)java.lang.Object");

/// `Unsafe.arrayBaseOffset(Class)` and `arrayIndexScale(Class)`.
const BASE_OF: (&str, &str, &str) =
    ("jdk.internal.misc.Unsafe", "arrayBaseOffset", "(java.lang.Class<?>)long");
const SCALE_OF: (&str, &str, &str) =
    ("jdk.internal.misc.Unsafe", "arrayIndexScale", "(java.lang.Class<?>)int");

/// The class whose methods make a `VarHandle` constant, and the class of `VarHandle`.
const METHOD_HANDLES: &str = "java.lang.invoke.MethodHandles";
const VAR_HANDLE: &str = "java.lang.invoke.VarHandle";

/// The classes of a `MethodHandle` and of its type.
const METHOD_HANDLE: &str = "java.lang.invoke.MethodHandle";
const METHOD_TYPE: &str = "java.lang.invoke.MethodType";

/// `Integer.numberOfLeadingZeros(int)`, which turns a power of two into its exponent.
const LEADING_ZEROS: (&str, &str, &str) = ("java.lang.Integer", "numberOfLeadingZeros", "(int)int");

/// The primitive types, as javac names them and as the kinds of arrays and of `Unsafe`'s
/// accesses; every other type is a reference.
pub(super) const PRIMITIVES: [&str; 8] =
    ["boolean", "byte", "short", "char", "int", "long", "float", "double"];

/// `node`, or in the body of a static method that `bound` gives the arguments of the parameters
/// of, the argument when `node` is a parameter; `None` for a parameter without one.
fn bind<'t>(node: Node<'t>, bound: &[(&'t str, Node<'t>)]) -> Option<Node<'t>> {
    let node = plain(node);
    match node.symbol().filter(|s| s.kind == "PARAMETER" && node.kind() == "IDENTIFIER") {
        Some(s) if !bound.is_empty() => bound.iter().find(|(name, _)| *name == s.name).map(|(_, a)| *a),
        _ => Some(node),
    }
}

/// The kind of the elements of an array, or of an access of `Unsafe`: a primitive type's name, or
/// `reference`.
type Kind = &'static str;

/// `node` without the parentheses around it.
pub(super) fn plain(mut node: Node) -> Node {
    while node.kind() == "PARENTHESIZED"
        && let Some(inner) = node.get("expression")
    {
        node = inner;
    }
    node
}

/// The arguments of `node` when it is a call of the method `method` (owner, name, signature).
fn call_of<'t>(node: Node<'t>, method: (&str, &str, &str)) -> Option<Vec<Node<'t>>> {
    let node = plain(node);
    let symbol = node.symbol().filter(|_| node.kind() == "METHOD_INVOCATION")?;
    ((symbol.owner, symbol.name, symbol.ty) == method).then(|| node.list("arguments").collect())
}

/// The kind of the elements of the array class that the class literal `node` names, or that a
/// variable of that class's type holds, which can be no other class.
fn literal_kind(node: Node) -> Option<Kind> {
    let node = plain(node);
    let class = if node.kind() == "MEMBER_SELECT" && node.str("identifier") == "class" {
        node.get("expression")?.java_type()
    } else if node.kind() == "IDENTIFIER" {
        node.java_type().strip_prefix("java.lang.Class<")?.strip_suffix('>')?
    } else {
        return None;
    };
    let element = class.strip_suffix("[]")?;
    Some(PRIMITIVES.into_iter().find(|p| *p == element).unwrap_or("reference"))
}

/// Whether a value of the primitive type `from` is one of `to` or converts to one by widening.
fn widens(from: &str, to: &str) -> bool {
    let wider: &[&str] = match from {
        "byte" => &["short", "int", "long", "float", "double"],
        "short" | "char" => &["int", "long", "float", "double"],
        "int" => &["long", "float", "double"],
        "long" => &["float", "double"],
        "float" => &["double"],
        _ => &[],
    };
    (from == to && PRIMITIVES.contains(&from)) || wider.contains(&to)
}

/// The class that boxes a value of the primitive type `kind`.
fn boxed(kind: Kind) -> &'static str {
    match kind {
        "boolean" => "java.lang.Boolean",
        "byte" => "java.lang.Byte",
        "short" => "java.lang.Short",
        "char" => "java.lang.Character",
        "int" => "java.lang.Integer",
        "long" => "java.lang.Long",
        "float" => "java.lang.Float",
        _ => "java.lang.Double",
    }
}

/// The kind of the elements of type `ty`.
fn type_kind(ty: Type) -> Kind {
    match ty {
        Type::Boolean => "boolean",
        Type::Byte => "byte",
        Type::Short => "short",
        Type::Char => "char",
        Type::Int => "int",
        Type::Long => "long",
        Type::Float => "float",
        Type::Double => "double",
        _ => "reference",
    }
}

/// The kind of the value that the access of `Unsafe` with javac's signature `signature` reads or
/// writes: its third parameter's (the value written, or the update's operand), or else what it
/// returns.
fn access_kind(signature: &str) -> Kind {
    let (params, ret) = signature.trim_start_matches('(').split_once(')').unwrap_or_default();
    let ty = params.split(',').nth(2).unwrap_or(ret);
    PRIMITIVES.into_iter().find(|p| *p == ty).unwrap_or("reference")
}

/// Calls `f` on every node of the tree under `v`.
pub(super) fn walk<'t>(v: &'t Value, f: &mut impl FnMut(Node<'t>)) {
    match v {
        Value::Object(map) => {
            f(Node(v));
            map.values().for_each(|c| walk(c, f));
        }
        Value::Array(items) => items.iter().for_each(|i| walk(i, f)),
        _ => {}
    }
}

/// Calls `f` on every node of the tree under `v`, with the innermost method declaration that
/// holds it, if any (`method` for `v` itself).
fn walk_in_methods<'t>(
    v: &'t Value,
    method: Option<Node<'t>>,
    f: &mut impl FnMut(Node<'t>, Option<Node<'t>>),
) {
    match v {
        Value::Object(map) => {
            let node = Node(v);
            let method = if node.kind() == "METHOD" { Some(node) } else { method };
            f(node, method);
            map.values().for_each(|c| walk_in_methods(c, method, f));
        }
        Value::Array(items) => items.iter().for_each(|i| walk_in_methods(i, method, f)),
        _ => {}
    }
}

/// Whether `node` is the variable of an assignment, compound assignment, `++` or `--` that
/// `target` tells apart.
fn assigns<'t>(node: Node<'t>, target: impl Fn(Node<'t>) -> bool) -> bool {
    assigned(node).is_some_and(target)
}

/// What `node` assigns, when it is an assignment, an increment or a decrement.
fn assigned(node: Node) -> Option<Node> {
    let kind = node.kind();
    let variable = if kind == "ASSIGNMENT" || kind.ends_with("_ASSIGNMENT") {
        node.get("variable")
    } else if kind.ends_with("_INCREMENT") || kind.ends_with("_DECREMENT") {
        node.get("expression")
    } else {
        None
    };
    variable.map(plain)
}

/// The writes of a class's static fields in its static blocks, by name: how many there are, and
/// the value and block of the last that is a statement assigning the field.
pub(super) type StaticWrites<'t> = HashMap<&'t str, (usize, Option<(Node<'t>, Option<Node<'t>>)>)>;

/// Whether evaluating `e` again gives the same value and does nothing else: a constant, a local
/// (which nothing assigns between the two reads), `this`, an enclosing instance, and their
/// negations and conversions.
fn is_reread(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Var(_) | ExprKind::This | ExprKind::Int(_) | ExprKind::Long(_) => true,
        ExprKind::Field { object, name } => name == super::inner::OUTER && is_reread(object),
        ExprKind::Unary(UnOp::Neg, value) | ExprKind::Cast(_, value) => is_reread(value),
        _ => false,
    }
}

/// Whether evaluating the argument `n` can neither throw nor do anything else: a literal, a
/// constant (which javac writes as its value), a local or `this`, perhaps negated, and
/// arithmetic of those that cannot throw.
fn quiet(n: Node) -> bool {
    let n = plain(n);
    let constant = || n.symbol().is_some_and(|s| s.kind == "FIELD" && s.constant.is_some());
    match n.kind() {
        "UNARY_MINUS" | "UNARY_PLUS" | "BITWISE_COMPLEMENT" | "LOGICAL_COMPLEMENT" => {
            n.get("expression").is_some_and(quiet)
        }
        "IDENTIFIER" => {
            n.str("name") == "this"
                || constant()
                || n.symbol().is_some_and(|s| {
                    matches!(
                        s.kind,
                        "LOCAL_VARIABLE" | "PARAMETER" | "EXCEPTION_PARAMETER" | "RESOURCE_VARIABLE"
                    )
                })
        }
        // Arithmetic on numbers that cannot throw, of quiet operands.
        "PLUS"
        | "MINUS"
        | "MULTIPLY"
        | "AND"
        | "OR"
        | "XOR"
        | "LEFT_SHIFT"
        | "RIGHT_SHIFT"
        | "UNSIGNED_RIGHT_SHIFT"
            if PRIMITIVES.contains(&n.java_type()) =>
        {
            n.get("leftOperand").is_some_and(quiet) && n.get("rightOperand").is_some_and(quiet)
        }
        // A constant of a class named by its name.
        "MEMBER_SELECT" => {
            constant()
                && n.get("expression").is_some_and(|e| {
                    plain(e)
                        .symbol()
                        .is_some_and(|s| matches!(s.kind, "CLASS" | "INTERFACE" | "ENUM" | "RECORD"))
                })
        }
        // Making a lambda, or a reference to a static method or one that takes its object,
        // evaluates nothing.
        "LAMBDA_EXPRESSION" => true,
        "MEMBER_REFERENCE" => plain(Node(&n.0["qualifierExpression"]))
            .symbol()
            .is_some_and(|s| matches!(s.kind, "CLASS" | "INTERFACE" | "ENUM" | "RECORD")),
        kind => kind.ends_with("_LITERAL"),
    }
}

/// The kind of the fields that the field updater class `ty` (as javac names the type, with its
/// type arguments) updates: `int`, `long` or `reference`.
fn updater_kind(ty: &str) -> Option<Kind> {
    let raw = ty.split_once('<').map_or(ty, |(raw, _)| raw);
    match raw.strip_prefix("java.util.concurrent.atomic.")? {
        "AtomicIntegerFieldUpdater" => Some("int"),
        "AtomicLongFieldUpdater" => Some("long"),
        "AtomicReferenceFieldUpdater" => Some("reference"),
        _ => None,
    }
}

/// What a `VarHandle` constant reads and writes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum VarHandleOf<'t> {
    /// The field of a class (`findVarHandle`), by the class's index.
    Field(usize, &'t str),
    /// The static field of a class (`findStaticVarHandle`), by the class's index.
    Static(usize, &'t str),
    /// The elements of an array of this kind (`arrayElementVarHandle`).
    Element(Kind),
    /// The values of this kind whose bytes are those of a `byte[]`, in big-endian order or not
    /// (`byteArrayViewVarHandle`).
    ByteView(Kind, bool),
    /// The values of this kind whose bytes are those of a `ByteBuffer`, in big-endian order or
    /// not (`byteBufferViewVarHandle`).
    BufferView(Kind, bool),
}

/// The method of `MethodHandles` (or of its `Lookup`) that `node` calls, and its arguments.
fn handles_call<'t>(node: Node<'t>) -> Option<(&'t str, Vec<Node<'t>>)> {
    let node = plain(node);
    let symbol = node.symbol().filter(|_| node.kind() == "METHOD_INVOCATION")?;
    let owner = symbol.owner.strip_prefix(METHOD_HANDLES)?;
    matches!(owner, "" | ".Lookup").then(|| (symbol.name, node.list("arguments").collect()))
}

/// Whether `node` is `ByteOrder.BIG_ENDIAN` (`Some(true)`) or `ByteOrder.LITTLE_ENDIAN`.
pub(super) fn byte_order(node: Node) -> Option<bool> {
    let symbol = plain(node).symbol().filter(|s| s.owner == "java.nio.ByteOrder")?;
    match symbol.name {
        "BIG_ENDIAN" => Some(true),
        "LITTLE_ENDIAN" => Some(false),
        _ => None,
    }
}

/// The access that the method `name` of `VarHandle` makes: whether it writes (`Some(true)`) or
/// reads (`Some(false)`), or else the atomic update it is.
fn var_handle_method(name: &str) -> Option<(Option<bool>, Option<FieldUpdate>)> {
    let update = |u| Some((None, Some(u)));
    match name {
        "get" | "getVolatile" | "getAcquire" | "getOpaque" => Some((Some(false), None)),
        "set" | "setVolatile" | "setRelease" | "setOpaque" => Some((Some(true), None)),
        "compareAndSet"
        | "weakCompareAndSet"
        | "weakCompareAndSetPlain"
        | "weakCompareAndSetAcquire"
        | "weakCompareAndSetRelease" => update(FieldUpdate::CompareAndSet),
        "compareAndExchange" | "compareAndExchangeAcquire" | "compareAndExchangeRelease" => {
            update(FieldUpdate::CompareAndExchange)
        }
        _ => {
            let mode = |prefix: &str| {
                name.strip_prefix(prefix).is_some_and(|m| matches!(m, "" | "Acquire" | "Release"))
            };
            if mode("getAndSet") {
                update(FieldUpdate::GetAndSet)
            } else if mode("getAndAdd") {
                update(FieldUpdate::GetAndAdd)
            } else if mode("getAndBitwiseOr") {
                update(FieldUpdate::GetAndBitwise(BinOp::BitOr))
            } else if mode("getAndBitwiseAnd") {
                update(FieldUpdate::GetAndBitwise(BinOp::BitAnd))
            } else if mode("getAndBitwiseXor") {
                update(FieldUpdate::GetAndBitwise(BinOp::BitXor))
            } else {
                None
            }
        }
    }
}

/// What an offset argument of `Unsafe` names.
enum Offset<'t> {
    /// The field of a class, by the class's index.
    Field(usize, &'t str),
    /// The static field of a class, by the class's index.
    Static(usize, &'t str),
    /// The element of the array at this index (an `int`, or a `long`), of an array of this kind.
    Element(Node<'t>, Kind),
    /// The element of an array of this kind whose offset this expression computes (see
    /// `computed_offset`).
    Computed(Node<'t>, Kind),
}

impl<'t> Lowering<'t> {
    /// The class and the name of the field that the static field `member` of class `owner` is
    /// the handle of.
    pub(super) fn handle_declaration(&self, owner: usize, member: Node<'t>) -> Option<(usize, &'t str)> {
        let (value, block) = self.static_value(owner, member)?;
        self.field_offset(value, block)
    }

    /// The value of the static final field `member` of class `owner`: its initializer, or the
    /// one assignment to it in the class's static blocks when that is a statement of its own,
    /// with that block, whose locals the value may read.
    pub(super) fn static_value(
        &self,
        owner: usize,
        member: Node<'t>,
    ) -> Option<(Node<'t>, Option<Node<'t>>)> {
        if member.kind() != "VARIABLE" || !member.has_flag("STATIC") || !member.has_flag("FINAL") {
            return None;
        }
        if let Some(init) = member.get("initializer") {
            return Some((init, None));
        }
        let writes = self.static_writes(owner);
        let &(count, found) = writes.get(member.str("name"))?;
        found.filter(|_| count == 1)
    }

    /// `StaticWrites` of class `owner`, made by one walk of its static blocks.
    fn static_writes(&self, owner: usize) -> Rc<StaticWrites<'t>> {
        if let Some(writes) = self.static_writes.borrow().get(&owner) {
            return writes.clone();
        }
        let javac = self.types[owner].javac.as_str();
        // The name of the static field of `owner` that `v` is.
        let field = |v: Node<'t>| {
            v.symbol().filter(|s| s.kind == "FIELD" && s.owner == javac && s.is_static).map(|s| s.name)
        };
        let mut writes: StaticWrites<'t> = HashMap::new();
        for block in self.types[owner].node.list("members").filter(|m| Self::is_static_block(*m)) {
            walk(block.0, &mut |n| {
                if let Some(name) = assigned(n).and_then(field) {
                    writes.entry(name).or_insert((0, None)).0 += 1;
                }
                if n.kind() == "EXPRESSION_STATEMENT"
                    && let Some(e) = n.get("expression").filter(|e| e.kind() == "ASSIGNMENT")
                    && let Some(name) = e.get("variable").and_then(|v| field(plain(v)))
                {
                    writes.entry(name).or_insert((0, None)).1 =
                        e.get("expression").map(|value| (value, Some(block)));
                }
            });
        }
        let writes = Rc::new(writes);
        self.static_writes.borrow_mut().insert(owner, writes.clone());
        writes
    }

    /// The class and declaration of the field handle whose value the statement `node` gives it,
    /// when it is that assignment in a static block (see `static_value`), which is left out.
    pub(super) fn handle_definition(&self, node: Node<'t>) -> Option<(usize, Node<'t>)> {
        let e = node.get("expression").filter(|_| node.kind() == "EXPRESSION_STATEMENT")?;
        self.handle_definition_of(e)
    }

    /// The class and declaration of the field handle that the assignment `e` gives its value.
    fn handle_definition_of(&self, e: Node<'t>) -> Option<(usize, Node<'t>)> {
        let symbol = e.get("variable").filter(|_| e.kind() == "ASSIGNMENT").and_then(|v| plain(v).symbol());
        let symbol = symbol.filter(|s| s.kind == "FIELD" && s.is_static)?;
        self.static_field(&symbol).filter(|&(owner, member)| self.is_handle(owner, member))
    }

    /// Whether `stmt`, a statement of `block`, declares a local that the block uses only to make
    /// handles, whose definitions are left out (`handle_definition`), so that the local is left
    /// out with them: a `Field` whose value is `getDeclaredField`, used only as the argument of
    /// `staticFieldOffset(f)`, `staticFieldBase(f)` or `objectFieldOffset(f)` in the values given
    /// to handles, a
    /// `MethodHandles.Lookup` whose value is `MethodHandles.lookup()`, used only in the values of
    /// `VarHandle` constants (`SharedThreadContainer` finds its handles so), or a field updater
    /// whose value is `newUpdater`, used only in the values of updater constants.
    pub(super) fn handle_field_local(&self, block: Node<'t>, stmt: Node<'t>) -> bool {
        let Some(ty) = stmt.symbol().filter(|_| stmt.kind() == "VARIABLE").map(|s| s.ty) else {
            return false;
        };
        let name = stmt.str("name");
        let Some(init) = stmt.get("initializer").map(plain) else { return false };
        let named = |n: Node| plain(n).kind() == "IDENTIFIER" && plain(n).str("name") == name;
        let (mut uses, mut in_handles) = (0, 0);
        match ty {
            "java.lang.reflect.Field" => {
                let found = init.symbol().filter(|_| init.kind() == "METHOD_INVOCATION");
                if found.is_none_or(|s| (s.owner, s.name) != ("java.lang.Class", "getDeclaredField")) {
                    return false;
                }
                // Only in the value given to a handle: an offset used as a number keeps its
                // `Field` (`field_offset_value`).
                walk(block.0, &mut |n| {
                    uses += usize::from(n.kind() == "IDENTIFIER" && n.str("name") == name);
                    let defines = n.kind() == "ASSIGNMENT" && self.handle_definition_of(n).is_some();
                    let Some(value) = n.get("expression").filter(|_| defines) else { return };
                    walk(value.0, &mut |v| {
                        let handle = [STATIC_OFFSET_OF, STATIC_BASE_OF, OFFSET_OF_FIELD, SUN_OFFSET_OF_FIELD]
                            .into_iter()
                            .find_map(|m| call_of(v, m));
                        in_handles +=
                            usize::from(handle.is_some_and(|args| args.first().is_some_and(|a| named(*a))));
                    });
                });
            }
            // An updater that a static block makes in a local, then gives its constant.
            _ if let Some(kind) = updater_kind(ty) => {
                let made = self.updater_value(init, kind, Some(block)).is_some_and(|(v, _)| {
                    v.symbol().is_some_and(|s| v.kind() == "METHOD_INVOCATION" && s.name == "newUpdater")
                });
                if !made {
                    return false;
                }
                walk(block.0, &mut |n| {
                    uses += usize::from(n.kind() == "IDENTIFIER" && n.str("name") == name);
                    let constant = n.kind() == "ASSIGNMENT"
                        && self
                            .handle_definition_of(n)
                            .is_some_and(|(owner, member)| self.updater_declaration(owner, member).is_some());
                    if let Some(value) = n.get("expression").filter(|_| constant) {
                        walk(value.0, &mut |v| {
                            in_handles += usize::from(v.kind() == "IDENTIFIER" && v.str("name") == name)
                        });
                    }
                });
            }
            "java.lang.invoke.MethodHandles.Lookup" | METHOD_TYPE => {
                let made = match ty {
                    METHOD_TYPE => self.constructor_type(init, Some(block)).is_some(),
                    _ => self.lookup_value(init, Some(block)),
                };
                if !made {
                    return false;
                }
                walk(block.0, &mut |n| {
                    uses += usize::from(n.kind() == "IDENTIFIER" && n.str("name") == name);
                    let constant = n.kind() == "ASSIGNMENT"
                        && n.get("variable").is_some_and(|v| {
                            self.var_handle(v).is_some() || self.constructor_handle(v).is_some()
                        });
                    if let Some(value) = n.get("expression").filter(|_| constant) {
                        walk(value.0, &mut |v| {
                            in_handles += usize::from(v.kind() == "IDENTIFIER" && v.str("name") == name)
                        });
                    }
                });
            }
            _ => return false,
        }
        uses == in_handles
    }

    /// The class and declaration of the static field that `symbol` names.
    pub(super) fn static_field(&self, symbol: &Symbol) -> Option<(usize, Node<'t>)> {
        let owner = self.type_index(symbol.owner)?;
        let member = self.types[owner]
            .node
            .list("members")
            .find(|m| m.kind() == "VARIABLE" && m.str("name") == symbol.name)?;
        Some((owner, member))
    }

    /// The class and field that `value`, a call of `objectFieldOffset` (of a class and a name,
    /// or of `getDeclaredField` of a class and a name), is the offset of. `block` is the static
    /// block it is in, if any.
    fn field_offset(&self, value: Node<'t>, block: Option<Node<'t>>) -> Option<(usize, &'t str)> {
        if self.offset_helper_call(value).is_some() {
            let args: Vec<Node<'t>> = plain(value).list("arguments").collect();
            let field = plain(*args.get(1)?);
            let name = field.0["value"].as_str().filter(|_| field.kind() == "STRING_LITERAL")?;
            return Some((self.offset_class(*args.first()?, block)?, name));
        }
        let Some(args) = call_of(value, OFFSET_OF) else {
            let field = call_of(value, OFFSET_OF_FIELD).or_else(|| call_of(value, SUN_OFFSET_OF_FIELD))?;
            return self.declared_field(*field.first()?, block);
        };
        let field = plain(*args.get(1)?);
        let name = field.0["value"].as_str().filter(|_| field.kind() == "STRING_LITERAL")?;
        Some((self.offset_class(*args.first()?, block)?, name))
    }

    /// When `value` calls a static method of the program whose one `return` gives
    /// `objectFieldOffset(c.getDeclaredField(name))` of its two parameters `c` and `name`, as
    /// JCTools' `UnsafeAccess.fieldOffset(Class, String)` does: that `objectFieldOffset` call and
    /// the method's class. The call is the offset of the field its arguments name.
    pub(super) fn offset_helper_call(&self, value: Node<'t>) -> Option<(Node<'t>, usize)> {
        let value = plain(value);
        let symbol = value.symbol().filter(|s| value.kind() == "METHOD_INVOCATION" && s.is_static)?;
        let index = self.method_index(&symbol)?;
        let method = self.methods[index].node;
        let params: Vec<&str> = method.list("parameters").map(|p| p.str("name")).collect();
        let [class, name] = params[..] else { return None };
        let mut returns = Vec::new();
        walk(method.get("body")?.0, &mut |n| {
            if n.kind() == "RETURN" {
                returns.push(n);
            }
        });
        let [returned] = returns[..] else { return None };
        let offset = plain(returned.get("expression")?);
        let get = plain(
            *call_of(offset, OFFSET_OF_FIELD).or_else(|| call_of(offset, SUN_OFFSET_OF_FIELD))?.first()?,
        );
        let found = get.symbol().filter(|_| get.kind() == "METHOD_INVOCATION")?;
        let parameter = |n: Option<Node>, p: &str| {
            n.map(plain).is_some_and(|n| {
                n.kind() == "IDENTIFIER"
                    && n.str("name") == p
                    && n.symbol().is_some_and(|s| s.kind == "PARAMETER")
            })
        };
        let of_parameters = (found.owner, found.name) == ("java.lang.Class", "getDeclaredField")
            && parameter(get.get("methodSelect").and_then(|m| m.get("expression")), class)
            && parameter(get.list("arguments").next(), name);
        of_parameters.then_some((offset, self.methods[index].owner))
    }

    /// The class and the name of the field that `get` is: a call of `getDeclaredField` of a class
    /// and a name, or a local of the static block `block` whose value is one.
    fn declared_field(&self, get: Node<'t>, block: Option<Node<'t>>) -> Option<(usize, &'t str)> {
        let get = plain(get);
        let Some(symbol) = get.symbol().filter(|_| get.kind() == "METHOD_INVOCATION") else {
            let (value, block) = self.defined(get, block)?;
            return self.declared_field(value, block);
        };
        if (symbol.owner, symbol.name) != ("java.lang.Class", "getDeclaredField") {
            return None;
        }
        let field = plain(get.list("arguments").next()?);
        let name = field.0["value"].as_str().filter(|_| field.kind() == "STRING_LITERAL")?;
        Some((self.offset_class(get.get("methodSelect")?.get("expression")?, block)?, name))
    }

    /// The static field that the static final field `member` of class `owner` is the handle of
    /// (`staticFieldOffset`), or with `base`, the base of (`staticFieldBase`), which only an
    /// access at that offset may use.
    fn static_declaration(&self, owner: usize, member: Node<'t>, base: bool) -> Option<(usize, &'t str)> {
        let (value, block) = self.static_value(owner, member)?;
        let method = if base { STATIC_BASE_OF } else { STATIC_OFFSET_OF };
        self.declared_field(*call_of(value, method)?.first()?, block)
    }

    /// The static field whose handle (or with `base`, whose base) the expression `node` reads.
    fn static_handle(&self, node: Node<'t>, base: bool) -> Option<(usize, &'t str)> {
        let symbol = plain(node).symbol().filter(|s| s.kind == "FIELD" && s.is_static)?;
        let (owner, member) = self.static_field(&symbol)?;
        self.static_declaration(owner, member, base)
    }

    /// The class that `node` names: a class literal, a local of the static block `block` or a
    /// static final that holds one, or a call of a static method of the program that returns its
    /// argument, cast to another `Class` type (netty's `cls(Node.class)`).
    fn offset_class(&self, node: Node<'t>, block: Option<Node<'t>>) -> Option<usize> {
        let node = plain(node);
        if node.kind() == "MEMBER_SELECT" && node.str("identifier") == "class" {
            return self.type_index(node.get("expression")?.java_type());
        }
        if node.kind() == "METHOD_INVOCATION" {
            let (_, mut returned, bound) = self.single_return(node, node.list("arguments").collect())?;
            while returned.kind() == "TYPE_CAST" && returned.java_type().starts_with("java.lang.Class") {
                returned = plain(returned.get("expression")?);
            }
            let returned = plain(returned);
            (returned.kind() == "IDENTIFIER").then_some(())?;
            let argument = bind(returned, &bound).filter(|a| !std::ptr::eq(a.0, returned.0))?;
            return self.offset_class(argument, block);
        }
        let (value, block) = self.defined(node, block)?;
        self.offset_class(value, block)
    }

    /// The value of `node` when it reads a static final field, or a local of the static block
    /// `block` that is assigned only where it is declared, in that block's statements, and the
    /// block that value is in.
    fn defined(&self, node: Node<'t>, block: Option<Node<'t>>) -> Option<(Node<'t>, Option<Node<'t>>)> {
        let symbol = node.symbol().filter(|_| matches!(node.kind(), "IDENTIFIER" | "MEMBER_SELECT"))?;
        if symbol.kind == "FIELD" && symbol.is_static {
            let (owner, member) = self.static_field(&symbol)?;
            return self.static_value(owner, member);
        }
        let block = block.filter(|_| symbol.kind == "LOCAL_VARIABLE" && node.kind() == "IDENTIFIER")?;
        let name = symbol.name;
        // The local's declaration, the only one of that name in the block (in a `try` of it, say).
        let mut declarations = Vec::new();
        walk(block.0, &mut |n| {
            if n.kind() == "VARIABLE" && n.str("name") == name {
                declarations.push(n);
            }
        });
        let [declaration] = declarations[..] else { return None };
        let init = declaration.get("initializer")?;
        let mut written = false;
        walk(block.0, &mut |n| {
            written |= assigns(n, |v| v.kind() == "IDENTIFIER" && v.str("name") == name);
        });
        (!written).then_some((init, Some(block)))
    }

    /// The field that the expression `offset` names, when it reads a field handle or is the call
    /// that makes one.
    fn handle(&self, offset: Node<'t>) -> Option<(usize, &'t str)> {
        let offset = plain(offset);
        if offset.kind() == "METHOD_INVOCATION" {
            return self.field_offset(offset, None);
        }
        let symbol = offset.symbol().filter(|s| s.kind == "FIELD" && s.is_static)?;
        let (owner, member) = self.static_field(&symbol)?;
        self.handle_declaration(owner, member)
    }

    /// The index and the kind of array of the element that `offset` names, when it is an element
    /// offset (see the module's comment).
    /// In the body of a static method that `bound` gives the arguments of the parameters of, the
    /// index must be a parameter, and is its argument.
    fn element_offset(&self, offset: Node<'t>, bound: &[(&'t str, Node<'t>)]) -> Option<(Node<'t>, Kind)> {
        let offset = plain(offset);
        // A call of a static method that returns an element offset (`ForkJoinPool.slotOffset(i)`).
        if offset.kind() == "METHOD_INVOCATION" {
            let args = offset.list("arguments").map(|a| bind(a, bound)).collect::<Option<Vec<_>>>()?;
            let (_, returned, params) = self.single_return(offset, args)?;
            return self.element_offset(returned, &params);
        }
        if offset.kind() != "PLUS" {
            return None;
        }
        let found = |index: Node<'t>, kind: Kind| match bound {
            [] => Some((index, kind)),
            _ => Some((bind(index, bound).filter(|i| !std::ptr::eq(i.0, index.0))?, kind)),
        };
        let (l, r) = (offset.get("leftOperand")?, offset.get("rightOperand")?);
        let (base, scaled) = match self.array_base(l, None) {
            Some(kind) => (kind, r),
            None => (self.array_base(r, None)?, l),
        };
        let scaled = plain(scaled);
        let int = |n: Node| matches!(n.java_type(), "int" | "short" | "char" | "byte");
        // The elements of a `byte[]` or `boolean[]` are a byte apart: the index is not scaled
        // (`ARRAY_BYTE_BASE_OFFSET + i`, where an `int` index is widened to the `long` base).
        if matches!(base, "byte" | "boolean") && !matches!(scaled.kind(), "LEFT_SHIFT" | "MULTIPLY") {
            let index = match scaled.kind() {
                "TYPE_CAST" if scaled.java_type() == "long" => {
                    scaled.get("expression").filter(|e| int(*e))?
                }
                _ if scaled.java_type() == "long" || int(scaled) => scaled,
                _ => return None,
            };
            return found(index, base);
        }
        let (l, r) = (scaled.get("leftOperand")?, scaled.get("rightOperand")?);
        let (index, kind) = match scaled.kind() {
            "LEFT_SHIFT" => (l, self.array_shift(r, None)?),
            "MULTIPLY" => match self.array_scale(r, None) {
                Some(kind) => (l, kind),
                None => (r, self.array_scale(l, None)?),
            },
            _ => return None,
        };
        // An `int` index widened before the arithmetic, which then cannot overflow, or a `long`.
        let index = plain(index);
        let index = match index.kind() {
            "TYPE_CAST" if index.java_type() == "long" => index.get("expression").filter(|e| int(*e))?,
            _ if index.java_type() == "long" => index,
            _ => return None,
        };
        if kind != base {
            return None;
        }
        found(index, kind)
    }

    /// The kind of array whose element offset the value of `offset` is, when the access is not
    /// given the arithmetic itself: an assignment of an element offset (`k = slotOffset(i)`), a
    /// `long` local of the method declaration `method` (the method being lowered when `None`) that
    /// every assignment gives an element offset of the same kind, as `ForkJoinPool` keeps one to
    /// compare and set the element it read, or a `long` parameter that every call passes one
    /// (`parameter_offset`). Such a value is the real offset that Java computes, from `Unsafe`'s
    /// answers for the base and scale, so the index is `(offset - base) / scale`
    /// (`computed_index`).
    fn computed_offset(&self, offset: Node<'t>, method: Option<Node<'t>>, depth: usize) -> Option<Kind> {
        let offset = plain(offset);
        if depth > 4 {
            return None;
        }
        match offset.kind() {
            "ASSIGNMENT" => self.computed_offset(offset.get("expression")?, method, depth + 1),
            "IDENTIFIER" => {
                let symbol = offset.symbol().filter(|s| s.ty == "long")?;
                let method = match method {
                    Some(method) => method,
                    None => self.methods[self.method_ctx.get()?].node,
                };
                if symbol.kind == "PARAMETER" {
                    return self.parameter_offset(method, symbol.name, depth);
                }
                if symbol.kind != "LOCAL_VARIABLE" {
                    return None;
                }
                let body = method.get("body")?;
                let (mut kinds, mut other) = (Vec::new(), false);
                walk(body.0, &mut |n| {
                    let value = match n.kind() {
                        "VARIABLE" if n.str("name") == symbol.name => n.get("initializer"),
                        "ASSIGNMENT"
                            if assigns(n, |v| v.kind() == "IDENTIFIER" && v.str("name") == symbol.name) =>
                        {
                            n.get("expression")
                        }
                        _ => {
                            other |= assigns(n, |v| v.kind() == "IDENTIFIER" && v.str("name") == symbol.name);
                            return;
                        }
                    };
                    if let Some(value) = value {
                        kinds.push(self.computed_offset(value, Some(method), depth + 1));
                    }
                });
                let first = *kinds.first()?;
                (!other && kinds.iter().all(|k| *k == first)).then_some(first?)
            }
            _ => self.element_offset(offset, &[]).map(|(_, kind)| kind),
        }
    }

    /// The kind of array whose element offset the `long` parameter `name` of the method
    /// declaration `method` always is, as `LazyCollections` passes one to its private
    /// `contentsAcquire(long offset)`: the method is private, so that only the code of its
    /// outermost class can name it, nothing assigns the parameter, and that code names the method
    /// only in calls, each in a method and passing an element offset of the same kind
    /// (`computed_offset` of the argument in its caller).
    fn parameter_offset(&self, method: Node<'t>, name: &str, depth: usize) -> Option<Kind> {
        if method.kind() != "METHOD" || !method.has_flag("PRIVATE") {
            return None;
        }
        let position = method.list("parameters").position(|p| p.str("name") == name)?;
        let mut written = false;
        walk(method.get("body")?.0, &mut |n| {
            written |= assigns(n, |v| v.kind() == "IDENTIFIER" && v.str("name") == name);
        });
        let declared = method.symbol()?;
        let named = |n: Node<'t>| {
            matches!(n.kind(), "METHOD_INVOCATION" | "MEMBER_REFERENCE")
                && n.symbol().is_some_and(|s| {
                    s.kind == "METHOD"
                        && (s.owner, s.name, s.ty) == (declared.owner, method.str("name"), declared.ty)
                })
        };
        // The outermost class: an enclosing class is in the same compilation unit, so lowered
        // with it, and a local or anonymous class is enclosed by code, which is not followed.
        let mut class = self.type_index(declared.owner)?;
        while let Some(outer) = self.types[class].node.symbol().and_then(|s| self.type_index(s.owner)) {
            class = outer;
        }
        if written || self.types[class].nested.is_some() {
            return None;
        }
        let (mut names, mut kinds) = (0, Vec::new());
        walk_in_methods(self.types[class].node.0, None, &mut |n, caller| {
            if !named(n) {
                return;
            }
            names += 1;
            let arg = n.list("arguments").nth(position).filter(|_| n.kind() == "METHOD_INVOCATION");
            kinds.push(arg.zip(caller).and_then(|(a, c)| self.computed_offset(a, Some(c), depth + 1)));
        });
        let first = *kinds.first()?;
        (names == kinds.len() && kinds.iter().all(|k| *k == first)).then_some(first?)
    }

    /// The body's one returned expression of the static method of the program that `call`
    /// calls, with the class it is in and its parameters, each bound to its argument in `args`.
    #[allow(clippy::type_complexity)]
    fn single_return(
        &self,
        call: Node<'t>,
        args: Vec<Node<'t>>,
    ) -> Option<(usize, Node<'t>, Vec<(&'t str, Node<'t>)>)> {
        let symbol = call.symbol().filter(|s| s.kind == "METHOD" && call.kind() == "METHOD_INVOCATION")?;
        let class = self.type_index(symbol.owner)?;
        let method = self.types[class].node.list("members").find(|m| {
            m.kind() == "METHOD"
                && m.has_flag("STATIC")
                && m.symbol().is_some_and(|s| s.ty == symbol.ty)
                && m.str("name") == symbol.name
        })?;
        let [ret] = &method.get("body")?.list("statements").collect::<Vec<_>>()[..] else { return None };
        let returned = ret.get("expression").filter(|_| ret.kind() == "RETURN")?;
        Some((class, returned, method.list("parameters").map(|p| p.str("name")).zip(args).collect()))
    }

    /// The kind of array whose base offset `node` is: `arrayBaseOffset(T[].class)`, or a static
    /// final or local whose value that is.
    fn array_base(&self, node: Node<'t>, block: Option<Node<'t>>) -> Option<Kind> {
        if let Some(args) = call_of(node, BASE_OF) {
            return literal_kind(*args.first()?);
        }
        let (value, block) = self.defined(plain(node), block)?;
        self.array_base(value, block)
    }

    /// The kind of array whose index scale `node` is: `arrayIndexScale(T[].class)`, or a static
    /// final or local whose value that is.
    fn array_scale(&self, node: Node<'t>, block: Option<Node<'t>>) -> Option<Kind> {
        if let Some(args) = call_of(node, SCALE_OF) {
            return literal_kind(*args.first()?);
        }
        let (value, block) = self.defined(plain(node), block)?;
        self.array_scale(value, block)
    }

    /// The kind of array whose index shift `node` is: `31 - Integer.numberOfLeadingZeros(scale)`
    /// of an index scale, or a static final or local whose value that is.
    fn array_shift(&self, node: Node<'t>, block: Option<Node<'t>>) -> Option<Kind> {
        let node = plain(node);
        if node.kind() == "MINUS" {
            let thirty_one = node
                .get("leftOperand")
                .map(plain)
                .is_some_and(|l| l.kind() == "INT_LITERAL" && l.str("value") == "31");
            let args = call_of(node.get("rightOperand")?, LEADING_ZEROS)?;
            return if thirty_one { self.array_scale(*args.first()?, block) } else { None };
        }
        let (value, block) = self.defined(node, block)?;
        self.array_shift(value, block)
    }

    /// Rejects a read of a field handle other than as the offset of an access, or of a
    /// `VarHandle` constant other than as the handle of one (`node` names a static field).
    pub(super) fn check_handle_use(&self, node: Node<'t>) -> Result<(), Error> {
        if self.handle(node).is_some() || self.static_handle(node, false).is_some() {
            return unsupported(node, "a field offset used as a value");
        }
        if self.static_handle(node, true).is_some() {
            return unsupported(node, "a static field's base used as a value");
        }
        if self.var_handle(node).is_some() {
            return unsupported(node, "a `VarHandle` used as a value");
        }
        if self.constructor_handle(node).is_some() {
            return unsupported(node, "a constructor's `MethodHandle` used as a value");
        }
        if self.updater(node).is_some() {
            return unsupported(node, "a field updater used as a value");
        }
        Ok(())
    }

    /// Whether the static field `member` of class `owner` is a handle: a field offset or a
    /// `VarHandle` constant, which is no field of the translation.
    pub(super) fn is_handle(&self, owner: usize, member: Node<'t>) -> bool {
        self.handle_declaration(owner, member).is_some()
            || self.var_handle_declaration(owner, member).is_some()
            || self.static_declaration(owner, member, false).is_some()
            || self.static_declaration(owner, member, true).is_some()
            || self.constructor_handle_declaration(owner, member).is_some()
            || self.updater_declaration(owner, member).is_some()
    }

    /// The class and the constructor's parameter types (as javac names them) that the static
    /// final field `member` of class `owner` is the `MethodHandle` of, a call of `findConstructor`
    /// (see the module's comment).
    fn constructor_handle_declaration(
        &self,
        owner: usize,
        member: Node<'t>,
    ) -> Option<(usize, Vec<&'t str>)> {
        if member.symbol().is_none_or(|s| s.ty != METHOD_HANDLE) {
            return None;
        }
        let (value, block) = self.static_value(owner, member)?;
        let ("findConstructor", args) = handles_call(value)? else { return None };
        let [class, ty] = args[..] else { return None };
        let lookup = plain(value).get("methodSelect")?.get("expression")?;
        if !self.lookup_value(lookup, block) {
            return None;
        }
        Some((self.offset_class(class, block)?, self.constructor_type(ty, block)?))
    }

    /// The constructor `MethodHandle` that the static field read `node` holds.
    fn constructor_handle(&self, node: Node<'t>) -> Option<(usize, Vec<&'t str>)> {
        let symbol =
            plain(node).symbol().filter(|s| s.kind == "FIELD" && s.is_static && s.ty == METHOD_HANDLE)?;
        let (owner, member) = self.static_field(&symbol)?;
        self.constructor_handle_declaration(owner, member)
    }

    /// Whether `node` is a `Lookup` that only grants access: `MethodHandles.lookup()`, or
    /// `privateLookupIn(C.class, l)` of one, or a local of the static block `block` whose value
    /// is one.
    fn lookup_value(&self, node: Node<'t>, block: Option<Node<'t>>) -> bool {
        if let Some((name, args)) = handles_call(node) {
            return match (name, &args[..]) {
                ("lookup", []) => true,
                ("privateLookupIn", [class, lookup]) => {
                    self.offset_class(*class, block).is_some() && self.lookup_value(*lookup, block)
                }
                _ => false,
            };
        }
        self.defined(plain(node), block).is_some_and(|(value, block)| self.lookup_value(value, block))
    }

    /// The parameter types of a constructor's `MethodType.methodType(void.class, P.class, ..)`,
    /// or of a local of the static block `block` whose value is one.
    fn constructor_type(&self, node: Node<'t>, block: Option<Node<'t>>) -> Option<Vec<&'t str>> {
        let node = plain(node);
        let Some(symbol) = node.symbol().filter(|_| node.kind() == "METHOD_INVOCATION") else {
            let (value, block) = self.defined(node, block)?;
            return self.constructor_type(value, block);
        };
        if (symbol.owner, symbol.name) != (METHOD_TYPE, "methodType") {
            return None;
        }
        let literal = |n: Node<'t>| {
            let n = plain(n);
            (n.kind() == "MEMBER_SELECT" && n.str("identifier") == "class")
                .then(|| n.get("expression"))
                .flatten()
                .map(|e| e.java_type())
        };
        let args: Vec<Node<'t>> = node.list("arguments").collect();
        let (result, params) = args.split_first()?;
        (literal(*result)? == "void").then_some(())?;
        params.iter().map(|p| literal(*p)).collect()
    }

    /// `(C) H.invoke(args)` (or `invokeExact`) of a constructor `MethodHandle` `H` of `C`, the
    /// cast `node`: `new C(args)`, of the constructor that reachability found. `None` for any
    /// other cast.
    pub(super) fn constructor_handle_call(
        &self,
        node: Node<'t>,
        ty: Type,
    ) -> Result<Option<ExprKind>, Error> {
        let call = plain(node.child("expression")?);
        let invoked = call.symbol().filter(|s| {
            call.kind() == "METHOD_INVOCATION"
                && s.owner == METHOD_HANDLE
                && matches!(s.name, "invoke" | "invokeExact")
        });
        let receiver = invoked.and_then(|_| call.get("methodSelect")?.get("expression"));
        let Some((class, params)) = receiver.and_then(|r| self.constructor_handle(r)) else {
            return Ok(None);
        };
        if !matches!(ty, Type::Class(name, _) if self.type_index(name) == Some(class)) {
            return unsupported(node, "a constructor's `MethodHandle` invoked as another class");
        }
        let args: Vec<Node<'t>> = call.list("arguments").collect();
        if args.len() != params.len() {
            return unsupported(node, "a constructor's `MethodHandle` invoked with other arguments");
        }
        let signature = format!("({})void", params.join(","));
        let javac = &self.types[class].javac;
        let reached = self.reach.is_none_or(|reach| {
            reach.methods.contains(&(javac.clone(), "<init>".to_string(), signature.clone()))
        });
        let ctor =
            self.methods.iter().position(|m| m.owner == class && m.name == "<init>" && m.sig == signature);
        let Some(ctor) = ctor.filter(|_| reached) else {
            return unsupported(
                node,
                "a constructor's `MethodHandle` of a constructor that is not translated",
            );
        };
        let mut lowered = Vec::new();
        for (arg, param) in args.iter().zip(&params) {
            let param = self.ty(*arg, param)?;
            lowered.push(self.convert(self.expr(*arg)?, param));
        }
        Ok(Some(ExprKind::New { class, args: lowered, ctor: Some(ctor) }))
    }

    /// Rejects a call of a `MethodHandle` that `constructor_handle_call` does not resolve.
    pub(super) fn check_method_handle_call(&self, node: Node<'t>, symbol: &Symbol<'t>) -> Result<(), Error> {
        if symbol.owner == METHOD_HANDLE {
            return unsupported(
                node,
                &format!("`MethodHandle.{}` other than a constructor's, cast to its class", symbol.name),
            );
        }
        Ok(())
    }

    /// The class and the name of the field that the static final field `member` of class `owner`
    /// is the field updater of: `AtomicIntegerFieldUpdater.newUpdater(C.class, "f")`, or that of
    /// `AtomicLongFieldUpdater`, or `AtomicReferenceFieldUpdater.newUpdater(C.class, V.class,
    /// "f")`, of a field for which `newUpdater` succeeds: a `volatile` field of `C` that is not
    /// static, of type `int`, `long`, or `V` itself (a reference type), which `owner` may access
    /// (in the same outermost class when it is private, in the same package when it is neither
    /// public nor protected; a protected one, whose updater checks the class of its objects
    /// against `owner`'s, is not followed).
    /// The expression that gives an updater of kind `kind` its value `value`: `value` itself, or
    /// what it casts to an updater of that kind (netty casts the updaters of generic classes to
    /// raw ones), or the value of the static block `block`'s local it reads (`Recycler` makes
    /// `STATE_UPDATER` so), with the block that is in.
    fn updater_value(
        &self,
        value: Node<'t>,
        kind: Kind,
        block: Option<Node<'t>>,
    ) -> Option<(Node<'t>, Option<Node<'t>>)> {
        match value.kind() {
            "TYPE_CAST" if updater_kind(value.java_type()) == Some(kind) => {
                self.updater_value(plain(value.get("expression")?), kind, block)
            }
            "IDENTIFIER" => {
                let (value, block) = self.defined(value, block)?;
                self.updater_value(plain(value), kind, block)
            }
            _ => Some((value, block)),
        }
    }

    fn updater_declaration(&self, owner: usize, member: Node<'t>) -> Option<(usize, &'t str)> {
        let kind = member.symbol().and_then(|s| updater_kind(s.ty))?;
        let (value, block) = self.static_value(owner, member)?;
        let (value, block) = self.updater_value(plain(value), kind, block)?;
        let symbol =
            value.symbol().filter(|s| value.kind() == "METHOD_INVOCATION" && s.name == "newUpdater")?;
        (updater_kind(symbol.owner)? == kind).then_some(())?;
        let args: Vec<Node<'t>> = value.list("arguments").collect();
        let (class, value_class, name) = match (kind, &args[..]) {
            ("reference", [class, value_class, name]) => (*class, Some(*value_class), *name),
            ("int" | "long", [class, name]) => (*class, None, *name),
            _ => return None,
        };
        let class = self.offset_class(class, block)?;
        let name = plain(name);
        let name = name.0["value"].as_str().filter(|_| name.kind() == "STRING_LITERAL")?;
        let field = self.types[class]
            .node
            .list("members")
            .find(|m| m.kind() == "VARIABLE" && m.str("name") == name)?;
        if field.has_flag("STATIC") || !field.has_flag("VOLATILE") || field.has_flag("PROTECTED") {
            return None;
        }
        let ty = field.symbol()?.ty;
        let ty = ty.split_once('<').map_or(ty, |(raw, _)| raw);
        // The value class is the field's type, or the erasure of its type variable (netty's
        // `volatile V val` with `Object.class`).
        let same = |named: &str| {
            named == ty
                || (!PRIMITIVES.contains(&ty) && self.type_var_around(ty, class, None).is_some() && {
                    let erased = |name: &str| self.declared_type(field, name, class, None).ok();
                    erased(named).is_some_and(|n| Some(n) == erased(ty))
                })
        };
        match value_class.map(plain) {
            None => (ty == kind).then_some(())?,
            Some(v) if v.kind() == "MEMBER_SELECT" && v.str("identifier") == "class" => {
                let named = v.get("expression")?.java_type();
                (same(named) && !PRIMITIVES.contains(&ty)).then_some(())?
            }
            // A class of the program that a local or a call names (`offset_class`).
            Some(v) => same(&self.types[self.offset_class(v, block)?].javac).then_some(())?,
        }
        let outermost = |mut t: usize| {
            while let Some(outer) = self.types[t].node.symbol().and_then(|s| self.type_index(s.owner)) {
                t = outer;
            }
            t
        };
        let package =
            |t: usize| self.types[outermost(t)].javac.rsplit_once('.').map_or("", |(p, _)| p).to_string();
        let accessible = if field.has_flag("PRIVATE") {
            outermost(class) == outermost(owner)
        } else {
            field.has_flag("PUBLIC") || package(class) == package(owner)
        };
        accessible.then_some((class, name))
    }

    /// The field whose updater the static field read `node` holds.
    fn updater(&self, node: Node<'t>) -> Option<(usize, &'t str)> {
        let symbol = plain(node).symbol().filter(|s| s.kind == "FIELD" && s.is_static)?;
        updater_kind(symbol.ty)?;
        let (owner, member) = self.static_field(&symbol)?;
        self.updater_declaration(owner, member)
    }

    /// `U.get(o)`, `U.set(o, x)`, `U.compareAndSet(o, expected, x)`, `U.getAndAdd(o, delta)` and
    /// the other accesses of a field updater constant `U` that take their operands as they are:
    /// those of `Unsafe` at the field's offset, which the updater's methods make. Its
    /// `getAndIncrement`, `incrementAndGet`, `addAndGet` and their kin are its `getAndAdd`, the
    /// new value computed from the old as the JDK's updater computes it, and its `getAndUpdate`,
    /// `accumulateAndGet` and their kin a call of a loop (`update_loop`). The updater checks its
    /// object first, and throws `ClassCastException` without a message for a null one, where the
    /// field's access would throw `NullPointerException`: an object other than `this` is tested
    /// (`Builtin::UpdaterRefused`), so it must be one that a second read gives again. `None` for
    /// any other call.
    pub(super) fn updater_access(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
    ) -> Result<Option<ExprKind>, Error> {
        if updater_kind(symbol.owner).is_none() {
            return Ok(None);
        }
        let receiver = node.child("methodSelect")?.get("expression");
        let owner = symbol.owner.rsplit('.').next().unwrap_or_default();
        let Some((class, field)) = receiver.and_then(|r| self.updater(r)) else {
            return unsupported(
                node,
                &format!("`{owner}.{}` of an updater that is not a constant", symbol.name),
            );
        };
        let looped = crate::program::updater_function(symbol.owner, symbol.name);
        // The delta of `getAndAdd` that the methods of `int` and `long` updaters add, and whether
        // they give the new value rather than the old (`None` for a delta that is their argument).
        let (put, update, added) = match symbol.name {
            "get" => (Some(false), None, None),
            "set" | "lazySet" => (Some(true), None, None),
            "compareAndSet"
            | "weakCompareAndSet"
            | "weakCompareAndSetPlain"
            | "weakCompareAndSetVolatile" => (None, Some(FieldUpdate::CompareAndSet), None),
            "getAndSet" => (None, Some(FieldUpdate::GetAndSet), None),
            "getAndAdd" => (None, Some(FieldUpdate::GetAndAdd), None),
            "getAndIncrement" => (None, None, Some((Some(1), false))),
            "getAndDecrement" => (None, None, Some((Some(-1), false))),
            "incrementAndGet" => (None, None, Some((Some(1), true))),
            "decrementAndGet" => (None, None, Some((Some(-1), true))),
            "addAndGet" => (None, None, Some((None, true))),
            _ if looped.is_some() => (None, None, None),
            name => return unsupported(node, &format!("`{owner}.{name}` of an updater")),
        };
        let args: Vec<Node> = node.list("arguments").collect();
        let (object, ty) = self.handle_field(node, symbol, args[0], class, field)?;
        let checked = !matches!(object.kind, ExprKind::This);
        if checked && !is_reread(&object) {
            return unsupported(
                node,
                &format!("`{owner}.{}` on an object other than `this` or a local", symbol.name),
            );
        }
        // Java tests the object after evaluating the other arguments, which the test here comes
        // before: arguments that can neither throw nor do anything else may come after it, and
        // an access's others are computed into locals before it (`held_arguments`).
        let hold = checked && !args[1..].iter().all(|a| quiet(*a));
        if hold && (looped.is_some() || added.is_some()) {
            return unsupported(
                node,
                &format!(
                    "`{owner}.{}` of an argument that is not a constant, a local or a lambda",
                    symbol.name
                ),
            );
        }
        let test = (checked && !hold).then(|| self.updater_test(node, &object));
        let ret = self.ty(node, node.java_type())?;
        let value = match added {
            _ if let Some(function) = looped => {
                let looped = self.update_loop(node, object, class, field, ty, &args[1..], function)?;
                self.convert(looped, ret)
            }
            Some((delta, new)) => {
                let delta = match delta {
                    Some(d) => {
                        let kind = if ty == Type::Long { ExprKind::Long(d) } else { ExprKind::Int(d as i32) };
                        self.synth(kind, node.pos(), ty)
                    }
                    None => self.stored(self.expr(args[1])?, ty),
                };
                if new && !is_reread(&delta) {
                    return unsupported(
                        node,
                        &format!("`{owner}.{}` of a delta that is not a local", symbol.name),
                    );
                }
                let again = new.then(|| self.reread(&delta));
                let old =
                    self.field_update(Some(object), class, field, ty, FieldUpdate::GetAndAdd, vec![delta]);
                let mut value = self.synth(old, node.pos(), ty);
                if let Some(again) = again {
                    let sum = ExprKind::Binary(BinOp::Add, Box::new(value), Box::new(again));
                    value = self.synth(sum, node.pos(), ty);
                }
                self.convert(value, ret)
            }
            None if hold => {
                let (held, object, values) = self.held_arguments(node, object, &args[1..])?;
                let test = self.updater_test(node, &object);
                let place = Place::Field { object: Box::new(object), name: field.to_string() };
                let access = self.access_of(node, place, ty, class, values, update, put, false)?;
                let value = self.synth(access, node.pos(), ret);
                let refused =
                    self.synth(ExprKind::Builtin(Builtin::UpdaterRefused, Vec::new()), node.pos(), ret);
                let tested = ExprKind::Conditional(Box::new(test), Box::new(refused), Box::new(value));
                let mut body = held;
                body.push(Stmt::Yield(self.synth(tested, node.pos(), ret)));
                let block = ExprKind::Switch {
                    selector: Box::new(self.synth(ExprKind::Int(0), node.pos(), Type::Int)),
                    cases: vec![Case { labels: vec![CaseLabel::Default], body }],
                };
                return Ok(Some(block));
            }
            None => {
                let place = Place::Field { object: Box::new(object), name: field.to_string() };
                let access = self.access_at(node, place, ty, class, &args[1..], update, put, false)?;
                self.synth(access, node.pos(), ret)
            }
        };
        let Some(test) = test else { return Ok(Some(value.kind)) };
        let refused = self.synth(ExprKind::Builtin(Builtin::UpdaterRefused, Vec::new()), node.pos(), ret);
        Ok(Some(ExprKind::Conditional(Box::new(test), Box::new(refused), Box::new(value))))
    }

    /// `object == null`, the test of an updater's object (`updater_access`).
    fn updater_test(&self, node: Node<'t>, object: &Expr) -> Expr {
        let null = Box::new(self.synth(ExprKind::Null, object.pos, Type::Null));
        let object = Box::new(self.reread(object));
        self.synth(ExprKind::Binary(BinOp::Eq, object, null), node.pos(), Type::Boolean)
    }

    /// The object of an updater's access and its other arguments `args`, computed in Java's
    /// order before the updater tests the object: the object and each argument up to the last
    /// that may do something are held in locals (`argument_order`'s `receiver$` and
    /// `argument$`), declared by the statements returned, and the access reads those.
    fn held_arguments(
        &self,
        node: Node<'t>,
        object: Expr,
        args: &[Node<'t>],
    ) -> Result<(Vec<Stmt>, Expr, Vec<Expr>), Error> {
        let pos = node.pos();
        let last = args.iter().rposition(|a| !quiet(*a)).expect("an argument that may do something");
        let mut held = Vec::new();
        let mut hold = |value: Expr, prefix: &str| {
            let name = format!("{prefix}u{}", value.id);
            let read = self.synth(ExprKind::Var(name.clone()), value.pos, value.ty);
            held.push(Stmt::Local { ty: value.ty, name, init: Some(value), pos });
            read
        };
        let object = hold(object, RECEIVER);
        let mut values = Vec::new();
        for (i, arg) in args.iter().enumerate() {
            let value = self.expr(*arg)?;
            let literal = plain(*arg).kind().ends_with("_LITERAL");
            values.push(if i <= last && !literal { hold(value, ARGUMENT) } else { value });
        }
        Ok((held, object, values))
    }

    /// `U.getAndUpdate(o, f)`, `U.accumulateAndGet(o, x, f)` and their kin, of the field `field`
    /// of class `class`, of type `ty`, whose function's method is `function` (the function's
    /// interface, the interface that declares the method, its name and its signature): a call of
    /// the method of the field's class that loops as the JDK's updater does (`update_method`),
    /// with `args` (`f`, or `x, f`) as its arguments.
    #[allow(clippy::too_many_arguments)]
    fn update_loop(
        &self,
        node: Node<'t>,
        object: Expr,
        class: usize,
        field: &str,
        ty: Type,
        args: &[Node<'t>],
        (interface, declarer, name, sig): (&str, &str, &'static str, &str),
    ) -> Result<Expr, Error> {
        let apply = self.method_named(declarer, name, sig);
        let Some(apply) = apply else {
            return unsupported(node, &format!("a field updater's loop without `{declarer}.{name}`"));
        };
        let function = self.ty(node, interface)?;
        let apply = (apply, name, self.param_types(apply)?, self.return_type(node, apply)?);
        let accumulates = args.len() == 2;
        let mut operands = Vec::new();
        if accumulates {
            operands.push(self.stored(self.expr(args[0])?, ty));
        }
        operands.push(self.convert(self.expr(args[args.len() - 1])?, function));
        let gives_new = node.symbol().is_some_and(|s| s.name.ends_with("AndGet"));
        let looped = UpdateLoop { accumulates, gives_new, function, apply };
        let name = self.update_method_name(class, field, ty, FieldUpdate::CompareAndSet, false, Some(looped));
        let call = ExprKind::MethodCall {
            receiver: Box::new(object),
            name,
            args: operands,
            resolved: None,
            is_super: false,
        };
        Ok(self.synth(call, node.pos(), ty))
    }

    /// Another read of `e`, an expression that `is_reread` accepts.
    fn reread(&self, e: &Expr) -> Expr {
        let kind = match &e.kind {
            ExprKind::Var(name) => ExprKind::Var(name.clone()),
            ExprKind::This => ExprKind::This,
            ExprKind::Int(v) => ExprKind::Int(*v),
            ExprKind::Long(v) => ExprKind::Long(*v),
            ExprKind::Field { object, name } => {
                ExprKind::Field { object: Box::new(self.reread(object)), name: name.clone() }
            }
            ExprKind::Unary(op, value) => ExprKind::Unary(*op, Box::new(self.reread(value))),
            ExprKind::Cast(ty, value) => ExprKind::Cast(*ty, Box::new(self.reread(value))),
            _ => unreachable!("only an expression that a second read gives again is read again"),
        };
        self.synth(kind, e.pos, e.ty)
    }

    /// What the static final field `member` of class `owner` is the `VarHandle` of.
    fn var_handle_declaration(&self, owner: usize, member: Node<'t>) -> Option<VarHandleOf<'t>> {
        if member.symbol().is_none_or(|s| s.ty != VAR_HANDLE) {
            return None;
        }
        let (value, block) = self.static_value(owner, member)?;
        self.var_handle_value(value, block, &[], owner)
    }

    /// What the `VarHandle` that the static field read `node` holds reaches.
    pub(super) fn var_handle(&self, node: Node<'t>) -> Option<VarHandleOf<'t>> {
        let symbol =
            plain(node).symbol().filter(|s| s.kind == "FIELD" && s.is_static && s.ty == VAR_HANDLE)?;
        let (owner, member) = self.static_field(&symbol)?;
        self.var_handle_declaration(owner, member)
    }

    /// What the `VarHandle` that `value` makes reaches: a call of `arrayElementVarHandle`,
    /// `byteArrayViewVarHandle` or `byteBufferViewVarHandle` of an array class literal, of
    /// `findVarHandle` of a class literal and a field's name, or of `MhUtil.findVarHandle` of a
    /// field of `owner` (the class of the lookup); a call of a static method that returns one of
    /// those of its parameters (`bound` gives their arguments), as `ByteArray.create(int[].class)`
    /// does; `withInvokeExactBehavior()` of one; or a constant whose value is one. `block` is the static block `value` is in, if any.
    pub(super) fn var_handle_value(
        &self,
        value: Node<'t>,
        block: Option<Node<'t>>,
        bound: &[(&str, Node<'t>)],
        owner: usize,
    ) -> Option<VarHandleOf<'t>> {
        let value = plain(value);
        // `withInvokeExactBehavior()` and `withInvokeBehavior()` make the same handle with
        // another check of the types of its accesses, which javac has made.
        if let Some(symbol) =
            value.symbol().filter(|s| s.owner == VAR_HANDLE && value.kind() == "METHOD_INVOCATION")
            && matches!(symbol.name, "withInvokeExactBehavior" | "withInvokeBehavior")
        {
            let receiver = value.get("methodSelect")?.get("expression")?;
            return self.var_handle_value(receiver, block, bound, owner);
        }
        // A parameter of the method whose body this is, or the node itself.
        let arg = |n: Node<'t>| {
            let n = plain(n);
            n.symbol()
                .filter(|s| s.kind == "PARAMETER" && n.kind() == "IDENTIFIER")
                .and_then(|s| bound.iter().find(|(name, _)| *name == s.name))
                .map_or(n, |(_, a)| *a)
        };
        let field_name = |n: Node<'t>| {
            let n = arg(n);
            n.0["value"].as_str().filter(|_| n.kind() == "STRING_LITERAL")
        };
        if let Some((name, args)) = handles_call(value) {
            return match (name, &args[..]) {
                ("arrayElementVarHandle", [class]) => Some(VarHandleOf::Element(literal_kind(arg(*class))?)),
                ("byteArrayViewVarHandle" | "byteBufferViewVarHandle", [class, order]) => {
                    let kind = literal_kind(arg(*class))?;
                    let big_endian = byte_order(arg(*order))?;
                    matches!(kind, "short" | "char" | "int" | "long" | "float" | "double").then_some(
                        match name {
                            "byteArrayViewVarHandle" => VarHandleOf::ByteView(kind, big_endian),
                            _ => VarHandleOf::BufferView(kind, big_endian),
                        },
                    )
                }
                ("findVarHandle", [class, name, _]) => {
                    Some(VarHandleOf::Field(self.offset_class(arg(*class), block)?, field_name(*name)?))
                }
                ("findStaticVarHandle", [class, name, _]) => {
                    Some(VarHandleOf::Static(self.offset_class(arg(*class), block)?, field_name(*name)?))
                }
                _ => None,
            };
        }
        let symbol = value.symbol().filter(|s| s.kind == "METHOD" && value.kind() == "METHOD_INVOCATION");
        if let Some(symbol) = symbol {
            let args: Vec<Node<'t>> = value.list("arguments").map(arg).collect();
            // `ConstantBootstraps.fieldVarHandle(lookup, "f", VarHandle.class, C.class, T.class)`,
            // the handle that `findVarHandle` makes (`SelectionKeyImpl` makes its own so).
            if symbol.owner == "java.lang.invoke.ConstantBootstraps"
                && let [_, name, _, class, _] = &args[..]
            {
                let (class, name) = (self.offset_class(*class, block)?, field_name(*name)?);
                return match symbol.name {
                    "fieldVarHandle" => Some(VarHandleOf::Field(class, name)),
                    "staticFieldVarHandle" => Some(VarHandleOf::Static(class, name)),
                    _ => None,
                };
            }
            if (symbol.owner, symbol.name) == ("jdk.internal.invoke.MhUtil", "findVarHandle") {
                return match &args[..] {
                    [_, name, _] => Some(VarHandleOf::Field(owner, field_name(*name)?)),
                    [_, class, name, _] => {
                        Some(VarHandleOf::Field(self.offset_class(*class, block)?, field_name(*name)?))
                    }
                    _ => None,
                };
            }
            // A static method of the program whose body returns a handle made of its parameters.
            let (class, returned, params) = self.single_return(value, args)?;
            return self.var_handle_value(returned, None, &params, class);
        }
        let (value, block) = self.defined(value, block)?;
        self.var_handle_value(value, block, &[], owner)
    }

    /// `H.get(a, i)`, `H.set(a, i, x)`, `H.compareAndSet(a, i, expected, x)` and the other
    /// accesses of a `VarHandle` constant `H` (`o` for a field's, in place of `a, i`): the read,
    /// write or atomic update of the element or field, as `Unsafe`'s at its offset, or of the
    /// value in the bytes of a byte array (`Builtin::ByteView`). `None` for any other call.
    pub(super) fn var_handle_access(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
    ) -> Result<Option<ExprKind>, Error> {
        if symbol.owner != VAR_HANDLE {
            return Ok(None);
        }
        // The static fences, which the JDK makes `Unsafe`'s `loadFence`, `storeFence` and
        // `fullFence`, each of which is a full fence (`Native::FullFence`).
        if symbol.is_static
            && matches!(
                symbol.name,
                "fullFence" | "acquireFence" | "releaseFence" | "loadLoadFence" | "storeStoreFence"
            )
            && node.list("arguments").next().is_none()
        {
            return Ok(Some(ExprKind::Builtin(Builtin::Native(Native::FullFence), Vec::new())));
        }
        // `layout.varHandle().get(segment, offset)` of a value layout: the translator's access
        // (`program::segment_access`), with the layout after the segment.
        if let Some((name, signature)) = crate::program::segment_access(node.0) {
            let class = crate::program::TRANSLATED_SEGMENTS;
            let resolved = self.method_named(class, name, &signature);
            if resolved.is_none() {
                return unsupported(
                    node,
                    "a segment's access through a layout's handle outside a whole program",
                );
            }
            let layout = node
                .child("methodSelect")?
                .child("expression")?
                .child("methodSelect")?
                .child("expression")?;
            let mut nodes: Vec<Node> = node.list("arguments").collect();
            nodes.insert(1, layout);
            let args = nodes.into_iter().map(|a| self.expr(a)).collect::<Result<Vec<_>, Error>>()?;
            let args = self.convert_generic_args(node, resolved, args)?;
            return Ok(Some(ExprKind::Call { name: name.to_string(), args, resolved }));
        }
        let receiver = node.child("methodSelect")?.get("expression");
        let Some((put, update)) = var_handle_method(symbol.name) else {
            return unsupported(node, &format!("`VarHandle.{}`", symbol.name));
        };
        if let Some(handle) = receiver.and_then(|r| self.var_handle(r)) {
            return self.var_handle_access_of(node, symbol, handle, put, update).map(Some);
        }
        // A handle that the code holds as a value, which can be one handle, or null
        // (`handle_values`).
        let Some((receiver, (handle, nullable))) = receiver.and_then(|r| Some((r, self.held_var_handle(r)?)))
        else {
            return unsupported(
                node,
                &format!(
                    "`VarHandle.{}` of a handle that is neither a constant nor one handle the translation follows",
                    symbol.name
                ),
            );
        };
        let access = self.var_handle_access_of(node, symbol, handle, put, update)?;
        self.through_value(node, interned_name(symbol.name), receiver, nullable, access).map(Some)
    }

    /// The access by `symbol` (of `VarHandle`), `node`, of the handle `handle`: a read or write
    /// (`put`), or the atomic update `update`.
    fn var_handle_access_of(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
        handle: VarHandleOf<'t>,
        put: Option<bool>,
        update: Option<FieldUpdate>,
    ) -> Result<ExprKind, Error> {
        let args: Vec<Node> = node.list("arguments").collect();
        let count = match handle {
            VarHandleOf::Static(..) => 0,
            VarHandleOf::Field(..) => 1,
            _ => 2,
        } + usize::from(put == Some(true))
            + match update {
                Some(FieldUpdate::CompareAndSet | FieldUpdate::CompareAndExchange) => 2,
                Some(_) => 1,
                None => 0,
            };
        if args.len() != count {
            return unsupported(node, &format!("`VarHandle.{}` with {} arguments", symbol.name, args.len()));
        }
        let kind = match handle {
            VarHandleOf::Field(class, field) => {
                let (object, ty) = self.handle_field(node, symbol, args[0], class, field)?;
                let place = Place::Field { object: Box::new(object), name: field.to_string() };
                return self.access_at(node, place, ty, class, &args[1..], update, put, false);
            }
            VarHandleOf::Static(class, field) => {
                let ty = self.static_field_type(node, class, field)?;
                let place = Place::Static { class, name: field.to_string() };
                return self.access_at(node, place, ty, class, &args, update, put, false);
            }
            VarHandleOf::Element(kind) => {
                let (array, index, ty) =
                    self.handle_element(node, symbol, args[0], (args[1], false), kind, None)?;
                let place = Place::Index(Box::new(array), Box::new(index));
                let volatile = matches!(symbol.name, "getVolatile" | "setVolatile");
                return self.access_at(node, place, ty, usize::MAX, &args[2..], update, put, volatile);
            }
            VarHandleOf::ByteView(kind, big_endian) => (kind, big_endian),
            VarHandleOf::BufferView(kind, big_endian) => {
                if update.is_some() {
                    let what = "of a view of a byte buffer";
                    return unsupported(node, &format!("`VarHandle.{}` {what}", symbol.name));
                }
                return self.buffer_view(node, symbol.name, &args, kind, big_endian);
            }
        };
        if update.is_some() {
            return unsupported(node, &format!("`VarHandle.{}` of a view of a byte array", symbol.name));
        }
        self.byte_view(node, symbol, &args, kind)
    }

    /// The read or write (`args[2]`) of the value of kind `kind.0` in the bytes of the `byte[]`
    /// `args[0]` from index `args[1]` on, in big-endian order when `kind.1`.
    fn byte_view(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
        args: &[Node<'t>],
        kind: (Kind, bool),
    ) -> Result<ExprKind, Error> {
        let (array, index, _) = self.handle_element(node, symbol, args[0], (args[1], false), "byte", None)?;
        let ty = match kind.0 {
            "short" => Type::Short,
            "char" => Type::Char,
            "int" => Type::Int,
            "long" => Type::Long,
            "float" => Type::Float,
            _ => Type::Double,
        };
        let pos = node.pos();
        let place = Place::Index(Box::new(array), Box::new(index));
        let builtin = Builtin::ByteView { ty, big_endian: kind.1 };
        if let Some(value) = args.get(2) {
            let value = self.convert(self.expr(*value)?, ty);
            let value = self.synth(ExprKind::Cast(Type::Byte, Box::new(value)), pos, Type::Byte);
            let store = self.synth(
                ExprKind::Assign { op: None, target: place, value: Box::new(value) },
                pos,
                Type::Byte,
            );
            return Ok(ExprKind::Builtin(builtin, vec![store]));
        }
        let element = match place {
            Place::Index(array, index) => self.synth(ExprKind::Index(array, index), pos, Type::Byte),
            _ => unreachable!("a view's element"),
        };
        let read = self.synth(ExprKind::Builtin(builtin, vec![element]), pos, ty);
        Ok(self.convert(read, self.ty(node, node.java_type())?).kind)
    }

    /// The read or write (`args[2]`) in the access mode of `method` (`get`, `setRelease`) of the
    /// value of kind `kind` in the bytes of the `ByteBuffer` `args[0]` from the absolute index
    /// `args[1]` on: a call of the translator's method that does what the handle does
    /// (`java/overlay/java/nio/TranslatedBufferViews.java`), with the handle's byte order last.
    fn buffer_view(
        &self,
        node: Node<'t>,
        method: &str,
        args: &[Node<'t>],
        kind: Kind,
        big_endian: bool,
    ) -> Result<ExprKind, Error> {
        let (ty, java) = match kind {
            "short" => (Type::Short, "Short"),
            "char" => (Type::Char, "Char"),
            "int" => (Type::Int, "Int"),
            "long" => (Type::Long, "Long"),
            "float" => (Type::Float, "Float"),
            _ => (Type::Double, "Double"),
        };
        let (name, signature) = crate::program::buffer_view_method(method, kind, java)
            .expect("a view's access is a get or a set");
        let class = crate::program::TRANSLATED_BUFFER_VIEWS;
        let resolved = self.method_named(class, &name, &signature);
        if resolved.is_none() {
            return unsupported(node, "a view of a byte buffer through a handle outside a whole program");
        }
        // The handle's access converts its operands and result from and to javac's types of the
        // call as `asType` does, which throws `WrongMethodTypeException` for a narrowing.
        let fits = match args.get(2) {
            Some(value) => {
                let from = plain(*value).java_type();
                widens(from, kind) || from == boxed(kind)
            }
            None => {
                let to = node.java_type();
                widens(kind, to) || to == boxed(kind) || to == "java.lang.Object"
            }
        };
        if !fits {
            return unsupported(
                node,
                &format!("`VarHandle.{method}` of a view at another type than its own"),
            );
        }
        let pos = node.pos();
        let mut operands = vec![
            self.convert(self.expr(args[0])?, Type::class("java.nio.ByteBuffer")),
            self.convert(self.expr(args[1])?, Type::Int),
        ];
        if let Some(value) = args.get(2) {
            operands.push(self.convert(self.expr(*value)?, ty));
        }
        operands.push(self.synth(ExprKind::Bool(big_endian), pos, Type::Boolean));
        let call = ExprKind::Call { name, args: operands, resolved };
        if args.len() == 3 {
            return Ok(call);
        }
        let read = self.synth(call, pos, ty);
        Ok(self.convert(read, self.ty(node, node.java_type())?).kind)
    }

    /// `U.get<Kind><Mode>(o, OFFSET)` and `U.put<Kind><Mode>(o, OFFSET, x)` of a field handle:
    /// the read `o.f`, or the assignment `o.f = x`; at an element offset, `a[i]` or `a[i] = x`.
    /// The atomic updates likewise (`field_update`, `element_update`). `None` for any other call.
    pub(super) fn handle_access(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
    ) -> Result<Option<ExprKind>, Error> {
        let key = (symbol.owner, symbol.name, symbol.ty);
        if key == OFFSET_OF_FIELD || key == SUN_OFFSET_OF_FIELD {
            return self.field_offset_value(node, key == SUN_OFFSET_OF_FIELD).map(Some);
        }
        if key == OFFSET_OF {
            return unsupported(node, "a field offset used as a value");
        }
        // A method of `sun.misc.Unsafe` that forwards to one of `jdk.internal.misc.Unsafe`'s is
        // that one's access, after its warning (`sun_unsafe.rs`).
        let forwarded =
            crate::program::sun_misc_forwarded(symbol.owner, symbol.name, symbol.ty).map(|internal| Symbol {
                owner: "jdk.internal.misc.Unsafe",
                name: interned_name(&internal),
                ..*symbol
            });
        let warned = forwarded.as_ref().map(|_| symbol.name);
        let symbol = forwarded.as_ref().unwrap_or(symbol);
        let update = crate::program::unsafe_field_update(symbol.owner, symbol.name, symbol.ty);
        let put = crate::program::unsafe_field_access(symbol.owner, symbol.name, symbol.ty);
        if update.is_none() && put.is_none() {
            return Ok(None);
        }
        let args: Vec<Node> = node.list("arguments").collect();
        let offset = match (self.handle(args[1]), self.static_handle(args[1], false)) {
            (Some((class, field)), _) => Offset::Field(class, field),
            (_, Some((class, field))) => Offset::Static(class, field),
            _ => match (self.element_offset(args[1], &[]), self.computed_offset(args[1], None, 0)) {
                (Some((index, kind)), _) => Offset::Element(index, kind),
                (_, Some(kind)) => Offset::Computed(args[1], kind),
                _ => {
                    // A plain read or write of a primitive in an array of primitives held as an
                    // `Object` (as `getCharUnaligned` and the buffers' views of a `byte[]` read):
                    // the bytes at the offset, whatever it is, as the JVM reads them; with a
                    // null base (`getInt(long address)`), at an address outside the heap.
                    let kind = access_kind(symbol.ty);
                    let base = plain(args[0]).java_type();
                    let raw_base = base == "java.lang.Object"
                        || plain(args[0]).kind() == "NULL_LITERAL"
                        || base.strip_suffix("[]").is_some_and(|e| PRIMITIVES.contains(&e));
                    if update.is_none() && kind != "reference" && raw_base && self.object.is_some() {
                        return self.raw_memory(node, &args, kind, put == Some(true)).map(Some);
                    }
                    let what = "an offset that is neither a field's nor an array element's";
                    return unsupported(node, &format!("`Unsafe.{}` with {what}", symbol.name));
                }
            },
        };
        let (place, ty, class) = match offset {
            Offset::Field(class, field) => {
                let (object, ty) = self.handle_field(node, symbol, args[0], class, field)?;
                (Place::Field { object: Box::new(object), name: field.to_string() }, ty, class)
            }
            Offset::Static(class, field) => {
                let ty = self.static_handle_field(node, symbol, args[0], class, field)?;
                (Place::Static { class, name: field.to_string() }, ty, class)
            }
            Offset::Element(index, kind) | Offset::Computed(index, kind) => {
                let access = Some(access_kind(symbol.ty));
                let computed = matches!(offset, Offset::Computed(..));
                let (array, index, ty) =
                    self.handle_element(node, symbol, args[0], (index, computed), kind, access)?;
                (Place::Index(Box::new(array), Box::new(index)), ty, usize::MAX)
            }
        };
        // `get<Kind>Volatile` and `put<Kind>Volatile`, which matter at an element offset (a
        // field's accesses are as its declaration makes them).
        let volatile = symbol.name.ends_with("Volatile");
        let access = self.access_at(node, place, ty, class, &args[2..], update, put, volatile)?;
        match warned {
            Some(method) => self.after_memory_access_warning(node, method, access).map(Some),
            None => Ok(Some(access)),
        }
    }

    /// `U.objectFieldOffset(f)` of a `Field` that is no field handle, used as a number, with `sun`
    /// `sun.misc.Unsafe`'s: the number that the translator's `Field.objectFieldOffset` gives
    /// (`program::FIELD_OFFSET_VALUE`), a number of the field's own that no access can use, as
    /// any access at an offset must name its field or element. So code that checks the layout of
    /// its fields before it reads them at offsets of its own computing (`joml`'s `MemUtil` checks
    /// that a matrix's floats are 4 bytes apart) finds that it is not the layout it expects, as on
    /// a JVM that lays them out otherwise, and takes its other way. `sun.misc.Unsafe`'s warns
    /// after its checks of the field, as Java's does.
    fn field_offset_value(&self, node: Node<'t>, sun: bool) -> Result<ExprKind, Error> {
        let (class, name, signature) = crate::program::FIELD_OFFSET_VALUE;
        let Some(method) = self.method_named(class, name, signature) else {
            return unsupported(node, "a field offset used as a value");
        };
        let select = node.child("methodSelect")?;
        let receiver = select.get("expression").filter(|_| select.kind() == "MEMBER_SELECT");
        let stateless = receiver.is_none_or(|r| match r.symbol() {
            Some(s) => s.kind == "FIELD" || s.name == "getUnsafe",
            None => false,
        });
        if !sun && !stateless {
            return unsupported(node, "`Unsafe.objectFieldOffset` on this receiver");
        }
        let pos = node.pos();
        let Some(field) = node.list("arguments").next() else {
            return unsupported(node, "a field offset used as a value");
        };
        let args = vec![self.expr(field)?, self.synth(ExprKind::Bool(sun), pos, Type::Boolean)];
        let call = ExprKind::Call { name: name.to_string(), args, resolved: Some(method) };
        if !sun {
            return Ok(call);
        }
        let local = self.fresh_name("offset");
        let call = self.synth(call, pos, Type::Long);
        let warning = self.memory_access_warning(node, "objectFieldOffset")?;
        let body = vec![
            Stmt::Local { ty: Type::Long, name: local.clone(), init: Some(call), pos },
            Stmt::Expr(warning),
            Stmt::Yield(self.synth(ExprKind::Var(local), pos, Type::Long)),
        ];
        Ok(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body }],
        })
    }

    /// `U.get<Kind>(base, offset)` or `U.put<Kind>(base, offset, x)` of a primitive of kind `kind`
    /// at an offset that is computed elsewhere, in a base that holds an array of primitives:
    /// `Builtin::RawMemory` of the base as an `Object`.
    fn raw_memory(
        &self,
        node: Node<'t>,
        args: &[Node<'t>],
        kind: Kind,
        put: bool,
    ) -> Result<ExprKind, Error> {
        let ty = match kind {
            "boolean" => Type::Boolean,
            "byte" => Type::Byte,
            "short" => Type::Short,
            "char" => Type::Char,
            "int" => Type::Int,
            "long" => Type::Long,
            "float" => Type::Float,
            _ => Type::Double,
        };
        let base = self.expr(args[0])?;
        self.note_object_array(base.ty);
        let mut operands =
            vec![self.convert(base, Type::class(OBJECT)), self.convert(self.expr(args[1])?, Type::Long)];
        if put {
            operands.push(self.convert(self.expr(args[2])?, ty));
            return Ok(ExprKind::Builtin(Builtin::RawMemory { ty, put }, operands));
        }
        let read = self.synth(ExprKind::Builtin(Builtin::RawMemory { ty, put }, operands), node.pos(), ty);
        Ok(self.convert(read, self.ty(node, node.java_type())?).kind)
    }

    /// The access of `Unsafe` or of a `VarHandle` `node` at `place`, of type `ty`, a field of class
    /// `class` or an element: the read, the write of `values[0]` (`put`), or the atomic update
    /// `update` with `values` as its operands. A `volatile` read or write of an element is one of
    /// `Class::volatile_elements`.
    #[allow(clippy::too_many_arguments)]
    fn access_at(
        &self,
        node: Node<'t>,
        place: Place,
        ty: Type,
        class: usize,
        values: &[Node<'t>],
        update: Option<FieldUpdate>,
        put: Option<bool>,
        volatile: bool,
    ) -> Result<ExprKind, Error> {
        // The update method of a static field compares with `==`, which the generator does not
        // write between a string's owned and borrowed forms.
        if ty == Type::String
            && matches!(place, Place::Static { .. })
            && matches!(update, Some(FieldUpdate::CompareAndSet | FieldUpdate::CompareAndExchange))
        {
            return unsupported(node, "a compare-and-set of a static `String` field");
        }
        let values = values.iter().map(|a| self.expr(*a)).collect::<Result<Vec<_>, Error>>()?;
        self.access_of(node, place, ty, class, values, update, put, volatile)
    }

    /// `access_at` with its values lowered.
    #[allow(clippy::too_many_arguments)]
    fn access_of(
        &self,
        node: Node<'t>,
        place: Place,
        ty: Type,
        class: usize,
        values: Vec<Expr>,
        update: Option<FieldUpdate>,
        put: Option<bool>,
        volatile: bool,
    ) -> Result<ExprKind, Error> {
        let ret = self.ty(node, node.java_type())?;
        if let Some(update) = update {
            let operands = values.into_iter().map(|v| self.stored(v, ty)).collect();
            let call = match place {
                Place::Field { object, name } => {
                    self.field_update(Some(*object), class, &name, ty, update, operands)
                }
                Place::Static { name, .. } => self.field_update(None, class, &name, ty, update, operands),
                place => self.element_update(node, place, ty, update, operands)?,
            };
            let result = if update == FieldUpdate::CompareAndSet { Type::Boolean } else { ty };
            let call = self.synth(call, node.pos(), result);
            return Ok(self.convert(call, ret).kind);
        }
        if let (Place::Index(_, index), true) = (&place, volatile) {
            self.volatile_elements.borrow_mut().insert(index.id);
        }
        if put == Some(true) {
            let value = values.into_iter().next().expect("a write has its value");
            let value = self.stored(value, ty);
            return Ok(ExprKind::Assign { op: None, target: place, value: Box::new(value) });
        }
        let read = match place {
            Place::Index(array, index) => ExprKind::Index(array, index),
            place => objects::read(place),
        };
        let read = self.synth(read, node.pos(), ty);
        Ok(self.convert(read, ret).kind)
    }

    /// `U.compareAndSet<Kind>(o, OFFSET, expected, x)` or another atomic update of field `field`
    /// of class `class`, of type `ty`, on `object`: a call of the method of the field's class that
    /// makes it (see the module's comment).
    fn field_update(
        &self,
        object: Option<Expr>,
        class: usize,
        field: &str,
        ty: Type,
        update: FieldUpdate,
        operands: Vec<Expr>,
    ) -> ExprKind {
        let name = self.update_method_name(class, field, ty, update, object.is_none(), None);
        match object {
            Some(object) => ExprKind::MethodCall {
                receiver: Box::new(object),
                name,
                args: operands,
                resolved: None,
                is_super: false,
            },
            None => ExprKind::Call { name, args: operands, resolved: None },
        }
    }

    /// `U.compareAndSet<Kind>(a, OFFSET, expected, x)` or another atomic update at an element
    /// offset: `Builtin::ElementUpdate` of the store it makes to `place` (`a[i] = x`, or
    /// `a[i] += delta`), after the expected value of the compare forms.
    fn element_update(
        &self,
        node: Node<'t>,
        place: Place,
        ty: Type,
        update: FieldUpdate,
        mut operands: Vec<Expr>,
    ) -> Result<ExprKind, Error> {
        // The runtime compares by identity what the generator can: objects, arrays, enums and
        // numbers. (An `Object` that may hold a string or a box is rejected by
        // `analysis::patterns`, as `==` on it is.)
        if matches!(ty, Type::String | Type::StringBuilder | Type::Boxed(_) | Type::JavaClass)
            || self.param_owner(ty).is_some()
        {
            return unsupported(node, &format!("an atomic update of an element of a `{ty}[]`"));
        }
        let value = Box::new(operands.pop().expect("an update has an operand"));
        let op = match update {
            FieldUpdate::GetAndAdd => Some(BinOp::Add),
            FieldUpdate::GetAndBitwise(op) => Some(op),
            _ => None,
        };
        let store = self.synth(ExprKind::Assign { op, target: place, value }, node.pos(), ty);
        operands.push(store);
        Ok(ExprKind::Builtin(Builtin::ElementUpdate(update), operands))
    }

    /// The object of an access of `Unsafe` (`object`, its first argument) at the offset of field
    /// `field` of class `class`, and the field's type.
    fn handle_field(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
        object: Node<'t>,
        class: usize,
        field: &str,
    ) -> Result<(Expr, Type), Error> {
        let object = self.expr(object)?;
        let of_class = match object.ty {
            Type::Class(name, _) => {
                self.type_index(name).is_some_and(|t| t == class || self.is_subclass(t, class))
            }
            _ => false,
        };
        if !of_class {
            return unsupported(node, &format!("`Unsafe.{}` on an object of another class", symbol.name));
        }
        let member = self.types[class]
            .node
            .list("members")
            .find(|m| m.kind() == "VARIABLE" && m.str("name") == field && !m.has_flag("STATIC"));
        let Some(member) = member else {
            return unsupported(node, &format!("the offset of `{field}`, which is not a field of the class"));
        };
        let ty = self.declared_type(member, member.symbol().map_or("", |s| s.ty), class, None)?;
        Ok((object, ty))
    }

    /// The type of the static field `field` of class `class`, which an access of `Unsafe` at its
    /// offset (`staticFieldOffset`) makes on `base`, which must be that field's base.
    fn static_handle_field(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
        base: Node<'t>,
        class: usize,
        field: &str,
    ) -> Result<Type, Error> {
        if self.static_handle(base, true).is_none_or(|b| b != (class, field)) {
            return unsupported(
                node,
                &format!("`Unsafe.{}` on another base than its static field's", symbol.name),
            );
        }
        self.static_field_type(node, class, field)
    }

    /// The type of the static field `field` of class `class`, which a handle names.
    fn static_field_type(&self, node: Node<'t>, class: usize, field: &str) -> Result<Type, Error> {
        let member = self.types[class]
            .node
            .list("members")
            .find(|m| m.kind() == "VARIABLE" && m.str("name") == field && m.has_flag("STATIC"));
        let Some(member) = member else {
            return unsupported(
                node,
                &format!("the offset of `{field}`, which is not a static field of the class"),
            );
        };
        self.declared_type(member, member.symbol().map_or("", |s| s.ty), class, None)
    }

    /// The array (`array`, the first argument) and index of an access of `Unsafe` at an element
    /// offset of an array of kind `kind` with index `index.0` (or, when `index.1`, at the offset
    /// that `index.0` computes, see `computed_offset`), and the elements' type, which must be of
    /// that kind, and of the access's.
    fn handle_element(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
        array: Node<'t>,
        (index, computed): (Node<'t>, bool),
        kind: Kind,
        access: Option<Kind>,
    ) -> Result<(Expr, Expr, Type), Error> {
        let array = self.expr(array)?;
        let owner = symbol.owner.rsplit('.').next().unwrap_or_default();
        let Type::Array(element) = array.ty else {
            let what = "at an element offset of a value that is not an array";
            return unsupported(node, &format!("`{owner}.{}` {what}", symbol.name));
        };
        let element = *element;
        if type_kind(element) != kind || access.is_some_and(|a| a != kind) {
            let what = "at an element offset of an array of another type";
            return unsupported(node, &format!("`{owner}.{}` {what}", symbol.name));
        }
        let index = self.expr(index)?;
        let index = if computed { self.computed_index(index, kind) } else { index };
        let index = match index.ty {
            Type::Long => {
                let pos = index.pos;
                self.synth(ExprKind::Cast(Type::Int, Box::new(index)), pos, Type::Int)
            }
            _ => self.convert(index, Type::Int),
        };
        Ok((array, index, element))
    }

    /// The index of the element of an array of kind `kind` at the offset `offset`, a `long`:
    /// `(offset - base) / scale`, of `Unsafe`'s base offset and index scale of such arrays.
    fn computed_index(&self, offset: Expr, kind: Kind) -> Expr {
        let pos = offset.pos;
        let offset = self.convert(offset, Type::Long);
        let native = |n| ExprKind::Builtin(Builtin::Native(n), Vec::new());
        let base = self.synth(native(Native::ArrayBaseOffset), pos, Type::Long);
        let element = if kind == "reference" { "java.lang.Object" } else { kind };
        let scale = self.synth(native(Native::ArrayIndexScale(element)), pos, Type::Int);
        let scale = self.convert(scale, Type::Long);
        let from_base =
            self.synth(ExprKind::Binary(BinOp::Sub, Box::new(offset), Box::new(base)), pos, Type::Long);
        self.synth(ExprKind::Binary(BinOp::Div, Box::new(from_base), Box::new(scale)), pos, Type::Long)
    }

    /// `value`, to be stored in a field of type `ty`: cast to it from `Unsafe`'s `Object` or
    /// wider type.
    fn stored(&self, value: Expr, ty: Type) -> Expr {
        if value.ty == ty || self.param_owner(ty).is_some() {
            return value;
        }
        let pos = value.pos;
        self.synth(ExprKind::Cast(ty, Box::new(value)), pos, ty)
    }

    /// The name of the method that makes the atomic update `update` of the field `field` of
    /// class `class`, of type `ty`, or the loop `looped` of it, which `field_update_methods` adds.
    fn update_method_name(
        &self,
        class: usize,
        field: &str,
        ty: Type,
        update: FieldUpdate,
        is_static: bool,
        looped: Option<UpdateLoop>,
    ) -> String {
        // A loop calls the compare-and-set that it loops on.
        if looped.is_some() {
            self.update_method_name(class, field, ty, update, is_static, None);
        }
        let mut updates = self.field_updates.borrow_mut();
        if let Some(u) = updates
            .iter()
            .find(|u| u.class == class && u.field == field && u.update == update && u.looped == looped)
        {
            return u.name.clone();
        }
        let verb = match update {
            _ if let Some(l) = &looped => match (l.accumulates, l.gives_new) {
                (false, false) => "getAndUpdate",
                (false, true) => "updateAndGet",
                (true, false) => "getAndAccumulate",
                (true, true) => "accumulateAndGet",
            },
            FieldUpdate::GetAndAdd => "getAndAdd",
            FieldUpdate::GetAndSet => "getAndSet",
            FieldUpdate::CompareAndSet => "compareAndSet",
            FieldUpdate::CompareAndExchange => "compareAndExchange",
            FieldUpdate::GetAndBitwise(BinOp::BitOr) => "getAndBitwiseOr",
            FieldUpdate::GetAndBitwise(BinOp::BitAnd) => "getAndBitwiseAnd",
            FieldUpdate::GetAndBitwise(_) => "getAndBitwiseXor",
        };
        let mut chars = field.chars();
        let capitalized: String =
            chars.next().map(|c| c.to_ascii_uppercase()).into_iter().chain(chars).collect();
        let base = format!("{verb}{capitalized}");
        let name = (1..)
            .map(|n| if n == 1 { base.clone() } else { format!("{base}{n}") })
            .find(|n| !updates.iter().any(|u| &u.name == n))
            .expect("a free name");
        updates.push(FieldUpdateMethod {
            class,
            field: field.to_string(),
            ty,
            update,
            is_static,
            name: name.clone(),
            looped,
        });
        name
    }

    /// The methods of the atomic updates that the code makes, appended to `methods`, and their
    /// calls resolved to them.
    pub(super) fn field_update_methods(&self, methods: &mut Vec<Method>) {
        let updates = self.field_updates.take();
        if updates.is_empty() {
            return;
        }
        let first = methods.len();
        let index: HashMap<String, usize> =
            updates.iter().enumerate().map(|(i, u)| (u.name.clone(), first + i)).collect();
        // A loop's body calls its compare-and-set.
        methods.extend(updates.iter().map(|u| self.update_method(u, &updates)));
        for m in methods.iter_mut() {
            for s in &mut m.body {
                rewrite_stmt(s, &mut |e| {
                    if let ExprKind::MethodCall { name, resolved: resolved @ None, .. }
                    | ExprKind::Call { name, resolved: resolved @ None, .. } = &mut e.kind
                        && let Some(&i) = index.get(name.as_str())
                    {
                        *resolved = Some(i);
                    }
                });
            }
        }
    }

    /// The method that makes the update `u`, one of `updates`: the field read, compared and
    /// written in its body.
    fn update_method(&self, u: &FieldUpdateMethod, updates: &[FieldUpdateMethod]) -> Method {
        let pos = Pos::default();
        let ty = u.ty;
        let field = || {
            if u.is_static {
                return Place::Static { class: u.class, name: u.field.clone() };
            }
            let this = self.synth(ExprKind::This, pos, self.self_type(u.class));
            Place::Field { object: Box::new(this), name: u.field.clone() }
        };
        let read = || self.synth(objects::read(field()), pos, ty);
        let var = |name: &str| self.synth(ExprKind::Var(name.to_string()), pos, ty);
        let assign = |value: Expr| {
            let kind = ExprKind::Assign { op: None, target: field(), value: Box::new(value) };
            Stmt::Expr(self.synth(kind, pos, ty))
        };
        let param = |name: &str| Param { name: name.to_string(), ty };
        let old = || Stmt::Local { ty, name: "old".to_string(), init: Some(read()), pos };
        let ret = |e: Expr| Stmt::Return(Some(e), pos);
        // `Unsafe` compares a `float` or `double` by its raw bits, as its `int` or `long`
        // compare-and-set does (so `NaN` is itself, and `-0.0` is not `0.0`).
        let bits = |e: Expr| {
            let (native, bits) = match ty {
                Type::Float => (Native::FloatToRawIntBits, Type::Int),
                Type::Double => (Native::DoubleToRawLongBits, Type::Long),
                _ => return e,
            };
            self.synth(ExprKind::Builtin(Builtin::Native(native), vec![e]), pos, bits)
        };
        let is = |a: Expr, b: Expr| {
            let (a, b) = (Box::new(bits(a)), Box::new(bits(b)));
            self.synth(ExprKind::Binary(BinOp::Eq, a, b), pos, Type::Boolean)
        };
        let boolean = |b: bool| self.synth(ExprKind::Bool(b), pos, Type::Boolean);
        let (params, result, body) = match u.update {
            _ if let Some(l) = &u.looped => self.loop_body(u, l, updates),
            FieldUpdate::GetAndAdd | FieldUpdate::GetAndBitwise(_) => {
                let op = if let FieldUpdate::GetAndBitwise(op) = u.update { op } else { BinOp::Add };
                let sum = ExprKind::Binary(op, Box::new(var("old")), Box::new(var("delta")));
                let sum = self.synth(sum, pos, ty);
                (vec![param("delta")], ty, vec![old(), assign(sum), ret(var("old"))])
            }
            FieldUpdate::GetAndSet => {
                (vec![param("value")], ty, vec![old(), assign(var("value")), ret(var("old"))])
            }
            FieldUpdate::CompareAndSet => {
                let then = Stmt::Block(vec![assign(var("value")), ret(boolean(true))]);
                let test =
                    Stmt::If { cond: is(read(), var("expected")), then: Box::new(then), otherwise: None };
                (vec![param("expected"), param("value")], Type::Boolean, vec![test, ret(boolean(false))])
            }
            FieldUpdate::CompareAndExchange => {
                let test = Stmt::If {
                    cond: is(var("old"), var("expected")),
                    then: Box::new(assign(var("value"))),
                    otherwise: None,
                };
                (vec![param("expected"), param("value")], ty, vec![old(), test, ret(var("old"))])
            }
        };
        Method {
            name: u.name.clone(),
            params,
            ret: result,
            body,
            is_main: false,
            is_static: u.is_static,
            is_ctor: false,
            class: u.class,
            pos,
            is_abstract: false,
            // A static field has no object whose lock the method holds: it holds its class's.
            is_synchronized: u.is_static,
            overrides: Vec::new(),
            type_params: Vec::new(),
            jdk_code: false,
            // A loop is code like any other, which calls the update that it loops on.
            field_update: u.looped.is_none().then(|| (u.update, u.field.clone())),
        }
    }

    /// The parameters, the type and the body of the loop `l` that the method `u`, one of
    /// `updates`, makes. As the JDK's updater does, it reads the field, applies the function
    /// and compares and sets, `do { prev = get(obj); next = f.apply(prev); } while
    /// (!compareAndSet(obj, prev, next)); return prev;`, so that a write between the read and the
    /// compare makes it apply the function again.
    fn loop_body(
        &self,
        u: &FieldUpdateMethod,
        l: &UpdateLoop,
        updates: &[FieldUpdateMethod],
    ) -> (Vec<Param>, Type, Vec<Stmt>) {
        let (pos, ty) = (Pos::default(), u.ty);
        let var = |name: &str| self.synth(ExprKind::Var(name.to_string()), pos, ty);
        let this = || self.synth(ExprKind::This, pos, self.self_type(u.class));
        let field = Place::Field { object: Box::new(this()), name: u.field.clone() };
        let read = self.synth(objects::read(field), pos, ty);
        let (apply, name, ref takes, gives) = l.apply;
        let mut operands = vec![var("prev")];
        if l.accumulates {
            operands.push(var("x"));
        }
        let operands = operands.into_iter().zip(takes).map(|(e, &t)| self.convert(e, t)).collect();
        let function = self.synth(ExprKind::Var("function".to_string()), pos, l.function);
        let applied = ExprKind::MethodCall {
            receiver: Box::new(function),
            name: name.to_string(),
            args: operands,
            resolved: Some(apply),
            is_super: false,
        };
        let applied = self.stored(self.synth(applied, pos, gives), ty);
        let compare = updates
            .iter()
            .find(|c| c.class == u.class && c.field == u.field && c.update == u.update && c.looped.is_none())
            .expect("a loop's compare-and-set is added with it");
        let compare = compare.name.clone();
        let compared = ExprKind::MethodCall {
            receiver: Box::new(this()),
            name: compare,
            args: vec![var("prev"), var("next")],
            resolved: None,
            is_super: false,
        };
        let compared = self.synth(compared, pos, Type::Boolean);
        let given = Stmt::Return(Some(var(if l.gives_new { "next" } else { "prev" })), pos);
        let body = vec![
            Stmt::Local { ty, name: "prev".to_string(), init: Some(read), pos },
            Stmt::Local { ty, name: "next".to_string(), init: Some(applied), pos },
            Stmt::If { cond: compared, then: Box::new(given), otherwise: None },
        ];
        let forever = Stmt::While {
            cond: self.synth(ExprKind::Bool(true), pos, Type::Boolean),
            body: Box::new(Stmt::Block(body)),
        };
        let mut params = if l.accumulates { vec![Param { name: "x".to_string(), ty }] } else { Vec::new() };
        params.push(Param { name: "function".to_string(), ty: l.function });
        (params, ty, vec![forever])
    }
}
