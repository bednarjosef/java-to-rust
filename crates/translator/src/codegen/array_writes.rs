//! Element writes to a shared array whose index or value runs code. The runtime helper takes the
//! array as a borrow, so a call in the index or value that changes the object holding the array
//! (`set(&self.memory, slot, self.pop())`) would borrow `self` twice. Java evaluates the array
//! first, then the index and the value; the array's handle is copied into a local at that point,
//! so the write holds no borrow of the object while that code runs.
//! An index or value that lends an array to a call (`a[last(&a.borrow())] = v`) is computed into
//! a local after it too: the borrow lives to the end of its statement, and the write would
//! panic with the cell already borrowed.
//!
//! The atomic updates of an element through `Unsafe` (`Builtin::ElementUpdate`) are writes of the
//! same kind, bound the same way; their runtime functions read, compare and write the element
//! under one borrow of the array, which in a program with threads is one hold of its lock.

use super::*;
use crate::program::FieldUpdate;

impl<'a> Gen<'a> {
    /// The handle of an array that an element write uses. When the write runs code (`bound`), the
    /// handle is first copied into a local, and that local is returned as the binding to put
    /// before the write. Returns the binding (if any) and the handle to pass to the helper.
    pub(super) fn write_handle(&mut self, array: &'a Expr, bound: bool) -> (Option<String>, String) {
        if !bound {
            return (None, self.handle(array));
        }
        let name = self.fresh_name(&["array", "target", "handle"]);
        let place = self.object_place(array);
        let binding = format!("let {name} = {place}.clone();");
        let handle = if self.nullable(array) {
            self.dereference(&[array.id], &format!("{name}.as_ref()"))
        } else {
            format!("&{name}")
        };
        (Some(binding), handle)
    }

    /// `text`, an index or value of a bound write, computed into a fresh local after the binding
    /// of the array. A call in it that is lent a shared array (`f(&a.borrow())`) keeps that
    /// borrow to the end of its statement, so it must not be in the statement of the write.
    pub(super) fn bind_first(
        &mut self,
        binding: &mut Option<String>,
        text: String,
        names: &[&'static str],
    ) -> String {
        let name = self.fresh_name(names);
        let mut lines = binding.take().unwrap_or_default();
        lines.push_str(&format!(" let {name} = {text};"));
        *binding = Some(lines);
        name.to_string()
    }

    /// The write `call`, after the binding of its array, if there is one, in a block.
    /// `Builtin::ElementUpdate`: the runtime's atomic update of the element that the store among
    /// `args` writes (see `lower::handles`). Java evaluates the array, the index, the expected
    /// value and the new one, in that order.
    pub(super) fn element_update_call(&mut self, update: FieldUpdate, args: &'a [Expr]) -> Code {
        let (expected, store) = match args {
            [store] => (None, store),
            [expected, store] => (Some(expected), store),
            _ => unreachable!("an element update has one or two operands"),
        };
        let ExprKind::Assign { target: Place::Index(array, index), value, .. } = &store.kind else {
            unreachable!("an element update makes a store to an element")
        };
        let elem = arrays::element(array.ty);
        let effects = has_effects(index) || has_effects(value) || expected.is_some_and(has_effects);
        let (mut binding, handle) = self.write_handle(array, effects);
        let mut i = self.value(index, Type::Int).text;
        let mut old = expected.map(|e| self.element_value(e, elem));
        let mut v = match update {
            FieldUpdate::GetAndAdd => self.value(value, elem).text,
            _ => self.element_value(value, elem),
        };
        if lends_array(index) || lends_array(value) || expected.is_some_and(lends_array) {
            i = self.bind_first(&mut binding, i, &["index", "slot", "at"]);
            old = old.map(|o| self.bind_first(&mut binding, o, &["expected", "old", "before"]));
            v = self.bind_first(&mut binding, v, &["value", "item", "new"]);
        }
        let call = match update {
            FieldUpdate::GetAndAdd => {
                let add = if elem == Type::Long { "i64::wrapping_add" } else { "i32::wrapping_add" };
                format!("{}({handle}, {i}, {v}, {add})", self.runtime("get_and_add_element"))
            }
            FieldUpdate::GetAndSet => format!("{}({handle}, {i}, {v})", self.runtime("get_and_set_element")),
            FieldUpdate::CompareAndSet | FieldUpdate::CompareAndExchange => {
                let name = if update == FieldUpdate::CompareAndSet {
                    "compare_and_set_element"
                } else {
                    "compare_and_exchange_element"
                };
                let old = old.expect("a compare form has an expected value");
                let same = self.element_identity(elem);
                format!("{}({handle}, {i}, {old}, {v}, {same})", self.runtime(name))
            }
        };
        Code::atom(Self::bound_write(binding, call))
    }

    /// The closure that tells whether two elements of type `elem` are the same, as Java's `==`
    /// compares them: objects and arrays by identity (`java::same`), and numbers and enums, whose
    /// `==` is identity, by value.
    fn element_identity(&mut self, elem: Type) -> String {
        let handle = match elem {
            Type::Array(_) => true,
            Type::Class(..) => self.enum_type(elem).is_none(),
            _ => false,
        };
        if !handle {
            return "|a, b| a == b".to_string();
        }
        let same = self.runtime("same");
        if self.nullable_element(elem) {
            format!("|a, b| {same}(a.as_ref(), b.as_ref())")
        } else {
            format!("|a, b| {same}(Some(a), Some(b))")
        }
    }

    pub(super) fn bound_write(binding: Option<String>, call: String) -> String {
        match binding {
            Some(binding) => format!("{{ {binding} {call} }}"),
            None => call,
        }
    }
}

/// Whether `e` passes an array to a method or a library call, which may borrow it.
pub(super) fn lends_array(e: &Expr) -> bool {
    let mut found = false;
    visit_expr(e, &mut |x| match &x.kind {
        ExprKind::Call { args, .. } | ExprKind::MethodCall { args, .. } | ExprKind::Builtin(_, args) => {
            found |= args.iter().any(|a| matches!(a.ty, Type::Array(_)))
        }
        _ => {}
    });
    found
}
