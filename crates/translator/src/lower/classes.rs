//! `java.lang.Class`, as far as a program uses it without reflection. Comparing the classes of
//! two objects, or the class of an object with a class literal of the program (`a.getClass() ==
//! b.getClass()`, `o.getClass().equals(Point.class)`), compares their binary names
//! (`Builtin::SameClass` of `Builtin::ClassName` and `Builtin::ClassLiteralName`), since no two
//! classes of a program have the same one, and the names of such a literal are constants. Any
//! other class is a value, a `java::Class` (`Type::JavaClass`): `x.getClass()`
//! (`Builtin::GetClass`), a class literal (`Builtin::ClassLiteral`), and the methods of `Class`
//! that need no reflection (`Builtin::ClassMethod`). `isInstance` and `cast` of any class value
//! test the binary names of the classes and interfaces of the object's class, which the generator
//! writes out per class (`Builtin::ClassIsInstance`, `Builtin::ClassCheck`).

use super::*;

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
        if matches!(symbol.owner, "java.util.HashMap" | "java.util.concurrent.ConcurrentHashMap")
            && symbol.name == "comparableClassFor"
        {
            let x = self.expr(node.list("arguments").next().expect("comparableClassFor has an argument"))?;
            return Ok(Some(ExprKind::Builtin(Builtin::ComparableClassFor, vec![x])));
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
        if symbol.owner == "java.lang.reflect.Constructor" {
            return Ok(Some(self.constructor_call(node, symbol, receiver)?));
        }
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
            // A translated program has no resources, not even the JDK's data files, such as
            // `CharacterName`'s `uniName.dat` or `Currency`'s `currency.data`, which are generated
            // when the JDK is built and are not in its source: the program ends when it reads one
            // (`Builtin::MissingResource`).
            ("isAssignableFrom", [class]) if is_literal(receiver) => {
                return Ok(Some(self.is_assignable_from(node, receiver, *class)?));
            }
            // A constructor without parameters is its class (`lower::by_name`).
            ("getConstructor" | "getDeclaredConstructor", []) => return Ok(Some(self.expr(receiver)?.kind)),
            ("newInstance", []) => {
                let class = self.expr(receiver)?;
                return Ok(Some(self.new_instance(node, class, receiver.java_type())?));
            }
            ("isInstance", [x]) => {
                let (class, x) = (self.expr(receiver)?, self.expr(*x)?);
                return Ok(Some(ExprKind::Builtin(Builtin::ClassIsInstance, vec![class, x])));
            }
            ("cast", [x]) => return Ok(Some(self.class_cast(node, receiver, *x)?)),
            ("getResourceAsStream", [name]) => {
                let class = receiver.java_type();
                let class =
                    class.strip_prefix("java.lang.Class<").and_then(|c| c.strip_suffix('>')).unwrap_or(class);
                let class = self.synth(ExprKind::Str(class.to_string()), node.pos(), Type::String);
                return Ok(Some(ExprKind::Builtin(Builtin::MissingResource, vec![class, self.expr(*name)?])));
            }
            _ => {}
        }
        let op = match (symbol.name, args.len()) {
            ("getName", 0) => ClassOp::GetName,
            ("getSimpleName", 0) => ClassOp::GetSimpleName,
            ("getSuperclass", 0) => ClassOp::GetSuperclass,
            ("getComponentType", 0) => ClassOp::GetComponentType,
            ("isArray", 0) => ClassOp::IsArray,
            ("isInterface", 0) => ClassOp::IsInterface,
            ("isPrimitive", 0) => ClassOp::IsPrimitive,
            ("isEnum", 0) => ClassOp::IsEnum,
            ("toString", 0) => ClassOp::ToString,
            ("hashCode", 0) => ClassOp::HashCode,
            ("desiredAssertionStatus", 0) => ClassOp::DesiredAssertionStatus,
            ("equals", 1) => ClassOp::Equals,
            _ => return unsupported(node, &format!("`Class.{}` (reflection)", symbol.name)),
        };
        let mut values = vec![self.expr(receiver)?];
        for arg in args {
            let value = self.expr(arg)?;
            if !matches!(value.ty, Type::JavaClass | Type::Null) {
                return unsupported(arg, "comparing a class with anything but a class");
            }
            values.push(value);
        }
        Ok(Some(ExprKind::Builtin(Builtin::ClassMethod(op), values)))
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
            Some(_) => self.convert(read(), to),
            None => self.synth(ExprKind::Null, pos, to),
        };
        block.push(Stmt::Yield(result));
        Ok(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body: block }],
        })
    }

    /// A call of a `Constructor` without parameters, which is its class (`lower::by_name`):
    /// `getModifiers()`, `setAccessible(flag)`, which does nothing but evaluate the flag, and
    /// `newInstance()` with no arguments or a `null` array of them.
    fn constructor_call(
        &self,
        node: Node<'t>,
        symbol: &Symbol,
        receiver: Node<'t>,
    ) -> Result<ExprKind, Error> {
        let args: Vec<Node<'t>> = node.list("arguments").collect();
        let null_array = |arg: &Node<'t>| {
            let arg = if arg.kind() == "TYPE_CAST" { arg.get("expression") } else { Some(*arg) };
            arg.is_some_and(|arg| arg.kind() == "NULL_LITERAL")
        };
        match (symbol.name, args.as_slice()) {
            ("getModifiers", []) => {
                let class = self.expr(receiver)?;
                self.constructor_modifiers(node, class, receiver.java_type())
            }
            ("setAccessible", [flag]) => Ok(self.expr(*flag)?.kind),
            ("newInstance", []) => {
                let class = self.expr(receiver)?;
                self.new_instance(node, class, receiver.java_type())
            }
            ("newInstance", [arg]) if null_array(arg) => {
                let class = self.expr(receiver)?;
                self.new_instance(node, class, receiver.java_type())
            }
            _ => unsupported(node, &format!("`Constructor.{}` (reflection)", symbol.name)),
        }
    }

    /// The constants of the enum class `cls`, which `EnumMap.getKeyUniverse(cls)` and
    /// `EnumSet.getUniverse(cls)` get from the JVM: `cls == A.class ? new Enum[] {A.X, ..} : ..
    /// : null`, over the program's enums.
    fn enum_universe(&self, node: Node<'t>) -> Result<ExprKind, Error> {
        let arg = node.list("arguments").next().expect("the universe of a class");
        let class = self.expr(arg)?;
        let ExprKind::Var(name) = &class.kind else {
            return unsupported(arg, "the constants of this class");
        };
        let array = self.ty(node, node.java_type())?;
        let element = array.element();
        let pos = node.pos();
        let (mut chain, mut chain_ty) = (ExprKind::Null, Type::Null);
        for e in (0..self.types.len()).rev().filter(|&t| self.types[t].node.kind() == "ENUM") {
            let ty = Type::enum_type(&self.types[e].name);
            let constants = (0..self.enum_constant_names(e).count())
                .map(|ordinal| {
                    let constant = self.synth(ExprKind::EnumConstant { class: e, ordinal }, pos, ty);
                    self.convert(constant, element)
                })
                .collect();
            let literal =
                self.synth(ExprKind::Builtin(Builtin::ClassLiteral(ty), Vec::new()), pos, Type::JavaClass);
            let var = self.synth(ExprKind::Var(name.clone()), pos, Type::JavaClass);
            let test =
                self.synth(ExprKind::Binary(BinOp::Eq, Box::new(var), Box::new(literal)), pos, Type::Boolean);
            let values = self.synth(ExprKind::ArrayLit(constants), pos, array);
            let rest = self.synth(chain, pos, chain_ty);
            chain_ty = array;
            chain = ExprKind::Conditional(Box::new(test), Box::new(values), Box::new(rest));
        }
        Ok(chain)
    }

    /// `T.class`, a class literal as a value.
    pub(super) fn class_literal(&self, node: Node<'t>) -> Result<ExprKind, Error> {
        let named = node.child("expression")?;
        let ty = self.ty(named, named.java_type())?;
        if !ty.is_reference() && !ty.is_numeric() && !matches!(ty, Type::Boolean | Type::Void | Type::Enum(_))
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
fn is_get_class(node: Node) -> bool {
    node.kind() == "METHOD_INVOCATION"
        && node.symbol().is_some_and(|s| s.owner == "java.lang.Object" && s.name == "getClass")
}

/// Whether `node` is a class literal, `Point.class`.
pub(super) fn is_literal(node: Node) -> bool {
    node.kind() == "MEMBER_SELECT" && node.str("identifier") == "class"
}
