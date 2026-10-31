//! What one method does with its arrays, before the solver decides which are plain: every
//! mention of an array variable is classified by where it stands.
//!
//! - Through its elements (`a[i]`, `a[i][j] = v`, `a.length`, `a[i].length`, for-each over a
//!   one-dimensional array, `System.arraycopy`): always fine.
//! - As an argument of a call: fine when the parameter borrows it (decided by the solver), or
//!   when the variable is moved into the call.
//! - Moved: the whole value of an assignment, a local's initializer, a returned value, a row
//!   assignment or an argument, in a statement that mentions the variable once and after which
//!   it is not live (see `liveness`). Nothing else sees the array afterwards, so it is handed on.
//! - Anywhere else (`==`, `java.util.Arrays`, a field, a row of an array kept in a variable, ...):
//!   the variable is shared.
//!
//! The values assigned to each array local are recorded as `Owned` conditions: a local is plain
//! only if everything it ever holds is an array that nobody else holds.

use super::super::Facts;
use super::super::{FastMap, FastSet};
use super::declarations;
use crate::analysis::nullable_var;
use crate::ast::*;

/// What makes an array value one that nobody else holds.
#[derive(Clone, Debug)]
pub(super) enum Owned {
    /// Never: a parameter, a field, a row of another array, `null`, a conditional, ...
    Never,
    /// A new array or an array literal.
    Fresh,
    /// The result of a call: owned when the method returns arrays it owns.
    Returned(usize),
    /// A local whose value is never read again: owned when the local is a plain array.
    Moved(String),
    /// An array literal of rows: owned when every row is.
    Rows(Vec<Owned>),
}

/// What one method does with its arrays.
#[derive(Default)]
pub(super) struct Scan {
    /// Array locals and parameters that qualify unless a condition below makes them shared.
    pub candidates: FastSet<String>,
    /// Array parameters that may be borrowed, as (index, name).
    pub params: Vec<(usize, String)>,
    /// Every array local and parameter, plain or not.
    pub arrays: FastSet<String>,
    /// Arrays with an element assigned in this method, at any depth.
    pub written: FastSet<String>,
    /// Arrays whose rows are replaced in this method (`m[i] = new int[3]`).
    pub reshaped: FastSet<String>,
    /// The calls made in this method.
    pub calls: Vec<Call>,
    /// The values assigned to array locals, the initializer included.
    pub assigned: Vec<(String, Owned)>,
    /// The values assigned to rows of array variables: (array, value).
    pub rows: Vec<(String, Owned)>,
    /// The values the method returns, if it returns an array.
    pub returns: Vec<Owned>,
    /// The mentions of array variables that move their value.
    pub moves: Vec<ExprId>,
    /// The for-each loops over arrays of arrays whose variable borrows each row: (array, row).
    pub row_loops: Vec<(String, String)>,
}

/// A call and what each argument is. A call of a method of an override group is a call of each
/// method of the group (`arrays::analyze`), which share the arguments: a call of `toString` on
/// the Minecraft server is one of thousands of methods.
#[derive(Clone)]
pub(super) struct Call {
    pub callee: usize,
    pub args: std::rc::Rc<[Arg]>,
}

#[derive(Clone)]
pub(super) struct Arg {
    /// The array variable the argument is, or whose row it is.
    pub name: Option<String>,
    /// Whether the argument is a row of `name` (`m[i]`), lent only when its indexes leave `name`
    /// alone.
    pub row: bool,
    /// What makes the argument an array that nobody else holds, such as a new array or the
    /// packed trailing arguments of a varargs call: then any parameter may borrow it.
    pub fresh: Owned,
    /// Whether the argument is a variable moved into the call.
    pub moved: bool,
    pub array: bool,
    /// The variables the argument mentions.
    pub mentions: FastSet<String>,
    /// Whether evaluating the argument may run code that reaches an array: a call, an assignment
    /// or a `java.util.Arrays` method.
    pub effects: bool,
}

/// Whether arrays of type `ty` can be plain at any depth and be shared later (`java::share`):
/// their innermost elements are primitives or strings, and no row may be null.
pub(super) fn shareable(ty: Type, facts: &Facts) -> bool {
    match ty {
        Type::Array(elem) => match *elem {
            Type::Array(_) => !facts.nullable_elements.contains(elem) && shareable(*elem, facts),
            elem => elem.is_numeric() || matches!(elem, Type::Boolean | Type::String),
        },
        _ => false,
    }
}

/// Whether the statement `s` mentions the variable `name` anywhere.
fn stmt_mentions(s: &Stmt, name: &str) -> bool {
    let mut found = false;
    visit_stmt(s, &mut |e| found |= mentions(e, name), &mut |_| {});
    found
}

/// How many array levels `ty` has: 2 for `int[][]`.
pub(super) fn depth(ty: Type) -> usize {
    match ty {
        Type::Array(elem) => 1 + depth(*elem),
        _ => 0,
    }
}

pub(super) fn scan(
    m: usize,
    method: &Method,
    facts: &Facts,
    live_after: &FastMap<ExprId, FastSet<String>>,
) -> Scan {
    let mut types = FastMap::default();
    let mut declared: FastMap<String, usize> = FastMap::default();
    for p in &method.params {
        *declared.entry(p.name.clone()).or_default() += 1;
        if matches!(p.ty, Type::Array(_)) {
            types.insert(p.name.clone(), p.ty);
        }
    }
    let mut references: FastSet<&str> = method.params.iter().map(|p| p.name.as_str()).collect();
    // The variables declared only by for-each loops, all of one type: the loops may share a
    // name, as each variable lives in its own loop and is never assigned.
    let mut loop_vars: FastMap<&str, Option<Type>> = FastMap::default();
    let mut other_vars: FastSet<&str> = references.clone();
    for d in declarations(method) {
        *declared.entry(d.name.to_string()).or_default() += 1;
        if matches!(d.ty, Type::Array(_)) {
            types.insert(d.name.to_string(), d.ty);
        }
        if d.loop_var {
            references.insert(d.name);
            let same = loop_vars.get(d.name).is_none_or(|&ty| ty == Some(d.ty));
            loop_vars.insert(d.name, same.then_some(d.ty));
        } else {
            other_vars.insert(d.name);
        }
    }
    let once = |name: &str| {
        let loops_only = loop_vars.get(name).is_some_and(|ty| ty.is_some()) && !other_vars.contains(name);
        declared.get(name) == Some(&1) || loops_only
    };
    let mut walk = Walk {
        facts,
        live_after,
        types: &types,
        references: &references,
        bad: FastSet::default(),
        top: None,
        switches: 0,
        scan: Scan::default(),
    };
    walk.block(&method.body, method.ret);
    let Walk { bad, mut scan, .. } = walk;
    for (i, p) in method.params.iter().enumerate() {
        if eligible(p.ty, facts) {
            scan.params.push((i, p.name.clone()));
        }
    }
    scan.candidates = types
        .iter()
        .filter(|&(name, &ty)| {
            once(name) && !bad.contains(name) && eligible(ty, facts) && !nullable_var(facts, m, name)
        })
        .map(|(name, _)| name.clone())
        .collect();
    scan.arrays = types.into_keys().collect();
    scan
}

/// Whether a variable of type `ty` can be a plain array: one-dimensional arrays of any element,
/// and arrays of any depth that can be shared later.
fn eligible(ty: Type, facts: &Facts) -> bool {
    depth(ty) == 1 || shareable(ty, facts)
}

struct Walk<'m> {
    facts: &'m Facts,
    live_after: &'m FastMap<ExprId, FastSet<String>>,
    /// The type of every array local and parameter.
    types: &'m FastMap<String, Type>,
    /// The parameters and for-each variables: they refer to arrays held elsewhere, so they are
    /// never moved and never take another array.
    references: &'m FastSet<&'m str>,
    /// The array variables that a use makes shared.
    bad: FastSet<String>,
    /// The expression of the simple statement being walked, where a variable may be moved.
    top: Option<&'m Expr>,
    /// How many switches and `try` statements enclose the code being walked; the liveness
    /// analysis does not look into them, so nothing is moved there.
    switches: usize,
    scan: Scan,
}

impl<'m> Walk<'m> {
    fn is_array(&self, name: &str) -> bool {
        self.types.contains_key(name)
    }

    /// Whether `name` is an array local (not a parameter) whose values can be handed on.
    fn movable_type(&self, name: &str) -> bool {
        self.types.get(name).is_some_and(|&ty| shareable(ty, self.facts)) && !self.references.contains(name)
    }

    /// Whether the mention `e` of the array local `name` can move its value: it is in a simple
    /// statement that mentions `name` nowhere else, after which `name` is not live.
    fn movable(&self, name: &str) -> bool {
        let Some(top) = self.top else { return false };
        if self.switches > 0 || !self.movable_type(name) {
            return false;
        }
        let mut reads = 0;
        visit_expr(top, &mut |x| {
            if let ExprKind::Var(v) = &x.kind {
                reads += usize::from(v == name);
            }
        });
        reads == 1 && self.live_after.get(&top.id).is_some_and(|live| !live.contains(name))
    }

    /// How the array value `e` comes to be owned, for a target of type `e.ty`.
    fn owned(&self, e: &Expr) -> Owned {
        let movable = shareable(e.ty, self.facts);
        match &e.kind {
            ExprKind::NewArray(sizes) if sizes.len() == depth(e.ty) => Owned::Fresh,
            ExprKind::ArrayLit(_) | ExprKind::Builtin(Builtin::MainArgs, _) if depth(e.ty) == 1 => {
                Owned::Fresh
            }
            ExprKind::ArrayLit(rows) if movable => {
                Owned::Rows(rows.iter().map(|r| self.owned_row(r)).collect())
            }
            ExprKind::Call { resolved: Some(callee), .. }
            | ExprKind::MethodCall { resolved: Some(callee), .. }
                if movable =>
            {
                Owned::Returned(*callee)
            }
            ExprKind::Var(v) if movable && self.movable(v) => Owned::Moved(v.clone()),
            _ => Owned::Never,
        }
    }

    /// A row of an array literal: never a variable, which the literal would share.
    fn owned_row(&self, e: &Expr) -> Owned {
        match e.kind {
            ExprKind::Var(_) => Owned::Never,
            _ => self.owned(e),
        }
    }

    fn block(&mut self, stmts: &'m [Stmt], ret: Type) {
        for s in stmts {
            self.stmt(s, ret);
        }
    }

    fn stmt(&mut self, s: &'m Stmt, ret: Type) {
        match s {
            Stmt::Local { name, init, .. } => {
                let Some(e) = init else { return };
                self.top = Some(e);
                if self.is_array(name) {
                    let owned = self.owned(e);
                    self.scan.assigned.push((name.clone(), owned));
                    self.moved(e);
                } else {
                    self.expr(e);
                }
                self.top = None;
            }
            Stmt::Expr(e) => {
                self.top = Some(e);
                self.statement(e);
                self.top = None;
            }
            Stmt::Return(Some(e), _) => {
                self.top = Some(e);
                if matches!(ret, Type::Array(_)) {
                    let owned = self.owned(e);
                    self.scan.returns.push(owned);
                    self.moved(e);
                } else {
                    self.expr(e);
                }
                self.top = None;
            }
            Stmt::If { cond, then, otherwise } => {
                self.expr(cond);
                self.stmt(then, ret);
                if let Some(o) = otherwise {
                    self.stmt(o, ret);
                }
            }
            Stmt::While { cond, body } | Stmt::DoWhile { body, cond } => {
                self.expr(cond);
                self.stmt(body, ret);
            }
            Stmt::For { init, cond, update, body } => {
                self.block(init, ret);
                if let Some(c) = cond {
                    self.expr(c);
                }
                update.iter().for_each(|u| self.expr(u));
                self.stmt(body, ret);
            }
            Stmt::ForEach { name, array, body, .. } => {
                // A for-each over a one-dimensional array reads its elements; one over an array
                // of arrays borrows each row in turn, while the body leaves the array alone.
                // A body that assigns the array itself leaves the loop holding the old one, a
                // second name for it.
                match &array.kind {
                    ExprKind::Var(v) if self.types.get(v).is_some_and(|&ty| depth(ty) == 1) => {
                        if assigns_any(std::slice::from_ref(&**body), v) {
                            self.bad.insert(v.clone());
                        }
                    }
                    ExprKind::Var(v) if self.is_array(v) && !stmt_mentions(body, v) => {
                        self.scan.row_loops.push((v.clone(), name.clone()));
                    }
                    _ => {
                        self.expr(array);
                        // Its variable is a second name for each row.
                        if self.is_array(name) {
                            self.bad.insert(name.clone());
                        }
                    }
                }
                self.stmt(body, ret);
            }
            Stmt::Switch { selector, cases } => {
                self.expr(selector);
                self.switches += 1;
                for case in cases {
                    self.block(&case.body, ret);
                }
                self.switches -= 1;
            }
            Stmt::Labeled { body, .. } => self.stmt(body, ret),
            Stmt::Block(stmts) => self.block(stmts, ret),
            Stmt::Yield(e) | Stmt::Throw(e) => self.expr(e),
            Stmt::Try { body, catches, finally } => {
                self.switches += 1;
                self.block(body, ret);
                for catch in catches {
                    self.block(&catch.body, ret);
                }
                if let Some(f) = finally {
                    self.block(f, ret);
                }
                self.switches -= 1;
            }
            Stmt::Return(None, _)
            | Stmt::Break
            | Stmt::Continue
            | Stmt::BreakLabel(_)
            | Stmt::ContinueLabel(_) => {}
        }
    }

    /// An expression statement: an assignment to an array variable or to a row is only accepted
    /// here, where nothing else in the statement can see the array change.
    fn statement(&mut self, e: &'m Expr) {
        match &e.kind {
            ExprKind::Assign { op: None, target: Place::Var(target), value } if self.is_array(target) => {
                // A borrowed parameter or row cannot take another array.
                if self.references.contains(target.as_str()) {
                    self.bad.insert(target.clone());
                }
                let owned = self.owned(value);
                self.scan.assigned.push((target.clone(), owned));
                self.moved(value);
            }
            ExprKind::Assign { op: None, target: Place::Index(array, index), value }
                if matches!(e.ty, Type::Array(_)) =>
            {
                let root = self.access(array);
                self.expr(index);
                let Some(root) = root else {
                    self.moved(value);
                    return;
                };
                // The new row is computed while the array is borrowed, so it must not mention it.
                if mentions(value, &root) || mentions(index, &root) {
                    self.bad.insert(root.clone());
                }
                let owned = self.owned(value);
                self.scan.rows.push((root.clone(), owned));
                self.scan.reshaped.insert(root.clone());
                self.scan.written.insert(root);
                self.moved(value);
            }
            _ => self.expr(e),
        }
    }

    /// `e` where its value is handed on: a variable there is moved, or else shared.
    fn moved(&mut self, e: &'m Expr) {
        match &e.kind {
            ExprKind::Var(v) if self.is_array(v) => {
                if self.movable(v) {
                    self.scan.moves.push(e.id);
                } else {
                    self.bad.insert(v.clone());
                }
            }
            _ => self.expr(e),
        }
    }

    /// The array expression of an access (`a` in `a[i]` or `a.length`, `a[i]` in `a[i][j]`):
    /// walks its indexes and returns the array variable at its root, if there is one.
    fn access(&mut self, array: &'m Expr) -> Option<String> {
        match &array.kind {
            ExprKind::Var(v) if self.is_array(v) => Some(v.clone()),
            ExprKind::Index(inner, index) => {
                self.expr(index);
                let root = self.access(inner)?;
                // Passing the array to a call inside the access could replace the row on the way.
                if calls_with(index, &root) {
                    self.bad.insert(root.clone());
                }
                Some(root)
            }
            _ => {
                self.expr(array);
                None
            }
        }
    }

    /// An element read or write `array[index]`, whose element is a row when `row` is set.
    fn element(&mut self, array: &'m Expr, index: &'m Expr, row: bool) -> Option<String> {
        let root = self.access(array);
        self.expr(index);
        if let Some(root) = &root {
            if row {
                // A row read as a value is a second name for it.
                self.bad.insert(root.clone());
            }
            if depth(self.types[root]) > 1 && calls_with(index, root) {
                self.bad.insert(root.clone());
            }
        }
        root
    }

    /// `e` as a value that is not handed on: an array variable here is shared.
    fn expr(&mut self, e: &'m Expr) {
        match &e.kind {
            ExprKind::Var(v) => {
                if self.is_array(v) {
                    self.bad.insert(v.clone());
                }
            }
            ExprKind::Index(array, index) => {
                self.element(array, index, matches!(e.ty, Type::Array(_)));
            }
            ExprKind::Length(array) => {
                self.access(array);
            }
            ExprKind::Assign { target, value, .. } => {
                match target {
                    Place::Var(v) => {
                        // An array assigned inside a larger expression changes under it.
                        if self.is_array(v) {
                            self.bad.insert(v.clone());
                        }
                    }
                    Place::Index(array, index) => {
                        let row = matches!(e.ty, Type::Array(_));
                        if let Some(root) = self.element(array, index, row) {
                            if depth(self.types[&root]) > 1 && calls_with(value, &root) {
                                self.bad.insert(root.clone());
                            }
                            self.scan.written.insert(root);
                        }
                    }
                    Place::Field { object, .. } => self.expr(object),
                    Place::Static { .. } => {}
                }
                self.expr(value);
            }
            ExprKind::IncDec { target, .. } => match target {
                Place::Index(array, index) => {
                    if let Some(root) = self.element(array, index, false) {
                        self.scan.written.insert(root);
                    }
                }
                Place::Field { object, .. } => self.expr(object),
                Place::Var(_) | Place::Static { .. } => {}
            },
            // An atomic update of an element takes the array's handle and borrows it whole, so the
            // array is shared (see `lower::handles`).
            // So does a view of the bytes of an array (`Builtin::ByteView`).
            ExprKind::Builtin(Builtin::ElementUpdate(_) | Builtin::ByteView { .. }, args) => {
                for a in args {
                    match &a.kind {
                        ExprKind::Assign { target: Place::Index(array, index), value, .. } => {
                            self.expr(array);
                            self.expr(index);
                            self.expr(value);
                        }
                        ExprKind::Index(array, index)
                            if matches!(e.kind, ExprKind::Builtin(Builtin::ByteView { .. }, _)) =>
                        {
                            self.expr(array);
                            self.expr(index);
                        }
                        _ => self.expr(a),
                    }
                }
            }
            // `System.arraycopy` reads its source and writes its destination, like indexing.
            ExprKind::Builtin(Builtin::Native(Native::ArrayCopy), args) => {
                for (i, a) in args.iter().enumerate() {
                    match &a.kind {
                        ExprKind::Var(v)
                            if (i == 0 || i == 2) && self.types.get(v).is_some_and(|&t| depth(t) == 1) =>
                        {
                            if i == 2 {
                                self.scan.written.insert(v.clone());
                            }
                        }
                        _ => self.expr(a),
                    }
                }
            }
            ExprKind::Call { args, resolved: Some(callee), .. }
            | ExprKind::New { args, ctor: Some(callee), .. }
            | ExprKind::ThisCall { args, resolved: Some(callee) }
            | ExprKind::SuperInit { args, resolved: Some(callee) } => self.call(*callee, args),
            ExprKind::MethodCall { receiver, args, resolved: Some(callee), .. } => {
                self.expr(receiver);
                self.call(*callee, args);
            }
            ExprKind::MethodCall { receiver, args, resolved: None, .. } => {
                self.expr(receiver);
                args.iter().for_each(|a| self.expr(a));
            }
            ExprKind::Call { args, resolved: None, .. }
            | ExprKind::New { args, ctor: None, .. }
            | ExprKind::ThisCall { args, resolved: None }
            | ExprKind::SuperInit { args, resolved: None }
            | ExprKind::Builtin(_, args)
            | ExprKind::NewArray(args)
            | ExprKind::ArrayLit(args) => args.iter().for_each(|a| self.expr(a)),
            ExprKind::Unary(_, x) | ExprKind::Cast(_, x) => self.expr(x),
            ExprKind::Field { object, .. } => self.expr(object),
            ExprKind::InstanceOf { value, .. } => self.expr(value),
            ExprKind::Binary(_, l, r) => {
                self.expr(l);
                self.expr(r);
            }
            ExprKind::Conditional(c, a, b) => {
                self.expr(c);
                self.expr(a);
                self.expr(b);
            }
            ExprKind::Switch { selector, cases } => {
                self.expr(selector);
                let top = self.top.take();
                self.switches += 1;
                for case in cases {
                    for label in &case.labels {
                        if let CaseLabel::Value(e) = label {
                            self.expr(e);
                        }
                    }
                    // The cases of a switch expression yield values, never return them.
                    self.block(&case.body, Type::Void);
                }
                self.switches -= 1;
                self.top = top;
            }
            ExprKind::Int(_)
            | ExprKind::Long(_)
            | ExprKind::Double(_)
            | ExprKind::Char(_)
            | ExprKind::Float(_)
            | ExprKind::Bool(_)
            | ExprKind::Str(_)
            | ExprKind::Units(_)
            | ExprKind::Null
            | ExprKind::This
            | ExprKind::Static { .. }
            | ExprKind::EnumConstant { .. } => {}
        }
    }

    /// A call of method `callee`: each argument is recorded for the solver, which decides
    /// whether the parameter borrows it.
    fn call(&mut self, callee: usize, args: &'m [Expr]) {
        let mut recorded = Vec::new();
        for a in args {
            let (name, row, moved) = match &a.kind {
                ExprKind::Var(v) if self.is_array(v) => {
                    let moved = self.movable(v);
                    if moved {
                        self.scan.moves.push(a.id);
                    }
                    (Some(v.clone()), false, moved)
                }
                ExprKind::Index(array, index) if matches!(a.ty, Type::Array(_)) => {
                    let root = self.access(array);
                    self.expr(index);
                    // A row is lent while its indexes are computed, so they must leave it alone.
                    let lendable = root.as_ref().is_some_and(|r| mentions_only_as_root(a, r));
                    match root {
                        Some(r) if lendable => (Some(r), true, false),
                        Some(r) => {
                            self.bad.insert(r);
                            (None, false, false)
                        }
                        None => (None, false, false),
                    }
                }
                _ => {
                    self.expr(a);
                    (None, false, false)
                }
            };
            let fresh = if name.is_none() { self.owned(a) } else { Owned::Never };
            let fresh = match fresh {
                Owned::Moved(_) => Owned::Never,
                other => other,
            };
            recorded.push(Arg::of(a, name, row, fresh, moved));
        }
        self.scan.calls.push(Call { callee, args: recorded.into() });
    }
}

/// Whether `array`, the array part of a row `array[i]`, mentions `root` only as its root, as
/// `m` in `m[i][j]`.
fn mentions_only_as_root(array: &Expr, root: &str) -> bool {
    match &array.kind {
        ExprKind::Var(v) => v == root,
        ExprKind::Index(inner, index) => !mentions(index, root) && mentions_only_as_root(inner, root),
        _ => false,
    }
}

/// Whether `e` passes the variable `name`, or something that mentions it, to a call.
fn calls_with(e: &Expr, name: &str) -> bool {
    let mut found = false;
    visit_expr(e, &mut |x| match &x.kind {
        ExprKind::Call { args, .. }
        | ExprKind::New { args, .. }
        | ExprKind::ThisCall { args, .. }
        | ExprKind::SuperInit { args, .. }
        | ExprKind::MethodCall { args, .. }
        | ExprKind::Builtin(_, args) => found |= args.iter().any(|a| mentions(a, name)),
        _ => {}
    });
    found
}

impl Arg {
    fn of(e: &Expr, name: Option<String>, row: bool, fresh: Owned, moved: bool) -> Arg {
        let mut mentions = FastSet::default();
        let mut effects = false;
        visit_expr(e, &mut |x| match &x.kind {
            ExprKind::Var(v) => {
                mentions.insert(v.clone());
            }
            ExprKind::Call { .. }
            | ExprKind::MethodCall { .. }
            | ExprKind::New { .. }
            | ExprKind::ThisCall { .. }
            | ExprKind::SuperInit { .. }
            | ExprKind::Assign { .. }
            | ExprKind::IncDec { .. }
            | ExprKind::Builtin(Builtin::ArraysFill | Builtin::ArraysSort | Builtin::Arrays(_), _) => {
                effects = true
            }
            _ => {}
        });
        Arg { name, row, fresh, moved, array: matches!(e.ty, Type::Array(_)), mentions, effects }
    }
}
