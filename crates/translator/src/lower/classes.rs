//! `java.lang.Class`, as far as a program uses it without reflection. Comparing the classes of
//! two objects, or the class of an object with a class literal of the program (`a.getClass() ==
//! b.getClass()`, `o.getClass().equals(Point.class)`), compares their binary names
//! (`Builtin::SameClass` of `Builtin::ClassName` and `Builtin::ClassLiteralName`), since no two
//! classes of a program have the same one, and the names of such a literal are constants. Any
//! other class is a value, a `java::Class` (`Type::JavaClass`): `x.getClass()`
//! (`Builtin::GetClass`), a class literal (`Builtin::ClassLiteral`), and the methods of `Class`
//! that need no reflection (`Builtin::ClassMethod`).

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
        if (symbol.owner, symbol.name) == ("java.util.HashMap", "comparableClassFor") {
            let x = self.expr(node.list("arguments").next().expect("comparableClassFor has an argument"))?;
            return Ok(Some(ExprKind::Builtin(Builtin::ComparableClassFor, vec![x])));
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
