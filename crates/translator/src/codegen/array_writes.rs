//! Element writes to a shared array whose index or value runs code. The runtime helper takes the
//! array as a borrow, so a call in the index or value that changes the object holding the array
//! (`set(&self.memory, slot, self.pop())`) would borrow `self` twice. Java evaluates the array
//! first, then the index and the value; the array's handle is copied into a local at that point,
//! so the write holds no borrow of the object while that code runs.

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

    /// The write `call`, after the binding of its array, if there is one, in a block.
    pub(super) fn bound_write(binding: Option<String>, call: String) -> String {
        match binding {
            Some(binding) => format!("{{ {binding} {call} }}"),
            None => call,
        }
    }
}
