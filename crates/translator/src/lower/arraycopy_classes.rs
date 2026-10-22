//! `System.arraycopy` between arrays of references to instances of two classes. Lowering keeps
//! the call as it is: the arrays' element types may differ in representation (a leaf class's
//! `Option<Rc<_>>`, an enum over several classes, or `Object`), so the code generator converts
//! each element and checks it when the destination's class may refuse one
//! (`codegen/arraycopy_classes.rs`). Arrays of `String`, boxes, enums and arrays are not lowered
//! this way.

use crate::ast::Type;

/// Whether `src` and `dest` are arrays of references to instances of classes.
pub(super) fn class_elements(src: Type, dest: Type) -> bool {
    matches!((&src, &dest), (Type::Array(s), Type::Array(d)) if matches!((&**s, &**d), (Type::Class(..), Type::Class(..))))
}
