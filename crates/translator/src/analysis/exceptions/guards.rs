//! Calls that raise nothing for the arguments they pass: a call whose arguments' values are known
//! skips the code of its callee that those values make dead (an `if (cond) throw ...;` whose
//! `cond` is false, a loop over an empty array), and when nothing the rest of the callee runs
//! can raise, the call raises nothing either. The JDK's static initializers have such calls:
//! `ArraysSupport.exactLog2(Unsafe.ARRAY_INT_INDEX_SCALE)`, and `new SetN<>()` and
//! `new MapN<>()` in `ImmutableCollections`, whose varargs constructors loop over no elements.
//!
//! An argument's value is known when it is an integer literal or constant, a static that its
//! class's initializer assigns once from what the runtime answers for the JVM
//! (`Unsafe.arrayIndexScale`), or a new array of known length. A read of such a static during a
//! circular initialization sees its default, so the call must raise nothing with that too.

use super::*;
use crate::analysis::constants::static_value;
use std::collections::HashMap;

/// How many times the program's code assigns each static, by its class and name, and the
/// statements of its class's `<clinit>` that assign it (by method and statement), found once for
/// the program instead of once for each read.
#[derive(Default)]
pub(in crate::analysis) struct StaticWrites {
    of: HashMap<(usize, String), Writes>,
}

/// How many times a static is assigned, and its `<clinit>`'s statements that assign it.
type Writes = (usize, Vec<(usize, usize)>);

pub(super) fn static_writes(class: &Class) -> StaticWrites {
    let mut of: HashMap<(usize, String), Writes> = HashMap::new();
    for (m, method) in class.methods.iter().enumerate() {
        let clinit = method.name == "<clinit>";
        for (i, s) in method.body.iter().enumerate() {
            if clinit
                && let Stmt::Expr(Expr {
                    kind: ExprKind::Assign { op: None, target: Place::Static { class: c, name }, .. },
                    ..
                }) = s
                && *c == method.class
            {
                of.entry((*c, name.clone())).or_default().1.push((m, i));
            }
            visit_stmt(
                s,
                &mut |e| {
                    visit_expr(e, &mut |y| {
                        if let ExprKind::Assign { target: Place::Static { class: c, name }, .. }
                        | ExprKind::IncDec { target: Place::Static { class: c, name }, .. } = &y.kind
                        {
                            of.entry((*c, name.clone())).or_default().0 += 1;
                        }
                    })
                },
                &mut |_| {},
            );
        }
    }
    StaticWrites { of }
}

/// A known value: an integer (or a boolean, 0 or 1), or an array of this length.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Value {
    Int(i128),
    Array(i128),
}

/// The tuples of argument values that the call `x` may pass, every combination of each
/// argument's possible values (`None` for an unknown one), when `x` calls one method or
/// constructor and some argument is known; else none.
pub(super) fn known_args(class: &Class, writes: &StaticWrites, x: &Expr) -> Vec<Vec<Option<Value>>> {
    let args = match &x.kind {
        ExprKind::Call { args, resolved: Some(_), .. } | ExprKind::New { args, ctor: Some(_), .. } => args,
        _ => return Vec::new(),
    };
    let choices: Vec<Vec<Option<Value>>> = args
        .iter()
        .map(|a| known(class, writes, a).map_or(vec![None], |v| v.into_iter().map(Some).collect()))
        .collect();
    if choices.iter().all(|c| c[..] == [None]) {
        return Vec::new();
    }
    let mut tuples: Vec<Vec<Option<Value>>> = vec![Vec::new()];
    for choice in &choices {
        tuples = tuples.iter().flat_map(|t| choice.iter().map(move |&v| [&t[..], &[v]].concat())).collect();
        if tuples.len() > 16 {
            return Vec::new();
        }
    }
    tuples
}

/// The values that `e` may have when read.
fn known(class: &Class, writes: &StaticWrites, e: &Expr) -> Option<Vec<Value>> {
    if let ExprKind::Static { class: owner, name } = &e.kind
        && static_value(class, *owner, name).is_none()
    {
        return static_values(class, writes, *owner, name).map(|v| v.into_iter().map(Value::Int).collect());
    }
    eval(class, writes, e, &HashMap::new()).map(|v| vec![v])
}

/// The values that the static `name` of class `owner` may have when read: what its class's
/// initializer assigns it once, and its default 0, which a read during a circular initialization
/// sees.
fn static_values(class: &Class, writes: &StaticWrites, owner: usize, name: &str) -> Option<Vec<i128>> {
    let value = assigned_once(class, writes, owner, name)?;
    let value = match jvm_value(value) {
        Some(v) => v,
        None => match eval(class, writes, value, &HashMap::new())? {
            Value::Int(v) => v,
            Value::Array(_) => return None,
        },
    };
    Some(if value == 0 { vec![0] } else { vec![value, 0] })
}

/// Whether condition `cond` is always true or always false, from literals, constants and the
/// statics that `static_values` knows: the JDK's `if (LegacyMergeSort.userRequested)`, whose
/// flag is `Boolean.getBoolean` of a property that a translated program never sets.
pub(super) fn known_condition(class: &Class, writes: &StaticWrites, cond: &Expr) -> Option<bool> {
    match eval(class, writes, cond, &HashMap::new())? {
        Value::Int(v) => Some(v != 0),
        Value::Array(_) => None,
    }
}

/// The value that class `owner` gives its static `name` when it is initialized: the one
/// statement of its `<clinit>` that assigns it (the lowering writes a static's initializer there
/// too) or else its initializer, when nothing else in the program assigns it.
fn assigned_once<'c>(class: &'c Class, writes: &StaticWrites, owner: usize, name: &str) -> Option<&'c Expr> {
    let (count, at) = writes.of.get(&(owner, name.to_string())).map_or((0, &[][..]), |(n, at)| (*n, &at[..]));
    let assigned: Vec<&Expr> = at
        .iter()
        .filter_map(|&(m, s)| match &class.methods[m].body[s] {
            Stmt::Expr(Expr { kind: ExprKind::Assign { value, .. }, .. }) => Some(&**value),
            _ => None,
        })
        .collect();
    let init = class.types[owner].statics.iter().find(|f| f.name == name)?.init.as_ref();
    match (&assigned[..], count, init) {
        (&[value], 1, _) => Some(value),
        ([], 0, Some(init)) => Some(init),
        _ => None,
    }
}

/// What the runtime answers for a native of the JVM that gives a number (see `vm.rs`).
fn jvm_value(e: &Expr) -> Option<i128> {
    match &e.kind {
        ExprKind::Builtin(Builtin::Native(Native::ArrayIndexScale(component)), _) => {
            Some(i128::from(java_runtime::array_index_scale(component)))
        }
        ExprKind::Builtin(Builtin::Native(Native::ArrayBaseOffset), _) => {
            Some(i128::from(java_runtime::array_base_offset()))
        }
        _ => None,
    }
}

/// The code of a method that a call with known arguments runs and skips, by the positions of
/// its expressions.
#[derive(Default)]
pub(super) struct Run {
    /// Expressions in code that does not run.
    dead: Vec<Pos>,
    /// Expressions in code that may run.
    live: Vec<Pos>,
    /// Array creations whose sizes are known not to be negative.
    sized: Vec<Pos>,
}

impl Run {
    /// Whether the point at `pos` that raises `raise` cannot raise in this run.
    pub(super) fn skips(&self, pos: Pos, raise: &Raise) -> bool {
        if matches!(raise, Raise::Runtime { class: "java.lang.NegativeArraySizeException", .. })
            && self.sized.contains(&pos)
        {
            return true;
        }
        // A position that code that runs shares (a synthesized expression) counts as running.
        !self.live.contains(&pos) && self.dead.contains(&pos)
    }
}

/// What method `m` runs when called with `args`.
pub(super) fn run(class: &Class, writes: &StaticWrites, m: usize, args: &[Option<Value>]) -> Run {
    let method = &class.methods[m];
    let mut env: HashMap<&str, Value> = HashMap::new();
    for (p, a) in method.params.iter().zip(args) {
        if let Some(v) = a {
            env.insert(&p.name, *v);
        }
    }
    let mut run = Run::default();
    run.live.push(method.pos);
    walk(class, writes, &method.body, &mut env, &mut run);
    run
}

fn walk<'c>(
    class: &'c Class,
    writes: &StaticWrites,
    stmts: &'c [Stmt],
    env: &mut HashMap<&'c str, Value>,
    run: &mut Run,
) {
    for s in stmts {
        stmt(class, writes, s, env, run);
    }
}

/// The positions of the expressions in `s`.
fn positions(s: &Stmt, out: &mut Vec<Pos>) {
    visit_stmt(s, &mut |e| visit_expr(e, &mut |y| out.push(y.pos)), &mut |_| {});
}

fn expr_positions(e: &Expr, out: &mut Vec<Pos>) {
    visit_expr(e, &mut |y| out.push(y.pos));
}

/// The locals that `s` declares or assigns.
fn assigned(s: &Stmt) -> Vec<String> {
    let (mut out, mut declared) = (Vec::new(), Vec::new());
    visit_stmt(
        s,
        &mut |e| {
            visit_expr(e, &mut |y| {
                if let ExprKind::Assign { target: Place::Var(v), .. }
                | ExprKind::IncDec { target: Place::Var(v), .. } = &y.kind
                {
                    out.push(v.clone());
                }
            })
        },
        &mut |s| {
            if let Stmt::Local { name, .. } | Stmt::ForEach { name, .. } = s {
                declared.push(name.clone());
            }
        },
    );
    out.extend(declared);
    out
}

fn forget(env: &mut HashMap<&str, Value>, names: &[String]) {
    for n in names {
        env.remove(n.as_str());
    }
}

fn stmt<'c>(
    class: &'c Class,
    writes: &StaticWrites,
    s: &'c Stmt,
    env: &mut HashMap<&'c str, Value>,
    run: &mut Run,
) {
    // The array creations of the statement's own expressions whose sizes are known.
    let own: Vec<&Expr> = match s {
        Stmt::Local { init: Some(e), .. } | Stmt::Expr(e) | Stmt::Return(Some(e), _) | Stmt::Throw(e) => {
            vec![e]
        }
        Stmt::If { cond, .. } | Stmt::While { cond, .. } => vec![cond],
        _ => Vec::new(),
    };
    for e in own {
        visit_expr(e, &mut |y| {
            if let ExprKind::NewArray(dims) = &y.kind
                && dims.iter().all(|d| matches!(eval(class, writes, d, env), Some(Value::Int(n)) if n >= 0))
            {
                run.sized.push(y.pos);
            }
        });
    }
    match s {
        Stmt::Local { name, init, .. } => {
            positions(s, &mut run.live);
            let value = init.as_ref().and_then(|e| eval(class, writes, e, env));
            forget(env, &assigned(s));
            if let Some(v) = value {
                env.insert(name, v);
            }
        }
        Stmt::Expr(Expr { kind: ExprKind::Assign { op: None, target: Place::Var(name), value }, .. }) => {
            positions(s, &mut run.live);
            let value = eval(class, writes, value, env);
            forget(env, &assigned(s));
            if let Some(v) = value {
                env.insert(name, v);
            }
        }
        Stmt::Block(stmts) => walk(class, writes, stmts, env, run),
        Stmt::If { cond, then, otherwise } => {
            expr_positions(cond, &mut run.live);
            match eval(class, writes, cond, env) {
                Some(Value::Int(0)) => {
                    positions(then, &mut run.dead);
                    if let Some(o) = otherwise {
                        stmt(class, writes, o, env, run);
                    }
                }
                Some(Value::Int(_)) => {
                    if let Some(o) = otherwise {
                        positions(o, &mut run.dead);
                    }
                    stmt(class, writes, then, env, run);
                }
                _ => {
                    let mut other = env.clone();
                    stmt(class, writes, then, env, run);
                    if let Some(o) = otherwise {
                        stmt(class, writes, o, &mut other, run);
                    }
                    // What both branches leave the same stays known.
                    env.retain(|k, v| other.get(k) == Some(v));
                }
            }
        }
        Stmt::For { init, cond, update, body } => {
            walk(class, writes, init, env, run);
            if let Some(c) = cond {
                expr_positions(c, &mut run.live);
                if eval(class, writes, c, env) == Some(Value::Int(0)) {
                    positions(body, &mut run.dead);
                    update.iter().for_each(|u| expr_positions(u, &mut run.dead));
                    return;
                }
            }
            forget(env, &assigned(s));
            update.iter().for_each(|u| expr_positions(u, &mut run.live));
            stmt(class, writes, body, &mut env.clone(), run);
            forget(env, &assigned(s));
        }
        Stmt::While { cond, body } => {
            expr_positions(cond, &mut run.live);
            if eval(class, writes, cond, env) == Some(Value::Int(0)) {
                positions(body, &mut run.dead);
                return;
            }
            forget(env, &assigned(s));
            stmt(class, writes, body, &mut env.clone(), run);
            forget(env, &assigned(s));
        }
        _ => {
            positions(s, &mut run.live);
            forget(env, &assigned(s));
        }
    }
}

/// The value of an expression of the locals `env` that uses only operations that cannot fail,
/// when it is within its type's range (Java's arithmetic wraps; within the range it is exact).
fn eval(class: &Class, writes: &StaticWrites, e: &Expr, env: &HashMap<&str, Value>) -> Option<Value> {
    let int = |e: &Expr| match eval(class, writes, e, env)? {
        Value::Int(v) => Some(v),
        Value::Array(_) => None,
    };
    let v = match &e.kind {
        ExprKind::Int(v) => i128::from(*v),
        ExprKind::Long(v) => i128::from(*v),
        ExprKind::Bool(b) => i128::from(*b),
        ExprKind::Var(name) => return env.get(name.as_str()).copied(),
        ExprKind::Static { class: owner, name } => match static_value(class, *owner, name) {
            Some(v) => v,
            // A static read before its class is initialized is its default, so its value is
            // known only when that is the value it is given.
            None => match static_values(class, writes, *owner, name)?[..] {
                [v] => v,
                _ => return None,
            },
        },
        ExprKind::NewArray(dims) => {
            let [d] = &dims[..] else { return None };
            return Some(Value::Array(int(d).filter(|&n| n >= 0)?));
        }
        ExprKind::ArrayLit(items) => return Some(Value::Array(items.len() as i128)),
        ExprKind::Length(array) => match eval(class, writes, array, env)? {
            Value::Array(n) => n,
            Value::Int(_) => return None,
        },
        ExprKind::Cast(Type::Int | Type::Long, x) if matches!(x.ty, Type::Int | Type::Long) => int(x)?,
        ExprKind::Unary(UnOp::Neg, x) => -int(x)?,
        ExprKind::Unary(UnOp::Plus, x) => int(x)?,
        ExprKind::Unary(UnOp::Not, x) => i128::from(int(x)? == 0),
        ExprKind::Unary(UnOp::BitNot, x) => !int(x)?,
        ExprKind::Conditional(c, a, b) => {
            return if int(c)? != 0 { eval(class, writes, a, env) } else { eval(class, writes, b, env) };
        }
        ExprKind::Binary(op, l, r) => {
            let (a, b) = (int(l)?, int(r)?);
            match op {
                BinOp::Add => a + b,
                BinOp::Sub => a - b,
                BinOp::Mul => a.checked_mul(b)?,
                BinOp::Shl if (0..32).contains(&b) => a.checked_mul(1 << b)?,
                BinOp::Shr if (0..32).contains(&b) => a >> b,
                BinOp::BitAnd => a & b,
                BinOp::BitOr => a | b,
                BinOp::BitXor => a ^ b,
                BinOp::And => i128::from(a != 0 && b != 0),
                BinOp::Or => i128::from(a != 0 || b != 0),
                BinOp::Eq => i128::from(a == b),
                BinOp::Ne => i128::from(a != b),
                BinOp::Lt => i128::from(a < b),
                BinOp::Le => i128::from(a <= b),
                BinOp::Gt => i128::from(a > b),
                BinOp::Ge => i128::from(a >= b),
                _ => return None,
            }
        }
        _ => return None,
    };
    let fits = match e.ty {
        Type::Int => (i128::from(i32::MIN)..=i128::from(i32::MAX)).contains(&v),
        Type::Long => (i128::from(i64::MIN)..=i128::from(i64::MAX)).contains(&v),
        Type::Boolean => v == 0 || v == 1,
        _ => false,
    };
    fits.then_some(Value::Int(v))
}
