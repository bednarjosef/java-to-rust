//! Owned objects: a class whose objects are never shared. An object of such a class is a plain
//! value held by the one local that created it, and its methods take `&self` or `&mut self`.
//! Nothing else may hold one: it is never a field, parameter or return value, and `this` only
//! receives a method call or a field access, so no alias can outlive the call that uses it.

use crate::ast::*;
use std::collections::{HashMap, HashSet};

/// Finds the owned classes among those that are not immutable (immutable objects are shared), by
/// qualified name. A class whose objects are never `created` is not owned: it has no objects to
/// hold as values, and its class tree keeps the representation of the others (see
/// `objects::celled_classes`).
pub fn owned_classes(
    class: &Class,
    immutable: &HashSet<String>,
    field_level: &HashSet<usize>,
    created: &[bool],
) -> HashSet<String> {
    let uses = Uses::scan(class);
    (0..class.types.len())
        .filter(|&t| {
            created[t]
                && !immutable.contains(&class.types[t].name)
                && !class.hierarchy.has_subtypes(t)
                && !runs_inherited_object_code(class, &uses, t, field_level)
                && is_owned(class, &uses, t)
        })
        .map(|t| class.types[t].name.clone())
        .collect()
}

/// Whether the objects of class `t` are never shared.
fn is_owned(class: &Class, uses: &Uses, t: usize) -> bool {
    let name = class.types[t].name.as_str();
    // A field, parameter or return value of the class, or the class as a type argument: a
    // generic class holds its type arguments' values as `T: Clone`, which a plain value is not.
    if uses.declared.contains(name) || uses.in_args.contains(name) || uses.delegating.contains(&t) {
        return false;
    }
    let Some(of) = uses.by_name.get(name) else { return true };
    let news = |id: &ExprId| of.news.contains(id) || uses.receivers.contains(id);
    !of.bad && of.locals.is_disjoint(&uses.assigned) && of.typed.iter().all(news)
}

/// Whether an object of class `t` runs a method of a superclass that needs the whole object
/// (one that does not live on the superclass's fields struct, see `hierarchy::field_level`): such
/// a method takes the object as a value of the superclass's type, which a plain value cannot be.
fn runs_inherited_object_code(class: &Class, uses: &Uses, t: usize, field_level: &HashSet<usize>) -> bool {
    let h = &class.hierarchy;
    let inherited =
        |m: usize| class.methods[m].class != t && !class.methods[m].is_static && !field_level.contains(&m);
    let name = class.types[t].name.as_str();
    // Turning the object into text runs its `toString`.
    if uses.shown.contains(name) && super::hierarchy::to_string_impl(class, t).is_some_and(inherited) {
        return true;
    }
    let calls = uses.by_name.get(name).map_or(&[][..], |of| &of.calls[..]);
    calls.iter().any(|&r| h.implementation(r, t).is_some_and(inherited))
        || uses.super_calls[t].iter().any(|&r| inherited(r))
}

/// The classes that are type arguments in `ty`, or in one of its type arguments, by name.
fn type_args(ty: Type, out: &mut HashSet<&'static str>) {
    match ty {
        Type::Class(_, args) => {
            for &a in args {
                if let Type::Class(c, _) = a {
                    out.insert(c);
                }
                type_args(a, out);
            }
        }
        Type::Array(elem) => type_args(*elem, out),
        _ => {}
    }
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

/// What the uses of the objects of the classes look like, scanned once for every class
/// (scanning the program again for each class takes the number of classes times the size of the
/// program).
#[derive(Default)]
struct Uses {
    /// The classes that are the type of a field, a parameter or a return value, by name.
    declared: HashSet<&'static str>,
    /// The classes that are a type argument of such a type, of a local or of an expression.
    in_args: HashSet<&'static str>,
    /// The classes with a constructor that calls `this(...)`, which is never allowed.
    delegating: HashSet<usize>,
    /// Receivers of calls and field accesses that are a variable or `this`.
    receivers: HashSet<ExprId>,
    /// The names that an assignment writes.
    assigned: HashSet<String>,
    /// The classes whose objects are printed or concatenated, by name.
    shown: HashSet<&'static str>,
    /// For each class, the methods called with `super.` in its code.
    super_calls: Vec<Vec<usize>>,
    /// The uses of each class's objects, by its name.
    by_name: HashMap<&'static str, Of>,
}

/// The uses of the objects of one class.
#[derive(Default)]
struct Of {
    /// `new C(...)` expressions that initialize a local of type C.
    news: HashSet<ExprId>,
    /// Every expression of type C.
    typed: Vec<ExprId>,
    /// The names of the locals of type C.
    locals: HashSet<String>,
    /// A local of type C that is not initialized by `new`.
    bad: bool,
    /// The methods called (other than with `super.`) on a receiver of type C.
    calls: Vec<usize>,
}

impl Uses {
    fn scan(class: &Class) -> Uses {
        let mut uses = Uses { super_calls: vec![Vec::new(); class.types.len()], ..Uses::default() };
        let note_declared = |ty: Type, uses: &mut Uses| {
            if let Type::Class(c, _) = ty {
                uses.declared.insert(c);
            }
            type_args(ty, &mut uses.in_args);
        };
        for d in &class.types {
            for f in d.fields.iter().chain(&d.statics) {
                note_declared(f.ty, &mut uses);
            }
        }
        for m in &class.methods {
            note_declared(m.ret, &mut uses);
            for p in &m.params {
                note_declared(p.ty, &mut uses);
            }
        }
        for m in &class.methods {
            if m.is_ctor && has_this_call(&m.body) {
                uses.delegating.insert(m.class);
            }
            for s in &m.body {
                visit_stmt(s, &mut |e| visit_expr(e, &mut |x| uses.expr(x, Some(m.class))), &mut |_| {});
                visit_stmt(s, &mut |_| {}, &mut |s| uses.decl(class, s));
            }
        }
        for d in &class.types {
            for f in d.fields.iter().chain(&d.statics) {
                if let Some(init) = &f.init {
                    visit_expr(init, &mut |x| uses.expr(x, None));
                }
            }
        }
        uses
    }

    /// A local of type C: it must be created by `new` of C, and no assignment may write its name.
    fn decl(&mut self, class: &Class, stmt: &Stmt) {
        let Stmt::Local { ty, name: local, init, .. } = stmt else { return };
        type_args(*ty, &mut self.in_args);
        let Type::Class(name, _) = *ty else { return };
        let of = self.by_name.entry(name).or_default();
        of.locals.insert(local.clone());
        match init {
            Some(Expr { kind: ExprKind::New { class: c, .. }, id, .. }) if class.types[*c].name == name => {
                of.news.insert(*id);
            }
            _ => of.bad = true,
        }
    }

    /// An expression of the code of class `at` (`None` in a field initializer, whose types do
    /// not count as type arguments).
    fn expr(&mut self, x: &Expr, at: Option<usize>) {
        if at.is_some() {
            type_args(x.ty, &mut self.in_args);
        }
        if let Type::Class(name, _) = x.ty {
            self.by_name.entry(name).or_default().typed.push(x.id);
        }
        match &x.kind {
            ExprKind::MethodCall { receiver, resolved, is_super, .. } => {
                self.receiver(receiver);
                match (resolved, is_super) {
                    (Some(r), true) => at.into_iter().for_each(|t| self.super_calls[t].push(*r)),
                    (Some(r), false) => {
                        if let Type::Class(name, _) = receiver.ty {
                            self.by_name.entry(name).or_default().calls.push(*r);
                        }
                    }
                    (None, _) => {}
                }
            }
            ExprKind::Field { object, .. } => self.receiver(object),
            // Printing and concatenation borrow an object to read its `toString`.
            ExprKind::Builtin(Builtin::Print | Builtin::Println, args) => {
                for a in args {
                    self.receiver(a);
                    self.show(a);
                }
            }
            ExprKind::Binary(BinOp::Add, l, r) if x.ty == Type::String => {
                for operand in [l, r] {
                    self.receiver(operand);
                    self.show(operand);
                }
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

    fn show(&mut self, e: &Expr) {
        if let Type::Class(name, _) = e.ty {
            self.shown.insert(name);
        }
    }
}
