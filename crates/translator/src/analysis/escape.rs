//! Escape analysis: the `new`s whose objects never leave the method that creates them, so that
//! the object can live in a slot on that method's stack instead of the collected heap
//! (`java::Slot`, docs/collector.md, "Objects on the stack").
//!
//! An object escapes its method when it is stored into a field, an array element or a static,
//! returned, thrown, captured by a lambda or a local class (whose constructor stores it into a
//! field), handed to the runtime (a builtin, a string concatenation, which calls its
//! `toString`), or passed to a call whose callee lets that parameter escape, or to a call that
//! is not known. Anything the analysis does not model escapes.
//!
//! Each method gets a summary of its parameters, `this` first: whether each may escape, and
//! whether each may be what the method returns (`ListBuffer.append` returns `this`), so that a
//! call's result is an alias of those arguments in the caller. The summaries start empty and grow
//! until nothing changes, so recursion is covered. A virtual call takes the union over every
//! implementation it may run (`Class::callees`); after `live_methods`, those that cannot run
//! have bodies that never run, which let nothing escape. An abstract method has no body: when
//! no implementation of it exists, the call cannot run.
//!
//! Within a method the analysis is flow-insensitive: each variable (by name) holds the set of
//! parameters and `new`s that any assignment in the method gives it, and every use of the
//! variable is a use of all of them.
//!
//! **Loops.** The translation gives each `new` one slot, which a `new` in a loop reuses on every
//! iteration. That is sound only if the object of the previous iteration can no longer be reached
//! when the next one is made. It is required that every variable that may hold the object is
//! declared inside the body of the innermost loop around the `new` (a variable declared there is
//! a new one on each iteration, and Java does not let the next iteration read it before it
//! assigns it), and since the object is never stored anywhere else, nothing else holds it.

use crate::ast::*;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// The `new`s (by expression id) whose objects never leave the method whose body holds them and,
/// in a loop, are no longer reachable when the next iteration makes the next one.
pub fn stack_objects(class: &Class) -> HashSet<ExprId> {
    let n = class.methods.len();
    let mut analysis = Analysis {
        class,
        summaries: class.methods.iter().map(|m| Summary::empty(arity(m))).collect(),
        targets: HashMap::new(),
        field_inits: field_inits(class),
    };
    // Who calls each method, to analyse them again when its summary grows.
    let mut callers: Vec<Vec<usize>> = vec![Vec::new(); n];
    for m in 0..n {
        let mut exprs = Vec::new();
        analysis.each_expr(m, &mut |x| exprs.push(x));
        let mut callees = HashSet::new();
        for x in exprs {
            if let Some(targets) = analysis.call_targets(x) {
                callees.extend(targets.iter().copied());
            }
        }
        for t in callees {
            callers[t].push(m);
        }
    }
    let mut stays: Vec<Vec<ExprId>> = vec![Vec::new(); n];
    let mut queued = vec![true; n];
    let mut work: Vec<usize> = (0..n).rev().collect();
    while let Some(m) = work.pop() {
        queued[m] = false;
        let (summary, staying) = analysis.method(m);
        stays[m] = staying;
        if summary != analysis.summaries[m] {
            analysis.summaries[m] = summary;
            for &caller in &callers[m] {
                if !std::mem::replace(&mut queued[caller], true) {
                    work.push(caller);
                }
            }
        }
    }
    stays.into_iter().flatten().collect()
}

/// The parameters of a method, `this` first for an instance method or a constructor.
fn arity(m: &Method) -> usize {
    m.params.len() + usize::from(!m.is_static)
}

/// What a method does with its parameters, `this` first.
#[derive(Clone, PartialEq, Eq)]
struct Summary {
    escapes: Vec<bool>,
    returned: Vec<bool>,
}

impl Summary {
    fn empty(arity: usize) -> Summary {
        Summary { escapes: vec![false; arity], returned: vec![false; arity] }
    }
}

/// A set of the sources of one method's values: its parameters, then its `new`s.
#[derive(Clone, Default, PartialEq, Eq)]
struct Sources(Vec<u64>);

impl Sources {
    fn one(i: usize) -> Sources {
        let mut s = Sources::default();
        s.insert(i);
        s
    }

    fn insert(&mut self, i: usize) {
        if self.0.len() <= i / 64 {
            self.0.resize(i / 64 + 1, 0);
        }
        self.0[i / 64] |= 1 << (i % 64);
    }

    fn contains(&self, i: usize) -> bool {
        self.0.get(i / 64).is_some_and(|w| w & (1 << (i % 64)) != 0)
    }

    /// Adds `other`; whether that added anything.
    fn union(&mut self, other: &Sources) -> bool {
        if self.0.len() < other.0.len() {
            self.0.resize(other.0.len(), 0);
        }
        let mut grew = false;
        for (w, &o) in self.0.iter_mut().zip(&other.0) {
            grew |= o & !*w != 0;
            *w |= o;
        }
        grew
    }
}

struct Analysis<'a> {
    class: &'a Class,
    summaries: Vec<Summary>,
    /// The implementations of each virtual call, by resolved method and receiver type.
    targets: HashMap<(usize, usize), Rc<[usize]>>,
    /// The instance field initializers of each class, which every constructor of the class that
    /// does not call another one with `this(..)` runs on its object.
    field_inits: Vec<Vec<&'a Expr>>,
}

fn field_inits(class: &Class) -> Vec<Vec<&Expr>> {
    class.types.iter().map(|t| t.fields.iter().filter_map(|f| f.init.as_ref()).collect()).collect()
}

/// One analysis of a method's code.
struct Code<'m> {
    /// The source of each `new` that may stay, by expression id.
    sites: HashMap<ExprId, usize>,
    /// The parameters by name, `this` excluded.
    params: HashMap<&'m str, usize>,
    has_this: bool,
    vars: HashMap<&'m str, Sources>,
}

impl<'a> Analysis<'a> {
    /// Calls `f` on every expression that method `m` runs: its body's, and for a constructor that
    /// does not delegate, its class's field initializers.
    fn each_expr(&self, m: usize, f: &mut dyn FnMut(&'a Expr)) {
        let method = &self.class.methods[m];
        for s in &method.body {
            visit_stmt(s, &mut |e| visit_expr(e, &mut *f), &mut |_| {});
        }
        if method.is_ctor && !delegates(&method.body) {
            for init in &self.field_inits[method.class] {
                visit_expr(init, &mut *f);
            }
        }
    }

    /// The methods that the call `x` may run, or `None` when `x` is not a call of the program's
    /// code. An empty list for a call of a method that has no implementation.
    fn call_targets(&mut self, x: &Expr) -> Option<Rc<[usize]>> {
        let class = self.class;
        match &x.kind {
            ExprKind::MethodCall { receiver, resolved: Some(r), is_super, .. } => {
                if *is_super || class.methods[*r].is_static {
                    return Some(Rc::from([*r]));
                }
                let Some(t) = class.class_index(receiver.ty) else { return Some(Rc::from([*r])) };
                let targets =
                    self.targets.entry((*r, t)).or_insert_with(|| class.hierarchy.targets(*r, t).into());
                Some(targets.clone())
            }
            ExprKind::Call { resolved: Some(r), .. }
            | ExprKind::New { ctor: Some(r), .. }
            | ExprKind::ThisCall { resolved: Some(r), .. }
            | ExprKind::SuperInit { resolved: Some(r), .. } => Some(Rc::from([*r])),
            _ => None,
        }
    }

    /// Analyses method `m` with the summaries known so far: its summary, and its `new`s whose
    /// objects stay in it.
    fn method(&mut self, m: usize) -> (Summary, Vec<ExprId>) {
        let class = self.class;
        let method = &class.methods[m];
        if method.is_abstract {
            return (Summary::empty(arity(method)), Vec::new());
        }
        let has_this = !method.is_static;
        let params: HashMap<&str, usize> = method
            .params
            .iter()
            .enumerate()
            .map(|(i, p)| (p.name.as_str(), i + usize::from(has_this)))
            .collect();
        let mut exprs: Vec<&'a Expr> = Vec::new();
        self.each_expr(m, &mut |x| exprs.push(x));
        let mut sites = HashMap::new();
        for s in &method.body {
            visit_stmt(
                s,
                &mut |e| {
                    visit_expr(e, &mut |x| {
                        if let ExprKind::New { ctor: Some(_), .. } = x.kind {
                            let next = arity(method) + sites.len();
                            sites.insert(x.id, next);
                        }
                    })
                },
                &mut |_| {},
            );
        }
        let mut vars: HashMap<&str, Sources> =
            params.iter().map(|(&name, &i)| (name, Sources::one(i))).collect();
        let mut code = Code { sites, params, has_this, vars: HashMap::new() };
        // The variables: what each assignment gives each, until nothing grows.
        let mut stmts: Vec<&'a Stmt> = Vec::new();
        for s in &method.body {
            visit_stmt(s, &mut |_| {}, &mut |s| stmts.push(s));
        }
        loop {
            code.vars = vars.clone();
            let mut grew = false;
            for s in &stmts {
                if let Stmt::Local { name, init: Some(init), .. } = s {
                    let value = self.eval(&code, init);
                    grew |= vars.entry(name.as_str()).or_default().union(&value);
                }
            }
            for x in &exprs {
                let (name, value) = match &x.kind {
                    ExprKind::Assign { op: None, target: Place::Var(name), value } => (name, value),
                    ExprKind::InstanceOf { value, binding: Some(name), .. } => (name, value),
                    _ => continue,
                };
                let value = self.eval(&code, value);
                grew |= vars.entry(name.as_str()).or_default().union(&value);
            }
            if !grew {
                break;
            }
        }
        code.vars = vars;
        // What escapes, and what is returned.
        let mut escaping = Sources::default();
        let mut returned = Sources::default();
        for s in &stmts {
            match s {
                Stmt::Return(Some(e), _) => _ = returned.union(&self.eval(&code, e)),
                Stmt::Throw(e) => _ = escaping.union(&self.eval(&code, e)),
                _ => {}
            }
        }
        for x in &exprs {
            self.escaping(&code, x, &mut escaping);
        }
        let mut summary = Summary::empty(arity(method));
        for i in 0..arity(method) {
            summary.escapes[i] = escaping.contains(i);
            summary.returned[i] = returned.contains(i);
        }
        let mut staying: Vec<(ExprId, usize)> = code
            .sites
            .iter()
            .filter(|&(_, &s)| !escaping.contains(s) && !returned.contains(s))
            .map(|(&id, &s)| (id, s))
            .collect();
        if !staying.is_empty() {
            let scopes = Scopes::of(method);
            staying.retain(|&(id, s)| scopes.dead_by_next_iteration(id, s, &code));
        }
        (summary, staying.into_iter().map(|(id, _)| id).collect())
    }

    /// The sources whose objects `e` may be.
    fn eval(&mut self, code: &Code, e: &Expr) -> Sources {
        match &e.kind {
            ExprKind::Var(name) => code.vars.get(name.as_str()).cloned().unwrap_or_default(),
            ExprKind::This if code.has_this => Sources::one(0),
            ExprKind::Cast(_, value) | ExprKind::Assign { op: None, value, .. } => self.eval(code, value),
            ExprKind::Conditional(_, a, b) => {
                let mut s = self.eval(code, a);
                s.union(&self.eval(code, b));
                s
            }
            ExprKind::Builtin(Builtin::NullChecked, args) if !args.is_empty() => {
                self.eval(code, &args[args.len() - 1])
            }
            ExprKind::Switch { cases, .. } => {
                let mut s = Sources::default();
                for stmt in cases.iter().flat_map(|c| &c.body) {
                    let mut yields = Vec::new();
                    visit_stmt_outside_switches(stmt, &mut |s| {
                        if let Stmt::Yield(e) = s {
                            yields.push(e);
                        }
                    });
                    for e in yields {
                        s.union(&self.eval(code, e));
                    }
                }
                s
            }
            ExprKind::Call { .. }
            | ExprKind::MethodCall { .. }
            | ExprKind::New { .. }
            | ExprKind::ThisCall { .. }
            | ExprKind::SuperInit { .. } => {
                let mut s = match &e.kind {
                    ExprKind::New { .. } => {
                        code.sites.get(&e.id).map(|&i| Sources::one(i)).unwrap_or_default()
                    }
                    _ => Sources::default(),
                };
                let Some(targets) = self.call_targets(e) else { return s };
                let Some(args) = self.arguments(code, e, &targets) else { return s };
                for &t in targets.iter() {
                    for (k, arg) in args.iter().enumerate() {
                        if self.summaries[t].returned.get(k) == Some(&true) {
                            s.union(arg);
                        }
                    }
                }
                s
            }
            _ => Sources::default(),
        }
    }

    /// The sources of the arguments of the call `x` in the positions of its targets' parameters,
    /// `this` first; `None` when they do not match.
    fn arguments(&mut self, code: &Code, x: &Expr, targets: &[usize]) -> Option<Vec<Sources>> {
        let class = self.class;
        let (receiver, args): (Option<Sources>, &[Expr]) = match &x.kind {
            ExprKind::MethodCall { receiver, args, resolved: Some(r), .. } => {
                (if class.methods[*r].is_static { None } else { Some(self.eval(code, receiver)) }, args)
            }
            ExprKind::Call { args, resolved: Some(r), .. } if class.methods[*r].is_static => (None, args),
            ExprKind::New { args, .. } => {
                (Some(code.sites.get(&x.id).map(|&i| Sources::one(i)).unwrap_or_default()), args)
            }
            ExprKind::ThisCall { args, .. } | ExprKind::SuperInit { args, .. } => {
                (Some(if code.has_this { Sources::one(0) } else { Sources::default() }), args)
            }
            _ => return None,
        };
        let mut all: Vec<Sources> = receiver.into_iter().collect();
        let start = all.len();
        all.extend(args.iter().map(|a| self.eval(code, a)));
        let fits = targets.iter().all(|&t| {
            let m = &class.methods[t];
            arity(m) == all.len() && m.is_static == (start == 0)
        });
        fits.then_some(all)
    }

    /// Adds to `escaping` what the expression `x` itself lets escape, not counting the
    /// expressions inside it.
    fn escaping(&mut self, code: &Code, x: &Expr, escaping: &mut Sources) {
        match &x.kind {
            ExprKind::Assign {
                target: Place::Field { .. } | Place::Static { .. } | Place::Index(..),
                value,
                ..
            }
            | ExprKind::Assign { op: Some(_), value, .. } => _ = escaping.union(&self.eval(code, value)),
            ExprKind::ArrayLit(items) => {
                for item in items {
                    escaping.union(&self.eval(code, item));
                }
            }
            // A concatenation calls the `toString` of the objects it is given.
            ExprKind::Binary(_, l, r) if x.ty == Type::String => {
                escaping.union(&self.eval(code, l));
                escaping.union(&self.eval(code, r));
            }
            ExprKind::Builtin(builtin, args) if !only_looks(builtin) => {
                for a in args {
                    escaping.union(&self.eval(code, a));
                }
            }
            ExprKind::Call { args, .. }
            | ExprKind::MethodCall { args, .. }
            | ExprKind::New { args, .. }
            | ExprKind::ThisCall { args, .. }
            | ExprKind::SuperInit { args, .. } => {
                let targets = self.call_targets(x);
                let all = targets.as_ref().and_then(|t| self.arguments(code, x, t));
                match (targets, all) {
                    (Some(targets), Some(all)) => {
                        for &t in targets.iter() {
                            for (k, arg) in all.iter().enumerate() {
                                if self.summaries[t].escapes[k] {
                                    escaping.union(arg);
                                }
                            }
                        }
                    }
                    // A call the analysis does not follow lets everything it is given escape.
                    _ => {
                        if let ExprKind::MethodCall { receiver, .. } = &x.kind {
                            escaping.union(&self.eval(code, receiver));
                        }
                        let with_this = matches!(
                            x.kind,
                            ExprKind::Call { .. } | ExprKind::ThisCall { .. } | ExprKind::SuperInit { .. }
                        );
                        if with_this && code.has_this {
                            escaping.insert(0);
                        }
                        if let Some(&s) = code.sites.get(&x.id) {
                            escaping.insert(s);
                        }
                        for a in args {
                            escaping.union(&self.eval(code, a));
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Whether `builtin` only looks at the objects it is given, keeping none: their class, their
/// identity hash, whether they are null.
fn only_looks(builtin: &Builtin) -> bool {
    matches!(
        builtin,
        Builtin::GetClass
            | Builtin::ClassName { .. }
            | Builtin::SameClass { .. }
            | Builtin::NullChecked
            | Builtin::Native(Native::IdentityHash | Native::SystemIdentityHash)
    )
}

/// Where a method's `new`s and variable declarations are, in its loops.
struct Scopes<'m> {
    /// How many loops have been numbered.
    count: usize,
    /// The loops around each `new`, outermost first, as numbers of the loops.
    sites: HashMap<ExprId, Vec<usize>>,
    /// For each variable, the loops around each of its declarations.
    declarations: HashMap<&'m str, Vec<Vec<usize>>>,
}

impl<'m> Scopes<'m> {
    fn of(method: &'m Method) -> Scopes<'m> {
        let mut scopes = Scopes { count: 0, sites: HashMap::new(), declarations: HashMap::new() };
        for p in &method.params {
            scopes.declarations.entry(p.name.as_str()).or_default().push(Vec::new());
        }
        for s in &method.body {
            scopes.stmt(s, &mut Vec::new());
        }
        scopes
    }

    /// Whether the object of `new` `id` (source `s`), if it is in a loop, can no longer be
    /// reached when the next iteration makes the next one: every variable that may hold it is
    /// declared inside the body of the innermost loop around it.
    fn dead_by_next_iteration(&self, id: ExprId, s: usize, code: &Code) -> bool {
        let Some(&innermost) = self.sites.get(&id).and_then(|loops| loops.last()) else { return true };
        code.vars.iter().filter(|(_, sources)| sources.contains(s)).all(|(name, _)| {
            !code.params.contains_key(name)
                && self.declarations.get(name).is_some_and(|ds| ds.iter().all(|d| d.contains(&innermost)))
        })
    }

    fn declare(&mut self, name: &'m str, loops: &[usize]) {
        self.declarations.entry(name).or_default().push(loops.to_vec());
    }

    /// Notes the `new`s and declarations of the statement `s`, inside the loops `loops`.
    fn stmt(&mut self, s: &'m Stmt, loops: &mut Vec<usize>) {
        let in_loop =
            |scopes: &mut Scopes<'m>, loops: &mut Vec<usize>, body: &'m Stmt, exprs: &[&'m Expr]| {
                scopes.count += 1;
                loops.push(scopes.count);
                // A pattern variable of the condition may stay in scope after the loop: it is
                // declared outside it, while a `new` there runs on every iteration.
                for e in exprs {
                    scopes.expr(e, loops, Some(loops.len() - 1));
                }
                scopes.stmt(body, loops);
                loops.pop();
            };
        match s {
            Stmt::Local { name, init, .. } => {
                self.declare(name, loops);
                if let Some(e) = init {
                    self.expr(e, loops, None);
                }
            }
            Stmt::Expr(e) | Stmt::Return(Some(e), _) | Stmt::Yield(e) | Stmt::Throw(e) => {
                self.expr(e, loops, None)
            }
            Stmt::If { cond, then, otherwise } => {
                self.expr(cond, loops, None);
                self.stmt(then, loops);
                if let Some(o) = otherwise {
                    self.stmt(o, loops);
                }
            }
            Stmt::While { cond, body } | Stmt::DoWhile { body, cond } => in_loop(self, loops, body, &[cond]),
            Stmt::For { init, cond, update, body } => {
                for s in init {
                    self.stmt(s, loops);
                }
                let exprs: Vec<&Expr> = cond.iter().chain(update).collect();
                in_loop(self, loops, body, &exprs);
            }
            Stmt::ForEach { name, array, body, .. } => {
                self.expr(array, loops, None);
                self.count += 1;
                loops.push(self.count);
                self.declare(name, loops);
                self.stmt(body, loops);
                loops.pop();
            }
            Stmt::Block(stmts) => stmts.iter().for_each(|s| self.stmt(s, loops)),
            Stmt::Switch { selector, cases } => {
                self.expr(selector, loops, None);
                for case in cases {
                    for label in &case.labels {
                        if let CaseLabel::Value(e) = label {
                            self.expr(e, loops, None);
                        }
                    }
                    case.body.iter().for_each(|s| self.stmt(s, loops));
                }
            }
            Stmt::Labeled { body, .. } => self.stmt(body, loops),
            Stmt::Try { body, catches, finally } => {
                body.iter().for_each(|s| self.stmt(s, loops));
                for catch in catches {
                    self.declare(&catch.name, loops);
                    catch.body.iter().for_each(|s| self.stmt(s, loops));
                }
                finally.iter().flatten().for_each(|s| self.stmt(s, loops));
            }
            Stmt::Return(None, _)
            | Stmt::Break
            | Stmt::Continue
            | Stmt::BreakLabel(_)
            | Stmt::ContinueLabel(_) => {}
        }
    }

    /// Notes the `new`s and pattern variables of `e`, inside the loops `loops`; pattern variables
    /// inside only the first `bindings` of them when that is given. The statements of a switch
    /// expression are walked again with their own loops, and a `new` keeps the most loops it is
    /// found inside.
    fn expr(&mut self, e: &'m Expr, loops: &mut Vec<usize>, bindings: Option<usize>) {
        let mut switches = Vec::new();
        visit_expr(e, &mut |x| match &x.kind {
            ExprKind::New { .. } => {
                let at = self.sites.entry(x.id).or_default();
                if at.len() <= loops.len() {
                    *at = loops.clone();
                }
            }
            ExprKind::InstanceOf { binding: Some(name), .. } => {
                let around = &loops[..bindings.unwrap_or(loops.len())];
                self.declarations.entry(name.as_str()).or_default().push(around.to_vec());
            }
            ExprKind::Switch { cases, .. } => switches.push(cases),
            _ => {}
        });
        for cases in switches {
            for s in cases.iter().flat_map(|c| &c.body) {
                self.stmt(s, loops);
            }
        }
    }
}
