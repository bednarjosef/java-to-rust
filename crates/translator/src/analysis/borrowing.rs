//! Methods that borrow their object. A shared object is an `Rc<RefCell<C>>`; a method that only
//! reads and writes the object's fields and calls such methods on it can borrow it for the whole
//! call, as `&self` or `&mut self`, and read its fields as `self.f`. That is only right when
//! nothing the method runs reaches the same object through another handle: the `RefCell` would
//! refuse the second borrow. So a method borrows its object when
//! - it uses `this` only to read or write a field or to call a method that borrows too, never as
//!   a value (passed, returned, stored or compared), and
//! - nothing it runs, directly or through the methods and constructors it calls, touches an
//!   object of its own class through a handle other than `this`.
//!
//! Methods of immutable and owned classes always borrow: an immutable object has no `RefCell`,
//! and an owned object has no other handle. So do the methods that live on a superclass's
//! fields struct (see `hierarchy::field_level`).
//!
//! A constructor builds a plain value and the caller wraps it, when the constructors of its class
//! use `this` the same way (no other code can reach an object under construction).
//!
//! A call may run any implementation of the method it names, so the reach of a method covers all
//! of them, and touching an object of a class type touches every concrete class under it.

use super::Facts;
use super::closure::{Bits, reached_union};
use super::hierarchy::{Home, home, to_string_impl};
use crate::ast::*;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub struct Borrowing {
    /// Instance methods (indexes into `Class::methods`) that take `&self` or `&mut self`.
    pub methods: HashSet<usize>,
    /// Instance methods that change their object: they take `&mut self` when they borrow it.
    pub mutating: HashSet<usize>,
    /// Classes, by qualified name, whose constructors build a plain value.
    pub plain_constructors: HashSet<String>,
    /// For each of the other classes, by index, the positions of the constructors and then of
    /// the field initializers that keep it from building a plain value.
    pub not_plain: HashMap<usize, Vec<Pos>>,
    /// Methods and constructors that touch no shared object but their own `this`, directly or
    /// through what they call.
    pub isolated: HashSet<usize>,
    /// Methods and constructors that may write a field of a shared object, directly or through
    /// what they call, so may borrow one mutably. A constructor's writes to its own `this` do not
    /// count: no other code can reach that object yet.
    pub writers: HashSet<usize>,
}

/// What each method does with objects, and the classes (by index in `Class::types`) whose
/// objects each may touch through a handle other than its `this`, directly or through what it
/// calls.
pub struct Reach<'c> {
    scans: Vec<Scan<'c>>,
    pub reach: Vec<Bits>,
}

pub fn reach(class: &Class) -> Reach<'_> {
    let mut memo = Memo::default();
    let scans: Vec<Scan> = class.methods.iter().map(|m| scan(class, m, &mut memo)).collect();
    let calls: Vec<Vec<usize>> =
        scans.iter().map(|s| s.calls.iter().map(|&(callee, _)| callee).collect()).collect();
    let touches: Vec<Bits> = scans.iter().map(|s| s.touches.clone()).collect();
    let reach = reached_union(&calls, &touches);
    Reach { scans, reach }
}

/// The implementation that a call on `this` runs in method `m` whose object is of concrete class
/// `c` (`None` when the object may be of several classes).
fn this_target(class: &Class, call: ThisCall, c: Option<usize>) -> Option<usize> {
    let h = &class.hierarchy;
    match call {
        ThisCall::Method { resolved, is_super: true } => Some(resolved),
        ThisCall::Method { resolved, is_super: false } => match c {
            Some(c) => h.implementation(resolved, c),
            None if !h.is_overridden(resolved) && !class.methods[resolved].is_abstract => Some(resolved),
            None => None,
        },
        ThisCall::Text => c.and_then(|c| to_string_impl(class, c)),
    }
}

pub fn borrowing(class: &Class, facts: &Facts, reach: &Reach) -> Borrowing {
    let Reach { scans, reach } = reach;
    let h = &class.hierarchy;
    let name = |t: usize| class.types[t].name.as_str();
    let always = |t: usize| {
        facts.immutable.contains(name(t))
            || facts.owned.contains(name(t))
            || class.types[t].enum_constants.is_some()
    };
    let homes: Vec<Home> = (0..class.methods.len()).map(|m| home(class, facts, m)).collect();
    let mut methods: HashSet<usize> = (0..scans.len())
        .filter(|&m| match homes[m] {
            _ if class.methods[m].is_ctor => false,
            Home::Class(c) => {
                always(c)
                    || (!scans[m].this_as_value && !reach[m].contains(c) && !facts.long_methods.contains(&m))
            }
            Home::Fields(_) => true,
            _ => false,
        })
        .collect();
    // A method that calls a method on `this` that cannot borrow cannot borrow either: each
    // method that loses its place is checked again in the methods that call it.
    let targets: HashMap<usize, Vec<usize>> = methods
        .iter()
        .filter_map(|&m| {
            let Home::Class(c) = homes[m] else { return None };
            (!always(c)).then(|| {
                (
                    m,
                    scans[m]
                        .this_calls
                        .iter()
                        .filter_map(|&call| this_target(class, call, Some(c)))
                        .collect(),
                )
            })
        })
        .collect();
    let mut callers: HashMap<usize, Vec<usize>> = HashMap::new();
    for (&m, ts) in &targets {
        for &t in ts {
            callers.entry(t).or_default().push(m);
        }
    }
    let mut losing: Vec<usize> =
        targets.iter().filter(|(_, ts)| ts.iter().any(|t| !methods.contains(t))).map(|(&m, _)| m).collect();
    while let Some(m) = losing.pop() {
        if methods.remove(&m) {
            losing.extend(callers.get(&m).into_iter().flatten().filter(|c| methods.contains(c)));
        }
    }
    // A call may run any method of an override group, so the group changes its object, or writes
    // or touches other objects, when one of its methods does; and so does a method that calls one.
    let mut this_callers = vec![Vec::new(); scans.len()];
    let mut all_callers = vec![Vec::new(); scans.len()];
    for (m, scan) in scans.iter().enumerate() {
        for &(c, on_this) in &scan.calls {
            if on_this {
                this_callers[c].push(m);
            }
            all_callers[c].push(m);
        }
    }
    let mutating = closed(h, &this_callers, (0..scans.len()).filter(|&m| scans[m].writes_this));
    // A call on `this` in a constructor of `t` (or its field initializers) keeps the object a plain
    // value when it runs a method that borrows the struct being built: one emitted on `t`'s own
    // struct or on the fields struct of `t` or a superclass.
    let plain_call = |t: usize, call: ThisCall| {
        let leaf = !h.has_subtypes(t) && h.is_concrete(t);
        let Some(target) = this_target(class, call, leaf.then_some(t)) else { return false };
        methods.contains(&target)
            && match homes[target] {
                Home::Class(c) => c == t,
                Home::Fields(d) => d == t || h.ancestors(t).contains(&d),
                _ => false,
            }
    };
    debug_assert_eq!(h.overriders.len(), class.methods.len(), "the hierarchy is out of date");
    let mut memo = Memo::default();
    let mut plain_constructors = HashSet::new();
    let mut not_plain = HashMap::new();
    for t in 0..class.types.len() {
        let ctors = h.methods_of(t).iter().copied().filter(|&m| class.methods[m].is_ctor);
        let failing_ctors = ctors
            .filter(|&m| {
                scans[m].this_as_value || !scans[m].this_calls.iter().all(|&call| plain_call(t, call))
            })
            .map(|m| class.methods[m].pos);
        let failing_inits = class.types[t].fields.iter().filter_map(|f| f.init.as_ref()).filter(|init| {
            let mut s = Scan::default();
            s.expr(class, init, &mut memo);
            s.this_as_value || !s.this_calls.iter().all(|&call| plain_call(t, call))
        });
        let failing: Vec<Pos> = failing_ctors.chain(failing_inits.map(|init| init.pos)).collect();
        if failing.is_empty() {
            plain_constructors.insert(class.types[t].name.clone());
        } else {
            not_plain.insert(t, failing);
        }
    }
    // Immutable and owned objects have no `RefCell` to borrow.
    let shared = |c: &str| !facts.immutable.contains(c) && !facts.owned.contains(c);
    let isolated = (0..scans.len()).filter(|&m| !reach[m].iter().any(|c| shared(name(c)))).collect();
    let writers = closed(
        h,
        &all_callers,
        (0..scans.len()).filter(|&m| {
            let method = &class.methods[m];
            let own_shared = || {
                let t = method.class;
                shared(name(t)) && (h.is_concrete(t) || class.types[t].enum_constants.is_some())
                    || h.concretes(t).into_iter().any(|c| shared(name(c)))
            };
            (scans[m].writes_this && !method.is_ctor && own_shared())
                || scans[m].writes.iter().any(|c| shared(c))
        }),
    );
    // A method of an immutable class changes nothing: its object has no `RefCell` to borrow
    // mutably, whatever the other methods of its group do.
    let mutating =
        mutating.into_iter().filter(|&m| !facts.immutable.contains(name(class.methods[m].class))).collect();
    let isolated: HashSet<usize> = isolated;
    let isolated =
        isolated.iter().copied().filter(|&m| h.group(m).iter().all(|g| isolated.contains(g))).collect();
    Borrowing { methods, mutating, plain_constructors, not_plain, isolated, writers }
}

/// The least set holding `seeds` with every method of the override group of each member, since a
/// call may run any of them, and every method that `callers` lists for a member.
fn closed(h: &Hierarchy, callers: &[Vec<usize>], seeds: impl Iterator<Item = usize>) -> HashSet<usize> {
    let mut set = HashSet::new();
    let mut queue: Vec<usize> = seeds.collect();
    while let Some(m) = queue.pop() {
        if set.insert(m) {
            queue.extend(h.group_members(m).iter().chain(&callers[m]).filter(|n| !set.contains(*n)));
        }
    }
    set
}

/// A call on `this`: of a method (as the call names it), or of `toString` by turning `this` into
/// text.
#[derive(Clone, Copy)]
enum ThisCall {
    Method { resolved: usize, is_super: bool },
    Text,
}

/// What one method does with objects.
#[derive(Default)]
struct Scan<'c> {
    /// Classes, by index, whose objects the method touches through a handle other than `this`.
    touches: Bits,
    /// Methods and constructors it may run, and whether on `this` (`this(...)` and `super(...)`
    /// count as on `this`).
    calls: Vec<(usize, bool)>,
    /// The method calls on `this`, other than `this(...)` and `super(...)`.
    this_calls: Vec<ThisCall>,
    /// Whether `this` is used other than to read or write a field or to receive a call.
    this_as_value: bool,
    /// Whether it writes a field of `this`.
    writes_this: bool,
    /// Classes whose objects' fields it writes through a handle other than `this`.
    writes: HashSet<&'c str>,
}

fn scan<'c>(class: &'c Class, m: &Method, memo: &mut Memo) -> Scan<'c> {
    let mut s = Scan::default();
    for stmt in &m.body {
        visit_stmt(stmt, &mut |e| s.expr(class, e, memo), &mut |_| {});
    }
    // The calls are a set: a call on an `Object` runs thousands of methods, again at each one.
    s.calls.sort_unstable();
    s.calls.dedup();
    s
}

/// What `Scan` finds of each class once: the classes a handle of it may hold, and the
/// `toString`s that turning one into text may run.
#[derive(Default)]
struct Memo {
    touches: HashMap<usize, Bits>,
    texts: HashMap<usize, Vec<usize>>,
}

/// The classes an object of type `ty` may be of: the type itself and the concrete classes
/// under it, by qualified name.
fn classes(class: &Class, ty: Type) -> Vec<&str> {
    class_indexes(class, ty).into_iter().map(|c| class.types[c].name.as_str()).collect()
}

/// `classes`, by index in `Class::types`.
fn class_indexes(class: &Class, ty: Type) -> Vec<usize> {
    let Some(t) = class.class_index(ty) else { return Vec::new() };
    let mut out = vec![t];
    out.extend(class.hierarchy.concretes(t));
    out
}

impl<'c> Scan<'c> {
    fn expr(&mut self, class: &'c Class, e: &Expr, memo: &mut Memo) {
        // `this` nodes in an allowed position, so that the remaining ones are values.
        let mut allowed = HashSet::new();
        let mut selves = Vec::new();
        visit_expr(e, &mut |x| {
            let is_this = |o: &Expr| matches!(o.kind, ExprKind::This);
            match &x.kind {
                ExprKind::This => selves.push(x.id),
                ExprKind::Field { object, .. } => {
                    if is_this(object) {
                        allowed.insert(object.id);
                    } else {
                        self.touch(class, object.ty, memo);
                    }
                }
                ExprKind::Assign { target: Place::Field { object, .. }, .. }
                | ExprKind::IncDec { target: Place::Field { object, .. }, .. } => {
                    if is_this(object) {
                        allowed.insert(object.id);
                        self.writes_this = true;
                    } else {
                        self.touch(class, object.ty, memo);
                        self.writes.extend(classes(class, object.ty));
                    }
                }
                ExprKind::MethodCall { receiver, resolved: Some(callee), is_super, .. } => {
                    let on_this = is_this(receiver);
                    if on_this {
                        allowed.insert(receiver.id);
                        self.this_calls.push(ThisCall::Method { resolved: *callee, is_super: *is_super });
                    } else if !class.methods[*callee].is_static {
                        self.touch(class, receiver.ty, memo);
                    }
                    self.calls.extend(class.callees(x).into_iter().map(|c| (c, on_this)));
                }
                ExprKind::Call { resolved: Some(callee), .. } | ExprKind::New { ctor: Some(callee), .. } => {
                    self.calls.push((*callee, false));
                }
                ExprKind::ThisCall { resolved: Some(callee), .. }
                | ExprKind::SuperInit { resolved: Some(callee), .. } => self.calls.push((*callee, true)),
                // Printing an object or concatenating it calls its `toString`.
                ExprKind::Builtin(Builtin::Print | Builtin::Println, args) => {
                    for a in args {
                        self.text(class, a, &mut allowed, memo);
                    }
                }
                ExprKind::Binary(BinOp::Add, l, r) if x.ty == Type::String => {
                    self.text(class, l, &mut allowed, memo);
                    self.text(class, r, &mut allowed, memo);
                }
                // A builder's `append(s, start, end)` calls `length` and `charAt` of `s`, which
                // is never `this` itself (it is converted to a `CharSequence`).
                ExprKind::Builtin(Builtin::Builder(BuilderOp::Append), args) if args.len() == 4 => {
                    let callees = class.callees(x);
                    if !callees.is_empty() {
                        self.touch(class, args[1].ty, memo);
                        self.calls.extend(callees.into_iter().map(|c| (c, false)));
                    }
                }
                _ => {}
            }
        });
        self.this_as_value |= selves.iter().any(|id| !allowed.contains(id));
    }

    /// An operand turned into text: an object's `toString` runs on it.
    fn text(&mut self, class: &'c Class, e: &Expr, allowed: &mut HashSet<ExprId>, memo: &mut Memo) {
        let Some(t) = class.class_index(e.ty) else { return };
        let on_this = matches!(e.kind, ExprKind::This);
        if on_this {
            allowed.insert(e.id);
            self.this_calls.push(ThisCall::Text);
        } else {
            self.touch(class, e.ty, memo);
        }
        let runs = memo.texts.entry(t).or_insert_with(|| {
            let mut runs: Vec<usize> = class.hierarchy.concretes(t);
            if !runs.contains(&t) {
                runs.push(t);
            }
            runs.into_iter().filter_map(|c| to_string_impl(class, c)).collect()
        });
        self.calls.extend(runs.iter().map(|&index| (index, on_this)));
    }

    fn touch(&mut self, class: &'c Class, ty: Type, memo: &mut Memo) {
        let Some(t) = class.class_index(ty) else { return };
        let touched = memo.touches.entry(t).or_insert_with(|| class_indexes(class, ty).into_iter().collect());
        self.touches.union(touched);
    }
}
