//! Reads of a static that may run while its class's initialization is under way, before the
//! static initializer has set it. Java's `static final Node accept = new Node();` in `Pattern`,
//! with `Node() { next = Pattern.accept; }`: the first `Node` is built while `accept` is still
//! null, so that read gives null, and `next` holds it. A static that starts empty
//! (`Facts::placeholder_statics`) is then null wherever such a read flows (see `nullness`).

use super::closure::Bits;
use crate::ast::*;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// What a method does that matters here: the statics it reads, by expression, and what running it
/// may run: the static initializers of the classes it uses and its callees, those of its virtual
/// calls as lists that the calls with the same method and receiver type share (`Uses::lists`).
#[derive(Default)]
struct Summary {
    reads: Vec<(usize, String, ExprId)>,
    runs: Vec<usize>,
    calls: Vec<usize>,
}

/// The reads of a static of an object type of a class with an eager static initializer that may
/// run, within that initializer or code it runs, before the initializer's own assignment of the
/// static: the statements of the `<clinit>` run in order, and a static counts as set after the
/// statement that assigns it, not during it.
pub fn early_reads(class: &Class, eager: &HashSet<usize>) -> HashSet<ExprId> {
    let clinits: HashMap<usize, usize> = (class.methods.iter().enumerate())
        .filter(|(_, m)| m.name == "<clinit>")
        .map(|(i, m)| (m.class, i))
        .collect();
    // The classes whose initializers are walked, each with its statics that start unset.
    let walked: Vec<Walked> = (eager.iter())
        .filter_map(|&t| {
            let unset: HashSet<&str> = (class.types[t].statics.iter())
                .filter(|f| matches!(f.ty, Type::Class(..)) && f.init.is_some())
                .map(|f| f.name.as_str())
                .collect();
            Some((t, *clinits.get(&t)?, unset)).filter(|(.., unset)| !unset.is_empty())
        })
        .collect();
    let uses = Uses { class, clinits: &clinits, callees: RefCell::default(), lists: RefCell::default() };
    let summaries: Vec<Summary> = (0..class.methods.len()).map(|m| uses.summary(m)).collect();
    let lists = uses.lists.borrow().clone();
    let groups = Groups::new(&summaries, &lists, &clinits, &walked);
    let reaching = Reaching::new(&groups);
    let early = walk(class, &uses, &walked, &groups, &reaching);
    if std::env::var_os("JAVA_TO_RUST_EARLY_CHECK").is_some() {
        let by_methods = walk_methods(class, &uses, &walked, &summaries, &lists);
        assert!(early == by_methods, "early check: the walk by groups found other early reads");
        eprintln!("early check: the same {} early reads", early.len());
    }
    early
}

/// A class whose initializer is walked, its `<clinit>`, and its statics that start unset.
type Walked<'c> = (usize, usize, HashSet<&'c str>);

/// Walks the initializer of each class of `walked`, statement by statement, through what it runs
/// (see `early_reads`). A group of methods that run each other (`Groups`) is entered whole: what
/// runs one of them runs them all.
fn walk(
    class: &Class,
    uses: &Uses,
    walked: &[Walked],
    groups: &Groups,
    reaching: &Reaching,
) -> HashSet<ExprId> {
    let mut early = HashSet::new();
    // The groups seen for the initializer of each class, as the number of that class's turn: a
    // set for each class without clearing one.
    let mut seen_in: Vec<u32> = vec![0; groups.next.len()];
    let mut turn = 0;
    for (w, (t, clinit, unset)) in walked.iter().enumerate() {
        let mut unset = unset.clone();
        let mut statements = Vec::new();
        flatten(&class.methods[*clinit].body, &mut statements);
        // The initializer under way returns at once when code it runs uses its class again.
        turn += 1;
        seen_in[groups.group[*clinit]] = turn;
        for s in statements {
            if unset.is_empty() {
                break;
            }
            let mut first = Summary::default();
            visit_stmt(s, &mut |e| uses.summarize(e, &mut first), &mut |_| {});
            for (c, name, id) in &first.reads {
                if c == t && unset.contains(name.as_str()) {
                    early.insert(*id);
                }
            }
            // A group from which no read of the class's statics can run is not walked: what it
            // runs marks nothing.
            let calls = first.calls.iter().map(|&l| groups.group[groups.methods + l]);
            let mut work: Vec<usize> = first
                .runs
                .iter()
                .map(|&m| groups.group[m])
                .chain(calls)
                .filter(|&g| reaching.reads(g, w))
                .collect();
            while let Some(g) = work.pop() {
                if seen_in[g] == turn {
                    continue;
                }
                seen_in[g] = turn;
                let of_group = &groups.reads[g];
                let from = of_group.partition_point(|r| r.0 < w);
                for (_, name, id) in of_group[from..].iter().take_while(|r| r.0 == w) {
                    if unset.contains(name) {
                        early.insert(*id);
                    }
                }
                work.extend(groups.next[g].iter().filter(|&&h| seen_in[h] != turn && reaching.reads(h, w)));
            }
            if let Stmt::Expr(Expr {
                kind: ExprKind::Assign { op: None, target: Place::Static { class: c, name }, .. },
                ..
            }) = s
                && c == t
            {
                unset.remove(name.as_str());
            }
        }
    }
    early
}

/// `walk` method by method, without leaving out any, for `JAVA_TO_RUST_EARLY_CHECK`.
fn walk_methods(
    class: &Class,
    uses: &Uses,
    walked: &[Walked],
    summaries: &[Summary],
    lists: &[Rc<[usize]>],
) -> HashSet<ExprId> {
    let runs = |s: &Summary| -> Vec<usize> {
        let mut runs = s.runs.clone();
        runs.extend(s.calls.iter().flat_map(|&l| lists[l].iter().copied()));
        runs
    };
    let runs_of: Vec<Vec<usize>> = summaries.iter().map(runs).collect();
    let mut early = HashSet::new();
    let mut seen_in: Vec<u32> = vec![0; class.methods.len()];
    let mut turn = 0;
    for (t, clinit, unset) in walked {
        let mut unset = unset.clone();
        let mut statements = Vec::new();
        flatten(&class.methods[*clinit].body, &mut statements);
        turn += 1;
        seen_in[*clinit] = turn;
        for s in statements {
            if unset.is_empty() {
                break;
            }
            let mut first = Summary::default();
            visit_stmt(s, &mut |e| uses.summarize(e, &mut first), &mut |_| {});
            let mut mark = |reads: &[(usize, String, ExprId)]| {
                for (c, name, id) in reads {
                    if c == t && unset.contains(name.as_str()) {
                        early.insert(*id);
                    }
                }
            };
            mark(&first.reads);
            let mut work: Vec<usize> = runs(&first);
            while let Some(m) = work.pop() {
                if seen_in[m] == turn {
                    continue;
                }
                seen_in[m] = turn;
                mark(&summaries[m].reads);
                work.extend(runs_of[m].iter().filter(|&&r| seen_in[r] != turn));
            }
            if let Stmt::Expr(Expr {
                kind: ExprKind::Assign { op: None, target: Place::Static { class: c, name }, .. },
                ..
            }) = s
                && c == t
            {
                unset.remove(name.as_str());
            }
        }
    }
    early
}

/// The methods, and the lists of methods that virtual calls run, in groups that run each other,
/// with the static initializers left out of what they are run by: a group is strongly connected
/// without a `<clinit>`, so reaching one of its methods reaches all of them in every class's walk,
/// where only that class's own `<clinit>` is cut off; each `<clinit>` is a group of its own. A
/// call on a receiver of a wide type runs thousands of methods, and many methods make it: by
/// group, what is run is listed once.
struct Groups<'c> {
    /// The number of methods: the node of list `l` is `methods + l`.
    methods: usize,
    /// The group of each node.
    group: Vec<usize>,
    /// The other groups that each group's nodes run.
    next: Vec<Vec<usize>>,
    /// The reads of each group's methods of the statics that start unset of a walked class, each
    /// with the class's place in `walked`, in that order.
    reads: Vec<Vec<(usize, &'c str, ExprId)>>,
}

impl<'c> Groups<'c> {
    fn new(
        summaries: &'c [Summary],
        lists: &[Rc<[usize]>],
        clinits: &HashMap<usize, usize>,
        walked: &[Walked],
    ) -> Groups<'c> {
        let methods = summaries.len();
        let mut is_clinit = vec![false; methods];
        for &m in clinits.values() {
            is_clinit[m] = true;
        }
        let edges = |node: usize| -> Vec<usize> {
            match summaries.get(node) {
                Some(s) => s.runs.iter().copied().chain(s.calls.iter().map(|&l| methods + l)).collect(),
                None => lists[node - methods].to_vec(),
            }
        };
        let into: Vec<Vec<usize>> = (0..methods + lists.len())
            .map(|n| edges(n).into_iter().filter(|&r| r >= methods || !is_clinit[r]).collect())
            .collect();
        let (group, components) = super::closure::components(&into);
        drop(into);
        let by_class: HashMap<usize, usize> = walked.iter().enumerate().map(|(w, &(t, ..))| (t, w)).collect();
        let mut listed = vec![usize::MAX; components.len()];
        let mut next = Vec::with_capacity(components.len());
        let mut reads = Vec::with_capacity(components.len());
        for (g, members) in components.iter().enumerate() {
            let mut out = Vec::new();
            let mut of_group = Vec::new();
            for &node in members {
                for r in edges(node) {
                    let h = group[r];
                    if h != g && listed[h] != g {
                        listed[h] = g;
                        out.push(h);
                    }
                }
                for (c, name, id) in summaries.get(node).into_iter().flat_map(|s| &s.reads) {
                    if let Some(&w) = by_class.get(c)
                        && walked[w].2.contains(name.as_str())
                    {
                        of_group.push((w, name.as_str(), *id));
                    }
                }
            }
            of_group.sort_unstable_by_key(|r| r.0);
            next.push(out);
            reads.push(of_group);
        }
        Groups { methods, group, next, reads }
    }
}

/// For each group, the initializers walked (by their place in `walked`) whose class's statics
/// that start unset its methods may read, themselves or through what they run: one set for each
/// set of groups that run each other.
struct Reaching {
    component: Vec<usize>,
    unions: Vec<Bits>,
}

impl Reaching {
    fn new(groups: &Groups) -> Reaching {
        let (component, components) = super::closure::components(&groups.next);
        let mut unions: Vec<Bits> = Vec::with_capacity(components.len());
        for (c, members) in components.iter().enumerate() {
            let mut union = Bits::default();
            for &g in members {
                for &(w, ..) in &groups.reads[g] {
                    union.insert(w);
                }
                for &h in &groups.next[g] {
                    if component[h] != c {
                        union.union(&unions[component[h]]);
                    }
                }
            }
            unions.push(union);
        }
        Reaching { component, unions }
    }

    /// Whether group `g` may read a static of the class of initializer `w` that starts unset.
    fn reads(&self, g: usize, w: usize) -> bool {
        self.unions[self.component[g]].contains(w)
    }
}

/// The statements of a static initializer in the order they run, with those of blocks and of
/// `try` blocks (a rewritten `<clinit>` runs its own statements in one, see
/// `lower::clinit_errors`) in place of them.
fn flatten<'s>(stmts: &'s [Stmt], out: &mut Vec<&'s Stmt>) {
    for s in stmts {
        match s {
            Stmt::Block(inner) | Stmt::Try { body: inner, .. } => flatten(inner, out),
            _ => out.push(s),
        }
    }
}

/// What the code of the program uses, method by method (`Summary`), with the methods that each
/// virtual call runs listed once for each method and receiver type: a call on a receiver of a
/// wide type runs thousands of methods, and finding them takes a look at every class below it.
struct Uses<'c> {
    class: &'c Class,
    clinits: &'c HashMap<usize, usize>,
    /// The list of each method and receiver type, by index into `lists`.
    callees: RefCell<HashMap<(usize, usize), usize>>,
    lists: RefCell<Vec<Rc<[usize]>>>,
}

impl Uses<'_> {
    /// The summary of method `m`. A constructor runs its class's field initializers too.
    fn summary(&self, m: usize) -> Summary {
        let class = self.class;
        let method = &class.methods[m];
        let mut out = Summary::default();
        for s in &method.body {
            visit_stmt(s, &mut |e| self.summarize(e, &mut out), &mut |_| {});
        }
        if method.is_ctor {
            for f in &class.types[method.class].fields {
                if let Some(init) = &f.init {
                    self.summarize(init, &mut out);
                }
            }
        }
        out.calls.sort_unstable();
        out.calls.dedup();
        out
    }

    /// The list of the methods that the virtual call `x` may run (`Class::callees`), by index
    /// into `lists`.
    fn list(&self, x: &Expr) -> Option<usize> {
        let class = self.class;
        let ExprKind::MethodCall { receiver, resolved: Some(m), is_super: false, .. } = &x.kind else {
            return None;
        };
        if class.methods[*m].is_static {
            return None;
        }
        let t = class.class_index(receiver.ty)?;
        if let Some(&l) = self.callees.borrow().get(&(*m, t)) {
            return Some(l);
        }
        let mut lists = self.lists.borrow_mut();
        lists.push(class.callees(x).into());
        self.callees.borrow_mut().insert((*m, t), lists.len() - 1);
        Some(lists.len() - 1)
    }

    /// Adds what evaluating `e` reads and runs to `out`.
    fn summarize(&self, e: &Expr, out: &mut Summary) {
        let class = self.class;
        visit_expr(e, &mut |x| {
            if let ExprKind::Static { class: c, name } = &x.kind {
                out.reads.push((*c, name.clone(), x.id));
            }
            let list = self.list(x);
            let lists = self.lists.borrow();
            let other;
            let callees: &[usize] = match list {
                Some(l) => &lists[l],
                None => {
                    other = class.callees(x);
                    &other
                }
            };
            // The first use of a class runs its static initializer, and its superclasses'.
            let mut used = match &x.kind {
                ExprKind::Static { class: c, .. }
                | ExprKind::Assign { target: Place::Static { class: c, .. }, .. }
                | ExprKind::IncDec { target: Place::Static { class: c, .. }, .. }
                | ExprKind::EnumConstant { class: c, .. }
                | ExprKind::Builtin(Builtin::InitializeClass { class: c }, _) => Some(*c),
                _ => callees
                    .iter()
                    .find(|&&c| class.methods[c].is_static || class.methods[c].is_ctor)
                    .map(|&c| class.methods[c].class),
            };
            while let Some(c) = used {
                out.runs.extend(self.clinits.get(&c));
                used = class.types[c].extends;
            }
            match list {
                Some(l) => out.calls.push(l),
                None => out.runs.extend(callees),
            }
        })
    }
}
