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
//! be written as the offset argument itself. The field may be one of another class, such as the
//! `threadLocalRandomProbe` of `Thread` that `ThreadLocalRandom` reads, as long as the object
//! of the access is of that class. A read or write through the handle is a read or write of the
//! field itself, so the call above lowers to `this.constant`. That is Java's meaning in a program
//! with one thread, and in a program with threads the field is the object's, read and written
//! under its lock, which orders the accesses at least as the plain, opaque, acquire, release and
//! volatile forms ask. Any other use of a handle is rejected.
//!
//! An atomic update through a handle, `U.compareAndSetInt(this, VALUE, expect, update)` and the
//! get-and-add, get-and-bitwise-or (and `And`, `Xor`), get-and-set and compare-and-exchange forms
//! of `int`, `long` and reference fields
//! (`program::FieldUpdate`), is a call of a method that lowering adds to the field's class, one
//! per field and form: `compareAndSetValue(expected, value)` reads the field, compares and
//! writes in its body. A method runs with its object borrowed, or in a program with threads with
//! the object's lock held, for the whole call, so the update is atomic as Java's is: no other
//! access of the object comes between the read and the write. The weak forms never fail
//! spuriously, which Java allows.
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
//! a read or write of `a[index]`, which in a program with threads holds the array's lock for the
//! access. An atomic update of an element is `Builtin::ElementUpdate`, whose operands are the
//! store it makes, `a[index] = x` (or `a[index] += delta`), so that every analysis sees the store,
//! and for the compare forms the expected value first; the runtime's function for it
//! (`java::compare_and_set_element`, ...) reads, compares and writes the element under one borrow
//! of the array, which in a program with threads is one hold of the array's lock, so no other
//! access of the array comes between the read and the write.
//!
//! An element offset may also be computed away from the access: by a static method whose body
//! returns one of its parameter (`ForkJoinPool.slotOffset(i)`), whose call is the element at its
//! argument, or into a `long` local that every assignment gives an element offset of one kind of
//! array (`U.getReferenceAcquire(a, k = slotOffset(i))`, then `U.compareAndSetReference(a, k, t,
//! null)`). The local holds the offset Java computes, from `Unsafe`'s answers for the base and
//! scale, so the element's index is `(k - base) / scale`. Any other offset arithmetic is rejected.
//!
//! The offset of a static field (`staticFieldOffset(f)`, with `staticFieldBase(f)` as the object,
//! of a field `f` that `getDeclaredField` finds, as `ForkJoinPool` finds `poolIds`) is a handle of
//! that field: an access is of the static field, and an atomic update is a static method of its
//! class that is `synchronized`, so it holds the class's lock in a program with threads. The
//! `Field` local is left out with the handles it makes.
//!
//! A `VarHandle` constant (a static final field) is a handle as well: of a field
//! (`MethodHandles.lookup().findVarHandle(C.class, "f", T.class)`, or `MhUtil.findVarHandle`), of
//! the elements of an array (`arrayElementVarHandle(T[].class)`), or of the values whose bytes a
//! `byte[]` holds (`byteArrayViewVarHandle(int[].class, ByteOrder.BIG_ENDIAN)`, as
//! `DataOutputStream` and `DataInputStream` write and read numbers). Its `get`, `set`,
//! `compareAndSet`, `getAndAdd` and the other accesses, in every memory mode, are those of
//! `Unsafe` at the field or element; a view's `get` and `set` are `Builtin::ByteView`, whose
//! operand is the access of the first byte (`a[i]`, or `a[i] = (byte) v`), so the analyses see
//! it, and which throws Java's `ArrayIndexOutOfBoundsException` when the value's bytes do not fit.
//! A static method that returns such a handle of its parameters (`ByteArray.create`) makes one
//! too. Any other use of a `VarHandle` is rejected.

use super::*;
use crate::program::FieldUpdate;
use std::collections::HashMap;

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

/// `Integer.numberOfLeadingZeros(int)`, which turns a power of two into its exponent.
const LEADING_ZEROS: (&str, &str, &str) = ("java.lang.Integer", "numberOfLeadingZeros", "(int)int");

/// The primitive types, as javac names them and as the kinds of arrays and of `Unsafe`'s
/// accesses; every other type is a reference.
const PRIMITIVES: [&str; 8] = ["boolean", "byte", "short", "char", "int", "long", "float", "double"];

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
fn plain(mut node: Node) -> Node {
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
fn walk<'t>(v: &'t Value, f: &mut impl FnMut(Node<'t>)) {
    match v {
        Value::Object(map) => {
            f(Node(v));
            map.values().for_each(|c| walk(c, f));
        }
        Value::Array(items) => items.iter().for_each(|i| walk(i, f)),
        _ => {}
    }
}

/// Whether `node` is the variable of an assignment, compound assignment, `++` or `--` that
/// `target` tells apart.
fn assigns<'t>(node: Node<'t>, target: impl Fn(Node<'t>) -> bool) -> bool {
    let kind = node.kind();
    let variable = if kind == "ASSIGNMENT" || kind.ends_with("_ASSIGNMENT") {
        node.get("variable")
    } else if kind.ends_with("_INCREMENT") || kind.ends_with("_DECREMENT") {
        node.get("expression")
    } else {
        None
    };
    variable.is_some_and(|v| target(plain(v)))
}

/// What a `VarHandle` constant reads and writes.
#[derive(Clone, Copy)]
enum VarHandleOf<'t> {
    /// The field of a class (`findVarHandle`), by the class's index.
    Field(usize, &'t str),
    /// The elements of an array of this kind (`arrayElementVarHandle`).
    Element(Kind),
    /// The values of this kind whose bytes are those of a `byte[]`, in big-endian order or not
    /// (`byteArrayViewVarHandle`).
    ByteView(Kind, bool),
}

/// The method of `MethodHandles` (or of its `Lookup`) that `node` calls, and its arguments.
fn handles_call<'t>(node: Node<'t>) -> Option<(&'t str, Vec<Node<'t>>)> {
    let node = plain(node);
    let symbol = node.symbol().filter(|_| node.kind() == "METHOD_INVOCATION")?;
    let owner = symbol.owner.strip_prefix(METHOD_HANDLES)?;
    matches!(owner, "" | ".Lookup").then(|| (symbol.name, node.list("arguments").collect()))
}

/// Whether `node` is `ByteOrder.BIG_ENDIAN` (`Some(true)`) or `ByteOrder.LITTLE_ENDIAN`.
fn byte_order(node: Node) -> Option<bool> {
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
    fn static_value(&self, owner: usize, member: Node<'t>) -> Option<(Node<'t>, Option<Node<'t>>)> {
        if member.kind() != "VARIABLE" || !member.has_flag("STATIC") || !member.has_flag("FINAL") {
            return None;
        }
        if let Some(init) = member.get("initializer") {
            return Some((init, None));
        }
        let (name, javac) = (member.str("name"), self.types[owner].javac.as_str());
        let is_field = |v: Node| {
            v.symbol().is_some_and(|s| s.kind == "FIELD" && s.name == name && s.owner == javac && s.is_static)
        };
        let (mut writes, mut found) = (0, None);
        for block in self.types[owner].node.list("members").filter(|m| Self::is_static_block(*m)) {
            walk(block.0, &mut |n| {
                writes += usize::from(assigns(n, is_field));
                if n.kind() == "EXPRESSION_STATEMENT"
                    && let Some(e) = n.get("expression").filter(|e| e.kind() == "ASSIGNMENT")
                    && e.get("variable").is_some_and(|v| is_field(plain(v)))
                {
                    found = e.get("expression").map(|value| (value, Some(block)));
                }
            });
        }
        found.filter(|_| writes == 1)
    }

    /// Whether the statement `node` is the assignment in a static block that gives a field
    /// handle its value (see `static_value`), which is left out.
    pub(super) fn handle_definition(&self, node: Node<'t>) -> bool {
        let Some(e) = node.get("expression").filter(|_| node.kind() == "EXPRESSION_STATEMENT") else {
            return false;
        };
        let symbol = e.get("variable").filter(|_| e.kind() == "ASSIGNMENT").and_then(|v| plain(v).symbol());
        let Some(symbol) = symbol.filter(|s| s.kind == "FIELD" && s.is_static) else { return false };
        self.static_field(&symbol).is_some_and(|(owner, member)| self.is_handle(owner, member))
    }

    /// Whether `stmt`, a statement of `block`, declares a local that the block uses only to make
    /// handles, whose definitions are left out (`handle_definition`), so that the local is left
    /// out with them: a `Field` whose value is `getDeclaredField`, used only as the argument of
    /// `staticFieldOffset(f)`, `staticFieldBase(f)` or `objectFieldOffset(f)`, or a
    /// `MethodHandles.Lookup` whose value is `MethodHandles.lookup()`, used only in the values of
    /// `VarHandle` constants (`SharedThreadContainer` finds its handles so).
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
                walk(block.0, &mut |n| {
                    uses += usize::from(n.kind() == "IDENTIFIER" && n.str("name") == name);
                    let handle = [STATIC_OFFSET_OF, STATIC_BASE_OF, OFFSET_OF_FIELD]
                        .into_iter()
                        .find_map(|m| call_of(n, m));
                    in_handles +=
                        usize::from(handle.is_some_and(|args| args.first().is_some_and(|a| named(*a))));
                });
            }
            "java.lang.invoke.MethodHandles.Lookup" => {
                if handles_call(init).is_none_or(|(name, args)| name != "lookup" || !args.is_empty()) {
                    return false;
                }
                walk(block.0, &mut |n| {
                    uses += usize::from(n.kind() == "IDENTIFIER" && n.str("name") == name);
                    let constant = n.kind() == "ASSIGNMENT"
                        && n.get("variable").is_some_and(|v| self.var_handle(v).is_some());
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
    fn static_field(&self, symbol: &Symbol) -> Option<(usize, Node<'t>)> {
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
        let Some(args) = call_of(value, OFFSET_OF) else {
            return self.declared_field(*call_of(value, OFFSET_OF_FIELD)?.first()?, block);
        };
        let field = plain(*args.get(1)?);
        let name = field.0["value"].as_str().filter(|_| field.kind() == "STRING_LITERAL")?;
        Some((self.offset_class(*args.first()?, block)?, name))
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

    /// The class that `node` names: a class literal, or a local of the static block `block` that
    /// holds one.
    fn offset_class(&self, node: Node<'t>, block: Option<Node<'t>>) -> Option<usize> {
        let node = plain(node);
        if node.kind() == "MEMBER_SELECT" && node.str("identifier") == "class" {
            return self.type_index(node.get("expression")?.java_type());
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
    /// given the arithmetic itself: an assignment of an element offset (`k = slotOffset(i)`), or
    /// a `long` local of the method being lowered that every assignment gives an element offset
    /// of the same kind, as `ForkJoinPool` keeps one to compare and set the element it read.
    /// Such a value is the real offset that Java computes, from `Unsafe`'s answers for the base
    /// and scale, so the index is `(offset - base) / scale` (`computed_index`).
    fn computed_offset(&self, offset: Node<'t>, depth: usize) -> Option<Kind> {
        let offset = plain(offset);
        if depth > 4 {
            return None;
        }
        match offset.kind() {
            "ASSIGNMENT" => self.computed_offset(offset.get("expression")?, depth + 1),
            "IDENTIFIER" => {
                let symbol = offset.symbol().filter(|s| s.kind == "LOCAL_VARIABLE" && s.ty == "long")?;
                let body = self.methods[self.method_ctx.get()?].node.get("body")?;
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
                        kinds.push(self.computed_offset(value, depth + 1));
                    }
                });
                let first = *kinds.first()?;
                (!other && kinds.iter().all(|k| *k == first)).then_some(first?)
            }
            _ => self.element_offset(offset, &[]).map(|(_, kind)| kind),
        }
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
        Ok(())
    }

    /// Whether the static field `member` of class `owner` is a handle: a field offset or a
    /// `VarHandle` constant, which is no field of the translation.
    pub(super) fn is_handle(&self, owner: usize, member: Node<'t>) -> bool {
        self.handle_declaration(owner, member).is_some()
            || self.var_handle_declaration(owner, member).is_some()
            || self.static_declaration(owner, member, false).is_some()
            || self.static_declaration(owner, member, true).is_some()
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
    fn var_handle(&self, node: Node<'t>) -> Option<VarHandleOf<'t>> {
        let symbol =
            plain(node).symbol().filter(|s| s.kind == "FIELD" && s.is_static && s.ty == VAR_HANDLE)?;
        let (owner, member) = self.static_field(&symbol)?;
        self.var_handle_declaration(owner, member)
    }

    /// What the `VarHandle` that `value` makes reaches: a call of `arrayElementVarHandle` or
    /// `byteArrayViewVarHandle` of an array class literal, of `findVarHandle` of a class literal
    /// and a field's name, or of `MhUtil.findVarHandle` of a field of `owner` (the class of the
    /// lookup); a call of a static method that returns one of those of its parameters (`bound`
    /// gives their arguments), as `ByteArray.create(int[].class)` does; or a constant whose value
    /// is one. `block` is the static block `value` is in, if any.
    fn var_handle_value(
        &self,
        value: Node<'t>,
        block: Option<Node<'t>>,
        bound: &[(&str, Node<'t>)],
        owner: usize,
    ) -> Option<VarHandleOf<'t>> {
        let value = plain(value);
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
                ("byteArrayViewVarHandle", [class, order]) => {
                    let kind = literal_kind(arg(*class))?;
                    matches!(kind, "short" | "char" | "int" | "long" | "float" | "double")
                        .then_some(VarHandleOf::ByteView(kind, byte_order(arg(*order))?))
                }
                ("findVarHandle", [class, name, _]) => {
                    Some(VarHandleOf::Field(self.offset_class(arg(*class), block)?, field_name(*name)?))
                }
                _ => None,
            };
        }
        let symbol = value.symbol().filter(|s| s.kind == "METHOD" && value.kind() == "METHOD_INVOCATION");
        if let Some(symbol) = symbol {
            let args: Vec<Node<'t>> = value.list("arguments").map(arg).collect();
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
            let resolved = self
                .methods
                .iter()
                .position(|m| self.types[m.owner].javac == class && m.name == name && m.sig == signature);
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
        let (Some((put, update)), Some(handle)) =
            (var_handle_method(symbol.name), receiver.and_then(|r| self.var_handle(r)))
        else {
            return unsupported(
                node,
                &format!("`VarHandle.{}` of a handle that is not a constant", symbol.name),
            );
        };
        let args: Vec<Node> = node.list("arguments").collect();
        let count = match handle {
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
                return self.access_at(node, place, ty, class, &args[1..], update, put).map(Some);
            }
            VarHandleOf::Element(kind) => {
                let (array, index, ty) =
                    self.handle_element(node, symbol, args[0], (args[1], false), kind, None)?;
                let place = Place::Index(Box::new(array), Box::new(index));
                return self.access_at(node, place, ty, usize::MAX, &args[2..], update, put).map(Some);
            }
            VarHandleOf::ByteView(kind, big_endian) => (kind, big_endian),
        };
        if update.is_some() {
            return unsupported(node, &format!("`VarHandle.{}` of a view of a byte array", symbol.name));
        }
        self.byte_view(node, symbol, &args, kind).map(Some)
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

    /// `U.get<Kind><Mode>(o, OFFSET)` and `U.put<Kind><Mode>(o, OFFSET, x)` of a field handle:
    /// the read `o.f`, or the assignment `o.f = x`; at an element offset, `a[i]` or `a[i] = x`.
    /// The atomic updates likewise (`field_update`, `element_update`). `None` for any other call.
    pub(super) fn handle_access(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
    ) -> Result<Option<ExprKind>, Error> {
        let key = (symbol.owner, symbol.name, symbol.ty);
        if key == OFFSET_OF || key == OFFSET_OF_FIELD {
            return unsupported(node, "a field offset used as a value");
        }
        let update = crate::program::unsafe_field_update(symbol.owner, symbol.name, symbol.ty);
        let put = crate::program::unsafe_field_access(symbol.owner, symbol.name, symbol.ty);
        if update.is_none() && put.is_none() {
            return Ok(None);
        }
        let args: Vec<Node> = node.list("arguments").collect();
        let offset = match (self.handle(args[1]), self.static_handle(args[1], false)) {
            (Some((class, field)), _) => Offset::Field(class, field),
            (_, Some((class, field))) => Offset::Static(class, field),
            _ => match (self.element_offset(args[1], &[]), self.computed_offset(args[1], 0)) {
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
        self.access_at(node, place, ty, class, &args[2..], update, put).map(Some)
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
    /// `update` with `values` as its operands.
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
    ) -> Result<ExprKind, Error> {
        let ret = self.ty(node, node.java_type())?;
        if let Some(update) = update {
            let operands =
                values.iter().map(|a| Ok(self.stored(self.expr(*a)?, ty))).collect::<Result<_, Error>>()?;
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
        if put == Some(true) {
            let value = self.stored(self.expr(values[0])?, ty);
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
        let name = self.update_method_name(class, field, ty, update, object.is_none());
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
    /// class `class`, of type `ty`, which `field_update_methods` adds.
    fn update_method_name(
        &self,
        class: usize,
        field: &str,
        ty: Type,
        update: FieldUpdate,
        is_static: bool,
    ) -> String {
        let mut updates = self.field_updates.borrow_mut();
        if let Some(u) = updates.iter().find(|u| u.class == class && u.field == field && u.update == update) {
            return u.name.clone();
        }
        let verb = match update {
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
        methods.extend(updates.iter().map(|u| self.update_method(u)));
    }

    /// The method that makes the update `u`: the field read, compared and written in its body.
    fn update_method(&self, u: &FieldUpdateMethod) -> Method {
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
        let is = |a: Expr, b: Expr| {
            self.synth(ExprKind::Binary(BinOp::Eq, Box::new(a), Box::new(b)), pos, Type::Boolean)
        };
        let boolean = |b: bool| self.synth(ExprKind::Bool(b), pos, Type::Boolean);
        let (params, result, body) = match u.update {
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
        }
    }
}
