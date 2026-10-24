//! Lowering of the type hierarchy: superclasses, abstract classes and methods, overrides,
//! `super(...)` and `super.m(...)`, and the implicit conversions between class types, which are
//! made explicit as `Cast` nodes so that the analyses and the generator see every upcast.

use super::*;

impl<'t> Lowering<'t> {
    /// The superclass of class `t` when the program declares it; `None` for `Object`, and for
    /// `Number`, which is the object model's (see `lower::object`). A class that extends another
    /// class of the runtime is rejected (in a census, it extends nothing, see `reject_class`).
    pub(super) fn superclass(&self, t: usize) -> Result<Option<usize>, Error> {
        let node = self.types[t].node;
        let Some(clause) = node.get("extendsClause") else { return Ok(None) };
        // A generic superclass is its erasure.
        let name = super::captures::raw(clause.java_type());
        let name = name.as_str();
        match self.type_index(name) {
            Some(s) if self.types[s].node.kind() == "CLASS" => Ok(Some(s)),
            None if name == "java.lang.Object" || name == "java.lang.Number" => Ok(None),
            // A census's stub of the class extends nothing, nor does a class it rejected.
            _ if self.stubbing.get() => Ok(None),
            _ => self.reject_class(t, unsupported(clause, &format!("extending `{name}`"))).map(|()| None),
        }
    }

    /// The program's interfaces that type `t` implements (a class or an enum) or extends (an
    /// interface), directly. `java.lang.Comparable` is accepted and left out: a class that
    /// implements it only declares its `compareTo`, which the program calls directly; so are
    /// `java.lang.AutoCloseable` and `java.io.Closeable`, whose `close` a try-with-resources
    /// calls directly.
    pub(super) fn interfaces(&self, t: usize) -> Result<Vec<usize>, Error> {
        let node = self.types[t].node;
        // javac lists the interfaces that an interface extends as its `implementsClause` too.
        let mut out = Vec::new();
        for clause in node.list("implementsClause") {
            let raw = &super::captures::raw(clause.java_type());
            if functional::is_marker(raw) && self.type_index(raw).is_none() {
                continue;
            }
            match self.type_index(raw) {
                Some(i) if self.types[i].node.kind() == "INTERFACE" => out.push(i),
                // `CharSequence` is the object model's (see `object_types`).
                None if matches!(
                    raw.as_str(),
                    "java.lang.Comparable"
                        | "java.lang.AutoCloseable"
                        | "java.io.Closeable"
                        | "java.lang.CharSequence"
                ) => {}
                _ if self.stubbing.get() => {}
                _ => self.reject_class(t, unsupported(clause, &format!("implementing `{raw}`")))?,
            }
        }
        Ok(out)
    }

    /// `value instanceof T` and `value instanceof T name` on a value of the program's types.
    pub(super) fn instance_of(&self, node: Node<'t>) -> Result<ExprKind, Error> {
        // A record pattern has no type of its own.
        if let Some(p) = node.get("pattern").filter(|p| p.kind() == "DECONSTRUCTION_PATTERN") {
            return self.record_instance_of(node, p);
        }
        let value = self.expr(node.child("expression")?)?;
        let type_node = node.child("type")?;
        let ty = self.ty(type_node, type_node.java_type())?;
        let binding = match node.get("pattern") {
            Some(p) if p.kind() == "BINDING_PATTERN" => Some(p.child("variable")?.str("name").to_string()),
            Some(p) => return unsupported(p, "this pattern"),
            None => None,
        };
        if let Some(name) = &binding {
            self.forget_alias(name);
        }
        // `Object` may also hold a string, a box, an array, a builder or a class, `CharSequence`
        // a string or a builder, `Appendable` a builder, and `Number` a numeric box.
        let char_sequence = matches!(value.ty, Type::Class(name, _) if name == CHAR_SEQUENCE);
        let target = matches!(ty, Type::Class(..) | Type::Enum(_))
            || (Self::is_object(value.ty)
                && matches!(
                    ty,
                    Type::String | Type::StringBuilder | Type::Boxed(_) | Type::Array(_) | Type::JavaClass
                ))
            || (char_sequence && matches!(ty, Type::String | Type::StringBuilder))
            || (self.is_appendable(value.ty) && ty == Type::StringBuilder)
            || Self::is_numeric_box_of(value.ty, ty);
        if !matches!(value.ty, Type::Class(..) | Type::Enum(_)) || !target {
            return unsupported(node, "instanceof on these types");
        }
        self.object_array(node, ty)?;
        Ok(ExprKind::InstanceOf { value: Box::new(value), ty, binding })
    }

    /// Whether `ty` is a box of a number, which a value of type `value`, `Number`, may be.
    pub(super) fn is_numeric_box_of(value: Type, ty: Type) -> bool {
        matches!(value, Type::Class(name, _) if name == NUMBER)
            && matches!(ty, Type::Boxed(b) if !matches!(b, Type::Boolean | Type::Char))
    }

    /// Whether type `a` is type `b` or a subtype of it, through superclasses and interfaces.
    pub(super) fn is_subclass(&self, a: usize, b: usize) -> bool {
        a == b
            || self.superclass(a).ok().flatten().is_some_and(|s| self.is_subclass(s, b))
            || self.interfaces(a).unwrap_or_default().into_iter().any(|i| self.is_subclass(i, b))
    }

    /// The methods of the program that a method declaration overrides, from the list javac
    /// gives with its symbol.
    pub(super) fn overrides(&self, node: Node<'t>) -> Vec<usize> {
        let Some(list) = node.get("symbol").and_then(|s| s.0["overrides"].as_array()) else {
            return Vec::new();
        };
        let name = node.str("name");
        list.iter()
            .filter_map(|o| {
                let owner = o["owner"].as_str()?;
                let sig = o["type"].as_str()?;
                self.methods
                    .iter()
                    .position(|m| self.types[m.owner].javac == owner && m.name == name && m.sig == sig)
            })
            .collect()
    }

    /// `super(args)` at the start of a constructor: nothing when the superclass is `Object`,
    /// whose constructor does nothing, else `SuperInit`.
    pub(super) fn super_init(&self, stmt: Node<'t>) -> Result<Option<Stmt>, Error> {
        let call = stmt.child("expression")?;
        let Some(superclass) = self.superclass(self.current.get())? else { return Ok(None) };
        let Some(symbol) = call.symbol() else { return unsupported(call, "this `super` call") };
        self.check_standard_member(call, &symbol)?;
        // `outer.super(..)` gives the enclosing instance of the superclass, which is otherwise
        // found from the current class.
        let outer = match call.child("methodSelect")?.get("expression") {
            Some(qualifier) => {
                let Some(nested) = self.nesting(superclass) else {
                    return unsupported(
                        qualifier,
                        "a qualified `super(..)` of a class without an enclosing instance",
                    );
                };
                let value = self.expr(qualifier)?;
                // A superclass whose code does not use its enclosing instance does not hold it.
                if !nested.outer {
                    if has_effects(&value) {
                        return unsupported(qualifier, "an enclosing instance with effects that is not used");
                    }
                    None
                } else {
                    Some(self.convert(value, self.self_type(nested.enclosing)))
                }
            }
            None => None,
        };
        let (first, last) = self.passed_on_from(superclass, call.pos(), outer)?;
        let args: Vec<Expr> = call.list("arguments").map(|a| self.expr(a)).collect::<Result<_, _>>()?;
        let args = self.packed_args(call, &symbol, args)?;
        let resolved = self.method_index(&symbol);
        let args = self.convert_args(resolved, args)?;
        let args = self.with_synthetic((first, last), args);
        let kind = ExprKind::SuperInit { args, resolved };
        Ok(Some(Stmt::Expr(self.synth(kind, call.pos(), Type::Void))))
    }

    /// The enclosing class that `Outer` names in `Outer.super` (`node`), when it is a class other
    /// than the current one.
    pub(super) fn outer_super(&self, node: Node<'t>) -> Option<usize> {
        if node.str("identifier") != "super" {
            return None;
        }
        let class =
            node.get("expression")?.symbol().and_then(|s| self.type_index(&super::captures::raw(s.ty)))?;
        (class != self.current.get() && self.types[class].node.kind() != "INTERFACE").then_some(class)
    }

    /// The receiver of a `super` method call (`receiver`): `this`, or for `Outer.super` the
    /// enclosing instance of `Outer`, whose superclass's method the call runs.
    pub(super) fn super_receiver(&self, receiver: Node<'t>) -> Result<Expr, Error> {
        if let Some(outer) = self.outer_super(receiver) {
            return self.enclosing_instance(outer, receiver.pos());
        }
        // In a lambda, `super` is that of the class whose code holds it.
        if let Some(holder) = self.lambda_this(receiver) {
            return holder;
        }
        let ty = self.ty(receiver, receiver.java_type())?;
        Ok(self.synth(ExprKind::This, receiver.pos(), ty))
    }

    /// The types of the parameters of method `index`.
    pub(super) fn param_types(&self, index: usize) -> Result<Vec<Type>, Error> {
        let owner = self.methods[index].owner;
        self.methods[index]
            .node
            .list("parameters")
            .map(|p| self.declared_type(p, p.symbol().map_or("", |s| s.ty), owner, Some(index)))
            .collect()
    }

    /// The arguments of a call of method `index`, each converted to its parameter's type.
    pub(super) fn convert_args(&self, index: Option<usize>, args: Vec<Expr>) -> Result<Vec<Expr>, Error> {
        let Some(index) = index else { return Ok(args) };
        let types = self.param_types(index)?;
        for (a, &ty) in args.iter().zip(&types) {
            self.fail_on_covariant(a.ty, ty);
            // A charset is the runtime's value, which is not an object of the translation: it
            // cannot be passed as an `Object` (`Objects.requireNonNull(cs)` erased).
            if a.ty == Type::Charset && matches!(ty, Type::Class(..)) && self.param_owner(ty).is_none() {
                return error(a.pos, "a charset passed as an object not supported yet".to_string());
            }
        }
        // The array of a Rust generic's type variable passed for an erased array (`asList`'s `T[]`
        // to `Arrays.ArrayList`'s `E[]`, an `Object[]`) erases the generic: it is lowered again.
        for (a, &ty) in args.iter().zip(&types) {
            if matches!((a.ty, ty), (Type::Array(_), Type::Array(_)))
                && self.has_param(a.ty)
                && !self.has_param(ty)
            {
                let mut owners = Vec::new();
                self.owners_in(a.ty, &mut owners);
                owners.into_iter().for_each(|o| self.fail(o));
                return Ok(args);
            }
        }
        // Arrays do not convert: a `Version[]` is an `Object[]` in the translation only when it is
        // covariant (see `covariant`).
        if let Some(arg) = args.iter().zip(&types).find_map(|(a, &ty)| {
            let mismatch = matches!(a.ty, Type::Array(_)) && matches!(ty, Type::Array(_)) && a.ty != ty;
            (mismatch && !self.has_param(ty) && !self.meets_object_array(a.ty, ty)).then_some(a)
        }) {
            return error(
                arg.pos,
                "an array passed as an array of a type variable not supported yet".to_string(),
            );
        }
        Ok(args
            .into_iter()
            .zip(types.into_iter().chain(std::iter::repeat(Type::Void)))
            .map(|(a, ty)| self.convert(a, ty))
            .collect())
    }

    /// `e` as a value of type `to`: an object of one class type that Java uses as another (an
    /// upcast) gets a `Cast`, which the generator turns into the conversion.
    pub(super) fn convert(&self, e: Expr, to: Type) -> Expr {
        // An enum that implements an interface is converted to it too, and any value but null
        // to `Object`.
        // A Rust generic's type variable takes any value as it is.
        if self.param_owner(to).is_some() {
            return e;
        }
        // Erasure's checked cast of a generic member's value is javac's only where the value's
        // declared type is not one of `to`'s (`TransTypes.retype`): javac's `JCTree result =
        // translate(assignOp, type)`, of `<T extends JCExpression> T translate(T, Type)`, casts
        // nothing, and the block that the call gives back for its `JCAssignOp` is no failure.
        if let ExprKind::Cast(_, inner) = &e.kind
            && self.erased_casts.borrow().contains(&inner.id)
            && self.upcast_to(inner.ty, to)
        {
            let ExprKind::Cast(_, inner) = e.kind else { unreachable!() };
            return self.convert(*inner, to);
        }
        if e.ty != to {
            self.meets_object_array(e.ty, to);
        }
        let differs = match (e.ty, to) {
            (_, Type::Class(b, _)) if b == OBJECT || b == COMPARABLE || b == NUMBER || b == CHAR_SEQUENCE => {
                !matches!(e.ty, Type::Null | Type::Void) && e.ty != to
            }
            (Type::Class(a, _) | Type::Enum(a), Type::Class(b, _)) => a != b,
            // A string, a builder, a box or a class as a value of one of the program's types: a
            // builder is an `Appendable`, and the generator rejects the others, which no type's
            // values include (see `codegen::dispatch::class_cast`).
            (Type::String | Type::StringBuilder | Type::Boxed(_) | Type::JavaClass, Type::Class(..)) => true,
            // A primitive boxed into an interface that its box implements (`Serializable`,
            // `Constable`): the value is the box, as it is for `Object` and `Comparable`.
            (
                Type::Int
                | Type::Long
                | Type::Byte
                | Type::Short
                | Type::Char
                | Type::Double
                | Type::Float
                | Type::Boolean,
                Type::Class(..),
            ) => true,
            _ => false,
        };
        if !differs {
            return e;
        }
        // A builder that becomes an `Object`, a `CharSequence` or an `Appendable` is a value of
        // its runtime class.
        if Self::is_object(to)
            || e.ty == Type::StringBuilder && (to == Type::class(CHAR_SEQUENCE) || self.is_appendable(to))
        {
            self.note_object_array(e.ty);
        }
        let pos = e.pos;
        self.synth(ExprKind::Cast(to, Box::new(e)), pos, to)
    }

    /// Whether a value of type `from` is one of type `to` without a checked cast: an object of
    /// one of the program's classes as `Object` or as one of its supertypes.
    fn upcast_to(&self, from: Type, to: Type) -> bool {
        match (from, to) {
            (Type::Class(..) | Type::Enum(_), _) if Self::is_object(to) => true,
            (Type::Class(a, _) | Type::Enum(a), Type::Class(b, _)) => {
                a == b
                    || self.type_index(a).zip(self.type_index(b)).is_some_and(|(a, b)| self.is_subclass(a, b))
            }
            _ => false,
        }
    }

    /// Whether `ty` is the JDK's `Appendable`, which the runtime's builders implement.
    pub(super) fn is_appendable(&self, ty: Type) -> bool {
        let appendable = self.object.and_then(|at| at.appendable);
        matches!(ty, Type::Class(name, _) if appendable.is_some_and(|a| self.types[a].name == name))
    }

    /// The operands of `==` or `!=` on objects of two class types, the one of the subclass
    /// converted to the other's type.
    pub(super) fn convert_operands(&self, left: Expr, right: Expr) -> (Expr, Expr) {
        if Self::is_object(left.ty) && right.ty != Type::Null {
            return (left, self.convert(right, Type::class(OBJECT)));
        }
        if Self::is_object(right.ty) && left.ty != Type::Null {
            return (self.convert(left, Type::class(OBJECT)), right);
        }
        // A builder compared with a `CharSequence` or an `Appendable` is one, as it is to an
        // `Object`.
        let holds_builder = |ty: Type| ty == Type::class(CHAR_SEQUENCE) || self.is_appendable(ty);
        if holds_builder(left.ty) && right.ty == Type::StringBuilder {
            let ty = left.ty;
            return (left, self.convert(right, ty));
        }
        if holds_builder(right.ty) && left.ty == Type::StringBuilder {
            let ty = right.ty;
            return (self.convert(left, ty), right);
        }
        // An object of a class that extends `Number` compared with a `Number` is one.
        let number = Type::class(NUMBER);
        let extends_number =
            |e: &Expr| self.class_of(e.ty).is_some_and(|c| self.extends_number(c).unwrap_or(false));
        if left.ty == number && extends_number(&right) {
            return (left, self.convert(right, number));
        }
        if right.ty == number && extends_number(&left) {
            return (self.convert(left, number), right);
        }
        let (Some(a), Some(b)) = (self.class_of(left.ty), self.class_of(right.ty)) else {
            return (left, right);
        };
        if a == b {
            (left, right)
        } else if self.is_subclass(a, b) {
            (self.convert(left, right.ty), right)
        } else if self.is_subclass(b, a) {
            let ty = left.ty;
            (left, self.convert(right, ty))
        } else if self.object.is_some() {
            // Neither is a subtype of the other, as with an interface and a class that does not
            // implement it (a subclass may): both are compared as `Object`s, which such a
            // comparison makes the program use (see `plan_object`).
            let object = Type::class(OBJECT);
            (self.convert(left, object), self.convert(right, object))
        } else {
            (left, right)
        }
    }

    fn class_of(&self, ty: Type) -> Option<usize> {
        match ty {
            Type::Class(name, _) | Type::Enum(name) => self.type_index(name),
            _ => None,
        }
    }
}
