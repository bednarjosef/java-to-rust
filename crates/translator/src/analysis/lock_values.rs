//! The classes whose objects the lock of a monitor site may be (`monitors::kept`): the object of a
//! `synchronized` block, of `wait`, `notify` and `holdsLock`, and `this` of a synchronized method.
//! Only those classes' objects carry a lock word (`java::Monitored`); every other object of a
//! program that waits or notifies has none.
//!
//! An expression's classes are found from where its value comes from, and where that is not
//! followed, from its type (`hierarchy::flows`: every class whose objects become values of it):
//!
//! - `new C(..)` is an object of `C`, `null` none, and `this` an object of a class under the
//!   method's;
//! - a cast is its operand's classes under the target type, `c ? a : b` both sides', and an
//!   assignment its value's;
//! - a field holds what any assignment to it stores, or its initializer: the field of the class
//!   that declares it, whatever object it is read from. Reflection, deserialization and the
//!   updates of `Unsafe` and `VarHandle` write fields in code that lowering wrote out, so their
//!   stores are among the assignments too;
//! - a local variable holds what its declaration and every assignment to it in the method give it;
//!   a constructor's or static method's parameter what its callers pass. A `for` loop's element,
//!   a caught exception, a pattern's binding, a name the method gives no value, the parameter of
//!   any other method (which a call through an interface, or the runtime, may pass anything), an
//!   array's element and the result of a call are their type's.
//!
//! A value that leads back to itself (a field that stores what it reads) stops at its type there,
//! which keeps the answer a superset of the classes the value can be.

use super::FastSet;
use crate::ast::*;
use std::collections::{HashMap, HashSet};

/// Where an expression is: in method `m`, or in a field initializer of class `at`.
type Home = (Option<usize>, usize);

/// A field: the class that declares it, its name and whether it is static.
type FieldKey<'a> = (usize, &'a str, bool);

/// What a value is read from: a field (by the class that declares it, its name and whether it is
/// static), a local variable of a method, or a parameter of a method.
#[derive(Clone, PartialEq, Eq, Hash)]
enum Source {
    Field(usize, String, bool),
    Local(usize, String),
    Param(usize, usize),
}

/// The definitions of the local variables of a method: what each name is given, and the names
/// that something else binds (a `for` loop's element, a `catch` parameter, a pattern's binding).
#[derive(Default)]
struct Locals<'a> {
    given: HashMap<&'a str, Vec<&'a Expr>>,
    bound: HashSet<&'a str>,
}

pub(super) struct LockValues<'a> {
    class: &'a Class,
    /// `hierarchy::flows`: the classes whose objects become values of each type.
    flows: &'a [FastSet<usize>],
    /// The values stored to each field, and the names of fields stored to whose class is not
    /// known, which are not followed.
    stores: HashMap<FieldKey<'a>, Vec<(&'a Expr, Home)>>,
    unknown_stores: HashSet<&'a str>,
    /// The arguments of the calls of each constructor and static method.
    calls: HashMap<usize, Vec<(&'a [Expr], Home)>>,
    locals: HashMap<usize, Locals<'a>>,
    found: HashMap<Source, FastSet<usize>>,
    visiting: HashSet<Source>,
}

impl<'a> LockValues<'a> {
    pub(super) fn new(class: &'a Class, flows: &'a [FastSet<usize>]) -> LockValues<'a> {
        let mut values = LockValues {
            class,
            flows,
            stores: HashMap::new(),
            unknown_stores: HashSet::new(),
            calls: HashMap::new(),
            locals: HashMap::new(),
            found: HashMap::new(),
            visiting: HashSet::new(),
        };
        for (m, method) in class.methods.iter().enumerate() {
            let home = (Some(m), method.class);
            for s in &method.body {
                visit_stmt(s, &mut |e| visit_expr(e, &mut |x| values.note(x, home)), &mut |_| {});
            }
        }
        for (t, decl) in class.types.iter().enumerate() {
            for (fields, is_static) in [(&decl.fields, false), (&decl.statics, true)] {
                for f in fields {
                    if let Some(init) = &f.init {
                        let home = (None, t);
                        values.stores.entry((t, f.name.as_str(), is_static)).or_default().push((init, home));
                        visit_expr(init, &mut |x| values.note(x, home));
                    }
                }
            }
        }
        values
    }

    /// Notes a field's store or a call's arguments, at `home`.
    fn note(&mut self, e: &'a Expr, home: Home) {
        let class = self.class;
        match &e.kind {
            ExprKind::Assign { target: Place::Field { object, name }, value, .. } => {
                match class.field_owner(object.ty, name) {
                    Some(owner) => {
                        self.stores.entry((owner, name.as_str(), false)).or_default().push((value, home))
                    }
                    None => _ = self.unknown_stores.insert(name),
                }
            }
            ExprKind::Assign { target: Place::Static { class: c, name }, value, .. } => {
                match self.static_owner(*c, name) {
                    Some(owner) => {
                        self.stores.entry((owner, name.as_str(), true)).or_default().push((value, home))
                    }
                    None => _ = self.unknown_stores.insert(name),
                }
            }
            ExprKind::New { args, ctor: Some(m), .. }
            | ExprKind::Call { args, resolved: Some(m), .. }
            | ExprKind::ThisCall { args, resolved: Some(m) }
            | ExprKind::SuperInit { args, resolved: Some(m) } => {
                self.calls.entry(*m).or_default().push((args, home))
            }
            ExprKind::MethodCall { args, resolved: Some(m), .. } if class.methods[*m].is_static => {
                self.calls.entry(*m).or_default().push((args, home))
            }
            _ => {}
        }
    }

    /// The class that declares the static field `name` named through class `t`.
    fn static_owner(&self, t: usize, name: &str) -> Option<usize> {
        let mut at = t;
        loop {
            if self.class.types[at].statics.iter().any(|f| f.name == name) {
                return Some(at);
            }
            at = self.class.hierarchy.superclass(at)?;
        }
    }

    /// The classes whose objects the values of type `ty` can be.
    fn of_type(&self, ty: Type) -> FastSet<usize> {
        match self.class.class_index(ty) {
            Some(t) => std::iter::once(t).chain(self.flows[t].iter().copied()).collect(),
            None => FastSet::default(),
        }
    }

    /// The classes whose objects `e`, in method `m` (`None` in a field's initializer of class
    /// `at`), can be.
    pub(super) fn of(&mut self, e: &'a Expr, m: Option<usize>, at: usize) -> FastSet<usize> {
        let class = self.class;
        match &e.kind {
            ExprKind::Null => FastSet::default(),
            ExprKind::New { class: c, .. } => std::iter::once(*c).collect(),
            ExprKind::This => {
                let t = m.map_or(at, |m| class.methods[m].class);
                class.hierarchy.concretes(t).into_iter().collect()
            }
            ExprKind::Cast(to, value) => {
                let values = self.of(value, m, at);
                match class.class_index(*to) {
                    Some(t) => values.into_iter().filter(|&c| class.hierarchy.is_subtype(c, t)).collect(),
                    None => values,
                }
            }
            ExprKind::Conditional(_, a, b) => {
                let mut values = self.of(a, m, at);
                values.extend(self.of(b, m, at));
                values
            }
            ExprKind::Assign { value, op: None, .. } => self.of(value, m, at),
            ExprKind::Field { object, name } => match class.field_owner(object.ty, name) {
                Some(owner) => self.source(Source::Field(owner, name.clone(), false), e.ty),
                None => self.of_type(e.ty),
            },
            ExprKind::Static { class: c, name } => match self.static_owner(*c, name) {
                Some(owner) => self.source(Source::Field(owner, name.clone(), true), e.ty),
                None => self.of_type(e.ty),
            },
            ExprKind::Var(name) => match m {
                Some(m) => self.source(Source::Local(m, name.clone()), e.ty),
                None => self.of_type(e.ty),
            },
            _ => self.of_type(e.ty),
        }
    }

    /// The classes of the values read from `source`, of type `ty`.
    fn source(&mut self, source: Source, ty: Type) -> FastSet<usize> {
        if let Some(found) = self.found.get(&source) {
            return found.clone();
        }
        if !self.visiting.insert(source.clone()) {
            return self.of_type(ty);
        }
        let values = self.read(&source, ty);
        self.visiting.remove(&source);
        self.found.insert(source, values.clone());
        values
    }

    fn read(&mut self, source: &Source, ty: Type) -> FastSet<usize> {
        let class = self.class;
        let mut values = FastSet::default();
        match source {
            Source::Field(owner, name, is_static) => {
                if self.unknown_stores.contains(name.as_str()) {
                    return self.of_type(ty);
                }
                let stores =
                    self.stores.get(&(*owner, name.as_str(), *is_static)).cloned().unwrap_or_default();
                for (value, (m, at)) in stores {
                    values.extend(self.of(value, m, at));
                }
            }
            Source::Local(m, name) => {
                let method = &class.methods[*m];
                let param = method.params.iter().position(|p| p.name == *name);
                if let Some(i) = param {
                    values.extend(self.source(Source::Param(*m, i), ty));
                }
                self.locals.entry(*m).or_insert_with(|| locals(method));
                let locals = &self.locals[m];
                let given = locals.given.get(name.as_str()).cloned().unwrap_or_default();
                // A name that something else binds, or that nothing in the method gives a value,
                // is not followed.
                if locals.bound.contains(name.as_str()) || param.is_none() && given.is_empty() {
                    return self.of_type(ty);
                }
                for value in given {
                    values.extend(self.of(value, Some(*m), method.class));
                }
            }
            Source::Param(m, i) => {
                let method = &class.methods[*m];
                let calls = self.calls.get(m).cloned().unwrap_or_default();
                if !(method.is_ctor || method.is_static && !method.is_main)
                    || calls.is_empty()
                    || calls.iter().any(|(args, _)| args.len() != method.params.len())
                {
                    return self.of_type(ty);
                }
                for (args, (caller, at)) in calls {
                    values.extend(self.of(&args[*i], caller, at));
                }
            }
        }
        values
    }
}

/// The definitions of the local variables of `method`.
fn locals(method: &Method) -> Locals<'_> {
    let mut out = Locals::default();
    let (mut exprs, mut stmts) = (Vec::new(), Vec::new());
    for s in &method.body {
        visit_stmt(s, &mut |e| visit_expr(e, &mut |x| exprs.push(x)), &mut |s| stmts.push(s));
    }
    for x in exprs {
        match &x.kind {
            ExprKind::Assign { target: Place::Var(name), value, op: None } => {
                out.given.entry(name.as_str()).or_default().push(value)
            }
            ExprKind::InstanceOf { binding: Some(name), .. } => _ = out.bound.insert(name.as_str()),
            _ => {}
        }
    }
    for s in stmts {
        match s {
            Stmt::Local { name, init: Some(init), .. } => {
                out.given.entry(name.as_str()).or_default().push(init)
            }
            Stmt::ForEach { name, .. } => _ = out.bound.insert(name.as_str()),
            Stmt::Try { catches, .. } => out.bound.extend(catches.iter().map(|c| c.name.as_str())),
            _ => {}
        }
    }
    out
}
