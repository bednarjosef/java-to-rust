//! The cast of an object of an enum to a class of `java.base` (a box, a string) that fails with
//! HotSpot's message, which names the object's class. The enum's variants give the class by a
//! `match`, so the message is written only where every class of the enum is of `java.base`: the
//! runtime's `cast_from` writes that form and nothing else.

use super::*;

impl<'a> Gen<'a> {
    /// The cast of `value`, an object of enum `s`, to `to`, failing with HotSpot's message
    /// (`java::cast_from`). `method` gives the object as a `to` (`as_long`), and `None` when no
    /// object of `s` is one, so that the cast always fails. `checked` is the form that a `catch`
    /// sees (`java::try_cast_from`). None where the message is not written: a class of `s` or
    /// `to` that is not of `java.base`.
    pub(super) fn enum_cast(
        &mut self,
        value: &'a Expr,
        s: usize,
        to: Type,
        method: Option<String>,
        checked: bool,
    ) -> Option<String> {
        let t = self.type_of(to)?;
        if !self.in_base(t) || self.enum_variants(s).into_iter().any(|c| !self.in_base(c)) {
            return None;
        }
        let runtime = self.runtime(if checked { "try_cast_from" } else { "cast_from" });
        let target = rust_string(&self.binary_name(t));
        let ty = self.ty_name(to);
        let enum_name = struct_name(&self.class.types[s].name);
        if self.nullable(value) {
            let option = self.object_place(value);
            let value = match method {
                Some(m) => format!("{enum_name}::{m}(o)"),
                None => format!("None::<{ty}>"),
            };
            let from = self.class_match(s, "o");
            return Some(format!("{option}.as_ref().map(|o| {runtime}({value}, {target}, || {from}))"));
        }
        let obj = self.object(value);
        let value = match method {
            Some(m) => format!("object.{m}()"),
            None => format!("None::<{ty}>"),
        };
        let from = self.class_match(s, "&object");
        Some(format!("{{ let object = &{obj}; {runtime}({value}, {target}, || {from}) }}"))
    }

    /// Whether the class `c` is one of `java.base`: a string or a box, or a standard class. An
    /// array is left out: its name in HotSpot's message is not checked against the JDK.
    pub(super) fn in_base(&self, c: usize) -> bool {
        let decl = &self.class.types[c];
        self.class.array_of(c).is_none()
            && decl.origin.java_names().is_none()
            && (decl.kind == TypeKind::Runtime || crate::lower::is_standard(&decl.name))
    }

    /// The binary name of the class of the object that `subject`, a reference to an object of
    /// enum `s`, holds, as a `match` over its variants. Every class of `s` is in `java.base`
    /// (see `enum_cast`).
    fn class_match(&self, s: usize, subject: &str) -> String {
        let arms: Vec<String> = self
            .enum_variants(s)
            .into_iter()
            .map(|c| format!("{}(_) => {}", self.variant(s, c), rust_string(&self.java_class_name(c, false))))
            .collect();
        format!("match {subject} {{ {} }}", arms.join(", "))
    }
}
