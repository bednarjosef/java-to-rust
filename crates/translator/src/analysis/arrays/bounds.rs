//! Indexes of arrays that are always in bounds, so that `a[i]` needs no check of its own: of plain
//! arrays, and of other arrays held in a local variable or a parameter.
//! An index `i` (or `i + c`) is in bounds at an access when the range analysis shows it is not
//! negative, and a condition that still holds there bounds it by the array's length:
//!
//! - `for (int i = 0; i < a.length; i++) a[i]`, and `while (i < n)`, `if (i <= limit)`, the left
//!   side of `&&` and the condition of `?:` bound `i` in what they guard, up to the first
//!   statement that assigns `i` or a variable of the bound;
//! - the length is the array's own (`a.length`, `m[i].length`, `this.f.length`), or the size it
//!   was made with
//!   (`new boolean[limit + 1]`, `new int[n][n]` with `int n = a.length`), given that neither the
//!   array nor the variables of the size are ever assigned again;
//! - outside the constructors, an array field of `this` that only constructors assign
//!   (`Facts::fixed_fields`, Java's `final` fields) is such an array too, as the JIT takes one;
//! - `i * i <= n` bounds `i` by `n` where `i` is at least 1 and the product cannot overflow.
//!
//! A row's length is known only while no row of the array is replaced and the array is not
//! passed to a method, which could replace one. Constant sizes are also compared with the range
//! analysis's bounds: `a[2]` of `new int[3]`.

use super::super::Facts;
use super::super::{FastMap, FastSet};
use super::declarations;
use crate::ast::*;

/// The accesses of arrays in `method` whose index is always in bounds: element reads and
/// writes and row reads, by expression id.
pub(super) fn in_bounds(
    class: &Class,
    method: &Method,
    plain: &FastSet<String>,
    reshaped: &FastSet<String>,
    facts: &Facts,
) -> Vec<ExprId> {
    let mut declared: FastMap<&str, usize> = FastMap::default();
    let mut inits: FastMap<&str, &Expr> = FastMap::default();
    // Variables declared by for-each loops only: each lives in its own loop and is not assigned
    // there, so a name reused by several loops is as good as one declared once.
    let mut loops_only: FastSet<&str> = FastSet::default();
    let mut others: FastSet<&str> = FastSet::default();
    for p in &method.params {
        *declared.entry(&p.name).or_default() += 1;
        others.insert(&p.name);
    }
    for d in declarations(method) {
        *declared.entry(d.name).or_default() += 1;
        if d.loop_var {
            loops_only.insert(d.name);
        } else {
            others.insert(d.name);
        }
        if let Some(init) = d.init {
            inits.insert(d.name, init);
        }
    }
    let mut assigned: FastSet<String> = FastSet::default();
    let mut lent: FastSet<String> = FastSet::default();
    for stmt in &method.body {
        visit_stmt(
            stmt,
            &mut |e| {
                visit_expr(e, &mut |x| match &x.kind {
                    ExprKind::Assign { target: Place::Var(v), .. }
                    | ExprKind::IncDec { target: Place::Var(v), .. } => {
                        assigned.insert(v.clone());
                    }
                    // A method that is given a plain array of arrays, or a row of one, may replace
                    // its rows; the library methods take only arrays that are not plain.
                    ExprKind::Call { args, .. }
                    | ExprKind::MethodCall { args, .. }
                    | ExprKind::New { args, .. }
                    | ExprKind::ThisCall { args, .. }
                    | ExprKind::SuperInit { args, .. } => {
                        let arrays = args.iter().filter(|a| matches!(a.ty, Type::Array(_)));
                        lent.extend(arrays.filter_map(root_of).map(str::to_string));
                    }
                    _ => {}
                })
            },
            &mut |_| {},
        );
    }
    let stable = declared
        .iter()
        .filter(|(name, n)| {
            (**n == 1 || (loops_only.contains(*name) && !others.contains(*name)))
                && !assigned.contains(**name)
        })
        .map(|(name, _)| *name)
        .collect();
    let mut walk = Walk {
        class,
        ctor: method.is_ctor,
        facts,
        plain,
        stable,
        inits,
        fixed_rows: plain.iter().filter(|r| !reshaped.contains(*r) && !lent.contains(*r)).cloned().collect(),
        guards: Vec::new(),
        proven: Vec::new(),
    };
    walk.block(&method.body);
    walk.proven
}

/// The array variable at the root of an access path: `m` of `m`, `m[i]` or `m[i][j]`.
fn root_of(e: &Expr) -> Option<&str> {
    match &e.kind {
        ExprKind::Var(v) => Some(v),
        ExprKind::Index(inner, _) => root_of(inner),
        _ => None,
    }
}

/// A symbolic bound: `base + offset`, or the constant `offset` when there is no base.
#[derive(Clone, Debug, PartialEq)]
struct Bound {
    base: Option<Base>,
    offset: i64,
}

#[derive(Clone, Debug, PartialEq)]
enum Base {
    /// The value of an integer variable.
    Var(String),
    /// The length of an array: of the variable, or of its row reached through the index
    /// variables, as `m[i].length`.
    Len(String, Vec<String>),
}

impl Bound {
    fn constant(offset: i64) -> Bound {
        Bound { base: None, offset }
    }

    fn plus(self, c: i64) -> Bound {
        Bound { base: self.base, offset: self.offset + c }
    }

    /// The variables the bound depends on.
    fn vars(&self) -> Vec<String> {
        match &self.base {
            None => Vec::new(),
            Some(Base::Var(v)) => vec![v.clone()],
            Some(Base::Len(root, rows)) => {
                std::iter::once(root.clone()).chain(rows.iter().cloned()).collect()
            }
        }
    }
}

/// `var <= bound` while the guard holds; with `positive`, only where `var` is at least 1.
#[derive(Clone, Debug)]
struct Guard {
    var: String,
    bound: Bound,
    positive: bool,
    /// Whether `var` is known not to be negative: a counter that counts up from zero.
    counts_up: bool,
    /// Every variable the guard depends on; assigning one ends it.
    vars: FastSet<String>,
}

struct Walk<'a> {
    class: &'a Class,
    /// Whether the method is a constructor, which may assign its fixed fields.
    ctor: bool,
    facts: &'a Facts,
    plain: &'a FastSet<String>,
    /// Variables declared once and never assigned after their declaration.
    stable: FastSet<&'a str>,
    inits: FastMap<&'a str, &'a Expr>,
    /// Plain arrays whose rows are never replaced, so the length of each row stays the same.
    fixed_rows: FastSet<String>,
    guards: Vec<Guard>,
    proven: Vec<ExprId>,
}

impl Walk<'_> {
    fn is_stable(&self, name: &str) -> bool {
        self.stable.contains(name)
    }

    /// The name standing for `this.f`, `"this.f"`, where `f` is an array field that only
    /// constructors assign and the method is not one: its array is the same all through the
    /// method, as a variable never assigned again is. No variable has a name with a dot.
    fn fixed_field(&self, e: &Expr) -> Option<String> {
        let ExprKind::Field { object, name } = &e.kind else { return None };
        if self.ctor || !matches!(object.kind, ExprKind::This) || !matches!(e.ty, Type::Array(_)) {
            return None;
        }
        let owner = self.class.field_owner(object.ty, name)?;
        self.facts.fixed_fields.contains(&(owner, name.clone())).then(|| format!("this.{name}"))
    }

    /// The bound that `e` evaluates to where a guard is tested, if it has a symbolic form.
    fn bound_of(&self, e: &Expr) -> Option<(Bound, FastSet<String>)> {
        match &e.kind {
            ExprKind::Int(c) => Some((Bound::constant(*c as i64), FastSet::default())),
            ExprKind::Var(v) => {
                let bound = self.named(v).unwrap_or(Bound { base: Some(Base::Var(v.clone())), offset: 0 });
                let mut vars = FastSet::from_iter([v.clone()]);
                vars.extend(bound.vars());
                Some((bound, vars))
            }
            ExprKind::Length(_) => self.length_of(e),
            ExprKind::Binary(op @ (BinOp::Add | BinOp::Sub), l, r)
                if self.facts.no_overflow.contains(&e.id) =>
            {
                let ExprKind::Int(c) = r.kind else { return None };
                let (bound, vars) = self.bound_of(l)?;
                Some((bound.plus(if *op == BinOp::Add { c as i64 } else { -(c as i64) }), vars))
            }
            _ => None,
        }
    }

    /// `path.length` as a bound, where the path is an array variable or a row of one reached
    /// through index variables.
    fn length_of(&self, e: &Expr) -> Option<(Bound, FastSet<String>)> {
        let ExprKind::Length(path) = &e.kind else { return None };
        let (root, rows) = self.index_vars(path)?;
        let vars = std::iter::once(root.clone()).chain(rows.iter().cloned()).collect();
        Some((Bound { base: Some(Base::Len(root, rows)), offset: 0 }, vars))
    }

    /// The root and the index variables of an access path whose indexes are all variables.
    fn index_vars(&self, path: &Expr) -> Option<(String, Vec<String>)> {
        match &path.kind {
            ExprKind::Var(v) => Some((v.clone(), Vec::new())),
            ExprKind::Field { .. } => Some((self.fixed_field(path)?, Vec::new())),
            ExprKind::Index(inner, index) => {
                let ExprKind::Var(i) = &index.kind else { return None };
                let (root, mut rows) = self.index_vars(inner)?;
                rows.push(i.clone());
                Some((root, rows))
            }
            _ => None,
        }
    }

    /// What a variable that is never assigned again stands for: the constant it starts with, or
    /// the length of an array variable that is never assigned again (`int n = a.length`).
    fn named(&self, v: &str) -> Option<Bound> {
        if !self.is_stable(v) {
            return None;
        }
        match &self.inits.get(v)?.kind {
            ExprKind::Int(c) => Some(Bound::constant(*c as i64)),
            ExprKind::Length(array) => match &array.kind {
                ExprKind::Var(a) if self.is_stable(a) => {
                    Some(Bound { base: Some(Base::Len(a.clone(), Vec::new())), offset: 0 })
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// The value of a size of a new array, which holds wherever the array is used: constants
    /// and variables that are never assigned. The length made is at least this value whenever
    /// the array exists, even where the size's arithmetic wraps (a negative size throws).
    fn size_of(&self, e: &Expr) -> Option<Bound> {
        match &e.kind {
            ExprKind::Int(c) => Some(Bound::constant(*c as i64)),
            ExprKind::Var(v) if self.is_stable(v) => {
                Some(self.named(v).unwrap_or(Bound { base: Some(Base::Var(v.clone())), offset: 0 }))
            }
            ExprKind::Length(array) => match &array.kind {
                ExprKind::Var(a) if self.is_stable(a) => {
                    Some(Bound { base: Some(Base::Len(a.clone(), Vec::new())), offset: 0 })
                }
                _ => None,
            },
            ExprKind::Binary(op @ (BinOp::Add | BinOp::Sub), l, r) => {
                let ExprKind::Int(c) = r.kind else { return None };
                Some(self.size_of(l)?.plus(if *op == BinOp::Add { c as i64 } else { -(c as i64) }))
            }
            _ => None,
        }
    }

    /// The sizes a stable array variable was made with, per level: `new int[n][m]` gives
    /// `[n, m]`, `{{1, 2}, {3, 4}}` gives `[2, 2]`.
    fn sizes(&self, root: &str) -> Vec<Option<Bound>> {
        if !self.is_stable(root) {
            return Vec::new();
        }
        let Some(init) = self.inits.get(root) else { return Vec::new() };
        let mut sizes = Vec::new();
        let mut level = Some(*init);
        while let Some(e) = level {
            level = None;
            match &e.kind {
                ExprKind::NewArray(dims) => {
                    sizes.extend(dims.iter().map(|d| self.size_of(d)));
                }
                ExprKind::ArrayLit(items) => {
                    sizes.push(Some(Bound::constant(items.len() as i64)));
                    // The next level is known when every row is a literal of the same length.
                    let lengths: FastSet<usize> = items
                        .iter()
                        .map(|item| match &item.kind {
                            ExprKind::ArrayLit(row) => Some(row.len()),
                            _ => None,
                        })
                        .collect::<Option<_>>()
                        .unwrap_or_default();
                    if lengths.len() == 1 {
                        level = items.first();
                    }
                }
                _ => {}
            }
        }
        sizes
    }

    /// What the length of the array at the access path `array` is known to be.
    fn lengths(&self, array: &Expr) -> Vec<Bound> {
        let mut rows = 0;
        let mut root = array;
        while let ExprKind::Index(inner, _) = &root.kind {
            rows += 1;
            root = inner;
        }
        if let Some(field) = self.fixed_field(root) {
            return if rows == 0 {
                vec![Bound { base: Some(Base::Len(field, Vec::new())), offset: 0 }]
            } else {
                Vec::new()
            };
        }
        let ExprKind::Var(root) = &root.kind else { return Vec::new() };
        if rows > 0 && !self.fixed_rows.contains(root) {
            return Vec::new();
        }
        let mut lengths = Vec::new();
        if let Some((root, vars)) = self.index_vars(array) {
            lengths.push(Bound { base: Some(Base::Len(root, vars)), offset: 0 });
        }
        if let Some(Some(size)) = self.sizes(root).get(rows) {
            lengths.push(size.clone());
        }
        lengths
    }

    /// The guards that the condition `cond` sets up where it is true.
    fn guards_of(&self, cond: &Expr) -> Vec<Guard> {
        let mut guards = Vec::new();
        for c in conjuncts(cond) {
            let ExprKind::Binary(op, l, r) = &c.kind else { continue };
            let (var, bound, strict, positive) = match (op, &l.kind, &r.kind) {
                (BinOp::Lt | BinOp::Le, ExprKind::Var(v), _) => (v, &**r, *op == BinOp::Lt, false),
                (BinOp::Gt | BinOp::Ge, _, ExprKind::Var(v)) => (v, &**l, *op == BinOp::Gt, false),
                (BinOp::Lt | BinOp::Le, ExprKind::Binary(BinOp::Mul, a, b), _)
                    if self.facts.no_overflow.contains(&l.id) =>
                {
                    match (&a.kind, &b.kind) {
                        (ExprKind::Var(x), ExprKind::Var(y)) if x == y => (x, &**r, *op == BinOp::Lt, true),
                        _ => continue,
                    }
                }
                _ => continue,
            };
            if !matches!(c.ty, Type::Boolean) || l.ty == Type::Long || r.ty == Type::Long {
                continue;
            }
            let Some((bound, mut vars)) = self.bound_of(bound) else { continue };
            let bound = if strict { bound.plus(-1) } else { bound };
            vars.insert(var.clone());
            vars.extend(bound.vars());
            guards.push(Guard { var: var.clone(), bound, positive, counts_up: false, vars });
        }
        guards
    }

    /// Whether `s` may change what guard `g` depends on.
    fn ends(&self, s: &Stmt, g: &Guard) -> bool {
        g.vars.iter().any(|v| assigns_stmt(s, v))
    }

    /// Walks `body` with `guards` holding at its start.
    fn guarded(&mut self, guards: Vec<Guard>, body: &Stmt) {
        let mark = self.guards.len();
        self.guards.extend(guards);
        match body {
            Stmt::Block(stmts) => self.block(stmts),
            _ => {
                self.end_guards(mark, body);
                self.stmt(body);
            }
        }
        self.guards.truncate(mark);
    }

    /// Drops the guards from `mark` on that statement `s` may end. Guards from outside the
    /// statement that encloses `s` were already checked against all of it.
    fn end_guards(&mut self, mark: usize, s: &Stmt) {
        let ended: Vec<bool> = self.guards[mark..].iter().map(|g| self.ends(s, g)).collect();
        let mut ended = ended.into_iter();
        let mut kept = self.guards.split_off(mark);
        kept.retain(|_| !ended.next().unwrap_or(false));
        self.guards.extend(kept);
    }

    /// The statements of a block, each guarded by what holds before it. A guard that a
    /// statement may end is dropped before it; only guards the block itself set up can be, as
    /// the statement enclosing the block was checked whole.
    fn block(&mut self, stmts: &[Stmt]) {
        for s in stmts {
            self.end_guards(0, s);
            self.stmt(s);
        }
    }

    fn stmt(&mut self, s: &Stmt) {
        match s {
            Stmt::Local { init, .. } => {
                if let Some(e) = init {
                    self.expr(e);
                }
            }
            Stmt::Expr(e) | Stmt::Return(Some(e), _) | Stmt::Yield(e) => self.expr(e),
            Stmt::If { cond, then, otherwise } => {
                self.expr(cond);
                let guards = self.guards_of(cond);
                self.guarded(guards, then);
                if let Some(o) = otherwise {
                    self.stmt(o);
                }
            }
            Stmt::While { cond, body } => {
                self.expr(cond);
                let guards = self.guards_of(cond);
                self.guarded(guards, body);
            }
            Stmt::DoWhile { body, cond } => {
                self.stmt(body);
                self.expr(cond);
            }
            Stmt::For { init, cond, update, body } => {
                self.block(init);
                let Some(cond) = cond else {
                    update.iter().for_each(|u| self.expr(u));
                    self.stmt(body);
                    return;
                };
                self.expr(cond);
                update.iter().for_each(|u| self.expr(u));
                let mut guards = self.guards_of(cond);
                for g in &mut guards {
                    g.counts_up = counts_up_from_zero(init, cond, update, body, &g.var);
                }
                self.guarded(guards, body);
            }
            Stmt::ForEach { array, body, .. } => {
                self.expr(array);
                self.stmt(body);
            }
            Stmt::Switch { selector, cases } => {
                self.expr(selector);
                for case in cases {
                    self.block(&case.body);
                }
            }
            Stmt::Labeled { body, .. } => self.stmt(body),
            Stmt::Block(stmts) => self.block(stmts),
            Stmt::Throw(e) => self.expr(e),
            // A `catch` or the `finally` may start after any part of the body; the guards from
            // outside were checked against the whole statement.
            Stmt::Try { body, catches, finally } => {
                self.block(body);
                for catch in catches {
                    self.block(&catch.body);
                }
                if let Some(f) = finally {
                    self.block(f);
                }
            }
            Stmt::Return(None, _)
            | Stmt::Break
            | Stmt::Continue
            | Stmt::BreakLabel(_)
            | Stmt::ContinueLabel(_) => {}
        }
    }

    fn expr(&mut self, e: &Expr) {
        match &e.kind {
            ExprKind::Binary(BinOp::And, l, r) => {
                self.expr(l);
                let guards: Vec<Guard> =
                    self.guards_of(l).into_iter().filter(|g| g.vars.iter().all(|v| !assigns(r, v))).collect();
                let mark = self.guards.len();
                self.guards.extend(guards);
                self.expr(r);
                self.guards.truncate(mark);
            }
            ExprKind::Conditional(c, a, b) => {
                self.expr(c);
                let guards: Vec<Guard> =
                    self.guards_of(c).into_iter().filter(|g| g.vars.iter().all(|v| !assigns(a, v))).collect();
                let mark = self.guards.len();
                self.guards.extend(guards);
                self.expr(a);
                self.guards.truncate(mark);
                self.expr(b);
            }
            ExprKind::Switch { selector, cases } => {
                self.expr(selector);
                for case in cases {
                    for label in &case.labels {
                        if let CaseLabel::Value(v) = label {
                            self.expr(v);
                        }
                    }
                    self.block(&case.body);
                }
            }
            _ => {
                match &e.kind {
                    ExprKind::Index(array, index)
                    | ExprKind::Assign { target: Place::Index(array, index), .. }
                    | ExprKind::IncDec { target: Place::Index(array, index), .. }
                        if self.proven(e.id, array, index) =>
                    {
                        self.proven.push(e.id);
                    }
                    _ => {}
                }
                for child in children(e) {
                    self.expr(child);
                }
            }
        }
    }

    /// Whether the access `array[index]` with id `id` is always in bounds.
    fn proven(&self, id: ExprId, array: &Expr, index: &Expr) -> bool {
        // An array that is not plain is reached through its variable alone, whose array is never
        // replaced where a guard of its length holds (`ends`), and whose length never changes.
        let plain = root_of(array).is_some_and(|r| self.plain.contains(r));
        if !plain && !matches!(array.kind, ExprKind::Var(_)) && self.fixed_field(array).is_none() {
            return false;
        }
        let range = self.facts.index_ranges.get(&id).copied();
        if range.is_some_and(|(lo, hi)| lo > hi) {
            // The access is never reached.
            return true;
        }
        let lengths = self.lengths(array);
        let (lo, hi) = range.unwrap_or((i64::MIN, i64::MAX));
        if lo >= 0 && lengths.iter().any(|l| l.base.is_none() && hi < l.offset) {
            return true;
        }
        let (var, c) = match &index.kind {
            ExprKind::Var(v) => (v, 0),
            ExprKind::Binary(op @ (BinOp::Add | BinOp::Sub), l, r)
                if self.facts.no_overflow.contains(&index.id) =>
            {
                match (&l.kind, &r.kind) {
                    (ExprKind::Var(v), ExprKind::Int(c)) => {
                        (v, if *op == BinOp::Add { *c as i64 } else { -(*c as i64) })
                    }
                    _ => return false,
                }
            }
            _ => return false,
        };
        self.guards.iter().filter(|g| &g.var == var).any(|g| {
            let low = lo >= 0 || (g.counts_up && c >= 0);
            let positive = !g.positive || (c == 0 && lo >= 1);
            low && positive && lengths.iter().any(|l| l.base == g.bound.base && g.bound.offset + c < l.offset)
        })
    }
}

/// Whether the `for` loop is a counting loop (`ast::counting`) that counts `var` up from a
/// constant that is not negative: then `var` is never negative in the body, as the loop never
/// wraps around.
fn counts_up_from_zero(init: &[Stmt], cond: &Expr, update: &[Expr], body: &Stmt, var: &str) -> bool {
    counting(init, Some(cond), update, body)
        .is_some_and(|c| c.var == var && !c.descending && matches!(c.from.kind, ExprKind::Int(k) if k >= 0))
}

/// The conjuncts of `a && b && ...`.
fn conjuncts(e: &Expr) -> Vec<&Expr> {
    match &e.kind {
        ExprKind::Binary(BinOp::And, l, r) => {
            let mut out = conjuncts(l);
            out.extend(conjuncts(r));
            out
        }
        _ => vec![e],
    }
}

/// The expressions directly inside `e`, in evaluation order. A switch expression's cases are
/// walked by the caller.
fn children(e: &Expr) -> Vec<&Expr> {
    match &e.kind {
        ExprKind::Unary(_, x)
        | ExprKind::Cast(_, x)
        | ExprKind::Length(x)
        | ExprKind::Field { object: x, .. } => {
            vec![x]
        }
        ExprKind::Assign { target, value, .. } => {
            let mut out = place_children(target);
            out.push(value);
            out
        }
        ExprKind::IncDec { target, .. } => place_children(target),
        ExprKind::Binary(_, l, r) | ExprKind::Index(l, r) => vec![l, r],
        ExprKind::Conditional(c, a, b) => vec![c, a, b],
        ExprKind::MethodCall { receiver, args, .. } => std::iter::once(&**receiver).chain(args).collect(),
        ExprKind::Call { args, .. }
        | ExprKind::Builtin(_, args)
        | ExprKind::New { args, .. }
        | ExprKind::ThisCall { args, .. }
        | ExprKind::SuperInit { args, .. }
        | ExprKind::NewArray(args)
        | ExprKind::ArrayLit(args) => args.iter().collect(),
        _ => Vec::new(),
    }
}

fn place_children(place: &Place) -> Vec<&Expr> {
    match place {
        Place::Index(array, index) => vec![array, index],
        Place::Field { object, .. } => vec![object],
        Place::Var(_) | Place::Static { .. } => Vec::new(),
    }
}
