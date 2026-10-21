//! `super.clone()` (or `clone()` without a receiver) of `Object`: a new object of the object's
//! runtime class whose fields are the object's fields, each copied as it is, its superclasses'
//! fields too (see `Builtin::ObjectClone`). In a class with subclasses the object may be one of
//! a subclass, so the copy is made for each of them; for a class that is not `Cloneable` (see
//! `TypeDecl::cloneable`) it throws `CloneNotSupportedException`, as Java's does.

use super::*;

impl<'t> Lowering<'t> {
    /// `super.clone()` of `Object`, or `clone()` that resolves to it, in the class being lowered:
    /// the copy, of the class's type, converted to the call's type (`Object`, unless a cast
    /// around it says otherwise, see `cast_clone`).
    pub(super) fn object_clone(&self, node: Node<'t>, receiver: Option<Node<'t>>) -> Result<ExprKind, Error> {
        let class = self.current.get();
        let is_this = match receiver {
            None => true,
            Some(r) => r.kind() == "IDENTIFIER" && r.str("name") == "super",
        };
        if !is_this {
            return unsupported(node, "`clone` of an object other than `this`");
        }
        let ty = self.self_type(class);
        let this = self.synth(ExprKind::This, node.pos(), ty);
        let copy = self.synth(ExprKind::Builtin(Builtin::ObjectClone, vec![this]), node.pos(), ty);
        let to = self.ty(node, node.java_type())?;
        Ok(if to == ty { copy.kind } else { ExprKind::Cast(to, Box::new(copy)) })
    }

    /// `(T) super.clone()` (`value` is the copy, see `is_clone`): the copy converted to `T`
    /// rather than to `Object`.
    pub(super) fn cast_clone(&self, value: Expr, to: Type) -> ExprKind {
        let copy = match value.kind {
            ExprKind::Cast(_, copy) => *copy,
            _ => value,
        };
        if copy.ty == to { copy.kind } else { ExprKind::Cast(to, Box::new(copy)) }
    }

    /// Whether `value` is `super.clone()` of `Object` (see `object_clone`).
    pub(super) fn is_clone(value: &Expr) -> bool {
        let copy = match &value.kind {
            ExprKind::Cast(_, copy) => copy,
            _ => value,
        };
        matches!(copy.kind, ExprKind::Builtin(Builtin::ObjectClone, _))
    }

    /// Whether the objects of class `t` may be cloned: it, a superclass or an interface they
    /// implement extends `Cloneable`.
    pub(super) fn is_cloneable(&self, t: usize) -> bool {
        let node = self.types[t].node;
        let interfaces = node.list("implementsClause").any(|c| {
            let name = c.java_type().split('<').next().unwrap_or_default();
            name == "java.lang.Cloneable" || self.type_index(name).is_some_and(|i| self.is_cloneable(i))
        });
        interfaces || self.superclass(t).ok().flatten().is_some_and(|s| self.is_cloneable(s))
    }
}
