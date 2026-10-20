//! Plain values: a constructor that builds its object as a plain value, as a struct literal and
//! then a local named `this`; and owned objects, which stay plain values.

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
    /// left. The literal holds the fields' initializers and the leading assignments of the body
    /// (see `analysis::leading_assignments`). Effects keep Java's order, the initializers in field
    /// order and then the assignments: when the literal's field order would change it, the values
    /// with effects are computed into locals first.
    fn struct_literal(&mut self, m: &'a Method, initialize: bool) -> (String, &'a [Stmt]) {
        let decl = &self.class.types[m.class];
        let fields = &decl.fields;
        let leading = crate::analysis::leading_assignments(self.class, m, self.facts);
        let mut values: Vec<Option<&'a Expr>> =
            fields.iter().map(|f| f.init.as_ref().filter(|_| !initialize)).collect();
        // The fields whose values have effects, in the order Java runs them.
        let mut effects: Vec<usize> =
            (0..fields.len()).filter(|&i| values[i].is_some_and(has_effects)).collect();
        for &(i, value) in &leading {
            values[i] = Some(value);
            if has_effects(value) {
                effects.push(i);
            }
        }
        let in_order = effects.windows(2).all(|w| w[0] < w[1]);
        let mut bound: Vec<Option<String>> = vec![None; fields.len()];
        if !in_order {
            let mut taken = Vec::new();
            for &i in &effects {
                let f = &fields[i];
                let mut temp = self.temp(&snake_case(&f.name));
                while taken.contains(&temp) {
                    temp.push('_');
                }
                let value = self.field_value(
                    values[i].expect("has a value"),
                    f.ty,
                    self.field_nullable(&decl.name, &f.name),
                );
                self.line(&format!("let {temp} = {value};"));
                taken.push(temp.clone());
                bound[i] = Some(temp);
            }
        }
        // A field read in one value must not hold its borrow while a later value borrows mutably.
        let conflicts = self.conflicts(&values.iter().flatten().copied().collect::<Vec<_>>());
        let outer = std::mem::replace(&mut self.guarded, conflicts);
        let parts: Vec<String> = fields
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let field = snake_case(&f.name);
                let value = match (&bound[i], values[i]) {
                    (Some(temp), _) => temp.clone(),
                    (None, Some(e)) => self.field_value(e, f.ty, self.field_nullable(&decl.name, &f.name)),
                    (None, None) => self.placeholder(f.ty, self.field_nullable(&decl.name, &f.name)),
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
        (literal, &m.body[leading.len()..])
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
