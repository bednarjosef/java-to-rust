//! Which expressions of the program may be a string or a box held as an object: a value of
//! `Object` (or of `Comparable`, `Number` or `CharSequence`) that came from a string, a box or a
//! primitive. A string or a box in an `Object` is a plain value whose identity the translation
//! does not keep, so `==` on two such values is rejected (`patterns::check_object_values`);
//! where this analysis shows that one side cannot be one, `==` is plain identity.
//!
//! The analysis is a points-to analysis that follows two things together until nothing changes:
//! whether a place may hold a string or a box, and which of the program's classes its objects
//! may be of. It follows them through variables, parameters, return values, fields, statics and
//! array elements (by element type). The classes decide which implementations a call runs, and
//! each instance method is followed once for every class whose objects run it, with `this` of
//! that class: an `IdentitySet` that inherits `Set`'s `add` and overrides its `contains` then
//! gets only what is added to identity sets, though other sets hold boxes. Anything it does not
//! follow (a library call's result, a variable bound by a construct it does not model, a
//! parameter of a method the runtime itself may call) may be anything.
//!
//! An expression of one of the program's own classes is never a string or a box, whatever it
//! reads: the translation's type for it holds none (a value read through an erased type variable
//! is cast to the class first).

use super::Facts;
use crate::ast::*;
use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap, HashSet};

/// The context of code that has no `this`: a static method, or a method that no class runs.
const STATIC: usize = usize::MAX;

/// The types whose values include strings or boxes: `Object` and the interfaces and classes of
/// the JDK that strings or boxes implement.
const HOLDING_VALUES: [&str; 10] = [
    OBJECT,
    COMPARABLE,
    NUMBER,
    CHAR_SEQUENCE,
    "java.lang.Object",
    "java.lang.Comparable",
    "java.lang.Number",
    "java.lang.CharSequence",
    "java.io.Serializable",
    "java.lang.constant.Constable",
];

/// What a place or an expression may hold.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
struct Flow {
    /// A string, a box or a primitive held as an object.
    value: bool,
    /// Objects of any class its type allows; `classes` lists them otherwise.
    any: bool,
    classes: BTreeSet<usize>,
}

impl Flow {
    fn top() -> Flow {
        Flow { value: true, any: true, classes: BTreeSet::new() }
    }

    fn of(class: usize) -> Flow {
        Flow { classes: BTreeSet::from([class]), ..Flow::default() }
    }

    /// Adds what `other` may hold; whether that changed anything.
    fn join(&mut self, other: &Flow) -> bool {
        let before = (self.value, self.any, self.classes.len());
        self.value |= other.value;
        self.any |= other.any;
        if !self.any {
            self.classes.extend(other.classes.iter().copied());
        }
        before != (self.value, self.any, self.classes.len())
    }
}

/// What the analysis found: the places that may hold a string or a box, and the classes of
/// their objects.
pub struct ObjectValues {
    /// The variables of each method in each of its contexts.
    vars: HashMap<(usize, usize), HashMap<String, Flow>>,
    returns: HashMap<(usize, usize), Flow>,
    /// Instance fields, by the class that declares them.
    fields: HashMap<(usize, String), Flow>,
    statics: HashMap<(usize, String), Flow>,
    elements: HashMap<Type, Flow>,
    /// The variables of each method that the analysis follows: its parameters (unless the
    /// runtime may call it) and the variables it declares, loops over or binds with a pattern.
    followed: Vec<HashSet<String>>,
    /// The classes whose objects run each method, `STATIC` for one without `this`.
    contexts: Vec<Vec<usize>>,
    types: Types,
}

/// What the analysis needs to know of the program's types.
struct Types {
    index: HashMap<String, usize>,
    /// Types no string or box is a value of.
    excluded: Vec<bool>,
    /// Types that objects of every class may be values of: `Object` and its kin, and type
    /// variables.
    open: Vec<bool>,
    /// Every supertype of each type, itself included.
    supers: Vec<HashSet<usize>>,
    /// The concrete classes that are each type or its subtypes.
    concretes: Vec<Vec<usize>>,
    all_concretes: Vec<usize>,
    /// The method with a body that each type declares for an override group.
    declared: HashMap<(usize, usize), usize>,
    implementations: RefCell<HashMap<(usize, usize), Option<usize>>>,
}

impl Types {
    fn new(class: &Class) -> Types {
        let n = class.types.len();
        let index = class.types.iter().enumerate().map(|(t, d)| (d.name.clone(), t)).collect();
        let open: Vec<bool> = class
            .types
            .iter()
            .map(|d| d.kind == TypeKind::Param || HOLDING_VALUES.contains(&d.name.as_str()))
            .collect();
        let excluded = class
            .types
            .iter()
            .enumerate()
            .map(|(t, d)| {
                !open[t] && matches!(d.kind, TypeKind::Class | TypeKind::Interface | TypeKind::Enum)
            })
            .collect();
        let mut supers = vec![HashSet::new(); n];
        for (t, set) in supers.iter_mut().enumerate() {
            let mut work = vec![t];
            while let Some(s) = work.pop() {
                if set.insert(s) {
                    let d = &class.types[s];
                    work.extend(d.extends.iter().chain(&d.implements));
                }
            }
        }
        let all_concretes: Vec<usize> = (0..n).filter(|&t| class.hierarchy.is_concrete(t)).collect();
        let mut concretes = vec![Vec::new(); n];
        for &c in &all_concretes {
            for &s in &supers[c] {
                concretes[s].push(c);
            }
        }
        let mut declared = HashMap::new();
        for (m, method) in class.methods.iter().enumerate() {
            if !method.is_abstract && !method.is_static && !method.is_ctor {
                declared.insert((method.class, class.hierarchy.group_id(m)), m);
            }
        }
        Types {
            index,
            excluded,
            open,
            supers,
            concretes,
            all_concretes,
            declared,
            implementations: RefCell::default(),
        }
    }

    /// The implementation that an object of class `c` runs for a call of a method of override
    /// group `group`: the nearest declaration in its superclass chain, then in its interfaces
    /// (as `Hierarchy::implementation`, remembered).
    fn implementation(&self, class: &Class, group: usize, c: usize) -> Option<usize> {
        if let Some(&found) = self.implementations.borrow().get(&(group, c)) {
            return found;
        }
        let mut chain = vec![c];
        chain.extend(class.hierarchy.ancestors(c));
        let mut found = chain.iter().find_map(|&t| self.declared.get(&(t, group)).copied());
        let mut work: Vec<usize> =
            chain.iter().flat_map(|&t| class.types[t].implements.iter().copied()).collect();
        let mut seen = HashSet::new();
        while found.is_none() && !work.is_empty() {
            let mut next = Vec::new();
            for i in work {
                if seen.insert(i) {
                    found = found.or_else(|| self.declared.get(&(i, group)).copied());
                    next.extend(class.types[i].implements.iter().copied());
                }
            }
            work = next;
        }
        self.implementations.borrow_mut().insert((group, c), found);
        found
    }

    /// The concrete classes whose objects a value of type `ty` may be.
    fn concretes_of(&self, ty: Type) -> &[usize] {
        match ty {
            Type::Class(name, _) | Type::Enum(name) => match self.index.get(name) {
                Some(&t) if !self.open[t] => &self.concretes[t],
                _ => &self.all_concretes,
            },
            _ => &[],
        }
    }

    /// What a value of type `ty` may hold, of what `flow` says.
    fn filter(&self, mut flow: Flow, ty: Type) -> Flow {
        match ty {
            Type::Class(name, _) => {
                if let Some(&t) = self.index.get(name) {
                    if self.excluded[t] {
                        flow.value = false;
                    }
                    if !self.open[t] {
                        flow.classes.retain(|&c| self.supers[c].contains(&t));
                    }
                }
                flow
            }
            Type::Enum(_) => Flow { any: true, ..Flow::default() },
            Type::Null | Type::Void | Type::Array(_) | Type::StringBuilder => Flow::default(),
            _ => Flow { value: true, ..Flow::default() },
        }
    }
}

impl ObjectValues {
    /// Whether `l` and `r`, the sides of a comparison in method `m`, may both be strings or boxes
    /// held as objects, on an object of some class that runs `m`.
    pub fn may_both_be_values(&self, class: &Class, m: usize, l: &Expr, r: &Expr) -> bool {
        self.contexts[m]
            .iter()
            .any(|&ctx| self.eval(class, m, ctx, l).value && self.eval(class, m, ctx, r).value)
    }

    fn var(&self, m: usize, ctx: usize, name: &str) -> Flow {
        if !self.followed.get(m).is_some_and(|f| f.contains(name)) {
            return Flow::top();
        }
        self.vars.get(&(m, ctx)).and_then(|v| v.get(name)).cloned().unwrap_or_default()
    }

    /// What `e`, evaluated in method `m` on an object of class `ctx`, may be.
    fn eval(&self, class: &Class, m: usize, ctx: usize, e: &Expr) -> Flow {
        let flow = match &e.kind {
            ExprKind::Null | ExprKind::NewArray(_) | ExprKind::ArrayLit(_) => Flow::default(),
            ExprKind::This if ctx == STATIC => Flow::top(),
            ExprKind::This => Flow::of(ctx),
            ExprKind::New { class: c, .. } => Flow::of(*c),
            ExprKind::Var(name) => self.var(m, ctx, name),
            ExprKind::Assign { op: None, value, .. } | ExprKind::Cast(_, value) => {
                self.eval(class, m, ctx, value)
            }
            ExprKind::Conditional(_, a, b) => {
                let mut flow = self.eval(class, m, ctx, a);
                flow.join(&self.eval(class, m, ctx, b));
                flow
            }
            ExprKind::Field { object, name } => match class.field_owner(object.ty, name) {
                Some(owner) => self.fields.get(&(owner, name.clone())).cloned().unwrap_or_default(),
                None => Flow::top(),
            },
            ExprKind::Static { class: c, name } => {
                self.statics.get(&(*c, name.clone())).cloned().unwrap_or_default()
            }
            ExprKind::Index(array, _) => self.elements.get(&array.ty.element()).cloned().unwrap_or_default(),
            ExprKind::Call { resolved: Some(_), .. } | ExprKind::MethodCall { resolved: Some(_), .. } => {
                let mut flow = Flow::default();
                for target in self.targets(class, m, ctx, e) {
                    flow.join(&self.returns.get(&target).cloned().unwrap_or_default());
                }
                flow
            }
            _ => Flow::top(),
        };
        self.types.filter(flow, e.ty)
    }

    /// The methods that the call `e` in method `m` on an object of class `ctx` may run, each with
    /// the class of the object it runs on.
    fn targets(&self, class: &Class, m: usize, ctx: usize, e: &Expr) -> Vec<(usize, usize)> {
        let context = |r: usize, ctx: usize| if class.methods[r].is_static { STATIC } else { ctx };
        match &e.kind {
            ExprKind::Call { resolved: Some(r), .. } => vec![(*r, context(*r, ctx))],
            ExprKind::New { class: c, ctor: Some(r), .. } => vec![(*r, *c)],
            ExprKind::ThisCall { resolved: Some(r), .. } | ExprKind::SuperInit { resolved: Some(r), .. } => {
                vec![(*r, ctx)]
            }
            ExprKind::MethodCall { resolved: Some(r), is_super, receiver, .. } => {
                if class.methods[*r].is_static || *is_super {
                    return vec![(*r, context(*r, ctx))];
                }
                let flow = self.eval(class, m, ctx, receiver);
                let group = class.hierarchy.group_id(*r);
                let classes: Vec<usize> = if flow.any {
                    self.types.concretes_of(receiver.ty).to_vec()
                } else {
                    flow.classes.into_iter().collect()
                };
                classes
                    .into_iter()
                    .filter_map(|c| Some((self.types.implementation(class, group, c)?, c)))
                    .collect()
            }
            _ => Vec::new(),
        }
    }
}

/// What one visit of a method learns about a place.
enum Update {
    Var(usize, usize, String, Flow),
    Return(usize, usize, Flow),
    Field(usize, String, Flow),
    Static(usize, String, Flow),
    Element(Type, Flow),
}

/// Follows the program's strings, boxes and objects through its places.
pub fn analyze(class: &Class, _facts: &Facts) -> ObjectValues {
    let types = Types::new(class);
    // Methods the runtime may call itself, such as `equals` from a library's map: their
    // parameters may be anything.
    let runtime_called: HashSet<usize> = class
        .methods
        .iter()
        .enumerate()
        .filter(|(_, method)| HOLDING_VALUES.contains(&class.types[method.class].name.as_str()))
        .map(|(m, _)| class.hierarchy.group_id(m))
        .collect();
    let followed = class
        .methods
        .iter()
        .enumerate()
        .map(|(m, method)| followed(method, !runtime_called.contains(&class.hierarchy.group_id(m))))
        .collect();
    let contexts = class
        .methods
        .iter()
        .enumerate()
        .map(|(m, method)| {
            let found: Vec<usize> = if method.is_static {
                Vec::new()
            } else if method.is_ctor {
                types.concretes[method.class].clone()
            } else {
                let group = class.hierarchy.group_id(m);
                types.concretes[method.class]
                    .iter()
                    .copied()
                    .filter(|&c| types.implementation(class, group, c) == Some(m))
                    .collect()
            };
            if found.is_empty() { vec![STATIC] } else { found }
        })
        .collect();
    let mut values = ObjectValues {
        vars: HashMap::new(),
        returns: HashMap::new(),
        fields: HashMap::new(),
        statics: HashMap::new(),
        elements: HashMap::new(),
        followed,
        contexts,
        types,
    };
    loop {
        let mut changed = false;
        for m in 0..class.methods.len() {
            for i in 0..values.contexts[m].len() {
                let ctx = values.contexts[m][i];
                let updates = visit(&values, class, m, ctx, &class.methods[m].body);
                changed |= apply(&mut values, updates);
            }
        }
        // Field initializers, and the exceptions that the runtime raises.
        let mut updates = Vec::new();
        for (t, decl) in class.types.iter().enumerate() {
            for (f, is_static) in
                decl.fields.iter().map(|f| (f, false)).chain(decl.statics.iter().map(|f| (f, true)))
            {
                let Some(init) = &f.init else { continue };
                let flow = values.types.filter(values.eval(class, usize::MAX, STATIC, init), f.ty);
                updates.push(match is_static {
                    true => Update::Static(t, f.name.clone(), flow),
                    false => Update::Field(t, f.name.clone(), flow),
                });
                updates.extend(visit_value(&values, class, usize::MAX, STATIC, init));
            }
        }
        for e in &class.raised {
            updates.extend(visit_value(&values, class, usize::MAX, STATIC, e));
        }
        changed |= apply(&mut values, updates);
        if !changed {
            return values;
        }
    }
}

fn apply(values: &mut ObjectValues, updates: Vec<Update>) -> bool {
    let mut changed = false;
    for update in updates {
        changed |= match update {
            Update::Var(m, ctx, name, flow) => {
                values.vars.entry((m, ctx)).or_default().entry(name).or_default().join(&flow)
            }
            Update::Return(m, ctx, flow) => values.returns.entry((m, ctx)).or_default().join(&flow),
            Update::Field(t, name, flow) => values.fields.entry((t, name)).or_default().join(&flow),
            Update::Static(t, name, flow) => values.statics.entry((t, name)).or_default().join(&flow),
            Update::Element(ty, flow) => values.elements.entry(ty).or_default().join(&flow),
        };
    }
    changed
}

/// What the statements `body` of method `m`, run on an object of class `ctx`, store.
fn visit(values: &ObjectValues, class: &Class, m: usize, ctx: usize, body: &[Stmt]) -> Vec<Update> {
    let mut out = Vec::new();
    let mut declared = Vec::new();
    for s in body {
        visit_stmt(s, &mut |e| out.extend(visit_value(values, class, m, ctx, e)), &mut |s| match s {
            Stmt::Local { ty, name, init: Some(init), .. } => {
                let flow = values.types.filter(values.eval(class, m, ctx, init), *ty);
                declared.push(Update::Var(m, ctx, name.clone(), flow));
            }
            Stmt::ForEach { ty, name, array, .. } => {
                let flow = values.elements.get(&array.ty.element()).cloned().unwrap_or_default();
                declared.push(Update::Var(m, ctx, name.clone(), values.types.filter(flow, *ty)));
            }
            Stmt::Return(Some(e), _) => declared.push(Update::Return(m, ctx, values.eval(class, m, ctx, e))),
            _ => {}
        });
    }
    out.extend(declared);
    out
}

/// What the expression `e` and the expressions in it store, in method `m` on an object of class
/// `ctx`.
fn visit_value(values: &ObjectValues, class: &Class, m: usize, ctx: usize, e: &Expr) -> Vec<Update> {
    let eval = |e: &Expr| values.eval(class, m, ctx, e);
    let mut out = Vec::new();
    visit_expr(e, &mut |x| match &x.kind {
        ExprKind::Assign { op, target, value } => {
            let flow = if op.is_some() { Flow::top() } else { eval(value) };
            match target {
                Place::Var(name) => out.push(Update::Var(m, ctx, name.clone(), flow)),
                Place::Field { object, name } => match class.field_owner(object.ty, name) {
                    Some(owner) => out.push(Update::Field(owner, name.clone(), flow)),
                    // A field the analysis cannot place: every field of that name.
                    None => {
                        for (t, d) in class.types.iter().enumerate() {
                            if d.fields.iter().any(|f| &f.name == name) {
                                out.push(Update::Field(t, name.clone(), flow.clone()));
                            }
                        }
                    }
                },
                Place::Static { class: c, name } => out.push(Update::Static(*c, name.clone(), flow)),
                Place::Index(array, _) => out.push(Update::Element(array.ty.element(), flow)),
            }
        }
        ExprKind::ArrayLit(items) => {
            for item in items {
                out.push(Update::Element(x.ty.element(), eval(item)));
            }
        }
        ExprKind::InstanceOf { value, ty, binding: Some(name) } => {
            out.push(Update::Var(m, ctx, name.clone(), values.types.filter(eval(value), *ty)));
        }
        ExprKind::Call { args, .. }
        | ExprKind::New { args, .. }
        | ExprKind::ThisCall { args, .. }
        | ExprKind::SuperInit { args, .. }
        | ExprKind::MethodCall { args, .. } => {
            let flows: Vec<Flow> = args.iter().map(eval).collect();
            for (t, c) in values.targets(class, m, ctx, x) {
                for (p, flow) in class.methods[t].params.iter().zip(&flows) {
                    out.push(Update::Var(t, c, p.name.clone(), values.types.filter(flow.clone(), p.ty)));
                }
            }
        }
        // A library function may store its arguments, or the elements of an array it is given,
        // into the arrays it is given.
        ExprKind::Builtin(_, args) if args.iter().any(|a| matches!(a.ty, Type::Array(_))) => {
            let mut flow = Flow::default();
            for a in args {
                match a.ty {
                    Type::Array(element) => {
                        flow.join(&values.elements.get(element).cloned().unwrap_or_default())
                    }
                    _ => flow.join(&eval(a)),
                };
            }
            for a in args {
                if let Type::Array(element) = a.ty {
                    out.push(Update::Element(*element, flow.clone()));
                }
            }
        }
        _ => {}
    });
    out
}

/// The variables of `method` that the analysis follows, its parameters among them when `params`.
/// A name that a catch clause binds anywhere in the method is left to be anything.
fn followed(method: &Method, params: bool) -> HashSet<String> {
    let mut names: HashSet<String> =
        if params { method.params.iter().map(|p| p.name.clone()).collect() } else { HashSet::new() };
    let mut bindings = Vec::new();
    let mut caught = Vec::new();
    for s in &method.body {
        visit_stmt(
            s,
            &mut |e| {
                visit_expr(e, &mut |x| {
                    if let ExprKind::InstanceOf { binding: Some(name), .. } = &x.kind {
                        bindings.push(name.clone());
                    }
                })
            },
            &mut |s| match s {
                Stmt::Local { name, .. } | Stmt::ForEach { name, .. } => _ = names.insert(name.clone()),
                Stmt::Try { catches, .. } => caught.extend(catches.iter().map(|c| c.name.clone())),
                _ => {}
            },
        );
    }
    names.extend(bindings);
    for name in caught {
        names.remove(&name);
    }
    names
}
