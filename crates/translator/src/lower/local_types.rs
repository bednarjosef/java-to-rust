//! Records, enums and interfaces declared in a method body or an initializer block.
//! `javac::records` moves them to the top level of their unit, where they capture nothing, so they
//! are ordinary classes named by their package and simple name (`app.Point`). Two of them can
//! share that name, or a top-level type can have it, so one that clashes is named after its method
//! as well (`app.PointInRun`, `app.PointInInit` for a block), as a local class is (see `inner`).

use super::*;

impl<'t> Lowering<'t> {
    /// Names the hoisted types that clash with another type's name (see the module's comment).
    pub(super) fn name_hoisted(&mut self) {
        for t in 0..self.types.len() {
            if !self.is_hoisted(t) {
                continue;
            }
            let base = self.types[t].name.clone();
            let method = self.hoisted_method(t);
            let taken = |name: &str, lowering: &Self| {
                let flat = name.replace('.', "");
                lowering
                    .types
                    .iter()
                    .enumerate()
                    .any(|(o, other)| o != t && other.name.replace('.', "") == flat)
            };
            let mut name = base.clone();
            if taken(&name, self) {
                name = format!("{base}In{method}");
            }
            let first = name.clone();
            let mut n = 2;
            while taken(&name, self) {
                name = format!("{first}{n}");
                n += 1;
            }
            self.types[t].name = name;
            self.indices.get_mut().forget_types();
        }
    }

    /// Whether class `t` is a record, enum or interface that was declared in a method body or an
    /// initializer block. Its symbol's owner is that method's signature (`run(int)`), or `{}` or
    /// `static {}` for a block, where a member's is a class.
    pub(super) fn is_hoisted(&self, t: usize) -> bool {
        self.types[t].nested.is_none()
            && self.types[t].node.symbol().is_some_and(|s| s.owner.contains('(') || s.owner.ends_with("{}"))
    }

    /// The method a hoisted class is declared in, capitalized as in `place_name`.
    fn hoisted_method(&self, t: usize) -> String {
        let owner = self.types[t].node.symbol().map_or("", |s| s.owner);
        let name = owner.split('(').next().unwrap_or_default();
        let name = match name {
            "<init>" => "new",
            // An initializer block, as `place_name` names it.
            _ if name.ends_with("{}") => "init",
            _ => name,
        };
        let mut chars = name.chars();
        chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
    }
}
