//! Object fields that are null only while their object is being built (`Facts::assigned_fields`).
//!
//! A field that the leading assignments of a plain constructor set is never null (see
//! `nullness`). Many constructors set their fields later: after a call, in both branches of an
//! `if`, or on an object they allocate first because they pass `this` on afterwards. Such a field
//! is null from the object's allocation to its assignment, so its cell is an `Option` that starts
//! empty; but when nothing can read it in that window, every read finds a value, and the read
//! needs no test (`java::assigned`).
//!
//! A field `f` of class `C` is assigned before it is read when:
//!
//! - every assignment of `f` anywhere stores a value that is not null (as `nullness` decides,
//!   or a variable that a null test around the assignment proves not null, `narrowed_stores`);
//! - every constructor of `C` that does not delegate with `this(..)` sets `f` to a value that is
//!   not null, by its initializer or by an assignment `this.f = v` on every path that completes
//!   normally, and before that uses `this` only to read and write other fields of it: no call of
//!   an instance method on it, no `this` passed, stored or captured, no read of `f`;
//! - the constructors and field initializers of `C`'s superclasses use `this` only that way too,
//!   so that they cannot run a method of `C` that reads `f` first.
//!
//! In a class tree built whole (`whole_objects`), `C`'s constructors are its `construct<Name>`
//! methods, which run on the zero object that `C`'s own constructor leaves; a call of a
//! superclass's `construct<Name>` on `this` is that superclass's part, quiet as above, one of
//! `C`'s own delegates as `this(..)` does, and `initializeFields()` runs the initializers.
//!
//! Until the assignment, `this` has not left the constructor, so no other code can reach the
//! object; a constructor that throws first leaves an object that nothing reaches.

use super::nullness::{maybe_null, non_null_names};
use super::{Facts, FastSet};
use crate::ast::*;

/// The assignments of a field, by expression id, whose value is a variable that a null test
/// around the assignment proves not null: `if (data != null) this.data = data;`, or after
/// `if (data == null) throw ..;`. A variable assigned anywhere in the code that the test guards
/// is not taken.
pub fn narrowed_stores(class: &Class) -> FastSet<ExprId> {
    let mut out = FastSet::default();
    for method in &class.methods {
        block(&method.body, &[], &mut out);
    }
    out
}

/// `narrowed_stores` for the statements `stmts`, where the variables `narrowed` are not null.
fn block(stmts: &[Stmt], narrowed: &[String], out: &mut FastSet<ExprId>) {
    let mut narrowed = narrowed.to_vec();
    for (k, s) in stmts.iter().enumerate() {
        stmt(s, &narrowed, out);
        // `if (x == null) throw ..;` (or `return`, `break`, `continue`): the rest of the block sees
        // `x` not null, unless it assigns it.
        if let Stmt::If { cond, then, otherwise: None } = s
            && exits(then)
        {
            let rest = &stmts[k + 1..];
            let names = non_null_names(cond, false).into_iter().filter(|v| !assigns_any(rest, v));
            narrowed.extend(names.map(str::to_string));
        }
    }
}

fn stmt(s: &Stmt, narrowed: &[String], out: &mut FastSet<ExprId>) {
    // The variables a test proves not null in `body`, which it guards, added to `narrowed`.
    let guarded = |names: Vec<&str>, body: &Stmt| -> Vec<String> {
        let names = names.into_iter().filter(|v| !assigns_stmt(body, v)).map(str::to_string);
        narrowed.iter().cloned().chain(names).collect()
    };
    match s {
        Stmt::If { cond, then, otherwise } => {
            exprs(cond, narrowed, out);
            stmt(then, &guarded(non_null_names(cond, true), then), out);
            if let Some(otherwise) = otherwise {
                stmt(otherwise, &guarded(non_null_names(cond, false), otherwise), out);
            }
        }
        Stmt::Block(stmts) => block(stmts, narrowed, out),
        Stmt::Local { init: Some(e), .. }
        | Stmt::Expr(e)
        | Stmt::Return(Some(e), _)
        | Stmt::Yield(e)
        | Stmt::Throw(e) => exprs(e, narrowed, out),
        Stmt::While { cond, body } | Stmt::DoWhile { body, cond } => {
            exprs(cond, narrowed, out);
            stmt(body, narrowed, out);
        }
        Stmt::For { init, cond, update, body } => {
            init.iter().for_each(|s| stmt(s, narrowed, out));
            cond.iter().chain(update).for_each(|e| exprs(e, narrowed, out));
            stmt(body, narrowed, out);
        }
        Stmt::ForEach { array, body, .. } => {
            exprs(array, narrowed, out);
            stmt(body, narrowed, out);
        }
        Stmt::Switch { selector, cases } => {
            exprs(selector, narrowed, out);
            cases.iter().for_each(|c| block(&c.body, narrowed, out));
        }
        Stmt::Labeled { body, .. } => stmt(body, narrowed, out),
        Stmt::Try { body, catches, finally } => {
            block(body, narrowed, out);
            catches.iter().for_each(|c| block(&c.body, narrowed, out));
            finally.iter().for_each(|f| block(f, narrowed, out));
        }
        Stmt::Local { init: None, .. }
        | Stmt::Return(None, _)
        | Stmt::Break
        | Stmt::Continue
        | Stmt::BreakLabel(_)
        | Stmt::ContinueLabel(_) => {}
    }
}

/// The field assignments in `e` whose value is one of the variables `narrowed`. The statements of
/// a switch expression inside `e` are taken without narrowing, as `visit_expr` reaches them.
fn exprs(e: &Expr, narrowed: &[String], out: &mut FastSet<ExprId>) {
    if narrowed.is_empty() {
        return;
    }
    visit_expr(e, &mut |x| {
        if let ExprKind::Assign { op: None, target: Place::Field { .. }, value } = &x.kind
            && let ExprKind::Var(v) = &peel_casts(value).kind
            && narrowed.contains(v)
        {
            out.insert(x.id);
        }
    });
}

fn peel_casts(e: &Expr) -> &Expr {
    match &e.kind {
        ExprKind::Cast(_, inner) => peel_casts(inner),
        _ => e,
    }
}

/// Whether `stmt` always leaves the block it is in.
fn exits(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return(..) | Stmt::Break | Stmt::Continue | Stmt::Throw(_) => true,
        Stmt::Block(stmts) => stmts.last().is_some_and(exits),
        _ => false,
    }
}

/// For each class, whether the constructors and field initializers of its superclasses use `this`
/// only to read and write its fields (see the module comment).
pub fn quiet_superclasses(class: &Class) -> Vec<bool> {
    let quiet: Vec<bool> = (0..class.types.len()).map(|t| quiet_class(class, t)).collect();
    (0..class.types.len()).map(|t| class.hierarchy.ancestors(t).iter().all(|&a| quiet[a])).collect()
}

/// Whether the constructors and field initializers of class `t` use `this` only to read and write
/// its fields.
fn quiet_class(class: &Class, t: usize) -> bool {
    let mut inits = class.types[t].fields.iter().filter_map(|f| f.init.as_ref());
    let builds = |m: usize| {
        let method = &class.methods[m];
        method.class == t && (method.is_ctor || class.construction_methods.contains_key(&m))
    };
    inits.all(|e| quiet(class, e, None))
        && class
            .hierarchy
            .methods_of(t)
            .iter()
            .copied()
            .filter(|&m| builds(m))
            .map(|m| &class.methods[m])
            .all(|m| {
                m.body.iter().all(|s| {
                    let mut ok = true;
                    visit_stmt(s, &mut |e| ok &= quiet(class, e, None), &mut |_| {});
                    ok
                })
            })
}

/// Whether `e` uses `this` only to read fields of it other than `field`, and to assign its fields:
/// it calls no instance method on it (with or without `this.`), and does not pass, store, compare
/// or capture it.
fn quiet(class: &Class, e: &Expr, field: Option<&str>) -> bool {
    let other = |name: &String| field.is_none_or(|f| f != name);
    let mut allowed = FastSet::default();
    let mut selves = Vec::new();
    let mut calls = false;
    visit_expr(e, &mut |x| match &x.kind {
        ExprKind::This => selves.push(x.id),
        ExprKind::Field { object, name } if matches!(object.kind, ExprKind::This) && other(name) => {
            allowed.insert(object.id);
        }
        ExprKind::Assign { op, target: Place::Field { object, name }, .. }
            if matches!(object.kind, ExprKind::This) && (op.is_none() || other(name)) =>
        {
            allowed.insert(object.id);
        }
        ExprKind::IncDec { target: Place::Field { object, name }, .. }
            if matches!(object.kind, ExprKind::This) && other(name) =>
        {
            allowed.insert(object.id);
        }
        // An unqualified call of an instance method runs on `this`.
        ExprKind::Call { resolved, .. } => calls |= resolved.is_none_or(|m| !class.methods[m].is_static),
        // The superclass's part of an object built whole (`whole_objects`), which is quiet itself
        // (`quiet_superclasses`).
        ExprKind::MethodCall { receiver, resolved: Some(m), is_super: false, .. }
            if matches!(receiver.kind, ExprKind::This) && class.construction_methods.contains_key(m) =>
        {
            allowed.insert(receiver.id);
        }
        ExprKind::MethodCall { is_super: true, .. } => calls = true,
        // `this(..)`, whose arguments cannot use `this` and whose constructor is one of those the
        // class's are checked with, is quiet.
        _ => {}
    });
    !calls && selves.iter().all(|id| allowed.contains(id))
}

/// Whether field `i` of class `t`, whose constructors are `ctors`, is set to a value that is not
/// null by every constructor before anything can read it (see the module comment).
pub fn assigned_before_read(
    class: &Class,
    facts: &Facts,
    ctors: &[usize],
    t: usize,
    i: usize,
    narrowed: &FastSet<ExprId>,
    quiet_superclasses: &[bool],
) -> bool {
    let decl = &class.types[t];
    // An enum's constants are values whose fields its accessors read.
    if ctors.is_empty() || !quiet_superclasses[t] || decl.kind == TypeKind::Enum {
        return false;
    }
    let name = decl.fields[i].name.as_str();
    // The field initializers run first, in their order, after `super(..)`.
    let mut by_initializer = false;
    for (j, f) in decl.fields.iter().enumerate() {
        let Some(init) = &f.init else { continue };
        if !quiet(class, init, Some(name)) {
            return false;
        }
        if j == i && !maybe_null(facts, usize::MAX, init) {
            by_initializer = true;
            break;
        }
    }
    let walk = Walk { class, facts, field: name, narrowed, t };
    // A class built whole (`whole_objects`) is built by its `construct<Name>` methods, which the
    // constructors of its subclasses and its factories call on the object they allocate, which
    // its own constructor leaves zero.
    let constructs: Vec<usize> = class
        .hierarchy
        .methods_of(t)
        .iter()
        .copied()
        .filter(|m| class.construction_methods.get(m) == Some(&true))
        .collect();
    if !constructs.is_empty() {
        return by_initializer
            || constructs.iter().all(|&m| walk.stmts(m, &class.methods[m].body) == Some(true));
    }
    ctors.iter().all(|&m| {
        let body = &class.methods[m].body;
        if delegates(body) || by_initializer {
            return true;
        }
        let (_, _, after) = split_explicit_call(body);
        walk.stmts(m, after) == Some(true)
    })
}

/// The walk of a constructor's body for `assigned_before_read`.
struct Walk<'a> {
    class: &'a Class,
    facts: &'a Facts,
    field: &'a str,
    narrowed: &'a FastSet<ExprId>,
    /// The field's class.
    t: usize,
}

impl Walk<'_> {
    /// Whether every path through `stmts` of constructor `m` that completes normally sets the
    /// field (`Some(true)`), or some may not (`Some(false)`); `None` where a statement before the
    /// assignment uses `this` otherwise than `quiet` allows, or returns.
    fn stmts(&self, m: usize, stmts: &[Stmt]) -> Option<bool> {
        for s in stmts {
            if self.stmt(m, s)? {
                return Some(true);
            }
        }
        Some(false)
    }

    fn stmt(&self, m: usize, s: &Stmt) -> Option<bool> {
        let quiet = |e: &Expr| quiet(self.class, e, Some(self.field)).then_some(());
        match s {
            Stmt::Expr(e) => match &e.kind {
                ExprKind::Assign { op: None, target: Place::Field { object, name }, value }
                    if matches!(object.kind, ExprKind::This) && name == self.field =>
                {
                    quiet(value)?;
                    Some(!maybe_null(self.facts, m, value) || self.narrowed.contains(&e.id))
                }
                // In a class built whole, `this.construct<Name>(..)` of its own class delegates as
                // `this(..)` does (the method called is checked too), and `this.initializeFields()`
                // runs the field initializers.
                ExprKind::MethodCall { receiver, resolved: Some(c), args, is_super: false, .. }
                    if matches!(receiver.kind, ExprKind::This) && self.class.methods[*c].class == self.t =>
                {
                    match self.class.construction_methods.get(c) {
                        Some(true) => args.iter().try_for_each(quiet).map(|_| true),
                        Some(false) => self.stmts(*c, &self.class.methods[*c].body),
                        None => None,
                    }
                }
                _ => quiet(e).map(|_| false),
            },
            Stmt::Local { init, .. } => init.iter().try_for_each(quiet).map(|_| false),
            Stmt::If { cond, then, otherwise } => {
                quiet(cond)?;
                let then = self.stmt(m, then)?;
                let otherwise = match otherwise {
                    Some(s) => self.stmt(m, s)?,
                    None => false,
                };
                Some(then && otherwise)
            }
            Stmt::Block(stmts) => self.stmts(m, stmts),
            // A constructor that throws leaves an object that nothing reaches.
            Stmt::Throw(e) => quiet(e).map(|_| true),
            Stmt::Try { body, catches, finally: None } => {
                let mut set = self.stmts(m, body)?;
                for c in catches {
                    set &= self.stmts(m, &c.body)?;
                }
                Some(set)
            }
            Stmt::Return(..) => None,
            // Anything else may run any number of times, or not at all: it sets nothing here, and
            // must not use `this` nor return.
            _ => {
                let mut quiet_exprs = true;
                let mut returns = false;
                visit_stmt(s, &mut |e| quiet_exprs &= quiet(e).is_some(), &mut |s| {
                    returns |= matches!(s, Stmt::Return(..))
                });
                (quiet_exprs && !returns).then_some(false)
            }
        }
    }
}
