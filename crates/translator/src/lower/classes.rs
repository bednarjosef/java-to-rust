//! The little of `java.lang.Class` that a program uses without reflection: comparing the
//! classes of two objects, or the class of an object with a class literal (`a.getClass() ==
//! b.getClass()`, `o.getClass().equals(Point.class)`), and the names of a class literal
//! (`Point.class.getSimpleName()`). A class stands for its binary name (`Builtin::ClassName` of
//! an object, `Builtin::ClassLiteralName` of a literal), and two classes are the same
//! (`Builtin::SameClass`) when their names are: no two classes of a program have the same
//! binary name.

use super::*;

impl<'t> Lowering<'t> {
    /// `a == b` or `a != b` of two classes (see `compares_classes`).
    pub(super) fn class_comparison(&self, node: Node<'t>) -> Result<ExprKind, Error> {
        let (left, right) = (node.child("leftOperand")?, node.child("rightOperand")?);
        self.same_class(node, left, right, node.kind() == "EQUAL_TO")
    }

    /// `a.equals(b)` of a class `a`, and `Point.class.getName()` or `getSimpleName()`; `None`
    /// for any other call. It runs before the arguments are lowered.
    pub(super) fn class_call(
        &self,
        node: Node<'t>,
        symbol: &Symbol,
        select: Node<'t>,
    ) -> Result<Option<ExprKind>, Error> {
        let receiver = select.get("expression").filter(|_| select.kind() == "MEMBER_SELECT");
        let Some(receiver) = receiver.filter(|r| is_class(*r)) else { return Ok(None) };
        match symbol.name {
            "equals" => {
                let arg = node.list("arguments").next().expect("equals has an argument");
                if !is_class(arg) {
                    return unsupported(node, "comparing a class with anything but a class");
                }
                Ok(Some(self.same_class(node, receiver, arg, true)?))
            }
            "getName" | "getSimpleName" if is_literal(receiver) => {
                let (class, simple) = (self.literal_class(receiver)?, symbol.name == "getSimpleName");
                Ok(Some(ExprKind::Builtin(Builtin::ClassLiteralName { class, simple }, Vec::new())))
            }
            _ => Ok(None),
        }
    }

    /// Whether the classes `left` and `right` are the same (`!same`: are not).
    fn same_class(
        &self,
        node: Node<'t>,
        left: Node<'t>,
        right: Node<'t>,
        same: bool,
    ) -> Result<ExprKind, Error> {
        let (left, right) = (self.class_value(node, left)?, self.class_value(node, right)?);
        Ok(ExprKind::Builtin(Builtin::SameClass { same }, vec![left, right]))
    }

    /// The binary name of a class: of `o.getClass()`, or of a class literal.
    fn class_value(&self, node: Node<'t>, class: Node<'t>) -> Result<Expr, Error> {
        let kind = if is_literal(class) {
            let class = self.literal_class(class)?;
            ExprKind::Builtin(Builtin::ClassLiteralName { class, simple: false }, Vec::new())
        } else if is_get_class(class) {
            let select = class.child("methodSelect")?;
            let object = match select.get("expression").filter(|_| select.kind() == "MEMBER_SELECT") {
                Some(o) => self.expr(o)?,
                None => self.this(class),
            };
            if !matches!(object.ty, Type::Class(..)) {
                return unsupported(class, "getClass() of this type");
            }
            ExprKind::Builtin(Builtin::ClassName { simple: false }, vec![object])
        } else {
            return unsupported(
                node,
                "comparing classes other than those of `getClass()` and class literals",
            );
        };
        Ok(self.synth(kind, class.pos(), Type::String))
    }

    /// The class that a class literal `Point.class` names, which the program declares.
    fn literal_class(&self, literal: Node<'t>) -> Result<usize, Error> {
        let named = literal.child("expression")?;
        match self.ty(named, named.java_type())? {
            Type::Class(name, _) => match self.type_index(name) {
                Some(class) => Ok(class),
                None => unsupported(literal, "the class literal of a standard class"),
            },
            _ => unsupported(literal, "the class literal of this type"),
        }
    }
}

/// Whether `node`, an `==` or `!=`, compares classes: `o.getClass()` or a class literal on
/// either side.
pub(super) fn compares_classes(node: Node) -> bool {
    ["leftOperand", "rightOperand"].into_iter().any(|side| node.get(side).is_some_and(is_class))
}

/// Whether `node` is a class: `o.getClass()` or a class literal.
fn is_class(node: Node) -> bool {
    is_get_class(node) || is_literal(node)
}

/// Whether `node` is the call `o.getClass()`.
fn is_get_class(node: Node) -> bool {
    node.kind() == "METHOD_INVOCATION"
        && node.symbol().is_some_and(|s| s.owner == "java.lang.Object" && s.name == "getClass")
}

/// Whether `node` is a class literal, `Point.class`.
fn is_literal(node: Node) -> bool {
    node.kind() == "MEMBER_SELECT" && node.str("identifier") == "class"
}
