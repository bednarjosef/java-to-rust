//! `null` for enums. A local of an enum type that is assigned `null` is an `Option` of the enum, as
//! a nullable string is an `Option<String>` (see `analysis::nullness`). So is a field, an array
//! element or a method's return value of an enum type that may hold `null` (`Places`): a field or
//! element that is assigned `null`, the elements of `new E[n]`, which start as `null`, and a
//! method that returns `null`, with the methods it overrides and that override it.
//!
//! Such a value is only set, tested against `null`, compared with `==` or `!=`, printed or
//! concatenated; one read from a field, an element or a call may also be stored in a local or in
//! another such place, returned from such a method, or dropped, as a call's result is. Those uses have a
//! `None` case in the generated code. Any other use would give an enum value a `None`, so it is
//! rejected, and so is `null` given to an enum anywhere else (a parameter, a conditional).

use super::*;
use std::cell::{Cell, RefCell};
use std::collections::HashSet;

/// The fields (by declaring class and name), array element types and methods of an enum type
/// that may hold or return `null`.
#[derive(Default)]
pub(super) struct Places {
    fields: HashSet<(String, String)>,
    elements: HashSet<Type>,
    methods: HashSet<usize>,
}

impl Places {
    /// Whether `place` is one of the places, which a nullable local may then be stored in.
    fn holds(&self, class: &Class, place: &Place) -> bool {
        match place {
            Place::Field { object, name } => class
                .field_owner_name(object.ty, name)
                .is_some_and(|owner| self.fields.contains(&(owner.to_string(), name.clone()))),
            Place::Static { class: c, name } => {
                self.fields.contains(&(class.types[*c].name.clone(), name.clone()))
            }
            Place::Index(array, _) => self.elements.contains(&array.ty.element()),
            Place::Var(_) => false,
        }
    }

    /// Whether `e` reads one of the places: a field, a static, an element or a call.
    fn read(&self, class: &Class, e: &Expr) -> bool {
        if !matches!(e.ty, Type::Enum(_)) {
            return false;
        }
        match &e.kind {
            ExprKind::Field { object, name } => class
                .field_owner_name(object.ty, name)
                .is_some_and(|owner| self.fields.contains(&(owner.to_string(), name.clone()))),
            ExprKind::Static { class: c, name } => {
                self.fields.contains(&(class.types[*c].name.clone(), name.clone()))
            }
            ExprKind::Index(..) => self.elements.contains(&e.ty),
            ExprKind::Call { resolved: Some(r), .. } | ExprKind::MethodCall { resolved: Some(r), .. } => {
                self.methods.contains(r)
            }
            _ => false,
        }
    }

    /// Whether `value`, given to an enum, may be `null`: the literal or a read of a place.
    fn null(&self, class: &Class, value: &Expr) -> bool {
        matches!(value.kind, ExprKind::Null) || self.read(class, value)
    }
}

/// The places of the program that may hold `null`, until nothing changes, from the fields that
/// start as `null` (by declaring class and name).
pub(super) fn places(class: &Class, starting_null: Vec<(String, String)>) -> Places {
    let mut places = Places { fields: starting_null.into_iter().collect(), ..Places::default() };
    loop {
        let before = (places.fields.len(), places.elements.len(), places.methods.len());
        let mut fields = Vec::new();
        let mut elements = Vec::new();
        let mut methods = Vec::new();
        let mut on_expr = |x: &Expr| match &x.kind {
            ExprKind::Assign { op: None, target, value }
                if matches!(x.ty, Type::Enum(_)) && places.null(class, value) =>
            {
                match target {
                    Place::Field { object, name } => {
                        if let Some(owner) = class.field_owner_name(object.ty, name) {
                            fields.push((owner.to_string(), name.clone()));
                        }
                    }
                    Place::Static { class: c, name } => {
                        fields.push((class.types[*c].name.clone(), name.clone()))
                    }
                    Place::Index(array, _) => elements.push(array.ty.element()),
                    Place::Var(_) => {}
                }
            }
            // `new E[n]` and `new E[n][m]` leave their enums null.
            ExprKind::NewArray(sizes) => {
                let element = x.ty.element_at(sizes.len());
                if matches!(element, Type::Enum(_)) {
                    elements.push(element);
                }
            }
            ExprKind::ArrayLit(items) if items.iter().any(|item| places.null(class, item)) => {
                elements.push(x.ty.element());
            }
            _ => {}
        };
        for (m, method) in class.methods.iter().enumerate() {
            for stmt in &method.body {
                visit_stmt(stmt, &mut |e| visit_expr(e, &mut on_expr), &mut |s| {
                    if let Stmt::Return(Some(value), _) = s
                        && matches!(method.ret, Type::Enum(_))
                        && places.null(class, value)
                    {
                        methods.extend(class.hierarchy.group(m));
                    }
                });
            }
        }
        for decl in &class.types {
            for init in decl.fields.iter().chain(&decl.statics).filter_map(|f| f.init.as_ref()) {
                visit_expr(init, &mut on_expr);
            }
        }
        places.fields.extend(fields);
        places.elements.extend(elements);
        places.methods.extend(methods);
        if before == (places.fields.len(), places.elements.len(), places.methods.len()) {
            return places;
        }
    }
}

/// The first position in method `m` where `null` is given to an enum outside the uses above, or a
/// nullable enum local, or a read of one of `places`, is used outside them.
pub(super) fn escape(class: &Class, m: usize, places: &Places) -> Option<Pos> {
    let method = &class.methods[m];
    // The enum locals that are assigned `null` or a read of a place, with the positions of those
    // assignments.
    let assigned = RefCell::new(Vec::new());
    walk(
        method,
        &mut |x| {
            if let ExprKind::Assign { op: None, target: Place::Var(name), value } = &x.kind
                && places.null(class, value)
                && matches!(x.ty, Type::Enum(_))
            {
                assigned.borrow_mut().push((name.clone(), x.pos));
            }
        },
        &mut |s| {
            if let Stmt::Local { ty: Type::Enum(_), name, init: Some(init), .. } = s
                && places.null(class, init)
            {
                assigned.borrow_mut().push((name.clone(), init.pos));
            }
        },
    );
    let assigned = assigned.into_inner();
    // A parameter is passed by its caller as a value, so it cannot be a `None`.
    if let Some(&(_, pos)) = assigned.iter().find(|(n, _)| method.params.iter().any(|p| &p.name == n)) {
        return Some(pos);
    }
    let names: HashSet<&str> = assigned.iter().map(|(n, _)| n.as_str()).collect();
    let is_local = |e: &Expr| matches!(&e.kind, ExprKind::Var(n) if names.contains(n.as_str()) && matches!(e.ty, Type::Enum(_)));
    // A value that may be null: such a local, or a read of a place.
    let nullable = |e: &Expr| is_local(e) || places.read(class, e);
    let is_null = |e: &Expr| matches!(e.kind, ExprKind::Null);
    let is_enum = |e: &Expr| matches!(e.ty, Type::Enum(_)) || is_null(e);
    // The uses that are allowed: the `null` of an assignment or declaration, the operands of a
    // null test, of `==` and `!=` on enums, of a string concatenation and of `print`, and the
    // value of an assignment to a place, of a return from a method of `places` and of a dropped
    // call.
    let allowed = RefCell::new(HashSet::new());
    let allow = |e: &Expr| {
        allowed.borrow_mut().insert(e as *const Expr);
    };
    walk(
        method,
        &mut |x| match &x.kind {
            ExprKind::Assign { op: None, target: Place::Var(_), value } if places.null(class, value) => {
                allow(value)
            }
            ExprKind::Assign {
                op: None,
                target: target @ (Place::Field { .. } | Place::Static { .. } | Place::Index(..)),
                value,
            } if matches!(x.ty, Type::Enum(_))
                && (places.null(class, value) || (is_local(value) && places.holds(class, target))) =>
            {
                allow(value)
            }
            ExprKind::Binary(BinOp::Eq | BinOp::Ne, a, b) if is_enum(a) && is_enum(b) => {
                for operand in [a, b] {
                    if nullable(operand) || is_null(operand) {
                        allow(operand);
                    }
                }
            }
            ExprKind::Binary(BinOp::Add, a, b) if x.ty == Type::String => {
                for operand in [a, b] {
                    if nullable(operand) {
                        allow(operand);
                    }
                }
            }
            ExprKind::Builtin(Builtin::Print | Builtin::Println, args) => {
                for arg in args.iter().filter(|a| nullable(a)) {
                    allow(arg);
                }
            }
            ExprKind::ArrayLit(items) if places.elements.contains(&x.ty.element()) => {
                for item in items.iter().filter(|item| places.null(class, item)) {
                    allow(item);
                }
            }
            _ => {}
        },
        &mut |s| match s {
            Stmt::Local { ty: Type::Enum(_), init: Some(init), .. } if places.null(class, init) => {
                allow(init)
            }
            Stmt::Return(Some(value), _) if places.methods.contains(&m) && places.null(class, value) => {
                allow(value)
            }
            Stmt::Expr(e) if matches!(e.kind, ExprKind::Call { .. } | ExprKind::MethodCall { .. }) => {
                allow(e)
            }
            _ => {}
        },
    );
    let allowed = allowed.into_inner();
    let found: Cell<Option<Pos>> = Cell::new(None);
    let not_allowed = |e: &Expr| !allowed.contains(&(e as *const Expr));
    walk(
        method,
        &mut |x| {
            let escapes = (nullable(x) && not_allowed(x)) || null_enum_in(x, class, &allowed);
            if escapes && found.get().is_none() {
                found.set(Some(x.pos));
            }
        },
        &mut |s| {
            if let Stmt::Return(Some(e), _) = s
                && null_for(e, method.ret, &allowed)
                && found.get().is_none()
            {
                found.set(Some(e.pos));
            }
        },
    );
    found.get()
}

/// Calls `on_expr` on every expression and `on_stmt` on every statement of `method`.
fn walk(method: &Method, on_expr: &mut dyn FnMut(&Expr), on_stmt: &mut dyn FnMut(&Stmt)) {
    for stmt in &method.body {
        visit_stmt(stmt, &mut |e| visit_expr(e, on_expr), on_stmt);
    }
}

/// Whether `value` is the `null` literal given to an enum, and not one of the allowed uses.
fn null_for(value: &Expr, ty: Type, allowed: &HashSet<*const Expr>) -> bool {
    matches!(value.kind, ExprKind::Null)
        && matches!(ty, Type::Enum(_))
        && !allowed.contains(&(value as *const Expr))
}

/// Whether an expression gives `null` to an enum: in an assignment, a conditional, a cast, an
/// array, or an argument of a call.
fn null_enum_in(e: &Expr, class: &Class, allowed: &HashSet<*const Expr>) -> bool {
    match &e.kind {
        ExprKind::Binary(BinOp::Eq | BinOp::Ne, a, b) => {
            null_for(a, b.ty, allowed) || null_for(b, a.ty, allowed)
        }
        ExprKind::Assign { value, .. } => null_for(value, e.ty, allowed),
        ExprKind::Conditional(_, a, b) => {
            matches!(e.ty, Type::Enum(_))
                && (matches!(a.kind, ExprKind::Null) || matches!(b.kind, ExprKind::Null))
        }
        ExprKind::Cast(ty, x) => null_for(x, *ty, allowed),
        ExprKind::ArrayLit(elements) => {
            matches!(e.ty, Type::Array(el) if matches!(el, Type::Enum(_)))
                && elements.iter().any(|x| null_for(x, e.ty.element(), allowed))
        }
        ExprKind::Call { args, resolved: Some(r), .. }
        | ExprKind::MethodCall { args, resolved: Some(r), .. }
        | ExprKind::New { args, ctor: Some(r), .. }
        | ExprKind::ThisCall { args, resolved: Some(r) } => {
            args.iter().zip(&class.methods[*r].params).any(|(a, p)| null_for(a, p.ty, allowed))
        }
        _ => false,
    }
}
