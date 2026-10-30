//! Arrays that no other name can see. Such an array is a plain `Vec` (a `Vec` of `Vec`s for an
//! array of arrays), not a shared `java::Array`, and a method that makes an array and hands it
//! back returns the `Vec`.
//!
//! A local qualifies when it is declared once in its method, everything it ever holds is an
//! array nobody else holds (a new array, a literal, the result of a method that returns its own
//! arrays, or another plain local moved into it), and it is used only through its elements and
//! lengths (`a[i]`, `m[i][j] = v`, `a.length`, for-each over a one-dimensional array, or over
//! the rows of an array of arrays whose body leaves the array alone): see `scan`. A local is moved when it is not read again before it is assigned (`a = b; b = c;`,
//! see `liveness`); a moved local may also go to a parameter or variable that is shared, which
//! then takes the `Vec` over (`java::share`). Copying an array to a second name that is used
//! later, comparing it, storing it in a field or giving it to `java.util.Arrays` makes it shared.
//! An array of arrays stays plain while its rows are only read through it: a row kept in a
//! variable is a second name for it. Arrays of arrays need elements that are primitives or
//! strings and rows that are never null.
//!
//! A parameter is borrowed, as `&[T]` or `&mut [T]` when its elements are written, when its
//! method uses it only through its elements and every call can lend it an array: a plain array
//! or one of its rows (`&v`, `&m[i]`), a new array, or a shared one-dimensional array
//! (`&a.borrow()`) when nothing else can reach that array while the call runs. An array that
//! threads share (`java::SharedArray`, `Facts::shared_arrays`) is never lent: other threads may
//! write its elements while the call runs, which no slice may see. Passing a plain array to a
//! borrowed parameter keeps it plain; passing it to any other parameter shares it, unless it is
//! moved there.
//!
//! A method returns a `Vec` when every array it returns is one nobody else holds.
//!
//! The indexes that are always in bounds are found by `bounds`, and the innermost counting loops
//! whose accesses can be checked once before the loop by `slices`. A counting loop
//! `for (int i = 0; i < a.length; i++)` over a plain array whose body reads `a[i]` and uses `i`
//! and `a` nowhere else can be `for x in a.iter()`.

mod bounds;
mod liveness;
mod scan;
mod slices;

pub use slices::{RowIndex, Slice, SlicedLoop};

use super::Facts;
use super::{FastMap, FastSet};
use crate::ast::*;
use scan::{Call, Owned, Scan};

pub(super) fn analyze(class: &Class, facts: &mut Facts) {
    let mut scans: Vec<Scan> = class
        .methods
        .iter()
        .enumerate()
        .map(|(m, method)| {
            let arrays: FastSet<String> = declarations(method)
                .into_iter()
                .filter(|d| matches!(d.ty, Type::Array(_)))
                .map(|d| d.name.to_string())
                .chain(
                    method.params.iter().filter(|p| matches!(p.ty, Type::Array(_))).map(|p| p.name.clone()),
                )
                .collect();
            let live_after = liveness::live_after(method, &arrays);
            scan::scan(m, method, facts, &live_after)
        })
        .collect();
    // A call may run any method of its override group, which all take their arrays alike.
    for s in &mut scans {
        let calls = std::mem::take(&mut s.calls);
        for call in calls {
            for g in class.hierarchy.group(call.callee) {
                s.calls.push(Call { callee: g, args: call.args.clone() });
            }
        }
    }
    // A shared array lent to a call is borrowed from its cell until the call returns; a field
    // holding an array would be another way to reach that cell meanwhile.
    let lend_shared = class
        .types
        .iter()
        .flat_map(|t| t.fields.iter().chain(&t.statics))
        .all(|f| !matches!(f.ty, Type::Array(_)));
    let solution = solve(class, &scans, lend_shared, facts);
    let written = written_names(class, &scans, &solution.borrowed);
    for (m, method) in class.methods.iter().enumerate() {
        let names: FastMap<String, bool> =
            solution.plain[m].iter().map(|name| (name.clone(), written[m].contains(name))).collect();
        for stmt in &method.body {
            visit_stmt(stmt, &mut |_| {}, &mut |s| {
                if let Some(counting) = counting_over(s, &names)
                    && let Some(reads) = element_reads(&counting)
                {
                    facts.element_loops.insert(counting.cond.id, reads);
                }
            });
        }
        let proven = bounds::in_bounds(method, &solution.plain[m], &scans[m].reshaped, facts);
        facts.in_bounds.extend(proven);
        facts.sliced_loops.extend(slices::sliced_loops(method, &solution.plain[m], facts));
        facts.moves.extend(scans[m].moves.iter().copied());
        for (name, written) in names {
            facts.plain_arrays.insert((m, name), written);
        }
        for (array, row) in &scans[m].row_loops {
            if solution.plain[m].contains(array) {
                facts.row_loops.insert((m, row.clone()));
            }
        }
        for (i, p) in method.params.iter().enumerate() {
            if solution.borrowed.contains(&(m, i)) {
                let group = class.hierarchy.group(m);
                let any_written = written[m].contains(&p.name)
                    || group.iter().any(|&g| written[g].contains(&class.methods[g].params[i].name));
                facts.borrowed_arrays.insert((m, i), any_written);
            }
        }
    }
    facts.owned_returns = solution.owned_returns;
}

/// A local of a method.
pub(super) struct Declaration<'m> {
    pub name: &'m str,
    pub ty: Type,
    pub init: Option<&'m Expr>,
    /// Whether it is the variable of a for-each loop.
    pub loop_var: bool,
}

/// Every local declared in `method`, including the variables of for-each loops and the locals
/// of the cases of switch expressions.
pub(super) fn declarations(method: &Method) -> Vec<Declaration<'_>> {
    fn stmt<'m>(s: &'m Stmt, out: &mut Vec<Declaration<'m>>) {
        match s {
            Stmt::Local { ty, name, init, .. } => {
                out.push(Declaration { name, ty: *ty, init: init.as_ref(), loop_var: false });
                if let Some(e) = init {
                    expr(e, out);
                }
            }
            Stmt::ForEach { ty, name, array, body } => {
                out.push(Declaration { name, ty: *ty, init: None, loop_var: true });
                expr(array, out);
                stmt(body, out);
            }
            Stmt::Expr(e) | Stmt::Return(Some(e), _) | Stmt::Yield(e) => expr(e, out),
            Stmt::If { cond, then, otherwise } => {
                expr(cond, out);
                stmt(then, out);
                if let Some(o) = otherwise {
                    stmt(o, out);
                }
            }
            Stmt::While { cond, body } | Stmt::DoWhile { body, cond } => {
                expr(cond, out);
                stmt(body, out);
            }
            Stmt::For { init, cond, update, body } => {
                init.iter().for_each(|s| stmt(s, out));
                if let Some(c) = cond {
                    expr(c, out);
                }
                update.iter().for_each(|u| expr(u, out));
                stmt(body, out);
            }
            Stmt::Switch { selector, cases } => {
                expr(selector, out);
                cases.iter().flat_map(|c| &c.body).for_each(|s| stmt(s, out));
            }
            Stmt::Labeled { body, .. } => stmt(body, out),
            Stmt::Block(stmts) => stmts.iter().for_each(|s| stmt(s, out)),
            Stmt::Throw(e) => expr(e, out),
            Stmt::Try { body, catches, finally } => {
                body.iter().for_each(|s| stmt(s, out));
                for catch in catches {
                    out.push(Declaration { name: &catch.name, ty: catch.ty, init: None, loop_var: false });
                    catch.body.iter().for_each(|s| stmt(s, out));
                }
                finally.iter().flatten().for_each(|s| stmt(s, out));
            }
            Stmt::Return(None, _)
            | Stmt::Break
            | Stmt::Continue
            | Stmt::BreakLabel(_)
            | Stmt::ContinueLabel(_) => {}
        }
    }
    fn expr<'m>(e: &'m Expr, out: &mut Vec<Declaration<'m>>) {
        let mut switches = Vec::new();
        collect_switches(e, &mut switches);
        for cases in switches {
            cases.iter().flat_map(|c| &c.body).for_each(|s| stmt(s, out));
        }
    }
    let mut out = Vec::new();
    method.body.iter().for_each(|s| stmt(s, &mut out));
    out
}

/// The cases of the switch expressions in `e` that are not inside another switch expression.
fn collect_switches<'e>(e: &'e Expr, out: &mut Vec<&'e [Case]>) {
    match &e.kind {
        ExprKind::Switch { selector, cases } => {
            collect_switches(selector, out);
            out.push(cases);
        }
        ExprKind::Unary(_, x)
        | ExprKind::Cast(_, x)
        | ExprKind::Length(x)
        | ExprKind::Field { object: x, .. } => collect_switches(x, out),
        ExprKind::Assign { target, value, .. } => {
            visit_place_switches(target, out);
            collect_switches(value, out);
        }
        ExprKind::IncDec { target, .. } => visit_place_switches(target, out),
        ExprKind::Binary(_, l, r) | ExprKind::Index(l, r) => {
            collect_switches(l, out);
            collect_switches(r, out);
        }
        ExprKind::Conditional(c, a, b) => {
            collect_switches(c, out);
            collect_switches(a, out);
            collect_switches(b, out);
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            collect_switches(receiver, out);
            args.iter().for_each(|a| collect_switches(a, out));
        }
        ExprKind::Call { args, .. }
        | ExprKind::Builtin(_, args)
        | ExprKind::New { args, .. }
        | ExprKind::ThisCall { args, .. }
        | ExprKind::SuperInit { args, .. }
        | ExprKind::NewArray(args)
        | ExprKind::ArrayLit(args) => args.iter().for_each(|a| collect_switches(a, out)),
        _ => {}
    }
}

fn visit_place_switches<'e>(place: &'e Place, out: &mut Vec<&'e [Case]>) {
    match place {
        Place::Index(array, index) => {
            collect_switches(array, out);
            collect_switches(index, out);
        }
        Place::Field { object, .. } => collect_switches(object, out),
        Place::Var(_) | Place::Static { .. } => {}
    }
}

/// What the solver decides.
struct Solution {
    /// The plain array variables of each method.
    plain: Vec<FastSet<String>>,
    /// The borrowed (method, parameter) pairs.
    borrowed: FastSet<(usize, usize)>,
    /// The methods that return arrays they own, as `Vec`s.
    owned_returns: FastSet<usize>,
}

/// Decides which arrays are plain, which parameters are borrowed and which methods return their
/// own arrays. All three only shrink, from everything that the scans allow, until every
/// condition holds: a local is plain only while all its values are owned; a plain array passed to
/// a parameter that is not borrowed stays plain only if it is moved there; a parameter is
/// borrowed only while it is plain in its method and every call of the method can lend it an
/// array (see `lends`), and a parameter that is not borrowed is shared; a method returns its own
/// arrays only while every value it returns is owned.
fn solve(class: &Class, scans: &[Scan], lend_shared: bool, facts: &Facts) -> Solution {
    let mut plain: Vec<FastSet<String>> = scans.iter().map(|s| s.candidates.clone()).collect();
    let called: FastSet<usize> = scans.iter().flat_map(|s| s.calls.iter().map(|c| c.callee)).collect();
    let mut borrowed: FastSet<(usize, usize)> = scans
        .iter()
        .enumerate()
        .filter(|(m, _)| called.contains(m))
        .flat_map(|(m, s)| s.params.iter().map(move |(i, _)| (m, *i)))
        .collect();
    // A call in a field initializer has no plain arrays to lend.
    for init in
        class.types.iter().flat_map(|t| t.fields.iter().chain(&t.statics)).filter_map(|f| f.init.as_ref())
    {
        visit_expr(init, &mut |x| match &x.kind {
            ExprKind::Call { args, resolved: Some(callee), .. }
            | ExprKind::MethodCall { args, resolved: Some(callee), .. }
            | ExprKind::New { args, ctor: Some(callee), .. } => {
                for g in class.hierarchy.group(*callee) {
                    for i in 0..args.len() {
                        borrowed.remove(&(g, i));
                    }
                }
            }
            _ => {}
        });
    }
    let mut owned_returns: FastSet<usize> = class
        .methods
        .iter()
        .enumerate()
        .filter(|(m, method)| {
            !method.is_ctor && scan::shareable(method.ret, facts) && !facts.nullable_returns.contains(m)
        })
        .map(|(m, _)| m)
        .collect();
    loop {
        let mut changed = false;
        for (m, scan) in scans.iter().enumerate() {
            for call in &scan.calls {
                for i in 0..call.args.len() {
                    let lent = Lending {
                        class,
                        scans,
                        plain: &plain,
                        borrowed: &borrowed,
                        owned_returns: &owned_returns,
                        lend_shared,
                        threads_share: &facts.shared_arrays,
                    };
                    if borrowed.contains(&(call.callee, i)) && !lent.lends(m, call, i) {
                        borrowed.remove(&(call.callee, i));
                        changed = true;
                    }
                }
            }
        }
        for (m, scan) in scans.iter().enumerate() {
            for call in &scan.calls {
                for (i, arg) in call.args.iter().enumerate() {
                    if let Some(name) = &arg.name
                        && !borrowed.contains(&(call.callee, i))
                        && (arg.row || !arg.moved)
                        && plain[m].remove(name)
                    {
                        changed = true;
                    }
                }
            }
        }
        // The methods of an override group borrow a parameter together or not at all, and
        // return a `Vec` together or not at all.
        let unaligned: Vec<(usize, usize)> = borrowed
            .iter()
            .copied()
            .filter(|&(m, i)| class.hierarchy.group(m).iter().any(|&g| !borrowed.contains(&(g, i))))
            .collect();
        for key in unaligned {
            changed |= borrowed.remove(&key);
        }
        let unowned: Vec<usize> = owned_returns
            .iter()
            .copied()
            .filter(|&m| class.hierarchy.group(m).iter().any(|g| !owned_returns.contains(g)))
            .collect();
        for m in unowned {
            changed |= owned_returns.remove(&m);
        }
        for (m, scan) in scans.iter().enumerate() {
            for (i, name) in &scan.params {
                let key = (m, *i);
                if !plain[m].contains(name) && borrowed.remove(&key) {
                    changed = true;
                }
                if !borrowed.contains(&key) && plain[m].remove(name) {
                    changed = true;
                }
            }
            let unowned: Vec<String> = scan
                .assigned
                .iter()
                .chain(&scan.rows)
                .filter(|(name, value)| plain[m].contains(name) && !owned(value, &plain[m], &owned_returns))
                .map(|(name, _)| name.clone())
                .collect();
            for name in unowned {
                changed |= plain[m].remove(&name);
            }
            // The variable of a for-each over the rows of a plain array borrows them, so both are
            // plain or neither is.
            for (array, row) in &scan.row_loops {
                if plain[m].contains(array) != plain[m].contains(row) {
                    plain[m].remove(array);
                    plain[m].remove(row);
                    changed = true;
                }
            }
            if owned_returns.contains(&m) && !scan.returns.iter().all(|r| owned(r, &plain[m], &owned_returns))
            {
                owned_returns.remove(&m);
                changed = true;
            }
        }
        if !changed {
            return Solution { plain, borrowed, owned_returns };
        }
    }
}

/// Whether a value is an array that nobody else holds, given the plain locals of its method and
/// the methods that return their own arrays.
fn owned(value: &Owned, plain: &FastSet<String>, owned_returns: &FastSet<usize>) -> bool {
    match value {
        Owned::Never => false,
        Owned::Fresh => true,
        Owned::Returned(callee) => owned_returns.contains(callee),
        Owned::Moved(name) => plain.contains(name),
        Owned::Rows(rows) => rows.iter().all(|r| owned(r, plain, owned_returns)),
    }
}

/// The state of `solve` that decides whether a call can lend an argument to a borrowed parameter.
struct Lending<'s> {
    class: &'s Class,
    scans: &'s [Scan],
    plain: &'s [FastSet<String>],
    borrowed: &'s FastSet<(usize, usize)>,
    owned_returns: &'s FastSet<usize>,
    lend_shared: bool,
    /// The families of the arrays that threads share, which are never lent.
    threads_share: &'s FastSet<Type>,
}

impl Lending<'_> {
    /// Whether argument `i` of `call`, made in method `m`, can be lent to its borrowed parameter.
    /// No other argument may see the array while it is lent mutably; while it is lent to be read,
    /// other arguments may read it, or lend it again to be read. A shared array is lent by
    /// borrowing its cell, which panics if anything else borrows that cell mutably meanwhile:
    /// then no other argument may run code that could, and if the parameter is written, no other
    /// argument may be a shared array. Only a one-dimensional shared array can be lent, as the
    /// rows of a shared array of arrays are shared arrays too.
    fn lends(&self, m: usize, call: &Call, i: usize) -> bool {
        let arg = &call.args[i];
        if owned(&arg.fresh, &self.plain[m], self.owned_returns) {
            return true;
        }
        let Some(name) = &arg.name else { return false };
        let writes = self.writes(call.callee, i);
        let others = || call.args.iter().enumerate().filter(move |(j, _)| *j != i);
        for (j, other) in others() {
            let same = other.name.as_ref() == Some(name);
            let lent_again = !self.writes(call.callee, j) && self.borrowed.contains(&(call.callee, j));
            if other.mentions.contains(name) && (writes || other.effects || (same && !lent_again)) {
                return false;
            }
        }
        if self.plain[m].contains(name) {
            return true;
        }
        let param = &self.class.methods[call.callee].params[i];
        if arg.row
            || scan::depth(param.ty) > 1
            || !self.lend_shared
            || !self.scans[m].arrays.contains(name)
            || self.threads_share.contains(&crate::analysis::array_family(param.ty))
        {
            return false;
        }
        others().all(|(j, other)| {
            let shared_array = other.array && !other.name.as_ref().is_some_and(|n| self.plain[m].contains(n));
            let read_only_pair =
                !writes && !self.writes(call.callee, j) && self.borrowed.contains(&(call.callee, j));
            !other.effects && (!shared_array || read_only_pair)
        })
    }

    /// Whether method `callee` may write the elements of its parameter `i`: it writes them, or
    /// passes the array on to another call.
    fn writes(&self, callee: usize, i: usize) -> bool {
        let Some(param) = self.class.methods[callee].params.get(i) else { return true };
        let scan = &self.scans[callee];
        scan.written.contains(&param.name)
            || scan.calls.iter().any(|c| c.args.iter().any(|a| a.mentions.contains(&param.name)))
    }
}

/// The arrays whose elements are written: the ones written in their method, any argument (or
/// array whose row is the argument) passed to a borrowed parameter that is written, which must
/// then be borrowed mutably, and any array whose rows a written for-each variable borrows.
fn written_names(class: &Class, scans: &[Scan], borrowed: &FastSet<(usize, usize)>) -> Vec<FastSet<String>> {
    let mut written: Vec<FastSet<String>> = scans.iter().map(|s| s.written.clone()).collect();
    loop {
        let mut changed = false;
        for (m, scan) in scans.iter().enumerate() {
            for call in &scan.calls {
                for (i, arg) in call.args.iter().enumerate() {
                    let Some(name) = &arg.name else { continue };
                    let param = &class.methods[call.callee].params[i].name;
                    let param_written = written[call.callee].contains(param);
                    if param_written && borrowed.contains(&(call.callee, i)) {
                        changed |= written[m].insert(name.clone());
                    }
                }
            }
            for (array, row) in &scan.row_loops {
                if written[m].contains(row) {
                    changed |= written[m].insert(array.clone());
                }
            }
        }
        if !changed {
            return written;
        }
    }
}

/// A counting loop `for (int i = 0; i < a.length; i++)` over a plain array `a` (see
/// `ast::counting`), which does not assign `a` in its body.
struct OverArray<'s> {
    cond: &'s Expr,
    array: &'s str,
    index: &'s str,
    body: &'s Stmt,
}

/// The counting loop `s`, if `s` is one over an array in `plain`.
fn counting_over<'s>(s: &'s Stmt, plain: &FastMap<String, bool>) -> Option<OverArray<'s>> {
    let Stmt::For { init, cond: Some(cond), update, body } = s else { return None };
    let (index, from, to) = counting_loop(init, Some(cond), update, body)?;
    let ExprKind::Length(array) = &to.kind else { return None };
    let ExprKind::Var(array) = &array.kind else { return None };
    if !matches!(from.kind, ExprKind::Int(0)) || !plain.contains_key(array) || assigns_stmt(body, array) {
        return None;
    }
    Some(OverArray { cond, array, index, body })
}

/// The reads `a[i]` in the body of a counting loop, when the body uses `a` only to read its
/// elements and `i` only as their index, so that `for x in a.iter()` gives the same values.
fn element_reads(c: &OverArray) -> Option<Vec<ExprId>> {
    let is = |e: &Expr, name: &str| matches!(&e.kind, ExprKind::Var(v) if v == name);
    let (mut array_mentions, mut array_uses, mut index_mentions) = (0, 0, 0);
    let mut reads = Vec::new();
    visit_stmt(
        c.body,
        &mut |e| {
            visit_expr(e, &mut |x| match &x.kind {
                ExprKind::Var(v) => {
                    array_mentions += usize::from(v == c.array);
                    index_mentions += usize::from(v == c.index);
                }
                ExprKind::Index(a, i) if is(a, c.array) => {
                    array_uses += 1;
                    if is(i, c.index) {
                        reads.push(x.id);
                    }
                }
                ExprKind::Length(a) if is(a, c.array) => array_uses += 1,
                _ => {}
            })
        },
        &mut |_| {},
    );
    let only_reads = array_mentions == array_uses && index_mentions == reads.len();
    (only_reads && !reads.is_empty()).then_some(reads)
}
