//! Owned objects: a class whose objects are never shared. An object of such a class is a plain
//! value held by the one local that created it, and its methods take `&self` or `&mut self`.
//! Nothing else may hold one: it is never a field, parameter or return value, and `this` only
//! receives a method call or a field access, so no alias can outlive the call that uses it.

use crate::ast::*;
use std::collections::HashSet;

/// The classes whose objects are owned, and the methods of those classes that change their object.
#[derive(Default)]
pub struct Owned {
    /// The qualified names of the owned classes.
    pub classes: HashSet<String>,
    /// Indices into `Class::methods` of the instance methods that write `this`; they take `&mut self`.
    pub mutating: HashSet<usize>,
}

/// Finds the owned classes among those that are not immutable (immutable objects are shared).
pub fn owned_classes(class: &Class, immutable: &HashSet<String>) -> Owned {
    let classes: HashSet<String> = (0..class.types.len())
        .filter(|&t| !immutable.contains(&class.types[t].name) && is_owned(class, t))
        .map(|t| class.types[t].name.clone())
        .collect();
    let mut mutating = HashSet::new();
    // A method that calls a mutating method on `this` changes its object too: repeat until stable.
    loop {
        let added: Vec<usize> = (0..class.methods.len())
            .filter(|i| {
                let m = &class.methods[*i];
                !mutating.contains(i)
                    && !m.is_static
                    && !m.is_ctor
                    && classes.contains(&class.types[m.class].name)
                    && writes_this(&m.body, &mutating)
            })
            .collect();
        if added.is_empty() {
            break;
        }
        mutating.extend(added);
    }
    Owned { classes, mutating }
}

/// Whether the objects of class `t` are never shared.
fn is_owned(class: &Class, t: usize) -> bool {
    let name = class.types[t].name.as_str();
    if class.types.iter().any(|d| d.fields.iter().chain(&d.statics).any(|f| is_class(f.ty, name))) {
        return false;
    }
    if class.methods.iter().any(|m| is_class(m.ret, name) || m.params.iter().any(|p| is_class(p.ty, name))) {
        return false;
    }
    let mut uses = Uses::default();
    for m in &class.methods {
        if m.is_ctor && m.class == t && has_this_call(&m.body) {
            return false;
        }
        for s in &m.body {
            uses.scan(s, class, t);
        }
    }
    for d in &class.types {
        for f in d.fields.iter().chain(&d.statics) {
            if let Some(init) = &f.init {
                visit_expr(init, &mut |x| uses.expr(x, name));
            }
        }
    }
    !uses.bad
        && uses.locals.is_disjoint(&uses.assigned)
        && uses.typed.iter().all(|id| uses.news.contains(id) || uses.receivers.contains(id))
}

fn is_class(ty: Type, name: &str) -> bool {
    matches!(ty, Type::Class(c) if c == name)
}

fn has_this_call(body: &[Stmt]) -> bool {
    let mut found = false;
    for s in body {
        visit_stmt(
            s,
            &mut |e| visit_expr(e, &mut |x| found |= matches!(x.kind, ExprKind::ThisCall { .. })),
            &mut |_| {},
        );
    }
    found
}

/// Whether a method body writes a field of `this`, or calls a mutating method on `this`.
fn writes_this(body: &[Stmt], mutating: &HashSet<usize>) -> bool {
    let mut found = false;
    for s in body {
        visit_stmt(s, &mut |e| visit_expr(e, &mut |x| found |= writes_this_here(x, mutating)), &mut |_| {});
    }
    found
}

fn writes_this_here(x: &Expr, mutating: &HashSet<usize>) -> bool {
    match &x.kind {
        ExprKind::Assign { target: Place::Field { object, .. }, .. }
        | ExprKind::IncDec { target: Place::Field { object, .. }, .. } => {
            matches!(object.kind, ExprKind::This)
        }
        ExprKind::MethodCall { receiver, resolved: Some(j), .. } => {
            matches!(receiver.kind, ExprKind::This) && mutating.contains(j)
        }
        _ => false,
    }
}

/// What the uses of the objects of one class look like.
#[derive(Default)]
struct Uses {
    /// `new C(...)` expressions that initialize a local of type C.
    news: HashSet<ExprId>,
    /// Receivers of calls and field accesses that are a variable or `this`.
    receivers: HashSet<ExprId>,
    /// Every expression of type C.
    typed: Vec<ExprId>,
    /// The names of the locals of type C, and of every name that an assignment writes.
    locals: HashSet<String>,
    assigned: HashSet<String>,
    /// A local of type C that is not initialized by `new`, or a `this(...)` that is never allowed.
    bad: bool,
}

impl Uses {
    fn scan(&mut self, stmt: &Stmt, class: &Class, t: usize) {
        let name = class.types[t].name.as_str();
        visit_stmt(stmt, &mut |e| visit_expr(e, &mut |x| self.expr(x, name)), &mut |_| {});
        visit_stmt(stmt, &mut |_| {}, &mut |s| self.decl(s, name, t));
    }

    /// A local of type C: it must be created by `new` of C, and no assignment may write its name.
    fn decl(&mut self, stmt: &Stmt, name: &str, t: usize) {
        let Stmt::Local { ty, name: local, init, .. } = stmt else { return };
        if !is_class(*ty, name) {
            return;
        }
        self.locals.insert(local.clone());
        match init {
            Some(Expr { kind: ExprKind::New { class, .. }, id, .. }) if *class == t => {
                self.news.insert(*id);
            }
            _ => self.bad = true,
        }
    }

    fn expr(&mut self, x: &Expr, name: &str) {
        if is_class(x.ty, name) {
            self.typed.push(x.id);
        }
        match &x.kind {
            ExprKind::MethodCall { receiver: object, .. } | ExprKind::Field { object, .. } => {
                self.receiver(object)
            }
            // Printing and concatenation borrow an object to read its `toString`.
            ExprKind::Builtin(Builtin::Print | Builtin::Println, args) => {
                args.iter().for_each(|a| self.receiver(a))
            }
            ExprKind::Binary(BinOp::Add, l, r) if x.ty == Type::String => {
                self.receiver(l);
                self.receiver(r);
            }
            ExprKind::Assign { target, .. } | ExprKind::IncDec { target, .. } => match target {
                Place::Var(n) => {
                    self.assigned.insert(n.clone());
                }
                Place::Field { object, .. } => self.receiver(object),
                Place::Static { .. } | Place::Index(..) => {}
            },
            _ => {}
        }
    }

    fn receiver(&mut self, e: &Expr) {
        if matches!(e.kind, ExprKind::Var(_) | ExprKind::This) {
            self.receivers.insert(e.id);
        }
    }
}
