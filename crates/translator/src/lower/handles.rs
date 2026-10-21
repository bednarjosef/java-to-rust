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
//! get-and-add, get-and-set and compare-and-exchange forms of `int`, `long` and reference fields
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
//! `int` widened to `long`, or a `long`. The offset names element `index` of the array the access
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
//! access of the array comes between the read and the write. Any other offset arithmetic, and an
//! element offset that is computed into a variable first, is rejected.

use super::*;
use crate::program::FieldUpdate;
use std::collections::HashMap;

/// A method that lowering adds for an atomic update of a field through `Unsafe`.
pub(super) struct FieldUpdateMethod {
    class: usize,
    field: String,
    ty: Type,
    update: FieldUpdate,
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

/// `Unsafe.arrayBaseOffset(Class)` and `arrayIndexScale(Class)`.
const BASE_OF: (&str, &str, &str) =
    ("jdk.internal.misc.Unsafe", "arrayBaseOffset", "(java.lang.Class<?>)long");
const SCALE_OF: (&str, &str, &str) =
    ("jdk.internal.misc.Unsafe", "arrayIndexScale", "(java.lang.Class<?>)int");

/// `Integer.numberOfLeadingZeros(int)`, which turns a power of two into its exponent.
const LEADING_ZEROS: (&str, &str, &str) = ("java.lang.Integer", "numberOfLeadingZeros", "(int)int");

/// The primitive types, as javac names them and as the kinds of arrays and of `Unsafe`'s
/// accesses; every other type is a reference.
const PRIMITIVES: [&str; 8] = ["boolean", "byte", "short", "char", "int", "long", "float", "double"];

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

/// The kind of the elements of the array class that the class literal `node` names.
fn literal_kind(node: Node) -> Option<Kind> {
    let node = plain(node);
    if node.kind() != "MEMBER_SELECT" || node.str("identifier") != "class" {
        return None;
    }
    let element = node.get("expression")?.java_type().strip_suffix("[]")?;
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

/// What an offset argument of `Unsafe` names.
enum Offset<'t> {
    /// The field of a class, by the class's index.
    Field(usize, &'t str),
    /// The element of the array at this index (an `int`, or a `long`), of an array of this kind.
    Element(Node<'t>, Kind),
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
        self.static_field(&symbol)
            .is_some_and(|(owner, member)| self.handle_declaration(owner, member).is_some())
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
        let (class, field) = if let Some(args) = call_of(value, OFFSET_OF) {
            (*args.first()?, *args.get(1)?)
        } else {
            let get = plain(*call_of(value, OFFSET_OF_FIELD)?.first()?);
            let symbol = get.symbol().filter(|_| get.kind() == "METHOD_INVOCATION")?;
            if (symbol.owner, symbol.name) != ("java.lang.Class", "getDeclaredField") {
                return None;
            }
            (get.get("methodSelect")?.get("expression")?, get.list("arguments").next()?)
        };
        let field = plain(field);
        let name = field.0["value"].as_str().filter(|_| field.kind() == "STRING_LITERAL")?;
        Some((self.offset_class(class, block)?, name))
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
        let init = block
            .list("statements")
            .find(|s| s.kind() == "VARIABLE" && s.str("name") == name)?
            .get("initializer")?;
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
    fn element_offset(&self, offset: Node<'t>) -> Option<(Node<'t>, Kind)> {
        let offset = plain(offset);
        if offset.kind() != "PLUS" {
            return None;
        }
        let (l, r) = (offset.get("leftOperand")?, offset.get("rightOperand")?);
        let (base, scaled) = match self.array_base(l, None) {
            Some(kind) => (kind, r),
            None => (self.array_base(r, None)?, l),
        };
        let scaled = plain(scaled);
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
            "TYPE_CAST" if index.java_type() == "long" => index
                .get("expression")
                .filter(|e| matches!(e.java_type(), "int" | "short" | "char" | "byte"))?,
            _ if index.java_type() == "long" => index,
            _ => return None,
        };
        (kind == base).then_some((index, kind))
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

    /// Rejects a read of a field handle other than as the offset of an access (`node` names a
    /// static field).
    pub(super) fn check_handle_use(&self, node: Node<'t>) -> Result<(), Error> {
        match self.handle(node) {
            Some(_) => unsupported(node, "a field offset used as a value"),
            None => Ok(()),
        }
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
        let offset = match self.handle(args[1]) {
            Some((class, field)) => Offset::Field(class, field),
            None => match self.element_offset(args[1]) {
                Some((index, kind)) => Offset::Element(index, kind),
                None => {
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
            Offset::Element(index, kind) => {
                let (array, index, ty) = self.handle_element(node, symbol, args[0], index, kind)?;
                (Place::Index(Box::new(array), Box::new(index)), ty, usize::MAX)
            }
        };
        let ret = self.ty(node, node.java_type())?;
        if let Some(update) = update {
            let operands = args[2..]
                .iter()
                .map(|a| Ok(self.stored(self.expr(*a)?, ty)))
                .collect::<Result<_, Error>>()?;
            let call = match place {
                Place::Field { object, name } => {
                    self.field_update(*object, class, &name, ty, update, operands)
                }
                place => self.element_update(node, place, ty, update, operands)?,
            };
            let result = if update == FieldUpdate::CompareAndSet { Type::Boolean } else { ty };
            let call = self.synth(call, node.pos(), result);
            return Ok(Some(self.convert(call, ret).kind));
        }
        if put == Some(true) {
            let value = self.stored(self.expr(args[2])?, ty);
            return Ok(Some(ExprKind::Assign { op: None, target: place, value: Box::new(value) }));
        }
        let read = match place {
            Place::Index(array, index) => ExprKind::Index(array, index),
            place => objects::read(place),
        };
        let read = self.synth(read, node.pos(), ty);
        Ok(Some(self.convert(read, ret).kind))
    }

    /// `U.compareAndSet<Kind>(o, OFFSET, expected, x)` or another atomic update of field `field`
    /// of class `class`, of type `ty`, on `object`: a call of the method of the field's class that
    /// makes it (see the module's comment).
    fn field_update(
        &self,
        object: Expr,
        class: usize,
        field: &str,
        ty: Type,
        update: FieldUpdate,
        operands: Vec<Expr>,
    ) -> ExprKind {
        let name = self.update_method_name(class, field, ty, update);
        ExprKind::MethodCall {
            receiver: Box::new(object),
            name,
            args: operands,
            resolved: None,
            is_super: false,
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
        // numbers. An `Object` may hold a string, which compares by value.
        if matches!(ty, Type::String | Type::StringBuilder | Type::Boxed(_) | Type::JavaClass)
            || Self::is_object(ty)
            || self.param_owner(ty).is_some()
        {
            return unsupported(node, &format!("an atomic update of an element of a `{ty}[]`"));
        }
        let value = Box::new(operands.pop().expect("an update has an operand"));
        let op = (update == FieldUpdate::GetAndAdd).then_some(BinOp::Add);
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

    /// The array (`array`, the first argument) and index of an access of `Unsafe` at an element
    /// offset of an array of kind `kind` with index `index`, and the elements' type, which must be
    /// of that kind, and of the access's.
    fn handle_element(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
        array: Node<'t>,
        index: Node<'t>,
        kind: Kind,
    ) -> Result<(Expr, Expr, Type), Error> {
        let array = self.expr(array)?;
        let Type::Array(element) = array.ty else {
            let what = "at an element offset of a value that is not an array";
            return unsupported(node, &format!("`Unsafe.{}` {what}", symbol.name));
        };
        let element = *element;
        if type_kind(element) != kind || access_kind(symbol.ty) != kind {
            let what = "at an element offset of an array of another type";
            return unsupported(node, &format!("`Unsafe.{}` {what}", symbol.name));
        }
        let index = self.expr(index)?;
        let index = match index.ty {
            Type::Long => {
                let pos = index.pos;
                self.synth(ExprKind::Cast(Type::Int, Box::new(index)), pos, Type::Int)
            }
            _ => self.convert(index, Type::Int),
        };
        Ok((array, index, element))
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
    fn update_method_name(&self, class: usize, field: &str, ty: Type, update: FieldUpdate) -> String {
        let mut updates = self.field_updates.borrow_mut();
        if let Some(u) = updates.iter().find(|u| u.class == class && u.field == field && u.update == update) {
            return u.name.clone();
        }
        let verb = match update {
            FieldUpdate::GetAndAdd => "getAndAdd",
            FieldUpdate::GetAndSet => "getAndSet",
            FieldUpdate::CompareAndSet => "compareAndSet",
            FieldUpdate::CompareAndExchange => "compareAndExchange",
        };
        let mut chars = field.chars();
        let capitalized: String =
            chars.next().map(|c| c.to_ascii_uppercase()).into_iter().chain(chars).collect();
        let base = format!("{verb}{capitalized}");
        let name = (1..)
            .map(|n| if n == 1 { base.clone() } else { format!("{base}{n}") })
            .find(|n| !updates.iter().any(|u| &u.name == n))
            .expect("a free name");
        updates.push(FieldUpdateMethod { class, field: field.to_string(), ty, update, name: name.clone() });
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
                    if let ExprKind::MethodCall { name, resolved: resolved @ None, .. } = &mut e.kind
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
            FieldUpdate::GetAndAdd => {
                let sum = ExprKind::Binary(BinOp::Add, Box::new(var("old")), Box::new(var("delta")));
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
            is_static: false,
            is_ctor: false,
            class: u.class,
            pos,
            is_abstract: false,
            is_synchronized: false,
            overrides: Vec::new(),
            type_params: Vec::new(),
        }
    }
}
