//! Field offsets: the JDK reaches some of its fields through `Unsafe` with the field's offset,
//! as `LazyConstantImpl` does (see docs/multi-threading.md, 5.3):
//!
//! ```java
//! private static final long CONSTANT_OFFSET = UNSAFE.objectFieldOffset(LazyConstantImpl.class, "constant");
//! ... UNSAFE.getReferenceAcquire(this, CONSTANT_OFFSET) ...
//! ```
//!
//! A static field whose initializer is such an `objectFieldOffset` is a *field handle*: it names
//! the field, and it is no field of the translation. A read or write through the handle is a read
//! or write of the field itself, so the call above lowers to `this.constant`. That is Java's
//! meaning in a program with one thread, and in a program with threads the field is the object's,
//! read and written under its lock, which orders the accesses at least as the plain, opaque,
//! acquire, release and volatile forms ask. Any other use of a handle is rejected.
//!
//! An atomic update through a handle, `U.compareAndSetInt(this, VALUE, expect, update)` and the
//! get-and-add, get-and-set and compare-and-exchange forms of `int`, `long` and reference fields
//! (`program::FieldUpdate`), is a call of a method that lowering adds to the field's class, one
//! per field and form: `compareAndSetValue(expected, value)` reads the field, compares and
//! writes in its body. A method runs with its object borrowed, or in a program with threads with
//! the object's lock held, for the whole call, so the update is atomic as Java's is: no other
//! access of the object comes between the read and the write. The weak forms never fail
//! spuriously, which Java allows.

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

impl<'t> Lowering<'t> {
    /// The class and the name of the field that the static field `member` is the handle of.
    pub(super) fn handle_declaration(&self, member: Node<'t>) -> Option<(usize, &'t str)> {
        if member.kind() != "VARIABLE" || !member.has_flag("STATIC") || !member.has_flag("FINAL") {
            return None;
        }
        let init = member.get("initializer").filter(|i| i.kind() == "METHOD_INVOCATION")?;
        let symbol = init.symbol()?;
        if (symbol.owner, symbol.name, symbol.ty) != OFFSET_OF {
            return None;
        }
        let mut args = init.list("arguments");
        let class = args.next().filter(|a| a.kind() == "MEMBER_SELECT" && a.str("identifier") == "class")?;
        let field = args.next().filter(|a| a.kind() == "STRING_LITERAL")?;
        let class = self.type_index(class.get("expression")?.java_type())?;
        Some((class, field.0["value"].as_str()?))
    }

    /// The field that the expression `offset` names, when it reads a field handle.
    fn handle(&self, offset: Node<'t>) -> Option<(usize, &'t str)> {
        let symbol = offset.symbol().filter(|s| s.kind == "FIELD" && s.is_static)?;
        let owner = self.type_index(symbol.owner)?;
        let member = self.types[owner]
            .node
            .list("members")
            .find(|m| m.kind() == "VARIABLE" && m.str("name") == symbol.name)?;
        self.handle_declaration(member)
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
    /// the read `o.f`, or the assignment `o.f = x`. `None` for any other call.
    pub(super) fn handle_access(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
    ) -> Result<Option<ExprKind>, Error> {
        if let Some(update) = crate::program::unsafe_field_update(symbol.owner, symbol.name, symbol.ty) {
            return self.field_update(node, symbol, update).map(Some);
        }
        let Some(put) = crate::program::unsafe_field_access(symbol.owner, symbol.name, symbol.ty) else {
            return Ok(None);
        };
        let args: Vec<Node> = node.list("arguments").collect();
        let (object, field, ty) = self.handle_field(node, symbol, &args)?;
        let place = Place::Field { object: Box::new(object), name: field.to_string() };
        if put {
            let value = self.stored(self.expr(args[2])?, ty);
            return Ok(Some(ExprKind::Assign { op: None, target: place, value: Box::new(value) }));
        }
        let read = self.synth(objects::read(place), node.pos(), ty);
        let ret = self.ty(node, node.java_type())?;
        Ok(Some(self.convert(read, ret).kind))
    }

    /// `U.compareAndSet<Kind>(o, OFFSET, expected, x)` or another atomic update of a field handle:
    /// a call of the method of the field's class that makes it (see the module's comment).
    fn field_update(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
        update: FieldUpdate,
    ) -> Result<ExprKind, Error> {
        let args: Vec<Node> = node.list("arguments").collect();
        let (object, field, ty) = self.handle_field(node, symbol, &args)?;
        let Some((class, _)) = self.handle(args[1]) else { unreachable!("`handle_field` found the handle") };
        let operands =
            args[2..].iter().map(|a| Ok(self.stored(self.expr(*a)?, ty))).collect::<Result<_, Error>>()?;
        let name = self.update_method_name(class, field, ty, update);
        let result = if update == FieldUpdate::CompareAndSet { Type::Boolean } else { ty };
        let call = ExprKind::MethodCall {
            receiver: Box::new(object),
            name,
            args: operands,
            resolved: None,
            is_super: false,
        };
        let call = self.synth(call, node.pos(), result);
        let ret = self.ty(node, node.java_type())?;
        Ok(self.convert(call, ret).kind)
    }

    /// The object and the name of the field that a call of `Unsafe` with the arguments `args`
    /// (an object and a field handle first) reaches, and the field's type.
    fn handle_field(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
        args: &[Node<'t>],
    ) -> Result<(Expr, &'t str, Type), Error> {
        let Some((class, field)) = self.handle(args[1]) else {
            return unsupported(
                node,
                &format!("`Unsafe.{}` with an offset that is not a field's", symbol.name),
            );
        };
        let object = self.expr(args[0])?;
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
        Ok((object, field, ty))
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
