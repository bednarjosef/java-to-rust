//! `java.lang.Class`, as far as a program uses it without reflection. Comparing the classes of
//! two objects, or the class of an object with a class literal of the program (`a.getClass() ==
//! b.getClass()`, `o.getClass().equals(Point.class)`), compares their binary names
//! (`Builtin::SameClass` of `Builtin::ClassName` and `Builtin::ClassLiteralName`), since no two
//! classes of a program have the same one, and the names of such a literal are constants. Any
//! other class is a value, a `java::Class` (`Type::JavaClass`): `x.getClass()`
//! (`Builtin::GetClass`), a class literal (`Builtin::ClassLiteral`), and the methods of `Class`
//! that need no reflection (`Builtin::ClassMethod`). `isInstance` and `cast` of any class value
//! test the binary names of the classes and interfaces of the object's class, which the generator
//! writes out per class (`Builtin::ClassIsInstance`, `Builtin::ClassCheck`); `isAssignableFrom`
//! tests those that each class value carries (`ClassOp::IsAssignableFrom`).

use super::*;

/// `Class.getEnumConstants`, which lowering writes as a block that checks the class
/// (`class_enum_constants`).
pub(crate) const ENUM_CONSTANTS: &str = "getEnumConstants";

impl<'t> Lowering<'t> {
    /// Whether `node`, an `==` or `!=`, compares the classes of objects of the program or class
    /// literals of its classes, by name (see the module's comment).
    pub(super) fn compares_classes(&self, node: Node<'t>) -> bool {
        ["leftOperand", "rightOperand"]
            .into_iter()
            .all(|side| node.get(side).is_some_and(|n| self.named_class(n)))
    }

    /// `a == b` or `a != b` of two classes (see `compares_classes`).
    pub(super) fn class_comparison(&self, node: Node<'t>) -> Result<ExprKind, Error> {
        let (left, right) = (node.child("leftOperand")?, node.child("rightOperand")?);
        self.same_class(left, right, node.kind() == "EQUAL_TO")
    }

    /// `o.getClass()` as a value, and the calls of `Class`'s methods: `a.equals(b)` of two
    /// classes named by name, `Point.class.getName()`, and the rest as `ClassMethod`; `None` for
    /// any other call. It runs before the arguments are lowered.
    pub(super) fn class_call(
        &self,
        node: Node<'t>,
        symbol: &Symbol,
        select: Node<'t>,
    ) -> Result<Option<ExprKind>, Error> {
        let receiver = select.get("expression").filter(|_| select.kind() == "MEMBER_SELECT");
        if symbol.owner == "java.lang.Object" && symbol.name == "getClass" {
            return Ok(Some(self.get_class(node, receiver)?));
        }
        if matches!(
            (symbol.owner, symbol.name),
            ("java.util.EnumMap", "getKeyUniverse") | ("java.util.EnumSet", "getUniverse")
        ) {
            return Ok(Some(self.enum_universe(node)?));
        }
        // `ConcurrentHashMap` has a copy of `HashMap`'s.
        if matches!(
            (symbol.owner, symbol.name),
            ("java.util.HashMap" | "java.util.concurrent.ConcurrentHashMap", "comparableClassFor")
        ) {
            let x = self.expr(node.list("arguments").next().expect("comparableClassFor has an argument"))?;
            return Ok(Some(ExprKind::Builtin(Builtin::ComparableClassFor, vec![x])));
        }
        if (symbol.owner, symbol.name) == ("sun.security.jca.TranslatedServices", "securityProperties") {
            return Ok(Some(match super::system::security_file() {
                Some(text) => ExprKind::Str(text.to_string()),
                None => ExprKind::Null,
            }));
        }
        if crate::program::constructs_services(node.0) {
            return Ok(Some(self.constructed_service(node)?));
        }
        if crate::program::is_ensure_initialized(node.0) {
            return Ok(Some(self.ensure_initialized(node)?));
        }
        if (symbol.owner, symbol.name) == ("java.lang.Class", "getPrimitiveClass") {
            return Ok(Some(self.primitive_class(node)?));
        }
        if let Some(name) = crate::program::module_for_name(node.0) {
            return Ok(Some(self.module_for_name(node, name)?));
        }
        if (symbol.owner, symbol.name) == ("java.lang.Class", "forName")
            && let Some(kind) = self.class_for_name(node)?
        {
            return Ok(Some(kind));
        }
        if crate::program::computed_for_name(node.0) {
            return Ok(Some(self.computed_for_name(node)?));
        }
        let Some(receiver) = receiver else { return Ok(None) };
        // `equals`, `hashCode` and `toString` of a class are `Object`'s.
        let of_class = |ty: &str| ty == "java.lang.Class" || ty.starts_with("java.lang.Class<");
        let owner = symbol.owner == "java.lang.Class"
            || (symbol.owner == "java.lang.Object" && of_class(receiver.java_type()));
        if !owner || symbol.is_static {
            return Ok(None);
        }
        let args: Vec<Node<'t>> = node.list("arguments").collect();
        match (symbol.name, args.as_slice()) {
            ("equals", [arg]) if self.named_class(receiver) && self.named_class(*arg) => {
                return Ok(Some(self.same_class(receiver, *arg, true)?));
            }
            ("getName" | "getSimpleName", []) if is_literal(receiver) && self.named_class(receiver) => {
                let (class, simple) = (self.literal_class(receiver)?, symbol.name == "getSimpleName");
                return Ok(Some(ExprKind::Builtin(Builtin::ClassLiteralName { class, simple }, Vec::new())));
            }
            // `class_name_call` names the class of the object.
            ("getName" | "getSimpleName", []) if is_get_class(receiver) && self.named_class(receiver) => {
                return Ok(None);
            }
            ("isInstance", [x]) => {
                let (class, x) = (self.expr(receiver)?, self.expr(*x)?);
                return Ok(Some(self.class_is_instance(class, x)));
            }
            ("cast", [x]) => return Ok(Some(self.class_cast(node, receiver, *x)?)),
            ("getInterfaces", []) if is_literal(receiver) && self.named_class(receiver) => {
                return Ok(Some(self.literal_interfaces(node, receiver)?));
            }
            ("getInterfaces", []) => return Ok(Some(self.class_interfaces(node, receiver)?)),
            ("getSigners", []) => return Ok(Some(self.class_signers(node, receiver)?)),
            ("isAnnotationPresent", [annotation]) => {
                return Ok(Some(self.annotation_present(node, receiver, *annotation)?));
            }
            ("getAnnotation", [annotation]) => {
                return Ok(Some(self.annotation_of(node, receiver, *annotation)?));
            }
            ("getGenericSuperclass", []) => return Ok(Some(self.generic_superclass(node, receiver)?)),
            ("getGenericInterfaces" | "getTypeParameters", []) => {
                return Ok(Some(self.generic_declarations(node, symbol.name, receiver)?));
            }
            ("getEnumConstants", []) => {
                let class = self.expr(receiver)?;
                return Ok(Some(self.class_enum_constants(node, class, true)?));
            }
            // A class's resources are its class loader's (`TranslatedModules.classResource`,
            // `MODULE_CALLS`). Its protection domain, the code source it was loaded from, is not
            // in the translation: the program ends when it asks (`Builtin::MissingResource`).
            ("getProtectionDomain", []) => {
                let class = self.class_name_text(node, receiver);
                let what = self.synth(
                    ExprKind::Str("(its protection domain)".to_string()),
                    node.pos(),
                    Type::String,
                );
                return Ok(Some(ExprKind::Builtin(Builtin::MissingResource, vec![class, what])));
            }
            _ => {}
        }
        if !symbol.is_static
            && let Some(kind) = self.reflection_lookup(node, symbol, receiver)?
        {
            return Ok(Some(kind));
        }
        let op = match (symbol.name, args.len()) {
            ("getName", 0) => ClassOp::GetName,
            ("getSimpleName", 0) => ClassOp::GetSimpleName,
            ("getTypeName", 0) => ClassOp::GetTypeName,
            ("getSuperclass", 0) => ClassOp::GetSuperclass,
            ("getComponentType" | "componentType", 0) => ClassOp::GetComponentType,
            ("isArray", 0) => ClassOp::IsArray,
            ("isInterface", 0) => ClassOp::IsInterface,
            ("isPrimitive", 0) => ClassOp::IsPrimitive,
            ("isEnum", 0) => ClassOp::IsEnum,
            ("toString", 0) => ClassOp::ToString,
            ("hashCode", 0) => ClassOp::HashCode,
            ("desiredAssertionStatus", 0) => ClassOp::DesiredAssertionStatus,
            ("getCanonicalName", 0) => ClassOp::GetCanonicalName,
            ("getPackageName", 0) => ClassOp::GetPackageName,
            ("descriptorString", 0) => ClassOp::DescriptorString,
            ("isAnnotation", 0) => ClassOp::IsAnnotation,
            ("getModifiers", 0) => ClassOp::GetModifiers,
            ("isAnonymousClass", 0) => ClassOp::IsAnonymousClass,
            ("isLocalClass", 0) => ClassOp::IsLocalClass,
            ("isMemberClass", 0) => ClassOp::IsMemberClass,
            ("getEnclosingClass", 0) => ClassOp::GetEnclosingClass,
            ("getDeclaringClass", 0) => ClassOp::GetDeclaringClass,
            ("equals", 1) => ClassOp::Equals,
            ("isAssignableFrom", 1) => ClassOp::IsAssignableFrom,
            _ => return unsupported(node, &format!("`Class.{}` (reflection)", symbol.name)),
        };
        let mut values = vec![self.expr(receiver)?];
        for arg in args {
            let value = self.expr(arg)?;
            if op == ClassOp::Equals && matches!(value.ty, Type::Class(..)) {
                return Ok(Some(self.class_equals_object(node, values.remove(0), value)));
            }
            if !matches!(value.ty, Type::JavaClass | Type::Null) {
                return unsupported(arg, "comparing a class with anything but a class");
            }
            values.push(value);
        }
        Ok(Some(ExprKind::Builtin(Builtin::ClassMethod(op), values)))
    }

    /// `c.equals(o)` of an `o` that is not typed a class (an `Object`, or a `Type` as log4j's
    /// `TypeUtil.isAssignable` compares): `o instanceof Class k && c.equals(k)`, with `c` and `o`
    /// evaluated first, in Java's order.
    fn class_equals_object(&self, node: Node<'t>, class: Expr, other: Expr) -> ExprKind {
        let pos = node.pos();
        let (c, o, k) = (self.fresh_name("class"), self.fresh_name("other"), self.fresh_name("held"));
        let var = |name: &str, ty: Type| self.synth(ExprKind::Var(name.to_string()), pos, ty);
        let ty = other.ty;
        let test = ExprKind::InstanceOf {
            value: Box::new(var(&o, ty)),
            ty: Type::JavaClass,
            binding: Some(k.clone()),
        };
        let same = ExprKind::Builtin(
            Builtin::ClassMethod(ClassOp::Equals),
            vec![var(&c, Type::JavaClass), var(&k, Type::JavaClass)],
        );
        let both = ExprKind::Binary(
            BinOp::And,
            Box::new(self.synth(test, pos, Type::Boolean)),
            Box::new(self.synth(same, pos, Type::Boolean)),
        );
        let body = vec![
            Stmt::Local { ty: Type::JavaClass, name: c, init: Some(class), pos },
            Stmt::Local { ty, name: o, init: Some(other), pos },
            Stmt::Yield(self.synth(both, pos, Type::Boolean)),
        ];
        ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body }],
        }
    }

    /// `X.class.isAssignableFrom(c)` of the class literal `literal`, which the runtime's class
    /// answers from the names of the classes and interfaces above `c` (`ClassOp::IsAssignableFrom`).
    pub(super) fn literal_assignable_from(
        &self,
        node: Node<'t>,
        literal: Node<'t>,
        class: Expr,
    ) -> Result<ExprKind, Error> {
        let literal = self.synth(self.class_literal(literal)?, node.pos(), Type::JavaClass);
        Ok(ExprKind::Builtin(Builtin::ClassMethod(ClassOp::IsAssignableFrom), vec![literal, class]))
    }

    /// The class that the type of `receiver`, a `Class<T>`, names, as text: `T`, which a missing
    /// resource's message names.
    fn class_name_text(&self, node: Node<'t>, receiver: Node<'t>) -> Expr {
        let class = receiver.java_type();
        let class = class.strip_prefix("java.lang.Class<").and_then(|c| c.strip_suffix('>')).unwrap_or(class);
        self.synth(ExprKind::Str(class.to_string()), node.pos(), Type::String)
    }

    /// `class.cast(x)`: `x`, checked against the class (`Builtin::ClassCheck`), then converted
    /// to the call's type as javac's cast of the result does, in a block when `x` needs a local.
    ///
    /// ```text
    /// switch (0) { default -> { T cast = x; check(class, cast); yield (T) cast; } }
    /// ```
    fn class_cast(&self, node: Node<'t>, receiver: Node<'t>, x: Node<'t>) -> Result<ExprKind, Error> {
        let class = self.expr(receiver)?;
        let value = self.expr(x)?;
        let to = self.ty(node, node.java_type())?;
        let pos = node.pos();
        // A class literal casts `null` to `null`.
        if is_literal(receiver) && matches!(value.kind, ExprKind::Null) {
            return Ok(ExprKind::Cast(to, Box::new(value)));
        }
        // An object that may be an annotation (`annotation_objects`): `{ Class c = ..; T v = ..;
        // if (!(v is an annotation of c)) check(c, v); yield (to) v; }`.
        let (class_local, value_local) = (self.fresh_name("class"), self.fresh_name("cast"));
        let class_var = self.synth(ExprKind::Var(class_local.clone()), pos, Type::JavaClass);
        let value_var = self.synth(ExprKind::Var(value_local.clone()), pos, value.ty);
        if let Some(test) = self.annotation_object_test(self.copy(&class_var), &value_var) {
            let check = ExprKind::Builtin(Builtin::ClassCheck, vec![class_var, self.copy(&value_var)]);
            let check = Stmt::Expr(self.synth(check, pos, Type::Void));
            let not = self.synth(ExprKind::Unary(UnOp::Not, Box::new(test)), pos, Type::Boolean);
            let body = vec![
                Stmt::Local { ty: Type::JavaClass, name: class_local, init: Some(class), pos },
                Stmt::Local { ty: value.ty, name: value_local, init: Some(value), pos },
                Stmt::If { cond: not, then: Box::new(check), otherwise: None },
                Stmt::Yield(self.class_result(|| self.copy(&value_var), to)),
            ];
            return Ok(block_of(self, pos, body));
        }
        let (ty, mut block, name) = match &value.kind {
            ExprKind::Var(name) => (value.ty, Vec::new(), Some(name.clone())),
            ExprKind::Null => (value.ty, Vec::new(), None),
            _ => {
                let (ty, name) = (value.ty, self.fresh_name("cast"));
                (ty, vec![Stmt::Local { ty, name: name.clone(), init: Some(value), pos }], Some(name))
            }
        };
        let read = || match &name {
            Some(name) => self.synth(ExprKind::Var(name.clone()), pos, ty),
            None => self.synth(ExprKind::Null, pos, Type::Null),
        };
        let check = ExprKind::Builtin(Builtin::ClassCheck, vec![class, read()]);
        block.push(Stmt::Expr(self.synth(check, pos, Type::Void)));
        // `cast(null)` is a null of the call's type.
        let result = match name {
            Some(_) => self.class_result(read, to),
            None => self.synth(ExprKind::Null, pos, to),
        };
        block.push(Stmt::Yield(result));
        Ok(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body: block }],
        })
    }

    /// The checked value that `read` gives, as the cast's type `to`: a conversion for an upcast,
    /// as `convert` gives it, and a `Cast` for a downcast, which `convert` leaves as it is. The
    /// block's type is the call's, so a value still of the wrong type would not compile.
    pub(super) fn class_result(&self, read: impl Fn() -> Expr, to: Type) -> Expr {
        let converted = self.convert(read(), to);
        if converted.ty == to {
            return converted;
        }
        let pos = converted.pos;
        self.synth(ExprKind::Cast(to, Box::new(read())), pos, to)
    }

    /// `class.isInstance(x)`, which for an object that may be an annotation also asks whether it
    /// is one of that class (`annotation_objects`): `{ Class c = ..; Object v = ..; yield
    /// (v is an annotation of c) || c.isInstance(v); }`.
    pub(super) fn class_is_instance(&self, class: Expr, x: Expr) -> ExprKind {
        let pos = x.pos;
        let (class_local, value_local) = (self.fresh_name("class"), self.fresh_name("object"));
        let class_var = self.synth(ExprKind::Var(class_local.clone()), pos, Type::JavaClass);
        let value_var = self.synth(ExprKind::Var(value_local.clone()), pos, x.ty);
        let Some(test) = self.annotation_object_test(self.copy(&class_var), &value_var) else {
            return ExprKind::Builtin(Builtin::ClassIsInstance, vec![class, x]);
        };
        let plain = ExprKind::Builtin(Builtin::ClassIsInstance, vec![class_var, value_var]);
        let plain = self.synth(plain, pos, Type::Boolean);
        let either =
            self.synth(ExprKind::Binary(BinOp::Or, Box::new(test), Box::new(plain)), pos, Type::Boolean);
        let body = vec![
            Stmt::Local { ty: Type::JavaClass, name: class_local, init: Some(class), pos },
            Stmt::Local { ty: x.ty, name: value_local, init: Some(x), pos },
            Stmt::Yield(either),
        ];
        block_of(self, pos, body)
    }

    /// The constants of the enum class `cls`, which `EnumMap.getKeyUniverse(cls)` and
    /// `EnumSet.getUniverse(cls)` get from the JVM (see `class_enum_constants`).
    fn enum_universe(&self, node: Node<'t>) -> Result<ExprKind, Error> {
        let arg = node.list("arguments").next().expect("the universe of a class");
        let class = self.expr(arg)?;
        self.class_enum_constants(node, class, false)
    }

    /// `cls.getEnumConstants()`, and the constants that `EnumMap` and `EnumSet` get: `{ Class c_ =
    /// cls; c_ == A.class ? new Enum[] {A.X, ..} : .. : null }`, over the program's enums, and the
    /// array of the enum's constants directly for a class literal. A `call` of
    /// `getEnumConstants()` checks the class first, as the call's receiver
    /// (`Builtin::NullChecked`), and holds it as a receiver (`lower/argument_order.rs`), so that
    /// the message of a null names it.
    fn class_enum_constants(&self, node: Node<'t>, class: Expr, call: bool) -> Result<ExprKind, Error> {
        let array = self.ty(node, node.java_type())?;
        let element = array.element();
        let pos = node.pos();
        let constants = |e: usize| {
            let ty = Type::enum_type(&self.types[e].name);
            let constants = (0..self.enum_constant_names(e).count())
                .map(|ordinal| {
                    let constant = self.synth(ExprKind::EnumConstant { class: e, ordinal }, pos, ty);
                    self.convert(constant, element)
                })
                .collect();
            self.synth(ExprKind::ArrayLit(constants), pos, array)
        };
        if let ExprKind::Builtin(Builtin::ClassLiteral(ty), _) = &class.kind {
            return Ok(match ty {
                Type::Enum(name) => match self.types.iter().position(|t| t.name == *name) {
                    Some(e) => constants(e).kind,
                    None => ExprKind::Null,
                },
                _ => ExprKind::Null,
            });
        }
        let name = match call {
            true => format!("{}{}", super::RECEIVER, class.id),
            false => self.fresh_name("class"),
        };
        let (mut chain, mut chain_ty) = (ExprKind::Null, Type::Null);
        for e in (0..self.types.len()).rev().filter(|&t| self.types[t].node.kind() == "ENUM") {
            let ty = Type::enum_type(&self.types[e].name);
            let literal =
                self.synth(ExprKind::Builtin(Builtin::ClassLiteral(ty), Vec::new()), pos, Type::JavaClass);
            let var = self.synth(ExprKind::Var(name.clone()), pos, Type::JavaClass);
            let test =
                self.synth(ExprKind::Binary(BinOp::Eq, Box::new(var), Box::new(literal)), pos, Type::Boolean);
            let rest = self.synth(chain, pos, chain_ty);
            chain_ty = array;
            chain = ExprKind::Conditional(Box::new(test), Box::new(constants(e)), Box::new(rest));
        }
        let mut body = vec![Stmt::Local { ty: Type::JavaClass, name: name.clone(), init: Some(class), pos }];
        if call {
            let var = self.synth(ExprKind::Var(name), pos, Type::JavaClass);
            let check = ExprKind::Builtin(Builtin::NullChecked(Some(ENUM_CONSTANTS)), vec![var]);
            body.push(Stmt::Expr(self.synth(check, pos, Type::Void)));
        }
        body.push(Stmt::Yield(self.synth(chain, pos, chain_ty)));
        Ok(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body }],
        })
    }

    /// `X.class.getInterfaces()` of a class `X` of the program: a new array of the class literals
    /// of the interfaces it implements, or extends, in the order it names them. An interface that
    /// the translation has no class for (`Serializable`, a marker that lowering leaves out) is
    /// rejected rather than left out.
    fn literal_interfaces(&self, node: Node<'t>, literal: Node<'t>) -> Result<ExprKind, Error> {
        let class = self.literal_class(literal)?;
        let pos = node.pos();
        let mut interfaces = Vec::new();
        for clause in self.types[class].node.list("implementsClause") {
            let raw = super::captures::raw(clause.java_type());
            let ty = match self.ty(clause, &raw) {
                Ok(ty @ (Type::Class(..) | Type::Enum(_))) => ty,
                _ => {
                    return unsupported(node, &format!("`getInterfaces` of a class that implements `{raw}`"));
                }
            };
            let literal = ExprKind::Builtin(Builtin::ClassLiteral(ty), Vec::new());
            // An `Object` where `Class[]` is covariant.
            let element = self.covariant_array(Type::array(Type::JavaClass)).element();
            interfaces.push(self.convert(self.synth(literal, pos, Type::JavaClass), element));
        }
        if interfaces.is_empty() {
            return Ok(ExprKind::NewArray(vec![self.synth(ExprKind::Int(0), pos, Type::Int)]));
        }
        // A `Class[]` may be an `Object[]` (`covariant`), whose elements convert to `Object`.
        let element = self.covariant_array(Type::array(Type::JavaClass)).element();
        Ok(ExprKind::ArrayLit(interfaces.into_iter().map(|i| self.convert(i, element)).collect()))
    }

    /// `c.getInterfaces()` of any class `c`: a new array filled from the interfaces that the class
    /// value lists (`ClassOp::InterfaceCount` and `InterfaceAt`), one by one, so that the array is
    /// one of the program's like any other, a `Class[]` that may be a covariant `Object[]`.
    fn class_interfaces(&self, node: Node<'t>, receiver: Node<'t>) -> Result<ExprKind, Error> {
        let class = self.expr(receiver)?;
        let pos = node.pos();
        let array = self.covariant_array(Type::array(Type::JavaClass));
        let var = |name: &str, ty: Type| self.synth(ExprKind::Var(name.to_string()), pos, ty);
        let op = |op: ClassOp, args: Vec<Expr>, ty: Type| {
            self.synth(ExprKind::Builtin(Builtin::ClassMethod(op), args), pos, ty)
        };
        let (c, out, i) =
            (self.fresh_name("class"), self.fresh_name("interfaces"), self.fresh_name("interface"));
        let count = op(ClassOp::InterfaceCount, vec![var(&c, Type::JavaClass)], Type::Int);
        let new = self.synth(ExprKind::NewArray(vec![count]), pos, array);
        let length = self.synth(ExprKind::Length(Box::new(var(&out, array))), pos, Type::Int);
        let cond = self.synth(
            ExprKind::Binary(BinOp::Lt, Box::new(var(&i, Type::Int)), Box::new(length)),
            pos,
            Type::Boolean,
        );
        let update = self.synth(
            ExprKind::IncDec { target: Place::Var(i.clone()), increment: true, prefix: false },
            pos,
            Type::Int,
        );
        let at =
            op(ClassOp::InterfaceAt, vec![var(&c, Type::JavaClass), var(&i, Type::Int)], Type::JavaClass);
        let element = array.element();
        let value = self.convert(at, element);
        let target = Place::Index(Box::new(var(&out, array)), Box::new(var(&i, Type::Int)));
        let store = self.synth(ExprKind::Assign { op: None, target, value: Box::new(value) }, pos, element);
        let zero = self.synth(ExprKind::Int(0), pos, Type::Int);
        let body = vec![
            Stmt::Local { ty: Type::JavaClass, name: c, init: Some(class), pos },
            Stmt::Local { ty: array, name: out.clone(), init: Some(new), pos },
            Stmt::For {
                init: vec![Stmt::Local { ty: Type::Int, name: i, init: Some(zero), pos }],
                cond: Some(cond),
                update: vec![update],
                body: Box::new(Stmt::Expr(store)),
            },
            Stmt::Yield(var(&out, array)),
        ];
        Ok(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body }],
        })
    }

    /// `c.getSigners()` where no source root has a jar's signatures, or the translation has no
    /// JDK sources: `null`, as Java gives for a class without signers (otherwise
    /// `TranslatedModules.signersOf` answers, `program::MODULE_CALLS`, which only then reaches
    /// the JDK's PKCS7 and X.509 code). A null `c` throws, as the other methods of a class do.
    fn class_signers(&self, node: Node<'t>, receiver: Node<'t>) -> Result<ExprKind, Error> {
        let class = self.expr(receiver)?;
        let (pos, signers) = (node.pos(), self.ty(node, node.java_type())?);
        let checked = ExprKind::Builtin(Builtin::ClassMethod(ClassOp::IsArray), vec![class]);
        // A local of the array's type, so that the `null` has its type where nothing else gives
        // it one (a call whose value is dropped).
        let local = self.fresh_name("signers");
        let null = self.synth(ExprKind::Null, pos, Type::Null);
        let body = vec![
            Stmt::Expr(self.synth(checked, pos, Type::Boolean)),
            Stmt::Local { ty: signers, name: local.clone(), init: Some(null), pos },
            Stmt::Yield(self.synth(ExprKind::Var(local), pos, signers)),
        ];
        Ok(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body }],
        })
    }

    /// `T.class`, a class literal as a value.
    pub(super) fn class_literal(&self, node: Node<'t>) -> Result<ExprKind, Error> {
        let named = node.child("expression")?;
        // An annotation type's, whose binary name is its name where no class declares it.
        let name = named.java_type();
        if self.annotation_types.contains_key(name) && self.annotation_binary_name(name).is_some() {
            self.note_annotation_class(name.to_string());
            return Ok(ExprKind::Builtin(Builtin::ClassLiteral(Type::class(name)), Vec::new()));
        }
        // A class that is no type of the translation, as it has no objects: its class is the
        // runtime's (`codegen::class_values`).
        if crate::codegen::OBJECTLESS_CLASSES.contains(&name) {
            return Ok(ExprKind::Builtin(Builtin::ClassLiteral(Type::class(name)), Vec::new()));
        }
        let ty = self.exact_ty(named, named.java_type())?;
        // `Charset.class` is the runtime's one class of charsets (`codegen::class_values`), which
        // the program has once a charset may be an `Object` (`Arena`'s proxy finds
        // `allocateFrom(String, Charset)` with it).
        if ty == Type::Charset {
            self.note_object_array(ty);
        } else if !ty.is_reference()
            && !ty.is_numeric()
            && !matches!(ty, Type::Boolean | Type::Void | Type::Enum(_))
        {
            return unsupported(node, "the class literal of this type");
        }
        Ok(ExprKind::Builtin(Builtin::ClassLiteral(ty), Vec::new()))
    }

    /// `o.getClass()` as a value.
    fn get_class(&self, node: Node<'t>, receiver: Option<Node<'t>>) -> Result<ExprKind, Error> {
        let object = match receiver {
            Some(o) => self.expr(o)?,
            None => self.this(node),
        };
        let known = matches!(
            object.ty,
            Type::Class(..)
                | Type::Enum(_)
                | Type::String
                | Type::Boxed(_)
                | Type::Array(_)
                | Type::StringBuilder
                | Type::JavaClass
        );
        if !known {
            return unsupported(node, "getClass() of this type");
        }
        Ok(ExprKind::Builtin(Builtin::GetClass, vec![object]))
    }

    /// Whether the classes `left` and `right` are the same (`!same`: are not), by name.
    fn same_class(&self, left: Node<'t>, right: Node<'t>, same: bool) -> Result<ExprKind, Error> {
        let (left, right) = (self.class_name_value(left)?, self.class_name_value(right)?);
        Ok(ExprKind::Builtin(Builtin::SameClass { same }, vec![left, right]))
    }

    /// Whether `node` is a class that `same_class` names: `o.getClass()` of an object of the
    /// program, or a class literal of one of its classes.
    fn named_class(&self, node: Node<'t>) -> bool {
        let class_of = |n: Node| self.ty(n, n.java_type()).ok().and_then(|ty| self.class_type(ty));
        if is_literal(node) {
            return node.get("expression").is_some_and(|e| class_of(e).is_some());
        }
        if !is_get_class(node) {
            return false;
        }
        let Ok(select) = node.child("methodSelect") else { return false };
        // An object of a class type, `Object` included, is of a class of the program.
        match select.get("expression").filter(|_| select.kind() == "MEMBER_SELECT") {
            Some(object) => matches!(self.ty(object, object.java_type()), Ok(Type::Class(..))),
            None => true,
        }
    }

    /// The class of the program that type `ty` names.
    fn class_type(&self, ty: Type) -> Option<usize> {
        match ty {
            Type::Class(name, _) => self.type_index(name),
            _ => None,
        }
    }

    /// The binary name of a class that `named_class` accepts.
    fn class_name_value(&self, class: Node<'t>) -> Result<Expr, Error> {
        let kind = if is_literal(class) {
            let class = self.literal_class(class)?;
            ExprKind::Builtin(Builtin::ClassLiteralName { class, simple: false }, Vec::new())
        } else {
            let select = class.child("methodSelect")?;
            let object = match select.get("expression").filter(|_| select.kind() == "MEMBER_SELECT") {
                Some(o) => self.expr(o)?,
                None => self.this(class),
            };
            ExprKind::Builtin(Builtin::ClassName { simple: false }, vec![object])
        };
        Ok(self.synth(kind, class.pos(), Type::String))
    }

    /// The class that a class literal `Point.class` names, which the program declares.
    fn literal_class(&self, literal: Node<'t>) -> Result<usize, Error> {
        let named = literal.child("expression")?;
        match self.class_type(self.ty(named, named.java_type())?) {
            Some(class) => Ok(class),
            None => unsupported(literal, "the class literal of this type"),
        }
    }
}

/// Whether `node` is the call `o.getClass()`.
pub(super) fn is_get_class(node: Node) -> bool {
    node.kind() == "METHOD_INVOCATION"
        && node.symbol().is_some_and(|s| s.owner == "java.lang.Object" && s.name == "getClass")
}

/// Whether `node` is a class literal, `Point.class`.
pub(super) fn is_literal(node: Node) -> bool {
    node.kind() == "MEMBER_SELECT" && node.str("identifier") == "class"
}

/// `switch (0) { default -> { body } }`, a block as an expression.
fn block_of(lowering: &Lowering, pos: Pos, body: Vec<Stmt>) -> ExprKind {
    ExprKind::Switch {
        selector: Box::new(lowering.synth(ExprKind::Int(0), pos, Type::Int)),
        cases: vec![Case { labels: vec![CaseLabel::Default], body }],
    }
}
