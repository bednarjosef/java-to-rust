//! `super.clone()` (or `clone()` without a receiver) of `Object` in a class that implements
//! `Cloneable`: a new object of the same class whose fields are the object's fields, each copied
//! as it is (see `Builtin::ObjectClone`). Only a class with no superclass and no subclasses is
//! written so far: the copy of a subclass's object, or of an object whose struct embeds a
//! superclass's fields, needs the variants and the `base` field copied too.

use super::*;

impl<'t> Lowering<'t> {
    /// `super.clone()` of `Object`, or `clone()` that resolves to it, in the class being lowered.
    pub(super) fn object_clone(&self, node: Node<'t>, receiver: Option<Node<'t>>) -> Result<ExprKind, Error> {
        let class = self.current.get();
        let is_this = match receiver {
            None => true,
            Some(r) => r.kind() == "IDENTIFIER" && r.str("name") == "super",
        };
        if !is_this {
            return unsupported(node, "`clone` of an object other than `this`");
        }
        let cloneable = self.types[class]
            .node
            .list("implementsClause")
            .any(|c| c.java_type().split('<').next() == Some("java.lang.Cloneable"));
        if !cloneable {
            return unsupported(node, "`clone` of a class that is not `Cloneable`");
        }
        if self.superclass(class)?.is_some() {
            return unsupported(node, "`super.clone()` of a class with a superclass");
        }
        let subclassed = (0..self.types.len())
            .any(|t| t != class && self.types[t].node.kind() == "CLASS" && self.is_subclass(t, class));
        if subclassed {
            return unsupported(node, "`super.clone()` of a class with subclasses");
        }
        let this = self.synth(ExprKind::This, node.pos(), self.self_type(class));
        Ok(ExprKind::Builtin(Builtin::ObjectClone, vec![this]))
    }
}
