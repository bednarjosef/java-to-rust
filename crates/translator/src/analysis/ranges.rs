//! Interval analysis of `int` and `long` values by abstract interpretation: every integer
//! variable gets the range of values it can hold at each point of the program.
//!
//! - Conditions narrow ranges (`if (n < 2) return;` leaves `n >= 2`), including `d * d <= n`,
//!   which bounds `d` by the square root of `n` provided `d * d` itself cannot overflow (checked
//!   afterwards; the analysis is redone without that narrowing when it can).
//! - Loops are iterated to a fixpoint. A bound that keeps growing is widened to the next constant
//!   of the program (a literal or a parameter bound) and, failing that, to the end of the type;
//!   then the ranges are narrowed again.
//! - A counter only ever increased inside a counting loop `for (i = a; i < b; i++)` grows by at
//!   most its increment times the number of iterations, which bounds counters like `count++`.
//! - Calls carry argument ranges into the called method and the method's return range back.
//!
//! Each arithmetic operation is checked against its type's bounds on the final fixpoint; one
//! that stays within them on every path cannot overflow.

use super::Facts;
use super::constants::static_value;
use crate::ast::*;
use crate::types::binary_type;
use std::collections::{HashMap, HashSet};

pub fn analyze(class: &Class, facts: &mut Facts) {
    let mut no_square_narrowing = HashSet::new();
    loop {
        let mut a = Analysis::new(class, no_square_narrowing.clone());
        a.run();
        let unsound: Vec<ExprId> =
            a.square_narrowed.iter().copied().filter(|id| a.overflow.contains(id)).collect();
        if unsound.is_empty() {
            facts.no_overflow.extend(a.visited.difference(&a.overflow));
            facts.safe_division.extend(a.visited.difference(&a.risky_division));
            facts.ascii_chars.extend(a.chars.difference(&a.wide_chars));
            let clamp = |v: i128| v.clamp(i64::MIN as i128, i64::MAX as i128) as i64;
            facts.index_ranges.extend(a.indexes.iter().map(|(&id, v)| (id, (clamp(v.lo), clamp(v.hi)))));
            return;
        }
        no_square_narrowing.extend(unsound);
    }
}

/// The integers from `lo` to `hi`; empty when `lo > hi`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Interval {
    lo: i128,
    hi: i128,
}

const EMPTY: Interval = Interval { lo: 1, hi: 0 };
const UNBOUNDED: i128 = 1 << 100;

impl Interval {
    fn new(lo: i128, hi: i128) -> Interval {
        if lo > hi { EMPTY } else { Interval { lo, hi } }
    }

    fn point(v: i128) -> Interval {
        Interval { lo: v, hi: v }
    }

    /// Every value of an integer type; for other types a placeholder that is never inspected.
    fn full(ty: Type) -> Interval {
        match ty {
            Type::Int => Interval { lo: i32::MIN as i128, hi: i32::MAX as i128 },
            Type::Byte | Type::Short | Type::Char => {
                let (lo, hi) = crate::types::integral_bounds(ty);
                Interval { lo: lo as i128, hi: hi as i128 }
            }
            _ => Interval { lo: i64::MIN as i128, hi: i64::MAX as i128 },
        }
    }

    fn is_empty(self) -> bool {
        self.lo > self.hi
    }

    fn join(self, other: Interval) -> Interval {
        if self.is_empty() {
            other
        } else if other.is_empty() {
            self
        } else {
            Interval { lo: self.lo.min(other.lo), hi: self.hi.max(other.hi) }
        }
    }

    fn meet(self, other: Interval) -> Interval {
        Interval::new(self.lo.max(other.lo), self.hi.min(other.hi))
    }

    fn contains(self, v: i128) -> bool {
        self.lo <= v && v <= self.hi
    }

    fn within(self, other: Interval) -> bool {
        self.is_empty() || (other.lo <= self.lo && self.hi <= other.hi)
    }
}

/// Converts a value of type `from` to type `to`, as an assignment or cast does.
fn convert(v: Interval, from: Type, to: Type) -> Interval {
    if !to.is_integral() {
        Interval::full(Type::Long)
    } else if from.is_integral() && v.within(Interval::full(to)) {
        v
    } else {
        Interval::full(to)
    }
}

#[derive(Clone, Copy)]
struct Var {
    ty: Type,
    range: Interval,
}

/// The ranges of the integer variables in scope; `None` where the program cannot be.
type Env = HashMap<String, Var>;

fn join(a: Option<Env>, b: Option<Env>) -> Option<Env> {
    match (a, b) {
        (None, x) | (x, None) => x,
        // A variable missing on one side went out of scope.
        (Some(a), Some(b)) => Some(
            a.into_iter()
                .filter_map(|(k, v)| {
                    let w = b.get(&k)?;
                    Some((k, Var { ty: v.ty, range: v.range.join(w.range) }))
                })
                .collect(),
        ),
    }
}

/// `env` with every variable that the statements assign at the full range of its type.
fn widened<'s>(env: &Env, stmts: impl Iterator<Item = &'s Stmt> + Clone) -> Env {
    env.iter()
        .map(|(k, v)| {
            let assigned = stmts.clone().any(|s| assigns_stmt(s, k));
            (k.clone(), if assigned { Var { ty: v.ty, range: Interval::full(v.ty) } } else { *v })
        })
        .collect()
}

fn within(a: &Env, b: &Env) -> bool {
    a.iter().all(|(k, v)| b.get(k).is_some_and(|w| v.range.within(w.range)))
}

/// Joins `next` into `head`, sending bounds that still move to the next threshold or the end of
/// the type.
fn widen(head: &Env, next: &Env, thresholds: &[i128]) -> Env {
    next.iter()
        .filter_map(|(k, n)| {
            let h = head.get(k)?;
            let range = if h.range.is_empty() {
                n.range
            } else if n.range.is_empty() {
                h.range
            } else {
                let full = Interval::full(n.ty);
                let lo = if n.range.lo < h.range.lo {
                    let i = thresholds.partition_point(|&t| t <= n.range.lo);
                    if i > 0 && thresholds[i - 1] >= full.lo { thresholds[i - 1] } else { full.lo }
                } else {
                    h.range.lo
                };
                let hi = if n.range.hi > h.range.hi {
                    let i = thresholds.partition_point(|&t| t < n.range.hi);
                    if i < thresholds.len() && thresholds[i] <= full.hi { thresholds[i] } else { full.hi }
                } else {
                    h.range.hi
                };
                Interval { lo, hi }
            };
            Some((k.clone(), Var { ty: n.ty, range }))
        })
        .collect()
}

/// What a frame of `Analysis::loops` is: a loop, a switch statement, a switch expression, or a
/// labeled statement that is not a loop.
#[derive(Clone, Copy, PartialEq)]
enum Frame {
    Loop,
    Switch,
    Expr,
    Labeled,
}

/// Where `break`, `continue` and `yield` leave a frame.
struct Flow {
    kind: Frame,
    label: Option<String>,
    breaks: Option<Env>,
    continues: Option<Env>,
    /// The values that `yield` gives (for a switch expression).
    value: Interval,
}

impl Flow {
    fn new(kind: Frame, label: Option<String>) -> Flow {
        Flow { kind, label, breaks: None, continues: None, value: EMPTY }
    }
}

struct Analysis<'a> {
    class: &'a Class,
    /// Ranges of each method's parameters, joined over all calls; empty until called.
    params: Vec<Vec<Interval>>,
    returns: Vec<Interval>,
    reached: Vec<bool>,
    /// False in the final pass, which only records facts.
    update_summaries: bool,
    method: usize,
    ret: Interval,
    /// Whether facts are recorded: only in the final pass over a stable state.
    record: bool,
    visited: HashSet<ExprId>,
    overflow: HashSet<ExprId>,
    /// The values each array index takes, keyed by the indexing or element assignment.
    indexes: HashMap<ExprId, Interval>,
    risky_division: HashSet<ExprId>,
    loops: Vec<Flow>,
    /// Upper bounds of counters inside counting loops: (variable, bound).
    budgets: Vec<(String, i128)>,
    no_square_narrowing: HashSet<ExprId>,
    square_narrowed: HashSet<ExprId>,
    /// `(char)` casts, and those whose value some recorded visit leaves outside ASCII.
    chars: HashSet<ExprId>,
    wide_chars: HashSet<ExprId>,
    /// Where widening stops before the end of the type, for the current method: its literals
    /// and the bounds of its parameters, sorted.
    thresholds: Vec<i128>,
    /// The last invariant found for each loop of the current method, keyed by the loop's
    /// address. An inner loop is solved again on every pass over the outer loop; starting from
    /// its last invariant usually confirms it in one pass instead of rebuilding it.
    heads: HashMap<usize, Env>,
}

impl<'a> Analysis<'a> {
    fn new(class: &'a Class, no_square_narrowing: HashSet<ExprId>) -> Analysis<'a> {
        let n = class.methods.len();
        Analysis {
            class,
            params: class.methods.iter().map(|m| vec![EMPTY; m.params.len()]).collect(),
            returns: vec![EMPTY; n],
            reached: vec![false; n],
            update_summaries: true,
            method: 0,
            ret: EMPTY,
            record: false,
            visited: HashSet::new(),
            overflow: HashSet::new(),
            indexes: HashMap::new(),
            risky_division: HashSet::new(),
            loops: Vec::new(),
            budgets: Vec::new(),
            no_square_narrowing,
            square_narrowed: HashSet::new(),
            chars: HashSet::new(),
            wide_chars: HashSet::new(),
            thresholds: Vec::new(),
            heads: HashMap::new(),
        }
    }

    fn full_params(&mut self, m: usize) {
        self.reached[m] = true;
        self.params[m] = self.class.methods[m].params.iter().map(|p| Interval::full(p.ty)).collect();
    }

    fn run(&mut self) {
        let n = self.class.methods.len();
        let callees_of: Vec<Vec<usize>> = self.class.methods.iter().map(|m| callees(self.class, m)).collect();
        let callers = callers(&callees_of);
        // Methods that nothing calls, and callees of field initializers (not analyzed here), take
        // any arguments. Methods reachable only through a cycle of calls from them do too, since
        // no caller's arguments can be seen. The others get their arguments from their callers.
        let initializers: HashSet<usize> = initializer_callees(self.class).into_iter().collect();
        let is_root: Vec<bool> = (0..n).map(|m| callers[m].is_empty() || initializers.contains(&m)).collect();
        let mut live = is_root.clone();
        let mut todo: Vec<usize> = (0..n).filter(|&m| is_root[m]).collect();
        while let Some(m) = todo.pop() {
            for &c in &callees_of[m] {
                if !live[c] {
                    live[c] = true;
                    todo.push(c);
                }
            }
        }
        for m in (0..n).filter(|&m| is_root[m] || !live[m]) {
            self.full_params(m);
        }
        // Every method is analyzed once, so that a callee whose parameters never change (one
        // without parameters) still has a return range; then again when its parameter ranges or
        // a callee's return range grew.
        // The first dirty method is taken first.
        let mut dirty: std::collections::BTreeSet<usize> = (0..n).collect();
        // How often each method's summary grew; recursion can grow it a step at a time.
        let mut growth = vec![0; n];
        while let Some(m) = dirty.pop_first() {
            let returned = self.returns[m];
            let params: Vec<(usize, Vec<Interval>)> =
                callees_of[m].iter().map(|&c| (c, self.params[c].clone())).collect();
            self.method(m);
            for (c, before) in params {
                if self.params[c] != before {
                    growth[c] += 1;
                    if growth[c] > 4 {
                        for (i, p) in self.class.methods[c].params.iter().enumerate() {
                            if !before[i].is_empty() && !self.params[c][i].within(before[i]) {
                                self.params[c][i] = Interval::full(p.ty);
                            }
                        }
                    }
                    dirty.insert(c);
                }
            }
            if self.returns[m] != returned {
                growth[m] += 1;
                if growth[m] > 4 && !returned.is_empty() {
                    self.returns[m] = Interval::full(self.class.methods[m].ret);
                }
                dirty.extend(&callers[m]);
            }
        }
        self.update_summaries = false;
        self.record = true;
        for m in 0..n {
            self.method(m);
        }
    }

    fn method(&mut self, m: usize) {
        let method = &self.class.methods[m];
        let mut env = Env::new();
        self.thresholds = literals(method);
        for (i, p) in method.params.iter().enumerate() {
            if p.ty.is_integral() {
                let range = self.params[m][i];
                env.insert(p.name.clone(), Var { ty: p.ty, range });
                if !range.is_empty() {
                    self.thresholds.extend([range.lo, range.hi]);
                }
            }
        }
        self.thresholds.sort_unstable();
        self.thresholds.dedup();
        self.method = m;
        self.ret = EMPTY;
        self.loops.clear();
        self.budgets.clear();
        self.heads.clear();
        self.stmts(&method.body, Some(env));
        if self.update_summaries && method.ret.is_integral() {
            self.returns[m] = self.returns[m].join(self.ret);
        }
    }

    fn visit(&mut self, id: ExprId) {
        if self.record {
            self.visited.insert(id);
        }
    }

    /// Records that the indexing or element assignment `id` used an index in `i`.
    fn indexed(&mut self, id: ExprId, i: Interval) {
        if self.record {
            let seen = self.indexes.entry(id).or_insert(EMPTY);
            *seen = seen.join(i);
        }
    }

    fn overflowed(&mut self, id: ExprId) {
        if self.record {
            self.overflow.insert(id);
        }
    }

    /// Records a `(char)` cast, and whether its value can leave ASCII (`0..=0x7f`).
    fn char_cast(&mut self, v: Interval, id: ExprId) {
        if self.record {
            self.chars.insert(id);
            if v.is_empty() || v.lo < 0 || v.hi > 0x7f {
                self.wide_chars.insert(id);
            }
        }
    }

    /// Records whether narrowing `v` to the small type `to` keeps every value, as for the
    /// overflow of an operation.
    fn narrowed(&mut self, v: Interval, to: Type, id: ExprId) {
        self.visit(id);
        if !v.within(Interval::full(to)) {
            self.overflowed(id);
        }
    }

    fn stmts(&mut self, stmts: &[Stmt], env: Option<Env>) -> Option<Env> {
        let mut env = env;
        let mut declared = Vec::new();
        for stmt in stmts {
            if let Stmt::Local { name, .. } = stmt {
                declared.push(name);
            }
            env = self.stmt(stmt, env);
            env.as_ref()?;
        }
        env.map(|mut env| {
            for name in declared {
                env.remove(name);
            }
            env
        })
    }

    fn stmt(&mut self, stmt: &Stmt, env: Option<Env>) -> Option<Env> {
        let mut env = env?;
        match stmt {
            Stmt::Local { ty, name, init, .. } => {
                let range = match init {
                    Some(e) => {
                        let v = self.expr(e, &mut env);
                        convert(v, e.ty, *ty)
                    }
                    None => EMPTY,
                };
                if ty.is_integral() {
                    env.insert(name.clone(), Var { ty: *ty, range });
                } else {
                    env.remove(name);
                }
                Some(env)
            }
            Stmt::Expr(e) => {
                self.expr(e, &mut env);
                Some(env)
            }
            Stmt::If { cond, then, otherwise } => {
                let (t, f) = self.cond(cond, env);
                let a = self.stmt(then, t);
                let b = match otherwise {
                    Some(o) => self.stmt(o, f),
                    None => f,
                };
                join(a, b)
            }
            Stmt::While { cond, body } => self.looping(env, Some(cond), body, &[], false),
            Stmt::DoWhile { body, cond } => self.looping(env, Some(cond), body, &[], true),
            Stmt::For { init, cond, update, body } => {
                let mut env = Some(env);
                for s in init {
                    env = self.stmt(s, env);
                }
                let env = env?;
                let budgets = self.budgets_for(init, cond.as_ref(), update, body, &env);
                let count = budgets.len();
                self.budgets.extend(budgets);
                let out = self.looping(env, cond.as_ref(), body, update, false);
                self.budgets.truncate(self.budgets.len() - count);
                out.map(|mut env| {
                    for s in init {
                        if let Stmt::Local { name, .. } = s {
                            env.remove(name);
                        }
                    }
                    env
                })
            }
            Stmt::Return(value, _) => {
                if let Some(value) = value {
                    let v = self.expr(value, &mut env);
                    let ret = self.class.methods[self.method].ret;
                    if ret.is_integral() {
                        self.ret = self.ret.join(convert(v, value.ty, ret));
                    }
                }
                None
            }
            Stmt::Break => {
                let flow = self.target(|f| matches!(f, Frame::Loop | Frame::Switch));
                flow.breaks = join(flow.breaks.take(), Some(env));
                None
            }
            Stmt::Continue => {
                let flow = self.target(|f| f == Frame::Loop);
                flow.continues = join(flow.continues.take(), Some(env));
                None
            }
            Stmt::BreakLabel(label) => {
                let flow = self.labeled(label);
                flow.breaks = join(flow.breaks.take(), Some(env));
                None
            }
            Stmt::ContinueLabel(label) => {
                // The loop that a labeled `continue` names is the frame inside its label.
                let i = self.labeled_index(label) + 1;
                let flow = &mut self.loops[i];
                flow.continues = join(flow.continues.take(), Some(env));
                None
            }
            Stmt::Yield(e) => {
                let v = self.expr(e, &mut env);
                let flow = self.target(|f| f == Frame::Expr);
                flow.value = flow.value.join(v);
                flow.breaks = join(flow.breaks.take(), Some(env));
                None
            }
            Stmt::Labeled { label, body } => {
                self.loops.push(Flow::new(Frame::Labeled, Some(label.clone())));
                let out = self.stmt(body, Some(env));
                let flow = self.loops.pop().expect("pushed above");
                join(out, flow.breaks)
            }
            Stmt::Switch { selector, cases } => {
                self.expr(selector, &mut env);
                let (out, _) = self.switch(cases, env, Frame::Switch);
                out
            }
            Stmt::Block(stmts) => self.stmts(stmts, Some(env)),
            Stmt::Throw(e) => {
                self.expr(e, &mut env);
                None
            }
            Stmt::Try { body, catches, finally } => {
                // An exception can leave the body at any point, so a handler starts from the
                // state before the `try` with what the body assigns at any value.
                let raised = widened(&env, body.iter());
                let mut out = self.stmts(body, Some(env));
                for catch in catches {
                    let handled = self.stmts(&catch.body, Some(raised.clone()));
                    out = join(out, handled);
                }
                let Some(finally) = finally else { return out };
                // The `finally` also runs on the way out of an exception, a `return`, a `break` or
                // a `continue`, from any point of the body and the handlers.
                let escaping = widened(&raised, catches.iter().flat_map(|c| &c.body));
                let after = self.stmts(finally, join(out.clone(), Some(escaping)));
                for flow in &mut self.loops {
                    flow.breaks = flow.breaks.take().map(|e| widened(&e, finally.iter()));
                    flow.continues = flow.continues.take().map(|e| widened(&e, finally.iter()));
                }
                out.and(after)
            }
            Stmt::ForEach { ty, name, array, body } => {
                self.expr(array, &mut env);
                if ty.is_integral() {
                    env.insert(name.clone(), Var { ty: *ty, range: Interval::full(*ty) });
                }
                let out = self.looping(env, None, body, &[], false);
                // The loop also ends when the elements run out, in the state at its head.
                let head = self.heads.get(&(&**body as *const Stmt as usize)).cloned();
                join(head, out).map(|mut env| {
                    env.remove(name);
                    env
                })
            }
        }
    }

    /// Runs the cases of a switch. A case starts from the state after the selector, joined with
    /// the state that falls through from the case before. Returns the state after the switch and
    /// the values of its `yield`s.
    fn switch(&mut self, cases: &[Case], env: Env, kind: Frame) -> (Option<Env>, Interval) {
        self.loops.push(Flow::new(kind, None));
        let mut fall = None;
        for case in cases {
            let entry = join(Some(env.clone()), fall.take());
            fall = match entry {
                Some(entry) => self.stmts(&case.body, Some(entry)),
                None => None,
            };
        }
        let flow = self.loops.pop().expect("pushed above");
        // No case may match, so the state after the selector also leaves the switch.
        (join(join(fall, flow.breaks), Some(env)), flow.value)
    }

    /// The innermost frame that a `break` or `yield` of the given kind leaves.
    fn target(&mut self, want: impl Fn(Frame) -> bool) -> &mut Flow {
        self.loops.iter_mut().rev().find(|f| want(f.kind)).expect("jump inside its target")
    }

    fn labeled_index(&self, label: &str) -> usize {
        self.loops
            .iter()
            .rposition(|f| f.label.as_deref() == Some(label))
            .expect("labeled jump inside its label")
    }

    fn labeled(&mut self, label: &str) -> &mut Flow {
        let i = self.labeled_index(label);
        &mut self.loops[i]
    }

    /// Finds the loop invariant at the head of a loop, then runs the body once more on it (the
    /// pass that records facts) and returns the state after the loop.
    fn looping(
        &mut self,
        entry: Env,
        cond: Option<&Expr>,
        body: &Stmt,
        update: &[Expr],
        do_while: bool,
    ) -> Option<Env> {
        let record = self.record;
        self.record = false;
        let key = body as *const Stmt as usize;
        // Any state that the check below accepts is a sound invariant, so the last one is a valid
        // place to start.
        let mut head = match self.heads.get(&key) {
            Some(last) => join(Some(entry.clone()), Some(last.clone())).expect("both states are reachable"),
            None => entry.clone(),
        };
        for iteration in 0.. {
            let (back, _) = self.iterate(&head, cond, body, update, do_while);
            let next = join(Some(entry.clone()), back).expect("the entry state is reachable");
            if within(&next, &head) {
                break;
            }
            head = if iteration >= 2 {
                widen(&head, &next, &self.thresholds)
            } else {
                join(Some(head), Some(next)).expect("both states are reachable")
            };
        }
        // Narrowing: the widened invariant is sound, and each step from it stays sound.
        for _ in 0..2 {
            let (back, _) = self.iterate(&head, cond, body, update, do_while);
            head = join(Some(entry.clone()), back).expect("the entry state is reachable");
        }
        self.record = record;
        let exit = self.iterate(&head, cond, body, update, do_while).1;
        self.heads.insert(key, head);
        exit
    }

    /// One pass through a loop from the state at its head: the state flowing back to the head
    /// and the state leaving the loop.
    fn iterate(
        &mut self,
        head: &Env,
        cond: Option<&Expr>,
        body: &Stmt,
        update: &[Expr],
        do_while: bool,
    ) -> (Option<Env>, Option<Env>) {
        self.loops.push(Flow::new(Frame::Loop, None));
        if do_while {
            let out = self.stmt(body, Some(head.clone()));
            let flow = self.loops.pop().expect("pushed above");
            let (t, f) = match join(out, flow.continues) {
                Some(env) => self.cond(cond.expect("do-while has a condition"), env),
                None => (None, None),
            };
            (t, join(f, flow.breaks))
        } else {
            let (t, f) = match cond {
                Some(c) => self.cond(c, head.clone()),
                None => (Some(head.clone()), None),
            };
            let out = self.stmt(body, t);
            let flow = self.loops.pop().expect("pushed above");
            let mut back = join(out, flow.continues);
            if let Some(env) = &mut back {
                for u in update {
                    self.expr(u, env);
                }
            }
            (back, join(f, flow.breaks))
        }
    }

    /// Evaluates `e` without recording facts or changing summaries.
    fn peek(&mut self, e: &Expr, env: &Env) -> Interval {
        let (record, update) = (self.record, self.update_summaries);
        self.record = false;
        self.update_summaries = false;
        let v = self.expr(e, &mut env.clone());
        self.record = record;
        self.update_summaries = update;
        v
    }

    /// Bounds for the counters of a counting loop: a variable that the body only ever increases
    /// by constants grows by at most that much per iteration.
    fn budgets_for(
        &mut self,
        init: &[Stmt],
        cond: Option<&Expr>,
        update: &[Expr],
        body: &Stmt,
        env: &Env,
    ) -> Vec<(String, i128)> {
        let Some((var, _, end)) = counting_loop(init, cond, update, body) else { return Vec::new() };
        let start = env[var].range;
        let end = self.peek(end, env);
        if start.is_empty() || end.is_empty() {
            return Vec::new();
        }
        let trips = (end.hi - start.lo).max(0);
        let mut context = env.clone();
        let var_ty = env[var].ty;
        context.insert(var.to_string(), Var { ty: var_ty, range: Interval::new(start.lo, end.hi - 1) });
        let mut budgets = Vec::new();
        for (name, v) in env {
            if name == var || v.range.is_empty() {
                continue;
            }
            if let Some(step) = self.increase(body, name, &context, body) {
                let bound = v.range.hi + step * trips;
                if step > 0 && bound <= Interval::full(v.ty).hi {
                    budgets.push((name.clone(), bound));
                }
            }
        }
        budgets
    }

    /// How much `stmt` can increase `name` in one pass, if it only ever increases it by
    /// constants; `None` if it can change it any other way. `context` holds the ranges at the
    /// start of the loop whose body `outer` is.
    fn increase(&mut self, stmt: &Stmt, name: &str, context: &Env, outer: &Stmt) -> Option<i128> {
        match stmt {
            Stmt::Expr(e) => match &e.kind {
                ExprKind::IncDec { target: Place::Var(target), increment: true, .. } if target == name => {
                    Some(1)
                }
                ExprKind::Assign { op: Some(BinOp::Add), target: Place::Var(target), value }
                    if target == name =>
                {
                    match value.kind {
                        ExprKind::Int(k) if k >= 0 => Some(k as i128),
                        ExprKind::Long(k) if k >= 0 => Some(k as i128),
                        _ => None,
                    }
                }
                _ if assigns(e, name) => None,
                _ => Some(0),
            },
            Stmt::Local { init, .. } => {
                if init.as_ref().is_some_and(|e| assigns(e, name)) {
                    None
                } else {
                    Some(0)
                }
            }
            Stmt::If { cond, then, otherwise } => {
                if assigns(cond, name) {
                    return None;
                }
                let a = self.increase(then, name, context, outer)?;
                let b = match otherwise {
                    Some(o) => self.increase(o, name, context, outer)?,
                    None => 0,
                };
                Some(a.max(b))
            }
            Stmt::Block(stmts) => {
                let mut total = 0i128;
                for s in stmts {
                    total = total.checked_add(self.increase(s, name, context, outer)?)?;
                }
                Some(total)
            }
            Stmt::For { init, cond, update, body } if assigns_stmt(stmt, name) => {
                // A nested counting loop whose bounds do not change in the outer loop.
                let (var, start, end) = counting_loop(init, cond.as_ref(), update, body)?;
                if var == name || !invariant(start, outer) || !invariant(end, outer) {
                    return None;
                }
                let inner = self.increase(body, name, context, outer)?;
                let (start, end) = (self.peek(start, context), self.peek(end, context));
                if start.is_empty() || end.is_empty() {
                    return None;
                }
                let trips = (end.hi - start.lo).clamp(0, UNBOUNDED);
                trips.checked_mul(inner)
            }
            Stmt::Return(Some(e), _) if assigns(e, name) => None,
            _ if assigns_stmt(stmt, name) => None,
            _ => Some(0),
        }
    }

    /// Lowers the upper bound of a counter about to be increased by `step`, by the budget of the
    /// enclosing counting loops.
    fn clamp(&self, name: &str, range: Interval, step: Option<i128>) -> Interval {
        let Some(step) = step else { return range };
        let mut range = range;
        for (counter, bound) in &self.budgets {
            if counter == name {
                range = range.meet(Interval::new(-UNBOUNDED, bound - step));
            }
        }
        range
    }

    /// Evaluates the object of a field place, which may itself contain calls or writes.
    fn place_object(&mut self, place: &Place, env: &mut Env) {
        if let Place::Field { object, .. } = place {
            self.expr(object, env);
        }
    }

    /// Records the argument values that reach `callee`, and returns its result range.
    /// A call of a method runs any method of its override group, so the arguments reach all of
    /// them and the result is any of theirs.
    fn call(&mut self, args: &[Expr], callee: usize, ty: Type, env: &mut Env) -> Interval {
        let values: Vec<Interval> = args.iter().map(|a| self.expr(a, env)).collect();
        let group = self.class.hierarchy.group_members(callee);
        if self.update_summaries {
            for &g in group {
                self.reached[g] = true;
                for (i, p) in self.class.methods[g].params.iter().enumerate() {
                    if p.ty.is_integral() {
                        let v = convert(values[i], args[i].ty, p.ty);
                        self.params[g][i] = self.params[g][i].join(v);
                    }
                }
            }
        }
        if ty.is_integral() {
            group.iter().fold(EMPTY, |r, &g| r.join(self.returns[g]))
        } else {
            Interval::full(Type::Long)
        }
    }

    fn expr(&mut self, e: &Expr, env: &mut Env) -> Interval {
        let ty = e.ty;
        let other = Interval::full(Type::Long);
        match &e.kind {
            ExprKind::Int(v) => Interval::point(*v as i128),
            ExprKind::Long(v) => Interval::point(*v as i128),
            ExprKind::Char(c) => Interval::point(*c as i128),
            ExprKind::Double(_)
            | ExprKind::Float(_)
            | ExprKind::Bool(_)
            | ExprKind::Str(_)
            | ExprKind::Units(_)
            | ExprKind::EnumConstant { .. }
            | ExprKind::Null => other,
            ExprKind::Var(name) => env.get(name).map_or(Interval::full(ty), |v| v.range),
            ExprKind::Unary(op, x) => {
                let v = self.expr(x, env);
                if !ty.is_integral() || v.is_empty() {
                    return if ty.is_integral() { v } else { other };
                }
                match op {
                    UnOp::Plus => v,
                    UnOp::BitNot => Interval::new(-v.hi - 1, -v.lo - 1),
                    UnOp::Neg => {
                        self.visit(e.id);
                        if v.contains(Interval::full(ty).lo) {
                            self.overflowed(e.id);
                            Interval::full(ty)
                        } else {
                            Interval::new(-v.hi, -v.lo)
                        }
                    }
                    UnOp::Not => other,
                }
            }
            ExprKind::Binary(op, ..) if matches!(op, BinOp::And | BinOp::Or) || op.is_comparison() => {
                let (t, f) = self.cond(e, env.clone());
                if let Some(after) = join(t, f) {
                    *env = after;
                }
                other
            }
            ExprKind::Binary(op, l, r) => {
                let a = self.expr(l, env);
                let b = self.expr(r, env);
                if ty.is_integral() { self.arith(*op, a, b, ty, e.id) } else { other }
            }
            ExprKind::Assign { op, target: Place::Var(target), value } => {
                let v = self.expr(value, env);
                let Some(var) = env.get(target).copied() else { return other };
                let range = match op {
                    None => convert(v, value.ty, var.ty),
                    Some(op) => {
                        // A boxed value is unboxed before the operation.
                        let result = binary_type(*op, var.ty, value.ty.unboxed()).expect("checked");
                        let step = match (op, &value.kind) {
                            (BinOp::Add, ExprKind::Int(k)) if *k >= 0 => Some(*k as i128),
                            (BinOp::Add, ExprKind::Long(k)) if *k >= 0 => Some(*k as i128),
                            _ => None,
                        };
                        let current = self.clamp(target, var.range, step);
                        let r = if result.is_integral() {
                            self.arith(*op, current, v, result, e.id)
                        } else {
                            other
                        };
                        // `b += 1` on a byte narrows the int result back to a byte.
                        if var.ty.is_small() && result.is_integral() {
                            self.narrowed(r, var.ty, e.id);
                        }
                        convert(r, result, var.ty)
                    }
                };
                env.insert(target.clone(), Var { ty: var.ty, range });
                range
            }
            ExprKind::IncDec { target: Place::Var(target), increment, prefix } => {
                let Some(var) = env.get(target).copied() else { return other };
                let current = if *increment { self.clamp(target, var.range, Some(1)) } else { var.range };
                let op = if *increment { BinOp::Add } else { BinOp::Sub };
                let range = self.arith(op, current, Interval::point(1), var.ty, e.id);
                env.insert(target.clone(), Var { ty: var.ty, range });
                if *prefix { range } else { current }
            }
            ExprKind::NewArray(sizes) => {
                // A size's values are kept like an index's: one that is never negative needs no check.
                for size in sizes {
                    let range = self.expr(size, env);
                    self.indexed(size.id, range);
                }
                other
            }
            ExprKind::ArrayLit(items) => {
                for item in items {
                    self.expr(item, env);
                }
                other
            }
            ExprKind::Index(array, index) => {
                self.expr(array, env);
                let i = self.expr(index, env);
                self.indexed(e.id, i);
                if ty.is_integral() { Interval::full(ty) } else { other }
            }
            ExprKind::Length(array) => {
                self.expr(array, env);
                // HotSpot refuses arrays of `i32::MAX - 1` elements or more (the header takes
                // words), so `length + 1` cannot overflow.
                Interval::new(0, i32::MAX as i128 - 2)
            }
            ExprKind::Assign { target: Place::Index(array, index), value, .. } => {
                self.expr(array, env);
                let i = self.expr(index, env);
                self.indexed(e.id, i);
                self.expr(value, env);
                if ty.is_integral() { Interval::full(ty) } else { other }
            }
            ExprKind::IncDec { target: Place::Index(array, index), .. } => {
                self.expr(array, env);
                let i = self.expr(index, env);
                self.indexed(e.id, i);
                if ty.is_integral() { Interval::full(ty) } else { other }
            }
            // Fields and statics are not locals, so their values carry no range.
            ExprKind::Assign { target, value, .. } => {
                self.place_object(target, env);
                self.expr(value, env);
                other
            }
            ExprKind::IncDec { target, .. } => {
                self.place_object(target, env);
                other
            }
            ExprKind::InstanceOf { value, .. } => {
                self.expr(value, env);
                other
            }
            ExprKind::Cast(to, x) => {
                let v = self.expr(x, env);
                if to.is_small() && x.ty.is_integral() {
                    self.narrowed(v, *to, e.id);
                }
                if *to == Type::Char {
                    self.char_cast(v, e.id);
                }
                convert(v, x.ty, *to)
            }
            ExprKind::Switch { selector, cases } => {
                // Each `yield` gives a value of the switch's type, so the values join as they are.
                self.expr(selector, env);
                let (out, value) = self.switch(cases, env.clone(), Frame::Expr);
                if let Some(out) = out {
                    *env = out;
                }
                if ty.is_integral() { value } else { other }
            }
            ExprKind::Conditional(c, a, b) => {
                let (t, f) = self.cond(c, env.clone());
                let (mut va, mut vb) = (EMPTY, EMPTY);
                let t = t.map(|mut env| {
                    va = convert(self.expr(a, &mut env), a.ty, ty);
                    env
                });
                let f = f.map(|mut env| {
                    vb = convert(self.expr(b, &mut env), b.ty, ty);
                    env
                });
                if let Some(after) = join(t, f) {
                    *env = after;
                }
                va.join(vb)
            }
            ExprKind::Call { args, resolved, .. } => {
                let callee = resolved.expect("calls are resolved by the checker");
                self.call(args, callee, ty, env)
            }
            ExprKind::MethodCall { receiver, args, resolved, .. } => {
                self.expr(receiver, env);
                let callee = resolved.expect("calls are resolved by the checker");
                self.call(args, callee, ty, env)
            }
            ExprKind::New { args, ctor: Some(callee), .. }
            | ExprKind::ThisCall { args, resolved: Some(callee) }
            | ExprKind::SuperInit { args, resolved: Some(callee) } => self.call(args, *callee, ty, env),
            ExprKind::New { args, .. }
            | ExprKind::ThisCall { args, .. }
            | ExprKind::SuperInit { args, .. } => {
                for a in args {
                    self.expr(a, env);
                }
                other
            }
            ExprKind::Static { class, name } => match static_value(self.class, *class, name) {
                Some(v) => Interval::point(v),
                None => other,
            },
            ExprKind::This => other,
            ExprKind::Field { object, .. } => {
                self.expr(object, env);
                other
            }
            ExprKind::Builtin(builtin, args) => {
                let values: Vec<Interval> = args.iter().map(|a| self.expr(a, env)).collect();
                if !ty.is_integral() {
                    return other;
                }
                let full = Interval::full(ty);
                match builtin {
                    Builtin::MathAbs => {
                        let v = values[0];
                        self.visit(e.id);
                        if v.is_empty() {
                            v
                        } else if v.contains(full.lo) {
                            self.overflowed(e.id);
                            full
                        } else if v.lo >= 0 {
                            v
                        } else if v.hi <= 0 {
                            Interval::new(-v.hi, -v.lo)
                        } else {
                            Interval::new(0, v.hi.max(-v.lo))
                        }
                    }
                    Builtin::MathMax | Builtin::MathMin => {
                        let (a, b) = (values[0], values[1]);
                        if a.is_empty() || b.is_empty() {
                            EMPTY
                        } else if *builtin == Builtin::MathMax {
                            Interval::new(a.lo.max(b.lo), a.hi.max(b.hi))
                        } else {
                            Interval::new(a.lo.min(b.lo), a.hi.min(b.hi))
                        }
                    }
                    Builtin::StringLength => Interval::new(0, i32::MAX as i128),
                    _ => full,
                }
            }
        }
    }

    /// `a op b` in the integer type `ty`, recording whether it can overflow or, for division,
    /// throw.
    fn arith(&mut self, op: BinOp, a: Interval, b: Interval, ty: Type, id: ExprId) -> Interval {
        self.visit(id);
        if a.is_empty() || b.is_empty() {
            return EMPTY;
        }
        let full = Interval::full(ty);
        let bits = if ty == Type::Long { 64 } else { 32 };
        let (record, overflow) = (self.record, &mut self.overflow);
        let mut checked = |r: Interval| {
            if r.within(full) {
                r
            } else {
                if record {
                    overflow.insert(id);
                }
                full
            }
        };
        match op {
            BinOp::Add => checked(Interval::new(a.lo + b.lo, a.hi + b.hi)),
            BinOp::Sub => checked(Interval::new(a.lo - b.hi, a.hi - b.lo)),
            BinOp::Mul => {
                let corners = [a.lo * b.lo, a.lo * b.hi, a.hi * b.lo, a.hi * b.hi];
                checked(Interval::new(*corners.iter().min().unwrap(), *corners.iter().max().unwrap()))
            }
            BinOp::Div | BinOp::Rem => {
                if (b.contains(0) || (a.contains(full.lo) && b.contains(-1))) && self.record {
                    self.risky_division.insert(id);
                }
                // The divisor without zero, split by sign.
                let parts = [Interval::new(b.lo, b.hi.min(-1)), Interval::new(b.lo.max(1), b.hi)];
                let mut result = EMPTY;
                for p in parts.into_iter().filter(|p| !p.is_empty()) {
                    result = result.join(if op == BinOp::Div {
                        let q = [a.lo / p.lo, a.lo / p.hi, a.hi / p.lo, a.hi / p.hi];
                        Interval::new(*q.iter().min().unwrap(), *q.iter().max().unwrap())
                    } else {
                        let m = p.lo.abs().max(p.hi.abs()) - 1;
                        Interval::new(
                            if a.lo < 0 { a.lo.max(-m) } else { 0 },
                            if a.hi > 0 { a.hi.min(m) } else { 0 },
                        )
                    });
                }
                // MIN / -1 wraps to MIN.
                if result.within(full) { result } else { full }
            }
            BinOp::Shl | BinOp::Shr | BinOp::UShr => {
                let shift = (b.lo == b.hi).then_some(b.lo & (bits - 1));
                match (op, shift) {
                    (BinOp::Shl, Some(s)) => {
                        let r = Interval::new(a.lo << s, a.hi << s);
                        if r.within(full) { r } else { full }
                    }
                    (BinOp::Shr, Some(s)) => Interval::new(a.lo >> s, a.hi >> s),
                    (BinOp::UShr, Some(s)) if a.lo >= 0 => Interval::new(a.lo >> s, a.hi >> s),
                    (BinOp::Shr, None) => Interval::new(a.lo.min(0), a.hi.max(0)),
                    (BinOp::UShr, None) if a.lo >= 0 => Interval::new(0, a.hi),
                    _ => full,
                }
            }
            BinOp::BitAnd => match (a.lo >= 0, b.lo >= 0) {
                (true, true) => Interval::new(0, a.hi.min(b.hi)),
                (true, false) => Interval::new(0, a.hi),
                (false, true) => Interval::new(0, b.hi),
                (false, false) => full,
            },
            BinOp::BitOr | BinOp::BitXor if a.lo >= 0 && b.lo >= 0 => {
                let top = a.hi.max(b.hi) as u128;
                Interval::new(0, ((top + 1).next_power_of_two() - 1) as i128)
            }
            _ => full,
        }
    }

    /// The states in which `e` is true and in which it is false.
    fn cond(&mut self, e: &Expr, env: Env) -> (Option<Env>, Option<Env>) {
        match &e.kind {
            ExprKind::Bool(true) => (Some(env), None),
            ExprKind::Bool(false) => (None, Some(env)),
            ExprKind::Unary(UnOp::Not, x) => {
                let (t, f) = self.cond(x, env);
                (f, t)
            }
            ExprKind::Binary(BinOp::And, l, r) => {
                let (lt, lf) = self.cond(l, env);
                let (rt, rf) = match lt {
                    Some(t) => self.cond(r, t),
                    None => (None, None),
                };
                (rt, join(lf, rf))
            }
            ExprKind::Binary(BinOp::Or, l, r) => {
                let (lt, lf) = self.cond(l, env);
                let (rt, rf) = match lf {
                    Some(f) => self.cond(r, f),
                    None => (None, None),
                };
                (join(lt, rt), rf)
            }
            ExprKind::Binary(op, l, r) if op.is_comparison() => {
                let mut env = env;
                let a = self.expr(l, &mut env);
                let b = self.expr(r, &mut env);
                if !l.ty.is_integral() || !r.ty.is_integral() {
                    return (Some(env.clone()), Some(env));
                }
                let negated = match op {
                    BinOp::Lt => BinOp::Ge,
                    BinOp::Le => BinOp::Gt,
                    BinOp::Gt => BinOp::Le,
                    BinOp::Ge => BinOp::Lt,
                    BinOp::Eq => BinOp::Ne,
                    _ => BinOp::Eq,
                };
                (self.narrow(env.clone(), *op, l, r, a, b), self.narrow(env, negated, l, r, a, b))
            }
            _ => {
                let mut env = env;
                self.expr(e, &mut env);
                (Some(env.clone()), Some(env))
            }
        }
    }

    /// The state in which `l op r` holds, where `l` evaluated to `a` and `r` to `b`; `None` if
    /// it cannot hold.
    fn narrow(&mut self, env: Env, op: BinOp, l: &Expr, r: &Expr, a: Interval, b: Interval) -> Option<Env> {
        if a.is_empty() || b.is_empty() || !feasible(op, a, b) {
            return None;
        }
        let mut env = env;
        if let ExprKind::Var(x) = &l.kind
            && !assigns(r, x)
        {
            restrict(&mut env, x, |v| refine(v, op, b))?;
        }
        if let ExprKind::Var(y) = &r.kind
            && !assigns(l, y)
        {
            let flipped = match op {
                BinOp::Lt => BinOp::Gt,
                BinOp::Le => BinOp::Ge,
                BinOp::Gt => BinOp::Lt,
                BinOp::Ge => BinOp::Le,
                other => other,
            };
            restrict(&mut env, y, |v| refine(v, flipped, a))?;
        }
        // `x * x <= b` bounds `x` by the square root of `b`, if `x * x` does not overflow.
        if let ExprKind::Binary(BinOp::Mul, p, q) = &l.kind
            && let (ExprKind::Var(x), ExprKind::Var(y)) = (&p.kind, &q.kind)
            && x == y
            && matches!(op, BinOp::Lt | BinOp::Le)
            && !assigns(r, x)
            && !self.no_square_narrowing.contains(&l.id)
        {
            if self.record {
                self.square_narrowed.insert(l.id);
            }
            let limit = if op == BinOp::Lt { b.hi - 1 } else { b.hi };
            if limit < 0 {
                return None;
            }
            let s = isqrt(limit);
            restrict(&mut env, x, |v| v.meet(Interval::new(-s, s)))?;
        }
        Some(env)
    }
}

/// Applies `f` to the range of `name`; `None` if that leaves no possible value.
fn restrict(env: &mut Env, name: &str, f: impl FnOnce(Interval) -> Interval) -> Option<()> {
    if let Some(v) = env.get_mut(name) {
        v.range = f(v.range);
        if v.range.is_empty() {
            return None;
        }
    }
    Some(())
}

fn feasible(op: BinOp, a: Interval, b: Interval) -> bool {
    match op {
        BinOp::Lt => a.lo < b.hi,
        BinOp::Le => a.lo <= b.hi,
        BinOp::Gt => a.hi > b.lo,
        BinOp::Ge => a.hi >= b.lo,
        BinOp::Eq => !a.meet(b).is_empty(),
        _ => !(a.lo == a.hi && b.lo == b.hi && a.lo == b.lo),
    }
}

/// The values of `x` for which `x op b` can hold.
fn refine(x: Interval, op: BinOp, b: Interval) -> Interval {
    match op {
        BinOp::Lt => x.meet(Interval::new(-UNBOUNDED, b.hi - 1)),
        BinOp::Le => x.meet(Interval::new(-UNBOUNDED, b.hi)),
        BinOp::Gt => x.meet(Interval::new(b.lo + 1, UNBOUNDED)),
        BinOp::Ge => x.meet(Interval::new(b.lo, UNBOUNDED)),
        BinOp::Eq => x.meet(b),
        _ if b.lo == b.hi && x.lo == b.lo => Interval::new(x.lo + 1, x.hi),
        _ if b.lo == b.hi && x.hi == b.lo => Interval::new(x.lo, x.hi - 1),
        _ => x,
    }
}

fn isqrt(n: i128) -> i128 {
    let mut s = (n as f64).sqrt() as i128;
    while s * s > n {
        s -= 1;
    }
    while (s + 1) * (s + 1) <= n {
        s += 1;
    }
    s
}

/// Whether `e` has the same value throughout every iteration of the loop with body `body`.
fn invariant(e: &Expr, body: &Stmt) -> bool {
    match &e.kind {
        ExprKind::Int(_) | ExprKind::Long(_) => true,
        ExprKind::Var(v) => !assigns_stmt(body, v),
        ExprKind::Unary(_, x) | ExprKind::Cast(_, x) => invariant(x, body),
        ExprKind::Binary(_, l, r) => invariant(l, body) && invariant(r, body),
        _ => false,
    }
}

/// The integer literals in a method, with their negations and neighbours, and zero.
fn literals(method: &Method) -> Vec<i128> {
    let mut out = vec![-1, 0, 1];
    {
        for stmt in &method.body {
            visit_stmt(
                stmt,
                &mut |e| {
                    visit_expr(e, &mut |x| {
                        let v = match x.kind {
                            ExprKind::Int(v) => v as i128,
                            ExprKind::Long(v) => v as i128,
                            ExprKind::Char(c) => c as i128,
                            _ => return,
                        };
                        out.extend([v - 1, v, v + 1, -v - 1, -v, -v + 1]);
                    })
                },
                &mut |_| {},
            );
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// The methods that `method` calls.
fn callees(class: &Class, method: &Method) -> Vec<usize> {
    let mut out = Vec::new();
    for stmt in &method.body {
        visit_stmt(stmt, &mut |e| visit_expr(e, &mut |x| out.extend(callee_of(class, x))), &mut |_| {});
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// The methods that a call or constructor call (`new`, `this(...)`, `super(...)`) may run: the
/// whole override group of the resolved method, whose summaries are shared.
fn callee_of(class: &Class, e: &Expr) -> Vec<usize> {
    match e.kind {
        ExprKind::Call { resolved: Some(m), .. }
        | ExprKind::MethodCall { resolved: Some(m), .. }
        | ExprKind::ThisCall { resolved: Some(m), .. }
        | ExprKind::SuperInit { resolved: Some(m), .. }
        | ExprKind::New { ctor: Some(m), .. } => class.hierarchy.group(m),
        _ => Vec::new(),
    }
}

/// The methods called from field initializers.
fn initializer_callees(class: &Class) -> Vec<usize> {
    let mut out = Vec::new();
    for t in &class.types {
        for f in t.fields.iter().chain(&t.statics) {
            if let Some(init) = &f.init {
                visit_expr(init, &mut |x| out.extend(callee_of(class, x)));
            }
        }
    }
    out
}

/// For each method, the methods that call it.
fn callers(callees: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut out = vec![Vec::new(); callees.len()];
    for (m, called) in callees.iter().enumerate() {
        for &c in called {
            out[c].push(m);
        }
    }
    out
}
