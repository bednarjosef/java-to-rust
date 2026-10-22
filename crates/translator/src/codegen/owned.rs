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
        let body = self.prologue(&m.body);
        let (start, rest, initialize) = match body.split_first() {
            Some((Stmt::Expr(Expr { kind: ExprKind::ThisCall { args, resolved }, .. }), rest)) => {
                let index = resolved.expect("calls are resolved by the checker");
                let args = self.call_args(index, args).join(", ");
                let call = Code::atom(format!("{}({args})", self.qualified_fn(index)));
                (self.raising(call, index).text, rest, false)
            }
            _ => {
                let initialize = !self.inline_inits(m.class) && decl.fields.iter().any(|f| f.init.is_some());
                let (base, body) = self.super_init(m.class, body);
                let (literal, rest) = self.struct_literal(m, initialize, base, body);
                (literal, rest, initialize)
            }
        };
        let rest = without_final_return(rest);
        if rest.is_empty() && !initialize {
            // A constructor that only delegates to one that throws returns its `Result`.
            let start = match start.strip_suffix('?') {
                Some(result) if self.throwing => result.to_string(),
                _ => self.ok(start),
            };
            self.line(&start);
            return;
        }
        let mutable = initialize || self.changes_owned(rest, &|e| matches!(e.kind, ExprKind::This));
        self.line(&format!("let {}this = {start};", if mutable { "mut " } else { "" }));
        if initialize {
            let call = self.run_inits(m.class, "this.field_inits()".to_string());
            self.line(&call);
        }
        self.ctor_result = Some("this");
        self.stmts(rest, false);
        self.ctor_result = None;
        let this = self.ok("this".to_string());
        self.line(&this);
    }

    /// The struct literal that a constructor starts from, and the statements of its body that are
    /// left. The literal holds the fields' initializers and the leading assignments of the body
    /// (see `analysis::leading_assignments`), written in Java's order, the initializers in field
    /// order and then the assignments, as the moves of variables expect. Effects keep that order:
    /// an initializer that a later one reads, or that has effects
    /// when a later one reads a field, is computed into a local first, and so is every value with
    /// effects when the literal's field order would change their order.
    /// The superclass's fields struct, `base`, comes first, built by the constructor's `super(...)`
    /// before anything else runs.
    fn struct_literal(
        &mut self,
        m: &'a Method,
        initialize: bool,
        base: Option<String>,
        body: &'a [Stmt],
    ) -> (String, &'a [Stmt]) {
        let decl = &self.class.types[m.class];
        let fields = &decl.fields;
        let leading = crate::analysis::leading_assignments(self.class, m, self.facts);
        let inits: Vec<Option<&'a Expr>> =
            fields.iter().map(|f| f.init.as_ref().filter(|_| !initialize)).collect();
        // The fields that a later initializer reads.
        let read: Vec<bool> = fields
            .iter()
            .map(|f| {
                inits.iter().flatten().any(|e| {
                    let mut found = false;
                    visit_expr(e, &mut |x| {
                        found |= matches!(&x.kind, ExprKind::Field { object, name }
                            if matches!(object.kind, ExprKind::This) && *name == f.name)
                    });
                    found
                })
            })
            .collect();
        // A leading assignment that reads a field an earlier one set computes them all into locals.
        let reads_leading = crate::analysis::reads_leading_fields(decl, &leading);
        let sequential = read.iter().any(|&r| r) || reads_leading;
        // `super(...)` runs first: before any value that is computed into a local first.
        let base_part = base.map(|b| {
            let values = inits.iter().flatten().copied().chain(leading.iter().map(|&(_, v)| v));
            // A borrow in `super(...)` (the enclosing instance's enclosing instance, read through
            // the `outer` parameter, or a field of a parameter's superclass through `base()`, as
            // javac's `new AssignItem(lhs)` reads `lhs.typecode`) lasts to the end of the literal,
            // which may move what it borrows.
            let borrows =
                [".borrow()", ".base()", ".base_mut()"].iter().any(|r| b.contains(r)) && !leading.is_empty();
            if sequential || borrows || values.into_iter().any(has_effects) {
                let temp = self.temp("base");
                self.line(&format!("let {temp} = {b};"));
                if temp == "base" { temp } else { format!("base: {temp}") }
            } else {
                format!("base: {b}")
            }
        });
        let mut bound: Vec<Option<String>> = vec![None; fields.len()];
        let mut taken = Vec::new();
        let bind = |g: &mut Self, i: usize, value: &'a Expr, taken: &mut Vec<String>| {
            let f = &fields[i];
            let mut temp = g.temp(&snake_case(&f.name));
            while taken.contains(&temp) {
                temp.push('_');
            }
            let nullable = g.field_nullable(&decl.name, &f.name);
            if !nullable && (f.ty.is_numeric() || f.ty == Type::Boolean) {
                // A number's type cannot come from the literal it starts as.
                let code = g.value(value, f.ty);
                let annotation = if code.untyped { format!(": {}", rust_type(f.ty)) } else { String::new() };
                g.line(&format!("let {temp}{annotation} = {};", code.text));
            } else {
                let text = g.field_value(value, f.ty, nullable);
                g.line(&format!("let {temp} = {text};"));
            }
            taken.push(temp.clone());
            temp
        };
        let mut values = inits.clone();
        if sequential {
            // Every initializer that is read or has effects is computed in field order, first.
            for i in 0..fields.len() {
                if let Some(init) = inits[i]
                    && (read[i] || has_effects(init))
                {
                    let local = bind(self, i, init, &mut taken);
                    if read[i] {
                        self.field_locals.insert(fields[i].name.clone(), local.clone());
                    }
                    bound[i] = Some(local);
                }
            }
        }
        if reads_leading {
            // The initializers with effects run before the body, then the leading assignments
            // in order, each a local that the later ones read.
            for i in 0..fields.len() {
                if bound[i].is_none()
                    && let Some(init) = inits[i].filter(|e| has_effects(e) || reads_static(e))
                {
                    bound[i] = Some(bind(self, i, init, &mut taken));
                }
            }
            for &(i, value) in &leading {
                let local = bind(self, i, value, &mut taken);
                self.field_locals.insert(fields[i].name.clone(), local.clone());
                values[i] = Some(value);
                bound[i] = Some(local);
            }
        }
        // The fields whose values have effects, in the order Java runs them. An initializer that
        // reads a static runs before the body, which may change it.
        let mut effects: Vec<usize> = (0..fields.len())
            .filter(|&i| bound[i].is_none() && values[i].is_some_and(|e| has_effects(e) || reads_static(e)))
            .collect();
        for &(i, value) in leading.iter().filter(|_| !reads_leading) {
            values[i] = Some(value);
            bound[i] = None;
            if has_effects(value) {
                effects.push(i);
            }
        }
        if !effects.windows(2).all(|w| w[0] < w[1]) {
            // The values before the last of them in Java's order are computed first too, in that
            // order: one left in the literal would run after the locals, and its move or clone of
            // a variable after a later value's. A number or a boolean moves nothing.
            let assigned: Vec<usize> = leading.iter().map(|&(i, _)| i).collect();
            let java_order =
                (0..fields.len()).filter(|i| !assigned.contains(i)).chain(assigned.iter().copied());
            let last = *effects.last().expect("unordered effects");
            for i in java_order {
                if bound[i].is_none()
                    && let Some(value) = values[i]
                    && (effects.contains(&i) || !(fields[i].ty.is_numeric() || fields[i].ty == Type::Boolean))
                {
                    bound[i] = Some(bind(self, i, value, &mut taken));
                }
                if i == last {
                    break;
                }
            }
        }
        // A field read in one value must not hold its borrow while a later value borrows mutably.
        let unbound: Vec<&Expr> =
            (0..fields.len()).filter(|&i| bound[i].is_none()).filter_map(|i| values[i]).collect();
        let conflicts = self.conflicts(&unbound);
        let outer = std::mem::replace(&mut self.guarded, conflicts);
        let outer_steady = self.steady.take();
        let mut parts: Vec<String> = base_part.into_iter().collect();
        // The values in Java's order, which a struct literal keeps whatever the order of its
        // fields: the initializers in field order, then the leading assignments in theirs. The
        // moves of variables follow that order, so a parameter that one field takes a clone of
        // and a later assignment moves is cloned first.
        let assigned: Vec<usize> = leading.iter().map(|&(i, _)| i).collect();
        let mut order: Vec<usize> = (0..fields.len()).filter(|i| !assigned.contains(i)).collect();
        for &i in &assigned {
            order.retain(|&j| j != i);
            order.push(i);
        }
        parts.extend(
            order
                .into_iter()
                .map(|i| (i, &fields[i]))
                .map(|(i, f)| {
                    let field = field_name(decl, &f.name);
                    let value = match (&bound[i], values[i]) {
                        (Some(local), _) => local.clone(),
                        (None, Some(e)) => {
                            self.field_value(e, f.ty, self.field_nullable(&decl.name, &f.name))
                        }
                        (None, None) => self.placeholder(f.ty, self.field_nullable(&decl.name, &f.name)),
                    };
                    if value == field { field } else { format!("{field}: {value}") }
                })
                .collect::<Vec<String>>(),
        );
        self.guarded = outer;
        self.steady = outer_steady;
        self.field_locals.clear();
        let literal = if parts.is_empty() {
            format!("{} {{}}", self.struct_of(m.class))
        } else {
            format!("{} {{ {} }}", self.struct_of(m.class), parts.join(", "))
        };
        (literal, &body[leading.len()..])
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
        let owned = matches!(ty, Type::Class(c, _) if self.facts.owned.contains(c));
        assigns_any(rest, name)
            || (owned && self.changes_owned(rest, &|e| matches!(&e.kind, ExprKind::Var(v) if v == name)))
    }

    /// `local_is_mut` of a local declared without a value: one that a statement of its own block
    /// assigns once, and nothing else assigns, is first given its value there, as Rust's
    /// `let x;` allows without `mut`.
    pub(super) fn declared_local_is_mut(&self, ty: Type, name: &str, rest: &[Stmt]) -> bool {
        let count = |e: &Expr| {
            let mut n = 0;
            visit_expr(e, &mut |x| match &x.kind {
                ExprKind::Assign { target: Place::Var(v), .. }
                | ExprKind::IncDec { target: Place::Var(v), .. }
                    if v == name =>
                {
                    n += 1
                }
                // A switch expression's cases may loop.
                ExprKind::Switch { .. } => n += 2,
                _ => {}
            });
            n
        };
        let assigns_once = |s: &Stmt| match s {
            Stmt::Local { init: Some(e), .. } | Stmt::Expr(e) => count(e) == 1,
            _ => false,
        };
        // A switch whose cases do not fall into the next, each assigning it once at most, in
        // a statement of its own (a local that the cases share, `lower::switch`).
        let once_per_case = |s: &Stmt| match s {
            Stmt::Switch { cases, .. } => cases.iter().enumerate().all(|(i, c)| {
                let assigning: Vec<&Stmt> = c.body.iter().filter(|s| assigns_stmt(s, name)).collect();
                (i + 1 == cases.len() || !exceptions::completes_all(&c.body))
                    && assigning.len() <= 1
                    && assigning.iter().all(|s| assigns_once(s))
            }),
            _ => false,
        };
        // The one statement that assigns it does so once (a later switch that does not assign it
        // at all, as after javac's `modifiersOpt` loop, is not that statement).
        let assigning: Vec<&Stmt> = rest.iter().filter(|s| assigns_stmt(s, name)).collect();
        let once = matches!(assigning[..], [s] if assigns_once(s) || once_per_case(s));
        if !once {
            return self.local_is_mut(ty, name, rest);
        }
        let owned = matches!(ty, Type::Class(c, _) if self.facts.owned.contains(c));
        owned && self.changes_owned(rest, &|e| matches!(&e.kind, ExprKind::Var(v) if v == name))
    }
}
