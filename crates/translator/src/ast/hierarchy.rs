//! The type hierarchy of the program: which classes extend which, which interfaces they
//! implement, and which methods override which. Every query a translation of inheritance needs
//! (subtypes, the concrete classes under a type, the implementation a call runs) is answered
//! here, from the declarations alone.

use super::*;
use std::collections::HashMap;

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
    /// Whether each method has no body.
    method_abstract: Vec<bool>,
    /// What each method overrides directly.
    overrides: Vec<Vec<usize>>,
    /// The override group of each method, as the smallest index in the group.
    group_ids: Vec<usize>,
    /// The methods of each group, in index order, by the group's id.
    groups: HashMap<usize, Vec<usize>>,
    /// The methods each type declares, in index order.
    by_class: Vec<Vec<usize>>,
    /// The methods of each group that each type declares, in index order, by group id and type.
    declared: HashMap<(usize, usize), Vec<usize>>,
    /// For each type, one bit per type that it is a subtype of (itself included), `words` words
    /// a type.
    supertype_bits: Vec<u64>,
    words: usize,
    /// The index of each type by its qualified name (the first, as `Class::type_index` finds it).
    names: HashMap<String, usize>,
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
        let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
        let mut declared: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
        let mut by_class = vec![Vec::new(); types.len()];
        for (m, method) in methods.iter().enumerate() {
            by_class[method.class].push(m);
            groups.entry(group_ids[m]).or_default().push(m);
            declared.entry((group_ids[m], method.class)).or_default().push(m);
        }
        let superclass: Vec<Option<usize>> = types.iter().map(|t| t.extends).collect();
        let interfaces: Vec<Vec<usize>> = types.iter().map(|t| t.implements.clone()).collect();
        let words = types.len().div_ceil(64);
        let mut supertype_bits = vec![0u64; types.len() * words];
        for t in 0..types.len() {
            let row = &mut supertype_bits[t * words..(t + 1) * words];
            let mut work = vec![t];
            while let Some(at) = work.pop() {
                let (word, bit) = (at / 64, 1u64 << (at % 64));
                if row[word] & bit == 0 {
                    row[word] |= bit;
                    work.extend(superclass[at].into_iter().chain(interfaces[at].iter().copied()));
                }
            }
        }
        let mut names = HashMap::new();
        for (t, decl) in types.iter().enumerate() {
            names.entry(decl.name.clone()).or_insert(t);
        }
        Hierarchy {
            subtypes,
            overriders,
            superclass,
            interfaces,
            concrete: types
                .iter()
                .map(|t| {
                    t.kind == TypeKind::Class && !t.is_abstract
                        || matches!(t.kind, TypeKind::Enum | TypeKind::Runtime)
                })
                .collect(),
            fields: types.iter().map(|t| t.fields.iter().map(|f| f.name.clone()).collect()).collect(),
            method_abstract: methods.iter().map(|m| m.is_abstract).collect(),
            overrides: methods.iter().map(|m| m.overrides.clone()).collect(),
            group_ids,
            groups,
            by_class,
            declared,
            supertype_bits,
            words,
            names,
        }
    }

    /// Whether type `a` is `b` or a subtype of it, through classes and interfaces.
    pub fn is_subtype(&self, a: usize, b: usize) -> bool {
        a == b || self.supertype_bits[a * self.words + b / 64] & (1u64 << (b % 64)) != 0
    }

    /// `t` and every type it is a subtype of, through classes and interfaces, each once.
    pub fn supertypes(&self, t: usize) -> Vec<usize> {
        let mut out = vec![t];
        let mut at = 0;
        while at < out.len() {
            let next = self.superclass[out[at]].into_iter().chain(self.interfaces[out[at]].iter().copied());
            for s in next {
                if !out.contains(&s) {
                    out.push(s);
                }
            }
            at += 1;
        }
        out
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
        let mut out: Vec<usize> = self.subtree(t).into_iter().filter(|&c| self.concrete[c]).collect();
        out.sort_unstable();
        out
    }

    /// `t` and its subtypes, each once, in no particular order.
    fn subtree(&self, t: usize) -> Vec<usize> {
        let mut seen = vec![false; self.subtypes.len()];
        seen[t] = true;
        let mut out = vec![t];
        let mut at = 0;
        while at < out.len() {
            for &s in &self.subtypes[out[at]] {
                if !std::mem::replace(&mut seen[s], true) {
                    out.push(s);
                }
            }
            at += 1;
        }
        out
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
        self.group_members(m).to_vec()
    }

    /// `group`, without a copy.
    pub fn group_members(&self, m: usize) -> &[usize] {
        self.groups.get(&self.group_ids[m]).map_or(&[], Vec::as_slice)
    }

    /// Whether some method overrides `m`.
    pub fn is_overridden(&self, m: usize) -> bool {
        !self.overriders[m].is_empty()
    }

    /// Makes the abstract method `m` concrete, as a census stubs one that an analysis rejects
    /// (`program/census.rs`).
    pub fn make_concrete(&mut self, m: usize) {
        self.method_abstract[m] = false;
    }

    /// The implementation an object of concrete class `c` runs for a call resolved to `m`: the
    /// member of `m`'s group declared nearest in `c`'s superclass chain, or else a default
    /// method of one of its interfaces. `None` when there is none in the program.
    pub fn implementation(&self, m: usize, c: usize) -> Option<usize> {
        let id = self.group_ids[m];
        let declared_in = |t: usize| {
            let declared = self.declared.get(&(id, t))?;
            declared.iter().copied().find(|&g| !self.method_abstract[g])
        };
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

    /// The methods that type `t` declares, in index order, as they were when the hierarchy was
    /// made.
    pub fn methods_of(&self, t: usize) -> &[usize] {
        &self.by_class[t]
    }

    /// The methods that `m` overrides directly.
    pub fn overridden(&self, m: usize) -> &[usize] {
        &self.overrides[m]
    }
}

impl Class {
    /// Whether method `m` is part of a class's static initialization: its `<clinit>`, or the
    /// functions that build the constants of an enum with tables (`lower/enum_tables.rs`), which
    /// only it calls. What it throws reaches the program as `ExceptionInInitializerError`.
    pub fn is_initializer(&self, m: usize) -> bool {
        let method = &self.methods[m];
        method.name == "<clinit>" || self.builds_constants(m)
    }

    /// Whether method `m` is a function that builds the constants of an enum with tables.
    pub fn builds_constants(&self, m: usize) -> bool {
        let method = &self.methods[m];
        let decl = &self.types[method.class];
        method.is_static
            && decl.kind == TypeKind::Enum
            && (method.name == constant_builder(&decl.name) || method.name == fields_builder(&decl.name))
    }

    /// The index of the type with this qualified name.
    pub fn type_index(&self, name: &str) -> Option<usize> {
        // The hierarchy's index, made from the types as they were when it was built.
        let found = match self.hierarchy.names.get(name) {
            Some(&t) if self.types.get(t).is_some_and(|d| d.name == name) => Some(t),
            Some(_) => self.types.iter().position(|t| t.name == name),
            None => None,
        };
        debug_assert_eq!(found, self.types.iter().position(|t| t.name == name), "type index of {name}");
        found
    }

    /// The type whose values are of type `ty`: a class or enum of the program, or a runtime class
    /// beside `Object` (a primitive stands for its box).
    pub fn type_of(&self, ty: Type) -> Option<usize> {
        match ty {
            Type::Class(name, _) | Type::Enum(name) => self.type_index(name),
            // An array that becomes an `Object` has a runtime class named after its type.
            Type::Array(_) => self.type_index(&ty.to_string()),
            Type::StringBuilder => self.type_index(STRING_BUILDER),
            _ => self.type_index(runtime_name(ty)?),
        }
    }

    /// The array type of runtime class `c`, when it is the class of arrays that become `Object`s
    /// (named after the type, `int[]` or `shapes.Point[][]`).
    pub fn array_of(&self, c: usize) -> Option<Type> {
        let decl = &self.types[c];
        if decl.kind != TypeKind::Runtime || !decl.name.ends_with("[]") {
            return None;
        }
        self.named_type(&decl.name)
    }

    /// The type of the shared handles of runtime class `c`, when its values are an array type's
    /// or `StringBuilder`'s: mutable objects whose identity an `Object` keeps.
    pub fn shared_of(&self, c: usize) -> Option<Type> {
        let decl = &self.types[c];
        if decl.kind == TypeKind::Runtime && decl.name == STRING_BUILDER {
            return Some(Type::StringBuilder);
        }
        self.array_of(c)
    }

    /// The type that `Type`'s text names: a primitive, a string, a box, an array, or a class or
    /// enum of the program.
    fn named_type(&self, name: &str) -> Option<Type> {
        if let Some(element) = name.strip_suffix("[]") {
            return Some(Type::array(self.named_type(element)?));
        }
        let primitives = [
            Type::Int,
            Type::Long,
            Type::Double,
            Type::Byte,
            Type::Short,
            Type::Char,
            Type::Float,
            Type::Boolean,
            Type::String,
            Type::StringBuilder,
        ];
        let known = primitives.into_iter().chain(runtime_values().into_iter().map(|(_, t)| t));
        if let Some(ty) = known.into_iter().find(|t| t.to_string() == name) {
            return Some(ty);
        }
        let t = self.type_index(name)?;
        let name = self.types[t].name.as_str();
        Some(if self.types[t].kind == TypeKind::Enum { Type::enum_type(name) } else { Type::class(name) })
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
