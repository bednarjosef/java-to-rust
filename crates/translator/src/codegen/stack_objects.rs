//! Objects on the stack: a `new` whose object never leaves its method (`Facts::stack_objects`)
//! makes it in a `java::Slot` that the method declares at its start, `slot_1.put(..)`, rather
//! than in the collected heap, `Arc::new(..)`. The handle is the same `java::Arc`, so nothing
//! else in the code changes (docs/collector.md, "Objects on the stack").
//!
//! A constructor that makes its object itself (one that uses `this` as a value, docs/object-model.md
//! 1.4) gets a second form, `new_on_stack(obj_slot, ..)`, which makes it in the slot it is given.

use super::*;

/// The most slots a method declares, so that a method that recursion runs deep keeps a small
/// frame.
const SLOTS_PER_METHOD: usize = 8;

/// The most fields (with those of its superclasses) of an object that goes in a slot.
const FIELDS_PER_SLOT: usize = 16;

impl<'a> Gen<'a> {
    /// Whether objects of class `c` can be made in a slot: a class whose objects are handles
    /// (not owned plain values or closures), with a struct that is not a Rust generic, and not
    /// too large for a frame.
    pub(super) fn stack_class(&self, c: usize) -> bool {
        let decl = &self.class.types[c];
        let fields: usize = std::iter::once(c)
            .chain(self.class.hierarchy.ancestors(c))
            .map(|t| self.class.types[t].fields.len())
            .sum();
        decl.kind == TypeKind::Class
            && !decl.is_abstract
            && decl.type_params.is_empty()
            && !self.facts.owned.contains(&decl.name)
            && !decl.implements.iter().any(|i| self.facts.closure_types.contains(i))
            && fields <= FIELDS_PER_SLOT
    }

    /// The `new`s of method `m` whose objects it makes in slots, with their classes, in the order
    /// of the code.
    pub(super) fn stack_sites(&self, m: usize) -> Vec<(ExprId, usize)> {
        if self.facts.stack_objects.is_empty() {
            return Vec::new();
        }
        let mut sites = Vec::new();
        for s in &self.class.methods[m].body {
            visit_stmt(
                s,
                &mut |e| {
                    visit_expr(e, &mut |x| {
                        if let ExprKind::New { class: c, ctor: Some(_), .. } = x.kind
                            && self.facts.stack_objects.contains(&x.id)
                            && self.stack_class(c)
                            && sites.len() < SLOTS_PER_METHOD
                        {
                            sites.push((x.id, c));
                        }
                    })
                },
                &mut |_| {},
            );
        }
        sites
    }

    /// The type of the value that a slot holds for an object of class `c`: what its handle
    /// points to.
    pub(super) fn slot_content(&self, c: usize) -> String {
        let name = &self.class.types[c].name;
        let structure = self.struct_of(c);
        if self.celled(name) && self.monitored(name) {
            format!("java::Monitored<{structure}>")
        } else if self.celled(name) || self.facts.immutable.contains(name) {
            structure
        } else {
            format!("{}<{structure}>", self.cell(c))
        }
    }

    /// The constructors that make their object themselves and that a `new` on the stack calls,
    /// directly or through `this(..)`: each gets a form that makes it in a slot.
    pub(super) fn stack_constructors(&self) -> HashSet<usize> {
        let mut found = HashSet::new();
        let mut work = Vec::new();
        for m in 0..self.class.methods.len() {
            for (id, c) in self.stack_sites(m) {
                if self.builds_plain(&self.class.types[c].name) {
                    continue;
                }
                let mut ctor = None;
                for s in &self.class.methods[m].body {
                    visit_stmt(
                        s,
                        &mut |e| {
                            visit_expr(e, &mut |x| {
                                if x.id == id
                                    && let ExprKind::New { ctor: Some(k), .. } = x.kind
                                {
                                    ctor = Some(k);
                                }
                            })
                        },
                        &mut |_| {},
                    );
                }
                work.extend(ctor);
            }
        }
        while let Some(k) = work.pop() {
            if !found.insert(k) {
                continue;
            }
            if let Some(Stmt::Expr(Expr { kind: ExprKind::ThisCall { resolved: Some(d), .. }, .. })) =
                explicit_call(&self.class.methods[k].body).map(|i| &self.class.methods[k].body[i])
            {
                work.push(*d);
            }
        }
        found
    }

    /// Names the slots of the `new`s `sites` of the method being written, distinct from its
    /// locals and the free functions, and the slot parameter of a constructor's form on the
    /// stack.
    pub(super) fn name_slots(&mut self, sites: &[(ExprId, usize)]) {
        let taken: HashSet<String> = self.locals.values().cloned().collect();
        let free = |name: &str| !taken.contains(name) && !self.static_fn_names.contains(name);
        let mut next = 1;
        let mut slots = HashMap::new();
        for &(id, c) in sites {
            let name = loop {
                let name = format!("slot_{next}");
                next += 1;
                if free(&name) {
                    break name;
                }
            };
            slots.insert(id, (name, c));
        }
        let mut param = "obj_slot".to_string();
        while !free(&param) {
            param.push('_');
        }
        self.slots = slots;
        self.slot_param = param;
    }

    /// Declares the slots named by `name_slots`, at the start of the method's body.
    pub(super) fn declare_slots(&mut self, sites: &[(ExprId, usize)]) {
        for &(id, c) in sites {
            let name = self.slots[&id].0.clone();
            let content = self.slot_content(c);
            self.line(&format!("let {name} = java::Slot::<{content}>::new();"));
        }
    }

    /// The slot that the `new` `id` makes its object in, if it is on the stack.
    pub(super) fn slot_of(&self, id: ExprId) -> Option<String> {
        self.slots.get(&id).map(|(name, _)| name.clone())
    }
}
