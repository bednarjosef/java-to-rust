//! The order of a call on an object that may be null. Java evaluates the receiver, then the
//! arguments, and only then throws the `NullPointerException` of a null receiver; the
//! translation checks the receiver where it reads it, so an argument's effects (what it prints,
//! what it throws, the classes it initializes) would not happen. Where an argument may have an
//! effect, its value is computed first into a local, and so is the receiver's, unless the
//! receiver is a local that no argument assigns: `o.f().m(g())` becomes `{ T receiver$0 = o.f();
//! A argument$1 = g(); yield receiver$0.m(argument$1); }`, and a call of a `void` method the same
//! block without the `yield`. The arguments before the last that may have an effect are computed
//! first too, as one of those may change what an earlier one reads. The message of the exception
//! names the receiver as Java's does: `npe_messages` follows a `receiver$` local back to its
//! value.
//!
//! The calls are those of the program's and the library's methods, and the methods of strings and
//! builders that the runtime has. A store `a[i] = value` is the same: Java computes the value
//! before it throws for a null array or an index out of its bounds, so the array (as a receiver)
//! and the index are held before the value. A compound `a[i] += value` reads the element first,
//! and a field's store already computes its value first.

use super::*;
use std::collections::HashSet;

/// The prefix of the locals that hold a receiver; `$` keeps them apart from the program's names.
pub(crate) const RECEIVER: &str = "receiver$";
const ARGUMENT: &str = "argument$";

/// Whether `name` is a local that holds a call's receiver or argument.
pub(crate) fn held_local(name: &str) -> bool {
    name.starts_with(RECEIVER) || name.starts_with(ARGUMENT)
}

/// The call or store of a block that the rewrite made (see the module's comment), which computes
/// its arguments first; `None` for any other expression.
pub(crate) fn ordered_call(e: &Expr) -> Option<&Expr> {
    let ExprKind::Switch { selector, cases } = &e.kind else { return None };
    let ([case], ExprKind::Int(0)) = (&cases[..], &selector.kind) else { return None };
    let (Stmt::Yield(call) | Stmt::Expr(call), held) = case.body.split_last()? else { return None };
    let holds = |s: &Stmt| matches!(s, Stmt::Local { name, .. } if held_local(name));
    (matches!(case.labels[..], [CaseLabel::Default]) && !held.is_empty() && held.iter().all(holds))
        .then_some(call)
}

impl<'t> Lowering<'t> {
    /// Rewrites the calls of `method` whose arguments may have effects (see the module's comment).
    pub(super) fn arguments_before_receivers(&self, method: &mut Method) {
        let mut names: HashSet<String> = method.params.iter().map(|p| p.name.clone()).collect();
        let mut operands = HashSet::new();
        for stmt in &method.body {
            visit_stmt(
                stmt,
                &mut |e| {
                    visit_expr(e, &mut |x| match &x.kind {
                        ExprKind::Var(name) | ExprKind::InstanceOf { binding: Some(name), .. } => {
                            names.insert(name.clone());
                        }
                        // A store that a builtin makes itself (`ElementUpdate`), which stays one.
                        ExprKind::Builtin(_, args) => operands.extend(args.iter().map(|a| a.id)),
                        _ => {}
                    })
                },
                &mut |_| {},
            );
        }
        let mut next = 0;
        let mut fresh = |prefix: &str| loop {
            let name = format!("{prefix}{next}");
            next += 1;
            if !names.contains(&name) {
                return name;
            }
        };
        for stmt in &mut method.body {
            rewrite_stmt(stmt, &mut |e| self.argument_order(e, &operands, &mut fresh));
        }
    }

    fn argument_order(
        &self,
        e: &mut Expr,
        operands: &HashSet<ExprId>,
        fresh: &mut dyn FnMut(&str) -> String,
    ) {
        if let ExprKind::Assign { op: None, target: Place::Index(..), .. } = &e.kind {
            if !operands.contains(&e.id) {
                self.store_order(e, fresh);
            }
            return;
        }
        let (receiver, args) = match &mut e.kind {
            ExprKind::MethodCall { receiver, args, is_super: false, .. } => (&mut **receiver, &mut args[..]),
            ExprKind::Builtin(builtin, args) if receives(builtin, args) => {
                let (receiver, rest) = args.split_first_mut().expect("a receiver");
                (receiver, rest)
            }
            _ => return,
        };
        if !may_be_null(receiver) {
            return;
        }
        let Some(last) = args.iter().rposition(|a| !self.quiet(a)) else { return };
        let (pos, ty) = (e.pos, e.ty);
        let mut body = Vec::new();
        let assigned = |name: &str| args[..=last].iter().any(|a| assigns(a, name));
        let keep = matches!(&receiver.kind, ExprKind::Var(name) if !assigned(name));
        if !keep {
            let name = fresh(RECEIVER);
            body.push(self.hold(receiver, name, pos));
        }
        for arg in &mut args[..=last] {
            if !self.literal(arg) {
                let name = fresh(ARGUMENT);
                body.push(self.hold(arg, name, pos));
            }
        }
        let call = std::mem::replace(e, self.synth(ExprKind::Null, pos, Type::Null));
        body.push(if ty == Type::Void { Stmt::Expr(call) } else { Stmt::Yield(call) });
        let block = ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body }],
        };
        *e = self.synth(block, pos, ty);
    }

    /// `a[i] = value` (see the module's comment): Java stores, and throws for a null array or an
    /// index out of its bounds, once the value is computed.
    fn store_order(&self, e: &mut Expr, fresh: &mut dyn FnMut(&str) -> String) {
        let ExprKind::Assign { target: Place::Index(array, index), value, .. } = &mut e.kind else { return };
        if !may_be_null(array) || self.quiet(value) {
            return;
        }
        let (pos, ty) = (e.pos, e.ty);
        let mut body = Vec::new();
        let assigned = |name: &str| assigns(index, name) || assigns(value, name);
        if !matches!(&array.kind, ExprKind::Var(name) if !assigned(name)) {
            body.push(self.hold(array, fresh(RECEIVER), pos));
        }
        if !self.literal(index) {
            body.push(self.hold(index, fresh(ARGUMENT), pos));
        }
        body.push(self.hold(value, fresh(ARGUMENT), pos));
        let store = std::mem::replace(e, self.synth(ExprKind::Null, pos, Type::Null));
        body.push(Stmt::Yield(store));
        let block = ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body }],
        };
        *e = self.synth(block, pos, ty);
    }

    /// `T name = value;`, with `value` replaced by a read of `name`.
    fn hold(&self, value: &mut Expr, name: String, pos: Pos) -> Stmt {
        let ty = value.ty;
        let read = self.synth(ExprKind::Var(name.clone()), value.pos, ty);
        let init = std::mem::replace(value, read);
        Stmt::Local { ty, name, init: Some(init), pos }
    }
}

/// Whether the builtin is a method of the string or builder `args[0]` that the runtime has.
fn receives(builtin: &Builtin, args: &[Expr]) -> bool {
    match builtin {
        Builtin::Builder(op) => *op != BuilderOp::New && !args.is_empty(),
        _ => {
            !args.is_empty()
                && args[0].ty == Type::String
                && crate::analysis::npe_messages::string_method(builtin, args).is_some()
        }
    }
}

/// Whether the receiver `e` may be null: anything but `this`, a new object and a literal.
fn may_be_null(e: &Expr) -> bool {
    !matches!(e.kind, ExprKind::This | ExprKind::New { .. } | ExprKind::Str(_))
}

impl<'t> Lowering<'t> {
    /// A literal, or a constant static field, which Java's class file holds as a literal: it
    /// reads nothing and initializes no class.
    fn literal(&self, e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Int(_)
            | ExprKind::Long(_)
            | ExprKind::Double(_)
            | ExprKind::Float(_)
            | ExprKind::Char(_)
            | ExprKind::Bool(_)
            | ExprKind::Str(_)
            | ExprKind::Null => true,
            ExprKind::Static { class, name } => self.types.get(*class).is_some_and(|t| {
                t.node
                    .list("members")
                    .any(|m| m.kind() == "VARIABLE" && m.str("name") == name && Lowering::is_constant(m))
            }),
            _ => false,
        }
    }

    /// Whether evaluating `e` has no effect and cannot throw: a literal, a local, `this`, and
    /// arithmetic or comparisons of those that cannot divide by zero.
    fn quiet(&self, e: &Expr) -> bool {
        match &e.kind {
            _ if self.literal(e) => true,
            ExprKind::Var(_) | ExprKind::This => true,
            ExprKind::Unary(_, x) => self.quiet(x) && (x.ty.is_numeric() || x.ty == Type::Boolean),
            ExprKind::Binary(op, a, b) => {
                let primitive = |t: Type| t.is_numeric() || t == Type::Boolean;
                !matches!(op, BinOp::Div | BinOp::Rem)
                    && primitive(a.ty)
                    && primitive(b.ty)
                    && self.quiet(a)
                    && self.quiet(b)
            }
            ExprKind::Cast(to, x) => self.quiet(x) && to.is_numeric() && x.ty.is_numeric(),
            _ => false,
        }
    }
}

/// Whether `e` assigns the local `name`.
fn assigns(e: &Expr, name: &str) -> bool {
    let mut found = false;
    visit_expr(e, &mut |x| {
        if let ExprKind::Assign { target: Place::Var(v), .. }
        | ExprKind::IncDec { target: Place::Var(v), .. } = &x.kind
        {
            found |= v == name;
        }
    });
    found
}
