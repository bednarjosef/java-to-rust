//! Fields that a loop only reads. In
//!
//! ```java
//! for (int j = i + 1; j < bodies.length; j++) {
//!     Body bj = bodies[j];
//!     double dx = bi.x - bj.x;
//!     ...
//!     bi.vx -= dx * bj.mass * mag;
//! }
//! ```
//!
//! `bi.x` is the same value at every turn: nothing in the loop assigns a field `x` of any object,
//! nor runs code that could. Java's compiler reads it once, before the loop, as it tells fields
//! apart by name. Rust's cannot: `bi.x` and `bj.vx` are two `f64`s in cells
//! (`java::Field`) of objects it knows nothing of, so the store to `bj.vx` might be the one to
//! `bi.x`, and it reads `bi.x` again at every turn. So the read is taken once before the loop,
//! into a local that the loop reads instead (`Gen::loop_fields`).
//!
//! A read is taken out of a loop when its object is a local variable that the loop does not
//! assign, nor declare, and that cannot be null there, or `this`; its field is a primitive of a
//! celled object that only one thread reaches (a `java::Field`, not a `SharedField`, whose
//! accesses each thread sees in order); no part of the loop (condition, updates or body) assigns
//! a field of that name of any object; and the loop runs no code but its own: no call, no `new`,
//! no other class's initialization, no string concatenation with an object, no library method
//! with an argument that is not a primitive. The object is not null, so the read cannot throw,
//! and taking it before the loop is no different from taking it at the first turn, even for a
//! loop that never turns.

use super::*;

/// Whether running `e` can run code other than its own (a call, a `new`, a class's
/// initialization, a `toString`) or reach an object through a library method.
fn runs_other_code(g: &Gen<'_>, e: &Expr) -> bool {
    let mut found = false;
    visit_expr(e, &mut |x| {
        found |= match &x.kind {
            ExprKind::Call { .. }
            | ExprKind::MethodCall { .. }
            | ExprKind::New { .. }
            | ExprKind::ThisCall { .. }
            | ExprKind::SuperInit { .. } => true,
            ExprKind::Builtin(_, args) => args.iter().any(|a| a.ty.is_reference()) || x.ty.is_reference(),
            ExprKind::Static { class, .. } | ExprKind::EnumConstant { class, .. } => {
                g.uninitialized(*class).is_some()
            }
            ExprKind::Binary(_, l, r) if x.ty == Type::String => {
                [l, r].iter().any(|o| o.ty.is_reference() && o.ty != Type::String)
            }
            _ => false,
        }
    });
    if let ExprKind::Assign { target: Place::Static { class, .. }, .. } = &e.kind {
        found |= g.uninitialized(*class).is_some();
    }
    found
}

/// The names of the fields that `e` assigns, of any object.
fn assigned_fields<'s>(e: &'s Expr, into: &mut HashSet<&'s str>) {
    visit_expr(e, &mut |x| match &x.kind {
        ExprKind::Assign { target: Place::Field { name, .. }, .. }
        | ExprKind::IncDec { target: Place::Field { name, .. }, .. } => {
            into.insert(name);
        }
        _ => {}
    });
}

impl<'a> Gen<'a> {
    /// Before a loop of condition `cond`, updates `update` and body `body`: binds each field read
    /// that the loop may take once (see the module comment) to a local, and reads the local in
    /// the loop instead (`loop_fields`). Gives back the reads, which the caller forgets after
    /// the loop.
    pub(super) fn bind_loop_fields(
        &mut self,
        cond: Option<&'a Expr>,
        update: &'a [Expr],
        body: &'a Stmt,
    ) -> Vec<ExprId> {
        let mut exprs: Vec<&'a Expr> = cond.into_iter().chain(update).collect();
        let mut declared: HashSet<&'a str> = HashSet::new();
        visit_stmt(body, &mut |e| exprs.push(e), &mut |s| match s {
            Stmt::Local { name, .. } | Stmt::ForEach { name, .. } => {
                declared.insert(name);
            }
            Stmt::Try { catches, .. } => declared.extend(catches.iter().map(|c| c.name.as_str())),
            _ => {}
        });
        if exprs.iter().any(|e| runs_other_code(self, e)) {
            return Vec::new();
        }
        let mut assigned = HashSet::new();
        for e in &exprs {
            assigned_fields(e, &mut assigned);
            visit_expr(e, &mut |x| {
                if let ExprKind::InstanceOf { binding: Some(b), .. } = &x.kind {
                    declared.insert(b);
                }
            });
        }
        let mut reads: Vec<&'a Expr> = Vec::new();
        for e in &exprs {
            visit_expr(e, &mut |x| {
                if let ExprKind::Field { name, .. } = &x.kind
                    && !assigned.contains(name.as_str())
                {
                    reads.push(x);
                }
            });
        }
        // The local of each (object, field) taken, by the object's Java name ("this" for `this`).
        let mut bound: Vec<(String, String, String)> = Vec::new();
        let mut taken_ids = Vec::new();
        for read in reads {
            let ExprKind::Field { object, name } = &read.kind else { continue };
            let object_name = match &object.kind {
                ExprKind::Var(v)
                    if self.locals.contains_key(v.as_str())
                        && !declared.contains(v.as_str())
                        && !exprs.iter().any(|e| assigns(e, v)) =>
                {
                    v.as_str()
                }
                ExprKind::This if !self.field_locals.contains_key(name.as_str()) => "this",
                _ => continue,
            };
            let fits = !read.ty.is_reference()
                && read.ty != Type::Void
                && !self.loop_fields.contains_key(&read.id)
                && !self.nullable(object)
                && self.celled_object(object)
                && !self.shared_object(object);
            if !fits {
                continue;
            }
            let local = match bound.iter().find(|(o, f, _)| o == object_name && f == name) {
                Some((_, _, local)) => local.clone(),
                None => {
                    let base = match object_name {
                        "this" => format!("this_{name}"),
                        v => format!("{}_{name}", self.local(v).trim_start_matches("r#")),
                    };
                    let taken = |n: &str| {
                        self.locals.values().any(|l| l == n)
                            || self.fn_name_set.contains(n)
                            || self.loop_fields.values().any(|l| l == n)
                            || bound.iter().any(|(_, _, l)| l == n)
                    };
                    let local = (0..)
                        .map(|k| if k == 0 { base.clone() } else { format!("{base}{k}") })
                        .find(|n| !taken(n))
                        .unwrap_or_default();
                    let value = self.field_read(object, name, read.ty).text;
                    self.line(&format!("let {local} = {value};"));
                    bound.push((object_name.to_string(), name.clone(), local.clone()));
                    local
                }
            };
            self.loop_fields.insert(read.id, local);
            taken_ids.push(read.id);
        }
        taken_ids
    }

    /// Forgets the reads that `bind_loop_fields` bound, after their loop.
    pub(super) fn unbind_loop_fields(&mut self, ids: Vec<ExprId>) {
        for id in ids {
            self.loop_fields.remove(&id);
        }
    }
}
