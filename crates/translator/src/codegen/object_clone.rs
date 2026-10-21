//! `super.clone()` of `Object` in a `Cloneable` class (see `Builtin::ObjectClone`): a new object
//! of the same class, whose fields are read from the object as they are. A reference field is
//! shared by both objects, as Java shares it, because cloning a shared object clones its handle.

use super::*;

impl<'a> Gen<'a> {
    /// A copy of `object`, the object of a class that has no superclass and no subclasses, built
    /// in the representation that its class's objects have.
    pub(super) fn object_clone(&mut self, object: &'a Expr) -> Code {
        let class = self.class;
        let c = class.class_index(object.ty).expect("`clone` of an object of a class");
        let decl = &class.types[c];
        let mut fields = Vec::new();
        for f in &decl.fields {
            let value = self.field_read(object, &f.name, f.ty).text;
            fields.push(format!("{}: {value}", snake_case(&f.name)));
        }
        let value = format!("{} {{ {} }}", self.struct_of(c), fields.join(", "));
        Code::atom(if self.facts.owned.contains(&decl.name) {
            value
        } else if self.facts.immutable.contains(&decl.name) {
            format!("Rc::new({value})")
        } else {
            format!("Rc::new(RefCell::new({value}))")
        })
    }
}
