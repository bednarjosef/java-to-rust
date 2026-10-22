//! Long chains of objects. When an object dies, Rust drops its fields, so freeing a linked list
//! of a million nodes would recurse a million frames deep through the `next` fields and overflow
//! the stack, where Java's collector has no such limit. A class whose objects can lead to objects
//! of the same class, through its own fields or through other classes (a cycle of classes), gets
//! an `Unlink` implementation that moves the fields that lead back out of a dying object, and a
//! `Drop` that frees what they lead to one object at a time on a heap stack, once the drops
//! use more stack than a shallow structure needs (runtime `chains.rs`):
//!
//! ```text
//! impl<'a> java::Unlink<'a> for Node {
//!     fn unlink(&mut self, chain: &mut java::Chain<'a>) {
//!         java::Link::unlink_in(&mut self.next, chain);
//!     }
//! }
//!
//! impl Drop for Node {
//!     fn drop(&mut self) {
//!         java::drop_links(self);
//!     }
//! }
//! ```
//!
//! The runtime implements `Link` for handles, `Option`s and arrays, so a field is unlinked the
//! same way whatever its form. The enum of a class type whose objects can be on a chain gets a
//! `Link` of its own, which matches the variant. A class that no cycle of field types goes
//! through is written as before.

use super::*;
use crate::analysis::closure::{Bits, reached_union};
use std::collections::BTreeSet;

/// Which structs and enums take part in chains, computed once per program.
#[derive(Default)]
pub(super) struct Chains {
    /// For each type with a struct, the fields it declares itself that may lead back to an
    /// object of a chain it is on: the fields its `Unlink` moves out (and the reason for its `Drop`).
    links: Vec<Vec<usize>>,
    /// For each type with a struct, whether it implements `Unlink`: it has links of its own, or
    /// the fields struct it embeds does.
    unlinks: Vec<bool>,
    /// The types whose enums implement `Link`: the enums of link fields.
    enums: BTreeSet<usize>,
}

impl<'a> Gen<'a> {
    /// Finds the classes on cycles of field types, and the fields that close those cycles.
    pub(super) fn find_chains(&self) -> Chains {
        let class = self.class;
        let n = class.types.len();
        let objects = |t: usize| self.live[t] && class.types[t].kind == TypeKind::Class;
        // The fields that each object of concrete class `c` has: its own and those it embeds.
        let fields = |c: usize| {
            let mut out: Vec<(usize, usize)> = (0..class.types[c].fields.len()).map(|f| (c, f)).collect();
            for a in class.hierarchy.ancestors(c) {
                out.extend((0..class.types[a].fields.len()).map(|f| (a, f)));
            }
            out
        };
        // Which classes the objects of each concrete class hold directly.
        let mut edges: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); n];
        for c in (0..n).filter(|&c| objects(c) && class.hierarchy.is_concrete(c)) {
            for (t, f) in fields(c) {
                self.held_classes(class.types[t].fields[f].ty, &mut edges[c]);
            }
        }
        // `reach[c]`: the classes reachable from `c` by one field or more.
        let next: Vec<Vec<usize>> = edges.iter().map(|e| e.iter().copied().collect()).collect();
        let own: Vec<Bits> = edges.iter().map(|e| e.iter().copied().collect()).collect();
        let reach = reached_union(&next, &own);
        let mut chains =
            Chains { links: vec![Vec::new(); n], unlinks: vec![false; n], enums: BTreeSet::new() };
        for (t, decl) in class.types.iter().enumerate() {
            if !objects(t) {
                continue;
            }
            for (f, field) in decl.fields.iter().enumerate() {
                let mut held = BTreeSet::new();
                self.held_classes(field.ty, &mut held);
                // A field of `t` leads back when an object it holds can reach an object of a
                // class that has the field.
                let closes = class
                    .hierarchy
                    .concretes(t)
                    .into_iter()
                    .filter(|&c| objects(c))
                    .any(|c| held.iter().any(|&d| d == c || reach[d].contains(c)));
                if closes {
                    chains.links[t].push(f);
                    self.held_enums(field.ty, &mut chains.enums);
                }
            }
        }
        // A struct unlinks when it or a fields struct it embeds has links; the embedded struct
        // comes first in the ancestors, so walk them from the root down.
        for t in 0..n {
            let mut line = class.hierarchy.ancestors(t);
            line.reverse();
            line.push(t);
            chains.unlinks[t] = line.iter().any(|&a| !chains.links[a].is_empty()) && objects(t);
        }
        chains
    }

    /// The concrete classes whose objects a value of type `ty` holds directly: the class an
    /// object type stands for, the variants of an enum of a class type, or the classes of an
    /// array's elements. A type variable of a Rust generic holds none that is known here.
    fn held_classes(&self, ty: Type, out: &mut BTreeSet<usize>) {
        match ty {
            Type::Array(element) => self.held_classes(*element, out),
            Type::Class(..) => {
                let Some(t) = self.class.class_index(ty) else { return };
                if self.class.types[t].kind == TypeKind::Param {
                    return;
                }
                match self.rep(t) {
                    hierarchy::Rep::Object(c) => {
                        out.insert(c);
                    }
                    hierarchy::Rep::Enum(e) => {
                        out.extend(self.enum_variants(e).into_iter().filter(|&c| !self.holds_value(c)));
                    }
                }
            }
            _ => {}
        }
    }

    /// The types whose enums a value of type `ty` is, directly or as an array's element.
    fn held_enums(&self, ty: Type, out: &mut BTreeSet<usize>) {
        match ty {
            Type::Array(element) => self.held_enums(*element, out),
            Type::Class(..) => {
                if let Some(e) = self.enum_type(ty) {
                    out.insert(e);
                }
            }
            _ => {}
        }
    }

    /// The `Unlink` and `Drop` of the struct of type `t`, when its objects can be on a chain.
    pub(super) fn chain_items(&mut self, t: usize) {
        if !self.chains.unlinks[t] {
            return;
        }
        let decl = &self.class.types[t];
        let names: Vec<String> = decl.type_params.iter().map(|p| struct_name(&p.to_string())).collect();
        let args = if names.is_empty() { String::new() } else { format!("<{}>", names.join(", ")) };
        let bounded: String = names.iter().map(|n| format!(", {n}: 'a")).collect();
        let name = self.struct_of(t);
        let (unlink, link, drop_links) =
            (self.runtime("Unlink"), self.runtime("Link"), self.runtime("drop_links"));
        let links: Vec<String> =
            self.chains.links[t].iter().map(|&f| field_name(decl, &decl.fields[f].name)).collect();
        self.gap();
        self.line(&format!("impl<'a{bounded}> {unlink}<'a> for {name}{args} {{"));
        self.indent += 1;
        self.line("fn unlink(&mut self, chain: &mut java::Chain<'a>) {");
        self.indent += 1;
        if let Some(base) = hierarchy::embedded(self.facts, self.class, t)
            && self.chains.unlinks[base]
        {
            self.line(&format!("{unlink}::unlink(&mut self.base, chain);"));
        }
        for field in &links {
            self.line(&format!("{link}::unlink_in(&mut self.{field}, chain);"));
        }
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.line("}");
        if links.is_empty() {
            return;
        }
        let params = if names.is_empty() { String::new() } else { format!("<{}>", names.join(", ")) };
        self.gap();
        self.line(&format!("impl{params} Drop for {name}{args} {{"));
        self.indent += 1;
        self.line("fn drop(&mut self) {");
        self.indent += 1;
        self.line(&format!("{drop_links}(self);"));
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.line("}");
    }

    /// The `Link` of the enum of type `t`, when it is the type of a field that leads along a
    /// chain: each variant whose objects can be on a chain is unlinked as its handle is.
    pub(super) fn chain_enum_items(&mut self, t: usize, variants: &[usize]) {
        if !self.chains.enums.contains(&t) {
            return;
        }
        let linked: Vec<usize> = variants.iter().copied().filter(|&c| self.chains.unlinks[c]).collect();
        if linked.is_empty() {
            return;
        }
        let rest = linked.len() < variants.len();
        let name = struct_name(&self.class.types[t].name);
        let link = self.runtime("Link");
        let paths: Vec<String> = linked.iter().map(|&c| self.variant(t, c)).collect();
        // An object that threads share is an `Arc`, which the chain keeps apart.
        let pushes: Vec<&str> = linked
            .iter()
            .map(|&c| if self.pointer(c) == "Arc" { "chain.push_shared(o)" } else { "chain.push(o)" })
            .collect();
        self.gap();
        self.line(&format!("impl<'a> {link}<'a> for {name} {{"));
        self.indent += 1;
        let unlink_in = format!("{link}::unlink_in(o, chain)");
        for (signature, calls) in [
            ("unlink_in(&mut self, chain: &mut java::Chain<'a>)", vec![unlink_in.as_str(); paths.len()]),
            ("into_chain(self, chain: &mut java::Chain<'a>)", pushes.clone()),
        ] {
            self.line(&format!("fn {signature} {{"));
            self.indent += 1;
            if let [path] = &paths[..]
                && rest
            {
                self.line(&format!("if let {path}(o) = self {{"));
                self.indent += 1;
                self.line(&format!("{};", calls[0]));
                self.indent -= 1;
                self.line("}");
                self.indent -= 1;
                self.line("}");
                continue;
            }
            self.line("match self {");
            self.indent += 1;
            for (path, call) in paths.iter().zip(&calls) {
                self.line(&format!("{path}(o) => {call},"));
            }
            if rest {
                self.line("_ => {}");
            }
            self.indent -= 1;
            self.line("}");
            self.indent -= 1;
            self.line("}");
        }
        self.indent -= 1;
        self.line("}");
    }
}
