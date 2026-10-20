//! Plain values: a constructor that builds its object as a plain value, as a struct literal and
//! then a local named `this`; and owned objects, which stay plain values.

use super::null::has_effects;
use super::objects::*;
use super::*;

impl<'a> Gen<'a> {
    /// The body of a constructor that builds a plain value, which `new` then shares unless the class
    /// is owned. The value starts as a struct literal of the fields' defaults and initializers, which
    /// takes in the leading assignments of the body that only read the parameters; the rest of the
    /// body runs on the value, a local named `this`, which is returned. A constructor that starts
    /// with `this(...)` starts from the value that the other constructor builds.
    pub(super) fn plain_constructor(&mut self, m: &'a Method) {
        let class = self.class;
        let decl = &class.types[m.class];
        self.this_form = ThisForm::Local;
        let (start, rest, initialize) = match m.body.split_first() {
            Some((Stmt::Expr(Expr { kind: ExprKind::ThisCall { args, resolved }, .. }), rest)) => {
                let index = resolved.expect("calls are resolved by the checker");
                let args = self.call_args(index, args).join(", ");
                (format!("{}({args})", self.qualified_fn(index)), rest, false)
            }
            _ => {
                let initialize = !self.inline_inits(m.class) && decl.fields.iter().any(|f| f.init.is_some());
                let (literal, rest) = self.struct_literal(m, initialize);
                (literal, rest, initialize)
            }
        };
        if rest.is_empty() && !initialize {
            self.line(&start);
            return;
        }
        let mutable = initialize || self.changes_owned(rest, &|e| matches!(e.kind, ExprKind::This));
        self.line(&format!("let {}this = {start};", if mutable { "mut " } else { "" }));
        if initialize {
            self.line("this.field_inits();");
        }
        self.stmts(rest, false);
        self.line("this");
    }

    /// The struct literal that a constructor starts from, and the statements of its body that are
    /// left. Effects keep Java's order: the initializers run in field order, then the body.
    fn struct_literal(&mut self, m: &'a Method, initialize: bool) -> (String, &'a [Stmt]) {
        let decl = &self.class.types[m.class];
        let fields = &decl.fields;
        let mut values: Vec<Option<&'a Expr>> =
            fields.iter().map(|f| f.init.as_ref().filter(|_| !initialize)).collect();
        // The fields whose values have effects, in the order Java runs them.
        let mut effects: Vec<usize> =
            (0..fields.len()).filter(|&i| values[i].is_some_and(has_effects)).collect();
        let mut taken = vec![false; fields.len()];
        let mut absorbed = 0;
        for stmt in &m.body {
            if initialize {
                break;
            }
            let Stmt::Expr(Expr {
                kind: ExprKind::Assign { op: None, target: Place::Field { object, name }, value },
                ..
            }) = stmt
            else {
                break;
            };
            if !matches!(object.kind, ExprKind::This) || mentions_this(value) {
                break;
            }
            let i = fields.iter().position(|f| &f.name == name).expect("a field of the class");
            if taken[i] || values[i].is_some_and(has_effects) {
                break;
            }
            if has_effects(value) {
                if effects.last().is_some_and(|&j| j >= i) {
                    break;
                }
                effects.push(i);
            }
            values[i] = Some(value);
            taken[i] = true;
            absorbed += 1;
        }
        // A field read in one value must not hold its borrow while a later value borrows mutably.
        let conflicts = self.conflicts(&values.iter().flatten().copied().collect::<Vec<_>>());
        let outer = std::mem::replace(&mut self.guarded, conflicts);
        let parts: Vec<String> = fields
            .iter()
            .zip(&values)
            .map(|(f, v)| {
                let field = snake_case(&f.name);
                let value = match v {
                    Some(e) => self.value(e, f.ty).text,
                    None => self.placeholder(f.ty),
                };
                if value == field { field } else { format!("{field}: {value}") }
            })
            .collect();
        self.guarded = outer;
        let literal = if parts.is_empty() {
            format!("{} {{}}", struct_name(&decl.name))
        } else {
            format!("{} {{ {} }}", struct_name(&decl.name), parts.join(", "))
        };
        (literal, &m.body[absorbed..])
    }

    /// Whether a statement list changes an object that `is_obj` picks out: a field write, or a call
    /// of a method that takes `&mut self`. An owned object so changed needs a `mut` binding.
    pub(super) fn changes_owned(&self, stmts: &[Stmt], is_obj: &dyn Fn(&Expr) -> bool) -> bool {
        let mut found = false;
        for s in stmts {
            visit_stmt(
                s,
                &mut |e| visit_expr(e, &mut |x| found |= self.changes_here(x, is_obj)),
                &mut |_| {},
            );
        }
        found
    }

    fn changes_here(&self, x: &Expr, is_obj: &dyn Fn(&Expr) -> bool) -> bool {
        match &x.kind {
            ExprKind::Assign { target: Place::Field { object, .. }, .. }
            | ExprKind::IncDec { target: Place::Field { object, .. }, .. } => is_obj(object),
            ExprKind::MethodCall { receiver, resolved: Some(j), .. } => {
                is_obj(receiver) && self.facts.mutating.contains(j)
            }
            _ => false,
        }
    }

    /// Whether a local needs `mut`: it is reassigned, or it is an owned object that a later statement
    /// changes.
    pub(super) fn local_is_mut(&self, ty: Type, name: &str, rest: &[Stmt]) -> bool {
        let owned = matches!(ty, Type::Class(c) if self.facts.owned.contains(c));
        assigns_any(rest, name)
            || (owned && self.changes_owned(rest, &|e| matches!(&e.kind, ExprKind::Var(v) if v == name)))
    }
}
