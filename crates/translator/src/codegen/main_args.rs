//! `main`'s `args` in generated code: the command-line arguments, which the runtime reads. The
//! array is built where it is read, so it is an array no other name can see, and it is a plain
//! `Vec` when the analysis makes `args` plain (see `plain_arrays`).

use super::*;
use crate::codegen::arrays::element;

impl<'a> Gen<'a> {
    /// The command-line arguments as a `Vec` of the elements of the array type `ty`: each one is a
    /// string, wrapped in `Some` where the elements of arrays of that type may be null.
    pub(super) fn main_args_vec(&mut self, ty: Type) -> String {
        let args = format!("{}()", self.runtime("main_args"));
        // A `String[]` that is an `Object[]` (see `lower::covariant`).
        if element(ty) != Type::String {
            let each = self.string_as_object("s");
            let each = if self.nullable_element(element(ty)) { format!("Some({each})") } else { each };
            return format!("{args}.into_iter().map(|s| {each}).collect::<Vec<_>>()");
        }
        if self.nullable_element(element(ty)) {
            format!("{args}.into_iter().map(Some).collect::<Vec<_>>()")
        } else {
            args
        }
    }

    /// The command-line arguments as an array of type `ty` (`from_vec(main_args())`).
    pub(super) fn main_args(&mut self, ty: Type) -> Code {
        let args = self.main_args_vec(ty);
        Code::atom(format!("{}({args})", self.array_maker("from_vec")))
    }
}
