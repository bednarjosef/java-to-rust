//! The `ClassCastException` of a call of `Comparable`'s `compareTo`. The class of the receiver
//! implements `compareTo(Object)` as a bridge that casts the argument to its own type, so a
//! raw `Comparable`, or a `T extends Comparable<T>` erased to it, fails for an argument of
//! another class, inside the call. The generator runs such a call inside `java::catching`
//! (`Facts::catching`): the failure unwinds there and reaches the `catch` that sees it.

use super::*;

/// Whether `x` is a call of `Comparable`'s own abstract `compareTo`, a call that may fail as above.
pub(super) fn is_compare_call(class: &Class, x: &Expr) -> bool {
    let ExprKind::MethodCall { resolved: Some(m), .. } = &x.kind else { return false };
    let t = &class.types[class.methods[*m].class];
    t.kind == TypeKind::Interface && t.name == COMPARABLE
}
