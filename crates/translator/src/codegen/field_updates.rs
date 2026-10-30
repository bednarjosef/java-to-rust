//! The atomic updates of a field through `Unsafe` (`Method::field_update`, see `lower::handles`).
//! On an object that threads share, whose field is a `java::SharedField`, the method's body is
//! one atomic action of the cell: `compare_and_set_by`, `compare_exchange_by`, `replace` or
//! `get_and_update`. Elsewhere the body reads, compares and writes the field as lowering wrote
//! it, which no other thread can see half done: the object is one thread's, or its lock is held.

use super::*;
use crate::program::FieldUpdate;

impl<'a> Gen<'a> {
    /// The body of the update method `m` as one atomic action of its field's cell, where the
    /// field is a `java::SharedField` and the method returns the cell's value as it is.
    pub(super) fn atomic_update(&mut self, m: &'a Method, index: usize) -> Option<String> {
        let (update, field) = m.field_update.as_ref()?;
        if m.is_static {
            return None;
        }
        // The body's read of the field names the object, `this`.
        let read = find(&m.body, &|e| matches!(&e.kind, ExprKind::Field { name, .. } if name == field))?;
        let ExprKind::Field { object, .. } = &read.kind else { unreachable!("found as a field read") };
        if !self.shared_object(object) {
            return None;
        }
        let ty = read.ty;
        let nullable = self.facts.field_nullable(object.ty, field);
        let cell = self.field_ty_name(ty, nullable);
        // A method that returns the old value returns it as the cell holds it.
        if !matches!(update, FieldUpdate::CompareAndSet) && self.ret_text(index) != format!(" -> {cell}") {
            return None;
        }
        let var = |name: &str| find(&m.body, &|e| matches!(&e.kind, ExprKind::Var(v) if v == name));
        let obj = self.object(object);
        let borrow = match self.enum_of(object) {
            Some(t) => self.enum_borrow(t, false),
            None => "",
        };
        let place = format!("{obj}{borrow}.{}", self.field_path(object, field));
        Some(match update {
            FieldUpdate::CompareAndSet | FieldUpdate::CompareAndExchange => {
                let expected = self.field_value(var("expected")?, ty, nullable);
                let value = self.field_value(var("value")?, ty, nullable);
                let same = self.field_identity(ty, nullable);
                let name = if matches!(update, FieldUpdate::CompareAndSet) {
                    "compare_and_set_by"
                } else {
                    "compare_exchange_by"
                };
                format!("{place}.{name}({expected}, {value}, {same})")
            }
            FieldUpdate::GetAndSet => {
                format!("{place}.replace({})", self.field_value(var("value")?, ty, nullable))
            }
            FieldUpdate::GetAndAdd => {
                let delta = self.value(var("delta")?, ty).text;
                match ty {
                    Type::Float | Type::Double => format!("{place}.get_and_update(|a| *a + {delta})"),
                    _ => format!("{place}.get_and_update(|a| a.wrapping_add({delta}))"),
                }
            }
            FieldUpdate::GetAndBitwise(op) => {
                let mask = self.value(var("delta")?, ty).text;
                format!("{place}.get_and_update(|a| *a {} {mask})", op.java())
            }
        })
    }

    /// The comparison of the compare forms of an update of a field of type `ty`: identity for a
    /// handle, as `==` is in Java, raw bits for a `float` or `double`, as `Unsafe` compares them,
    /// else equality.
    fn field_identity(&mut self, ty: Type, nullable: bool) -> String {
        if matches!(ty, Type::Float | Type::Double) {
            return "|a, b| a.to_bits() == b.to_bits()".to_string();
        }
        let handle = match ty {
            Type::Array(_) => true,
            Type::Class(..) => self.enum_type(ty).is_none(),
            _ => false,
        };
        if !handle {
            return "|a, b| a == b".to_string();
        }
        let same = self.runtime("same");
        if nullable {
            format!("|a, b| {same}(a.as_ref(), b.as_ref())")
        } else {
            format!("|a, b| {same}(Some(a), Some(b))")
        }
    }
}

/// The first expression of `stmts`, outermost first, that `pick` picks out.
fn find<'b>(stmts: &'b [Stmt], pick: &dyn Fn(&Expr) -> bool) -> Option<&'b Expr> {
    stmts.iter().find_map(|s| find_in_stmt(s, pick))
}

fn find_in_stmt<'b>(s: &'b Stmt, pick: &dyn Fn(&Expr) -> bool) -> Option<&'b Expr> {
    match s {
        Stmt::Expr(e) | Stmt::Return(Some(e), _) | Stmt::Local { init: Some(e), .. } => find_in_expr(e, pick),
        Stmt::If { cond, then, otherwise } => find_in_expr(cond, pick)
            .or_else(|| find_in_stmt(then, pick))
            .or_else(|| otherwise.as_deref().and_then(|o| find_in_stmt(o, pick))),
        Stmt::Block(stmts) => find(stmts, pick),
        _ => None,
    }
}

fn find_in_expr<'b>(e: &'b Expr, pick: &dyn Fn(&Expr) -> bool) -> Option<&'b Expr> {
    if pick(e) {
        return Some(e);
    }
    match &e.kind {
        ExprKind::Assign { value, .. } => find_in_expr(value, pick),
        ExprKind::Binary(_, a, b) => find_in_expr(a, pick).or_else(|| find_in_expr(b, pick)),
        ExprKind::Cast(_, inner) => find_in_expr(inner, pick),
        _ => None,
    }
}
