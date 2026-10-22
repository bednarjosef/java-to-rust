//! Which boxed kinds have their identity observed. `==` on two boxed values compares the objects,
//! so the boxes of such a kind are shared objects, as Java has them. Every other boxed value is
//! just its primitive, which is what the program can see.

use super::*;

/// Records in `Facts::faithful_boxes` the boxed kinds that some `==` or `!=` compares as two
/// objects. The decision is per kind over the whole program: coarser than per variable, but a
/// shared object is never observed differently from Java's.
pub fn analyze(class: &Class, facts: &mut Facts) {
    let mut kinds = Vec::new();
    for method in &class.methods {
        for stmt in &method.body {
            visit_stmt(stmt, &mut |e| identity_kinds(e, &mut kinds), &mut |_| {});
        }
    }
    for decl in &class.types {
        for field in decl.fields.iter().chain(&decl.statics) {
            if let Some(init) = &field.init {
                identity_kinds(init, &mut kinds);
            }
        }
    }
    facts.faithful_boxes.extend(kinds);
}

/// The boxed kinds of the `==` and `!=` comparisons in `e` between two boxed operands. A
/// `Boolean` or a `Byte` is always one of the objects that `valueOf` caches, as the translation
/// has no `new Boolean(..)` or `new Byte(..)`: two of them are the same object exactly when their
/// values are equal, so comparing the values is Java's identity.
fn identity_kinds(e: &Expr, kinds: &mut Vec<Type>) {
    visit_expr(e, &mut |x| {
        if let ExprKind::Binary(BinOp::Eq | BinOp::Ne, l, r) = &x.kind
            && matches!((l.ty, r.ty), (Type::Boxed(_), Type::Boxed(_)))
            && !matches!(l.ty.unboxed(), Type::Boolean | Type::Byte)
        {
            kinds.push(l.ty);
        }
    });
}
