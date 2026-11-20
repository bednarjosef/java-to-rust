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
//! value. The language of `s.toLowerCase(locale)`, which lowering reads as the argument
//! (`case_in_locale`), is read after the check of the receiver, as the JDK's code reads it.
//!
//! The calls are those of the program's and the library's methods, and the methods of strings and
//! builders that the runtime has (but `equals`, which code generation orders itself). A receiver
//! that is never null needs none of this: `this`, a new object, or a local only ever given such
//! values. A store `a[i] = value` is the same: Java computes the value
//! before it throws for a null array or an index out of its bounds, so the array (as a receiver)
//! and the index are held before the value. A compound `a[i] += value` reads the element first,
//! and a field's store already computes its value first.

use super::*;
use std::collections::HashSet;

/// The prefix of the locals that hold a receiver; `$` keeps them apart from the program's names.
pub(crate) const RECEIVER: &str = "receiver$";
pub(crate) const ARGUMENT: &str = "argument$";

/// Whether `name` is a local that holds a call's receiver or argument.
pub(crate) fn held_local(name: &str) -> bool {
    name.starts_with(RECEIVER) || name.starts_with(ARGUMENT)
}

/// The call or store of a block that the rewrite made (see the module's comment), or that
/// `lower::regex` makes, which computes its arguments first; `None` for any other expression.
pub(crate) fn ordered_call(e: &Expr) -> Option<&Expr> {
    let ExprKind::Switch { selector, cases } = &e.kind else { return None };
    let ([case], ExprKind::Int(0)) = (&cases[..], &selector.kind) else { return None };
    let (Stmt::Yield(call) | Stmt::Expr(call), held) = case.body.split_last()? else { return None };
    // The receiver's check of a call that the JDK's `Pattern` runs (`lower::regex`) too.
    let holds = |s: &Stmt| match s {
        Stmt::Local { name, .. } => held_local(name),
        Stmt::Expr(e) => matches!(e.kind, ExprKind::Builtin(Builtin::String(StringOp::Receiver(..)), _)),
        _ => false,
    };
    (matches!(case.labels[..], [CaseLabel::Default]) && !held.is_empty() && held.iter().all(holds))
        .then_some(call)
}

impl<'t> Lowering<'t> {
    /// Rewrites the calls of `method` whose arguments may have effects (see the module's comment).
    pub(super) fn arguments_before_receivers(&self, method: &mut Method) {
        let mut names: HashSet<String> = method.params.iter().map(|p| p.name.clone()).collect();
        let mut known = Known { operands: HashSet::new(), non_null: non_null_locals(method) };
        for stmt in &method.body {
            visit_stmt(
                stmt,
                &mut |e| {
                    visit_expr(e, &mut |x| match &x.kind {
                        ExprKind::Var(name) | ExprKind::InstanceOf { binding: Some(name), .. } => {
                            names.insert(name.clone());
                        }
                        // A store that a builtin makes itself (`ElementUpdate`), which stays one.
                        ExprKind::Builtin(_, args) => known.operands.extend(args.iter().map(|a| a.id)),
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
            rewrite_stmt(stmt, &mut |e| self.argument_order(e, &known, &mut fresh));
        }
    }

    fn argument_order(&self, e: &mut Expr, known: &Known, fresh: &mut dyn FnMut(&str) -> String) {
        if let ExprKind::Assign { op: None, target: Place::Index(..), .. } = &e.kind {
            if !known.operands.contains(&e.id) {
                self.store_order(e, known, fresh);
            }
            return;
        }
        let case_in_locale = case_in_locale(e);
        let (receiver, args) = match &mut e.kind {
            ExprKind::MethodCall { receiver, args, is_super: false, .. } => (&mut **receiver, &mut args[..]),
            ExprKind::Builtin(builtin, args) if receives(builtin, args) => {
                let (receiver, rest) = args.split_first_mut().expect("a receiver");
                (receiver, rest)
            }
            _ => return,
        };
        if !may_be_null(receiver) || matches!(&receiver.kind, ExprKind::Var(v) if known.non_null.contains(v))
        {
            return;
        }
        let Some(last) = args.iter().rposition(|a| !self.quiet(a, known)) else { return };
        let (pos, ty) = (e.pos, e.ty);
        let mut body = Vec::new();
        let assigned = |name: &str| args[..=last].iter().any(|a| assigns(a, name));
        let keep = matches!(&receiver.kind, ExprKind::Var(name) if !assigned(name));
        if !keep {
            let name = fresh(RECEIVER);
            body.push(self.hold(receiver, name, pos));
        }
        for arg in &mut args[..=last] {
            if let Some(method) = case_in_locale
                && let ExprKind::MethodCall { receiver: locale, .. } = &mut arg.kind
            {
                if !self.literal(locale) {
                    body.push(self.hold(locale, fresh(ARGUMENT), pos));
                }
                let ExprKind::Var(name) = &receiver.kind else { unreachable!("a held receiver") };
                let read = self.synth(ExprKind::Var(name.clone()), pos, receiver.ty);
                let check = ExprKind::Builtin(
                    Builtin::String(StringOp::Receiver(method, "java.util.Locale")),
                    vec![read],
                );
                body.push(Stmt::Expr(self.synth(check, pos, Type::Void)));
            } else if !self.literal(arg) {
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
    /// An array that is not null still throws for an index out of its bounds.
    fn store_order(&self, e: &mut Expr, known: &Known, fresh: &mut dyn FnMut(&str) -> String) {
        let ExprKind::Assign { target: Place::Index(array, index), value, .. } = &mut e.kind else { return };
        if !may_be_null(array) || self.quiet(value, known) {
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

/// What the rewrite knows of a method.
struct Known {
    /// The stores that a builtin makes as one of its operands (`ElementUpdate`), which stay
    /// stores.
    operands: HashSet<ExprId>,
    /// The locals that are never null (`non_null_locals`).
    non_null: HashSet<String>,
}

/// The locals of `method` that are never null: each value it is given is a new object, a string
/// literal, `this` or another such local, as in `P p = new P(); p.set(p.x + 1)`, and a pattern's
/// variable (`o instanceof Tag t`) is given only the object it matched. A name that the method also
/// gives a parameter or a loop's or a catch's variable is left out, as is one a compound assignment
/// changes.
fn non_null_locals(method: &Method) -> HashSet<String> {
    let mut values: std::collections::HashMap<String, Vec<&Expr>> = std::collections::HashMap::new();
    let mut other: HashSet<String> = method.params.iter().map(|p| p.name.clone()).collect();
    for stmt in &method.body {
        visit_stmt(
            stmt,
            &mut |e| {
                visit_expr(e, &mut |x| match &x.kind {
                    ExprKind::Assign { op: None, target: Place::Var(v), value } => {
                        values.entry(v.clone()).or_default().push(value);
                    }
                    ExprKind::InstanceOf { binding: Some(v), .. } => {
                        values.entry(v.clone()).or_default();
                    }
                    ExprKind::Assign { target: Place::Var(v), .. } => {
                        other.insert(v.clone());
                    }
                    _ => {}
                })
            },
            &mut |_| {},
        );
    }
    for stmt in &method.body {
        visit_stmt(stmt, &mut |_| {}, &mut |s| match s {
            Stmt::Local { name, init, .. } => {
                values.entry(name.clone()).or_default().extend(init.as_ref());
            }
            Stmt::ForEach { name, .. } => {
                other.insert(name.clone());
            }
            Stmt::Try { catches, .. } => other.extend(catches.iter().map(|c| c.name.clone())),
            _ => {}
        });
    }
    let mut found: HashSet<String> = values.keys().filter(|v| !other.contains(*v)).cloned().collect();
    loop {
        let never_null = |e: &Expr| match &e.kind {
            ExprKind::New { .. } | ExprKind::Str(_) | ExprKind::This => true,
            ExprKind::Var(v) => found.contains(v),
            _ => false,
        };
        let lost: Vec<String> =
            found.iter().filter(|v| !values[*v].iter().all(|e| never_null(e))).cloned().collect();
        if lost.is_empty() {
            return found;
        }
        for v in lost {
            found.remove(&v);
        }
    }
}

/// For `s.toLowerCase(locale)` or `s.toUpperCase(locale)`, whose argument is the locale's language
/// (`case_in_locale`), the method's name: the JDK's code reads the language once the call has
/// checked its receiver, so the locale is held, then the receiver checked.
fn case_in_locale(e: &Expr) -> Option<&'static str> {
    let ExprKind::Builtin(Builtin::String(op @ (StringOp::ToLowerCase | StringOp::ToUpperCase)), args) =
        &e.kind
    else {
        return None;
    };
    let [_, language] = &args[..] else { return None };
    let ExprKind::MethodCall { name, .. } = &language.kind else { return None };
    (name == crate::program::LOCALE_LANGUAGE.1).then_some(if *op == StringOp::ToLowerCase {
        "toLowerCase"
    } else {
        "toUpperCase"
    })
}

/// Whether the builtin is a method of the string or builder `args[0]` that the runtime has.
fn receives(builtin: &Builtin, args: &[Expr]) -> bool {
    match builtin {
        // Code generation computes the argument first itself (`equals_optional`).
        Builtin::StringEquals | Builtin::String(StringOp::EqualsOther) => false,
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

    /// Whether evaluating `e` has no effect and cannot throw: a literal, a local, `this`, a field
    /// of `this` or of a local that is never null, arithmetic or comparisons of those that cannot divide by zero, and their
    /// conversions that cannot fail (a primitive's, its boxing, any value as an `Object`).
    fn quiet(&self, e: &Expr, known: &Known) -> bool {
        match &e.kind {
            _ if self.literal(e) => true,
            ExprKind::Var(_) | ExprKind::This => true,
            ExprKind::Field { object, .. } => match &object.kind {
                ExprKind::This => true,
                ExprKind::Var(v) => known.non_null.contains(v),
                _ => false,
            },
            ExprKind::Unary(_, x) => self.quiet(x, known) && (x.ty.is_numeric() || x.ty == Type::Boolean),
            ExprKind::Binary(op, a, b) => {
                let primitive = |t: Type| t.is_numeric() || t == Type::Boolean;
                // Only an integer division by zero throws.
                let divides = matches!(op, BinOp::Div | BinOp::Rem)
                    && !matches!(e.ty, Type::Float | Type::Double)
                    && !matches!(b.kind, ExprKind::Int(n) if n != 0)
                    && !matches!(b.kind, ExprKind::Long(n) if n != 0);
                !divides && primitive(a.ty) && primitive(b.ty) && self.quiet(a, known) && self.quiet(b, known)
            }
            ExprKind::Cast(to, x) => {
                self.quiet(x, known)
                    && match to {
                        Type::Boxed(primitive) => **primitive == x.ty,
                        Type::Class(name, _) => *name == OBJECT,
                        _ => to.is_numeric() && x.ty.is_numeric(),
                    }
            }
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
