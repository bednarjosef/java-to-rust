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

use super::*;

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
        let cas = crate::program::unsafe_compare_and_set(symbol.owner, symbol.name, symbol.ty);
        let access = crate::program::unsafe_field_access(symbol.owner, symbol.name, symbol.ty);
        let put = match access {
            Some(put) => put,
            None if cas => false,
            None => return Ok(None),
        };
        let args: Vec<Node> = node.list("arguments").collect();
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
        if cas {
            return self.compare_and_set(node, object, field, ty, args[2], args[3]).map(Some);
        }
        let place = Place::Field { object: Box::new(object), name: field.to_string() };
        if put {
            let value = self.expr(args[2])?;
            let value = if value.ty == ty || self.param_owner(ty).is_some() {
                value
            } else {
                let pos = value.pos;
                self.synth(ExprKind::Cast(ty, Box::new(value)), pos, ty)
            };
            return Ok(Some(ExprKind::Assign { op: None, target: place, value: Box::new(value) }));
        }
        let read = self.synth(objects::read(place), node.pos(), ty);
        let ret = self.ty(node, node.java_type())?;
        Ok(Some(self.convert(read, ret).kind))
    }

    /// `U.compareAndSet<Kind>(o, OFFSET, expected, x)` of a field handle: `{ boolean same = o.f ==
    /// expected; if (same) o.f = x; yield same; }`, comparing references by identity as the JVM
    /// does. The object is evaluated twice, so it must be `this` or a local. The comparison and
    /// the write are two steps, not one atomic one: the JDK's code uses it where one thread
    /// closes a stream (`BufferedInputStream`'s buffer), and a close that races a read in
    /// another thread is the documented deviation (docs/natives-and-input.md).
    fn compare_and_set(
        &self,
        node: Node<'t>,
        object: Expr,
        field: &str,
        ty: Type,
        expected: Node<'t>,
        value: Node<'t>,
    ) -> Result<ExprKind, Error> {
        if !matches!(object.kind, ExprKind::This | ExprKind::Var(_)) {
            return unsupported(node, "`Unsafe.compareAndSet` on an object that is not `this` or a local");
        }
        let pos = node.pos();
        let copy = |e: &Expr| {
            self.synth(
                match &e.kind {
                    ExprKind::Var(name) => ExprKind::Var(name.clone()),
                    _ => ExprKind::This,
                },
                pos,
                e.ty,
            )
        };
        let place = |o: Expr| Place::Field { object: Box::new(o), name: field.to_string() };
        let current = self.synth(objects::read(place(copy(&object))), pos, ty);
        let expected = self.convert(self.expr(expected)?, ty);
        let value = self.convert(self.expr(value)?, ty);
        let same = self.fresh_name("same");
        let var = || self.synth(ExprKind::Var(same.clone()), pos, Type::Boolean);
        let test = self.synth(
            ExprKind::Binary(BinOp::Eq, Box::new(current), Box::new(expected)),
            pos,
            Type::Boolean,
        );
        let set = ExprKind::Assign { op: None, target: place(object), value: Box::new(value) };
        let block = vec![
            Stmt::Local { ty: Type::Boolean, name: same.clone(), init: Some(test), pos },
            Stmt::If { cond: var(), then: Box::new(Stmt::Expr(self.synth(set, pos, ty))), otherwise: None },
            Stmt::Yield(var()),
        ];
        Ok(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body: block }],
        })
    }
}
