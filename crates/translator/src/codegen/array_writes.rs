//! Element writes to a shared array whose index or value runs code. The runtime helper takes the
//! array as a borrow, so a call in the index or value that changes the object holding the array
//! (`set(&self.memory, slot, self.pop())`) would borrow `self` twice. Java evaluates the array
//! first, then the index and the value; the array's handle is copied into a local at that point,
//! so the write holds no borrow of the object while that code runs.
//! An index or value that lends an array to a call (`a[last(&a.borrow())] = v`) is computed into
//! a local after it too: the borrow lives to the end of its statement, and the write would
//! panic with the cell already borrowed.

use super::*;

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
            format!("{}({name}.as_ref())", self.runtime("non_null"))
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
