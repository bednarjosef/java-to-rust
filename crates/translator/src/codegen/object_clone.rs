//! `super.clone()` of `Object` (see `Builtin::ObjectClone`): a new object of the object's runtime
//! class, whose fields, its superclasses' in the `base` structs too, are read from the object as
//! they are. A reference field is shared by both objects, as Java shares it, because cloning a
//! shared object clones its handle. On the enum of a class tree the copy is made for each
//! variant; a variant whose class is not `Cloneable` throws `CloneNotSupportedException`.

use super::*;

impl<'a> Gen<'a> {
    /// A copy of `object`, `this` of the code being written, in the representation that the
    /// objects of its runtime class have; `id` is the copy's expression, which may fail.
    pub(super) fn object_clone(&mut self, object: &'a Expr, id: ExprId) -> Code {
        if let Some(t) = self.enum_of(object) {
            let obj = self.object(object);
            let mut arms = Vec::new();
            for c in self.enum_variants(t) {
                let (variant, pattern) = (self.variant(t, c), self.view_pattern(t, c));
                let arm = if self.class.types[c].cloneable {
                    let name = &self.class.types[c].name;
                    let celled = self.celled(name);
                    let (read, borrow) = if self.facts.immutable.contains(name) || celled {
                        ("o", "")
                    } else {
                        ("o", "let o = o.borrow(); ")
                    };
                    let literal = self.copy_literal(c, &|g, path, f, rust| {
                        let clone = if celled {
                            ".get()"
                        } else if g.clones(f.ty) {
                            ".clone()"
                        } else {
                            ""
                        };
                        format!("{read}.{path}{rust}{clone}")
                    });
                    format!("{pattern}(o) => {{ {borrow}{variant}({}) }}", self.copy_handle(c, literal))
                } else {
                    format!("{pattern}(_) => {}", self.clone_refused(c, id).text)
                };
                arms.push(arm);
            }
            return Code::atom(super::class_values::variant_match(&obj, &arms));
        }
        let c = self.struct_class(object).expect("`clone` of an object of a class");
        if !self.class.types[c].cloneable {
            return self.clone_refused(c, id);
        }
        let reads: Vec<(String, String)> = self
            .copied_fields(c)
            .into_iter()
            .map(|(name, ty)| {
                let value = self.field_read(object, &name, ty).text;
                (name, value)
            })
            .collect();
        let literal = self.copy_literal(c, &|_, _, f, _| {
            reads.iter().find(|(name, _)| *name == f.name).map(|(_, v)| v.clone()).unwrap_or_default()
        });
        Code::atom(self.copy_handle(c, literal))
    }

    /// The struct literal of an object of class `c`: each field's value is `read(path, field, rust)`,
    /// where `path` leads from the object's struct to the struct that holds the field (`base.`),
    /// the fields struct of a superclass that the struct embeds as `base`.
    fn copy_literal(&self, c: usize, read: &dyn Fn(&Self, &str, &Field, &str) -> String) -> String {
        self.literal_at(c, "", read)
    }

    fn literal_at(&self, k: usize, path: &str, read: &dyn Fn(&Self, &str, &Field, &str) -> String) -> String {
        let mut parts = Vec::new();
        if let Some(a) = hierarchy::embedded(self.facts, self.class, k) {
            parts.push(format!("base: {}", self.literal_at(a, &format!("{path}base."), read)));
        }
        let decl = &self.class.types[k];
        for f in &decl.fields {
            let rust = field_name(decl, &f.name);
            parts.push(format!("{rust}: {}", self.field_cell(k, f, read(self, path, f, &rust))));
        }
        format!("{} {{ {} }}", self.struct_of(k), parts.join(", "))
    }

    /// The fields of an object of class `c`, its superclasses' first, by Java name and type.
    fn copied_fields(&self, c: usize) -> Vec<(String, Type)> {
        let mut out = match hierarchy::embedded(self.facts, self.class, c) {
            Some(a) => self.copied_fields(a),
            None => Vec::new(),
        };
        out.extend(self.class.types[c].fields.iter().map(|f| (f.name.clone(), f.ty)));
        out
    }

    /// The struct `literal` of an object of class `c` as such an object: a plain value for an
    /// owned class, else behind its handle.
    fn copy_handle(&self, c: usize, literal: String) -> String {
        let name = &self.class.types[c].name;
        let pointer = self.pointer(c);
        if self.facts.owned.contains(name) {
            literal
        } else if self.celled(name) {
            self.celled_new(c, &literal)
        } else if self.facts.immutable.contains(name) {
            format!("{pointer}::new({literal})")
        } else {
            format!("{pointer}::new({}::new({literal}))", self.cell(c))
        }
    }

    /// The `CloneNotSupportedException` of a copy of an object of class `c`, which is not
    /// `Cloneable`: its message is the class's binary name.
    fn clone_refused(&mut self, c: usize, id: ExprId) -> Code {
        let name = rust_string(&self.java_class_name(c, false));
        self.may_fail(id, "clone_refused", "", &name)
    }
}
