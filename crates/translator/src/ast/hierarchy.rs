//! The type hierarchy of the program: which classes extend which, which interfaces they
//! implement, and which methods override which. Every query a translation of inheritance needs
//! (subtypes, the concrete classes under a type, the implementation a call runs) is answered
//! here, from the declarations alone.

use super::*;

/// The supertypes, subtypes and overrides of the program's types and methods.
#[derive(Default)]
pub struct Hierarchy {
    /// Direct subtypes of each type: classes extending it, interfaces extending it, classes
    /// implementing it.
    pub subtypes: Vec<Vec<usize>>,
    /// Methods that directly override each method.
    pub overriders: Vec<Vec<usize>>,
    /// The superclass of each type.
    superclass: Vec<Option<usize>>,
    /// The interfaces each type implements or extends directly.
    interfaces: Vec<Vec<usize>>,
    /// Whether each type can have values of exactly its own type: a class that is not abstract,
    /// or an enum (whose constants are its values; an enum has no subtypes).
    concrete: Vec<bool>,
    /// The instance fields each type declares, by name.
    fields: Vec<Vec<String>>,
    /// The class of each method, and whether it has a body.
    method_class: Vec<usize>,
    method_abstract: Vec<bool>,
    /// What each method overrides directly.
    overrides: Vec<Vec<usize>>,
    /// The override group of each method, as the smallest index in the group.
    group_ids: Vec<usize>,
}

impl Hierarchy {
    pub fn new(types: &[TypeDecl], methods: &[Method]) -> Hierarchy {
        let mut subtypes = vec![Vec::new(); types.len()];
        for (t, decl) in types.iter().enumerate() {
            for &s in decl.extends.iter().chain(&decl.implements) {
                subtypes[s].push(t);
            }
        }
        let mut overriders = vec![Vec::new(); methods.len()];
        for (m, method) in methods.iter().enumerate() {
            for &o in &method.overrides {
                overriders[o].push(m);
            }
        }
        // Groups: the connected parts of the override relation.
        let mut group_ids: Vec<usize> = (0..methods.len()).collect();
        fn find(ids: &mut [usize], m: usize) -> usize {
            let mut r = m;
            while ids[r] != r {
                r = ids[r];
            }
            ids[m] = r;
            r
        }
        for (m, method) in methods.iter().enumerate() {
            for &o in &method.overrides {
                let (a, b) = (find(&mut group_ids, m), find(&mut group_ids, o));
                let (lo, hi) = (a.min(b), a.max(b));
                group_ids[hi] = lo;
            }
        }
        for m in 0..methods.len() {
            let root = find(&mut group_ids, m);
            group_ids[m] = root;
        }
        Hierarchy {
            subtypes,
            overriders,
            superclass: types.iter().map(|t| t.extends).collect(),
            interfaces: types.iter().map(|t| t.implements.clone()).collect(),
            concrete: types
                .iter()
                .map(|t| {
                    t.kind == TypeKind::Class && !t.is_abstract
                        || matches!(t.kind, TypeKind::Enum | TypeKind::Runtime)
                })
                .collect(),
            fields: types.iter().map(|t| t.fields.iter().map(|f| f.name.clone()).collect()).collect(),
            method_class: methods.iter().map(|m| m.class).collect(),
            method_abstract: methods.iter().map(|m| m.is_abstract).collect(),
            overrides: methods.iter().map(|m| m.overrides.clone()).collect(),
            group_ids,
        }
    }

    /// Whether type `a` is `b` or a subtype of it, through classes and interfaces.
    pub fn is_subtype(&self, a: usize, b: usize) -> bool {
        a == b
            || self.superclass[a].is_some_and(|s| self.is_subtype(s, b))
            || self.interfaces[a].iter().any(|&i| self.is_subtype(i, b))
    }

    /// The superclass of `t`, if the program declares it.
    pub fn superclass(&self, t: usize) -> Option<usize> {
        self.superclass[t]
    }

    /// The superclass chain of `t`, nearest first, without `t`.
    pub fn ancestors(&self, t: usize) -> Vec<usize> {
        let mut out = Vec::new();
        let mut at = self.superclass[t];
        while let Some(s) = at {
            out.push(s);
            at = self.superclass[s];
        }
        out
    }

    /// Whether `t` has subtypes in the program.
    pub fn has_subtypes(&self, t: usize) -> bool {
        !self.subtypes[t].is_empty()
    }

    /// Whether `t` can have values of exactly its own type: a class that is not abstract, or an
    /// enum.
    pub fn is_concrete(&self, t: usize) -> bool {
        self.concrete[t]
    }

    /// The concrete classes that are `t` or subtypes of `t`, in `Class::types` order.
    pub fn concretes(&self, t: usize) -> Vec<usize> {
        (0..self.concrete.len()).filter(|&c| self.concrete[c] && self.is_subtype(c, t)).collect()
    }

    /// The root of the class tree of `t`: its topmost superclass in the program.
    pub fn root(&self, t: usize) -> usize {
        self.ancestors(t).last().copied().unwrap_or(t)
    }

    /// The concrete classes of the class tree of `t`, which decide immutability together.
    pub fn family(&self, t: usize) -> Vec<usize> {
        self.concretes(self.root(t))
    }

    /// The override group of `m`, as a representative method index shared by the group.
    pub fn group_id(&self, m: usize) -> usize {
        self.group_ids[m]
    }

    /// Every method in the override group of `m`: `m`, what it overrides, what overrides those,
    /// transitively.
    pub fn group(&self, m: usize) -> Vec<usize> {
        let id = self.group_ids[m];
        (0..self.group_ids.len()).filter(|&o| self.group_ids[o] == id).collect()
    }

    /// Whether some method overrides `m`.
    pub fn is_overridden(&self, m: usize) -> bool {
        !self.overriders[m].is_empty()
    }

    /// The implementation an object of concrete class `c` runs for a call resolved to `m`: the
    /// member of `m`'s group declared nearest in `c`'s superclass chain, or else a default
    /// method of one of its interfaces. `None` when there is none in the program.
    pub fn implementation(&self, m: usize, c: usize) -> Option<usize> {
        let group = self.group(m);
        let declared_in =
            |t: usize| group.iter().copied().find(|&g| self.method_class[g] == t && !self.method_abstract[g]);
        let mut chain = vec![c];
        chain.extend(self.ancestors(c));
        if let Some(found) = chain.iter().find_map(|&t| declared_in(t)) {
            return Some(found);
        }
        // Default methods, from the interfaces nearest to the class.
        let mut work: Vec<usize> = chain.iter().flat_map(|&t| self.interfaces[t].clone()).collect();
        let mut seen = Vec::new();
        while !work.is_empty() {
            let mut next = Vec::new();
            for i in work {
                if seen.contains(&i) {
                    continue;
                }
                seen.push(i);
                if let Some(found) = declared_in(i) {
                    return Some(found);
                }
                next.extend(self.interfaces[i].iter().copied());
            }
            work = next;
        }
        None
    }

    /// The implementations that a call resolved to `m` on a receiver of static type `t` may run.
    pub fn targets(&self, m: usize, t: usize) -> Vec<usize> {
        let mut out: Vec<usize> = Vec::new();
        for c in self.concretes(t) {
            if let Some(i) = self.implementation(m, c)
                && !out.contains(&i)
            {
                out.push(i);
            }
        }
        out
    }

    /// The class that declares the instance field `name` as seen from type `t`: `t` or the
    /// nearest superclass that declares it.
    pub fn field_owner(&self, t: usize, name: &str) -> usize {
        let mut at = t;
        loop {
            if self.fields[at].iter().any(|f| f == name) {
                return at;
            }
            match self.superclass[at] {
                Some(s) => at = s,
                None => return t,
            }
        }
    }

    /// The methods that `m` overrides directly.
    pub fn overridden(&self, m: usize) -> &[usize] {
        &self.overrides[m]
    }
}

impl Class {
    /// The index of the type with this qualified name.
    pub fn type_index(&self, name: &str) -> Option<usize> {
        self.types.iter().position(|t| t.name == name)
    }

    /// The type whose values are of type `ty`: a class or enum of the program, or a runtime class
    /// beside `Object` (a primitive stands for its box).
    pub fn type_of(&self, ty: Type) -> Option<usize> {
        match ty {
            Type::Class(name, _) | Type::Enum(name) => self.type_index(name),
            _ => self.type_index(runtime_name(ty)?),
        }
    }

    /// The type that a class type names.
    pub fn class_index(&self, ty: Type) -> Option<usize> {
        match ty {
            Type::Class(name, _) => self.type_index(name),
            _ => None,
        }
    }

    /// The class that declares the field `name` of an object of type `ty`.
    pub fn field_owner(&self, ty: Type, name: &str) -> Option<usize> {
        self.class_index(ty).map(|t| self.hierarchy.field_owner(t, name))
    }

    /// The qualified name of the class that declares the field `name` of an object of type `ty`.
    pub fn field_owner_name(&self, ty: Type, name: &str) -> Option<&str> {
        self.field_owner(ty, name).map(|t| self.types[t].name.as_str())
    }

    /// The methods and constructors a call expression may run: every implementation of a
    /// virtual call, the resolved method of any other call; empty for anything else.
    pub fn callees(&self, e: &Expr) -> Vec<usize> {
        match &e.kind {
            ExprKind::MethodCall { receiver, resolved: Some(m), is_super, .. } => {
                if *is_super || self.methods[*m].is_static {
                    return vec![*m];
                }
                match self.class_index(receiver.ty) {
                    Some(t) => {
                        let targets = self.hierarchy.targets(*m, t);
                        if targets.is_empty() { vec![*m] } else { targets }
                    }
                    None => vec![*m],
                }
            }
            ExprKind::Call { resolved: Some(m), .. }
            | ExprKind::New { ctor: Some(m), .. }
            | ExprKind::ThisCall { resolved: Some(m), .. }
            | ExprKind::SuperInit { resolved: Some(m), .. } => vec![*m],
            _ => Vec::new(),
        }
    }
}
