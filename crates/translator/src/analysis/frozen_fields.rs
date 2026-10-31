//! Frozen array fields: instance fields of a one-dimensional array type that only their class's
//! constructors give arrays, each a new one (`new T[n]`, `{..}`), and write elements of, through
//! `this`, while the object is a value the constructor holds (a class whose constructors build a
//! plain value, `Facts::plain_constructors`); every other code of the program only indexes them to
//! read and measures them. Once the object is built nothing writes such an array, and nothing else
//! holds it, so it is a `java::Frozen`: its elements are read with no atomic load and no borrow
//! flag, as the JIT reads a `final` field's array, and the compiler may keep a read from one use
//! to the next.
//!
//! A store `this.f[i] = v` whose value may have an effect holds the array first, in a local
//! (`lower::argument_order`); that local, used only to index and store, is the field too.
//!
//! The constant length of the arrays, where every array a frozen static or field is given has the
//! same one (`Facts::frozen_lengths`), is the `java::Frozen`'s too: an index the compiler can bound
//! below it (`h & 15`) needs no check.

use super::{Facts, FastMap, FastSet};
use crate::ast::*;
use crate::lower::RECEIVER;

/// The length of the new array `e` when it is a constant: `new T[n]` with `n` a literal or an
/// `int` constant, or a literal of its elements.
fn constant_length(class: &Class, e: &Expr) -> Option<usize> {
    match &e.kind {
        ExprKind::ArrayLit(items) => Some(items.len()),
        ExprKind::NewArray(sizes) => match &sizes[..] {
            [size] => match &size.kind {
                ExprKind::Int(n) => usize::try_from(*n).ok(),
                ExprKind::Static { class: c, name } => {
                    let f = class.types[*c].statics.iter().find(|f| f.name == *name && f.constant)?;
                    match f.init.as_ref().map(|i| &i.kind) {
                        Some(ExprKind::Int(n)) => usize::try_from(*n).ok(),
                        _ => None,
                    }
                }
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

fn new_array(e: &Expr) -> bool {
    matches!(&e.kind, ExprKind::ArrayLit(_))
        || matches!(&e.kind, ExprKind::NewArray(sizes) if sizes.len() == 1)
}

/// The one length of `arrays`, when each has the same constant length, above 0.
fn one_length<'e>(class: &Class, mut arrays: impl Iterator<Item = &'e Expr>) -> Option<usize> {
    let first = constant_length(class, arrays.next()?)?;
    (first > 0 && arrays.all(|a| constant_length(class, a) == Some(first))).then_some(first)
}

/// Sets `frozen_fields`, `frozen_news` and `frozen_lengths` (for the fields, and for the
/// `frozen_statics` already found).
pub fn analyze(class: &Class, facts: &mut Facts) {
    static_lengths(class, facts);
    // The candidates: one-dimensional array fields of classes whose constructors build a plain
    // value, whose objects are not celled, that are never null.
    let mut candidates: FastSet<(usize, String)> = FastSet::default();
    for (t, decl) in class.types.iter().enumerate() {
        if !facts.plain_constructors.contains(&decl.name)
            || facts.celled.contains(&decl.name)
            || decl.kind == TypeKind::Enum
        {
            continue;
        }
        for f in &decl.fields {
            if matches!(f.ty, Type::Array(_))
                && !matches!(f.ty.element(), Type::Array(_))
                && !f.volatile
                && !facts.nullable_fields.contains(&(decl.name.clone(), f.name.clone()))
                && !facts.assigned_fields.contains(&(decl.name.clone(), f.name.clone()))
                && f.init.as_ref().is_none_or(new_array)
            {
                candidates.insert((t, f.name.clone()));
            }
        }
    }
    if candidates.is_empty() {
        return;
    }
    let mut scan = Scan {
        class,
        candidates: &candidates,
        bad: FastSet::default(),
        mentions: FastMap::default(),
        given: FastMap::default(),
        held: FastMap::default(),
        var_mentions: FastMap::default(),
    };
    for (m, method) in class.methods.iter().enumerate() {
        let ctor = method.is_ctor.then_some(method.class);
        for stmt in &method.body {
            let scan = std::cell::RefCell::new(&mut scan);
            visit_stmt(stmt, &mut |e| visit_expr(e, &mut |x| scan.borrow_mut().expr(x, m, ctor)), &mut |s| {
                scan.borrow_mut().stmt(s, m, ctor)
            });
        }
    }
    // A field initializer runs in the constructor, on the object it builds.
    for (t, decl) in class.types.iter().enumerate() {
        for f in &decl.fields {
            if let Some(init) = &f.init {
                visit_expr(init, &mut |x| scan.expr(x, usize::MAX, Some(t)));
                if candidates.contains(&(t, f.name.clone())) {
                    scan.given.entry((t, f.name.clone())).or_default().push(init);
                }
            }
        }
    }
    let Scan { mut bad, mentions, given, held, var_mentions, .. } = scan;
    for (local, field) in &held {
        if var_mentions.get(local).is_some_and(|(all, good)| all != good) {
            bad.insert(field.clone());
        }
    }
    for (field, (all, good)) in &mentions {
        if all != good {
            bad.insert(field.clone());
        }
    }
    for field in &candidates {
        if bad.contains(field) {
            continue;
        }
        let arrays = given.get(field).map(Vec::as_slice).unwrap_or_default();
        let length = one_length(class, arrays.iter().copied());
        facts.frozen_news.extend(arrays.iter().map(|a| (a.id, length.is_some())));
        if let Some(n) = length {
            facts.frozen_lengths.insert(field.clone(), n);
        }
        facts.frozen_fields.insert(field.clone());
    }
}

/// The lengths of the frozen statics whose arrays all have one constant length.
fn static_lengths(class: &Class, facts: &mut Facts) {
    if facts.frozen_statics.is_empty() {
        return;
    }
    let mut given: FastMap<(usize, &str), Vec<&Expr>> = FastMap::default();
    for (t, decl) in class.types.iter().enumerate() {
        for f in &decl.statics {
            if let Some(init) = &f.init
                && facts.frozen_statics.contains(&(t, f.name.clone()))
            {
                given.entry((t, f.name.as_str())).or_default().push(init);
            }
        }
    }
    for method in &class.methods {
        for stmt in &method.body {
            visit_stmt_exprs(stmt, &mut |x| {
                if let ExprKind::Assign { target: Place::Static { class: c, name }, value, .. } = &x.kind
                    && facts.frozen_statics.contains(&(*c, name.clone()))
                {
                    given.entry((*c, name.as_str())).or_default().push(value);
                }
            });
        }
    }
    for (t, name) in facts.frozen_statics.clone() {
        let arrays = given.get(&(t, name.as_str())).map(Vec::as_slice).unwrap_or_default();
        if let Some(n) = one_length(class, arrays.iter().copied()) {
            facts.frozen_lengths.insert((t, name), n);
        }
    }
}

struct Scan<'a> {
    class: &'a Class,
    candidates: &'a FastSet<(usize, String)>,
    /// Candidates used some other way.
    bad: FastSet<(usize, String)>,
    /// Each candidate's mentions as an expression, and those that index, measure, store into
    /// it from its own constructor, or are held for such a store.
    mentions: FastMap<(usize, String), (usize, usize)>,
    /// The new arrays each candidate is given.
    given: FastMap<(usize, String), Vec<&'a Expr>>,
    /// The locals of a constructor (method, name) that hold a candidate for a store.
    held: FastMap<(usize, String), (usize, String)>,
    /// The mentions of each local of a method, and those that index, measure or store into it.
    var_mentions: FastMap<(usize, String), (usize, usize)>,
}

impl<'a> Scan<'a> {
    /// The candidate that `e` reads, `object.f`.
    fn field(&self, e: &Expr) -> Option<(usize, String)> {
        let ExprKind::Field { object, name } = &e.kind else { return None };
        let owner = self.class.field_owner(object.ty, name)?;
        let key = (owner, name.clone());
        self.candidates.contains(&key).then_some(key)
    }

    /// Whether `e` reads candidate `key` of `this`, in a constructor of its class.
    fn own(&self, e: &Expr, key: &(usize, String), ctor: Option<usize>) -> bool {
        matches!(&e.kind, ExprKind::Field { object, .. } if matches!(object.kind, ExprKind::This))
            && ctor == Some(key.0)
    }

    /// Counts a use of `array` that indexes, measures or stores into it (`store` with whether it
    /// stores, which only the field's own constructor may).
    fn used(&mut self, array: &Expr, m: usize, ctor: Option<usize>, store: bool) {
        if let Some(key) = self.field(array) {
            if store && !self.own(array, &key, ctor) {
                self.bad.insert(key.clone());
            }
            self.mentions.entry(key).or_default().1 += 1;
        } else if let ExprKind::Var(name) = &array.kind {
            self.var_mentions.entry((m, name.clone())).or_default().1 += 1;
        }
    }

    fn expr(&mut self, x: &'a Expr, m: usize, ctor: Option<usize>) {
        match &x.kind {
            ExprKind::Field { .. } => {
                if let Some(key) = self.field(x) {
                    self.mentions.entry(key).or_default().0 += 1;
                }
            }
            ExprKind::Var(name) => self.var_mentions.entry((m, name.clone())).or_default().0 += 1,
            ExprKind::Index(a, _) | ExprKind::Length(a) => self.used(a, m, ctor, false),
            ExprKind::Assign { target: Place::Index(a, _), .. }
            | ExprKind::IncDec { target: Place::Index(a, _), .. } => self.used(a, m, ctor, true),
            ExprKind::Assign { target: Place::Field { object, name }, op, value } => {
                let Some(owner) = self.class.field_owner(object.ty, name) else { return };
                let key = (owner, name.clone());
                if !self.candidates.contains(&key) {
                    return;
                }
                if op.is_none()
                    && new_array(value)
                    && matches!(object.kind, ExprKind::This)
                    && ctor == Some(owner)
                {
                    self.given.entry(key).or_default().push(value);
                } else {
                    self.bad.insert(key);
                }
            }
            ExprKind::IncDec { target: Place::Field { object, name }, .. } => {
                if let Some(owner) = self.class.field_owner(object.ty, name) {
                    self.bad.insert((owner, name.clone()));
                }
            }
            _ => {}
        }
    }

    fn stmt(&mut self, s: &'a Stmt, m: usize, ctor: Option<usize>) {
        if let Stmt::Local { name, init: Some(init), .. } = s
            && name.starts_with(RECEIVER)
            && let Some(key) = self.field(init)
            && self.own(init, &key, ctor)
        {
            self.mentions.entry(key.clone()).or_default().1 += 1;
            self.held.insert((m, name.clone()), key);
        }
    }
}
