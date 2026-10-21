//! Which places of the program may hold a string or a box as an object: a value of `Object` (or
//! of another of the program's types that a string or a box converts to, such as `Comparable`)
//! that came from a string, a box or a primitive. A string or a box in an `Object` is a plain
//! value whose identity the translation does not keep, so `==` on two such values is rejected
//! (`patterns::check_object_values`); where this analysis shows that neither side can be one,
//! `==` is plain identity.
//!
//! The analysis follows values through variables, parameters (of every method an override group
//! may run), return values, fields, statics and array elements (by element type), until nothing
//! changes. It starts at the conversions of a string, a box or a primitive to a class type, and
//! anything it does not follow (a library call's result, a variable bound by a construct it does
//! not model) counts as one that may be a string or a box.

use super::Facts;
use crate::ast::*;
use std::collections::HashSet;

/// The places that may hold a string or a box.
#[derive(Default)]
pub struct ObjectValues {
    vars: HashSet<(usize, String)>,
    returns: HashSet<usize>,
    fields: HashSet<(String, String)>,
    statics: HashSet<(usize, String)>,
    elements: HashSet<Type>,
    /// The variables of each method that the analysis follows: its parameters and the variables
    /// it declares, loops over, binds with a pattern or catches.
    followed: Vec<HashSet<String>>,
}

impl ObjectValues {
    /// Whether `e`, evaluated in method `m`, may be a string or a box held as an object.
    pub fn may_be_value(&self, class: &Class, m: usize, e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Null | ExprKind::This | ExprKind::New { .. } | ExprKind::EnumConstant { .. } => false,
            ExprKind::NewArray(_) | ExprKind::ArrayLit(_) => false,
            ExprKind::Cast(to, inner) => {
                if matches!(to, Type::Class(..)) && is_value_type(inner.ty) {
                    return true;
                }
                if !matches!(inner.ty, Type::Class(..)) {
                    return false;
                }
                self.may_be_value(class, m, inner)
            }
            _ if !matches!(e.ty, Type::Class(..)) => false,
            ExprKind::Var(name) => {
                !self.followed.get(m).is_some_and(|f| f.contains(name)) || self.vars.contains(&(m, name.clone()))
            }
            ExprKind::Assign { op: None, value, .. } => self.may_be_value(class, m, value),
            ExprKind::Conditional(_, a, b) => self.may_be_value(class, m, a) || self.may_be_value(class, m, b),
            ExprKind::Field { object, name } => match class.field_owner_name(object.ty, name) {
                Some(owner) => self.fields.contains(&(owner.to_string(), name.clone())),
                None => true,
            },
            ExprKind::Static { class: c, name } => self.statics.contains(&(*c, name.clone())),
            ExprKind::Index(..) => self.elements.contains(&e.ty),
            ExprKind::Call { resolved: Some(r), .. } | ExprKind::MethodCall { resolved: Some(r), .. } => {
                self.returns.contains(r)
            }
            _ => true,
        }
    }
}

/// Whether values of type `ty` are strings, boxes or primitives, which a conversion to a class
/// type makes a plain value in an `Object`.
fn is_value_type(ty: Type) -> bool {
    matches!(ty, Type::String | Type::Boxed(_)) || ty.is_numeric() || matches!(ty, Type::Boolean | Type::Char)
}

/// The places of the program that may hold a string or a box as an object.
pub fn analyze(class: &Class, _facts: &Facts) -> ObjectValues {
    let mut values = ObjectValues { followed: class.methods.iter().map(followed).collect(), ..Default::default() };
    loop {
        let before = (
            values.vars.len(),
            values.returns.len(),
            values.fields.len(),
            values.statics.len(),
            values.elements.len(),
        );
        let mut vars = Vec::new();
        let mut returns = Vec::new();
        let mut fields = Vec::new();
        let mut statics = Vec::new();
        let mut elements = Vec::new();
        let mut params = Vec::new();
        let mut declared = Vec::new();
        let v = &values;
        for (m, method) in class.methods.iter().enumerate() {
            let value = |e: &Expr| v.may_be_value(class, m, e);
            let mut on_expr = |x: &Expr| match &x.kind {
                ExprKind::Assign { op: None, target, value: rhs } if value(rhs) => match target {
                    Place::Var(name) => vars.push((m, name.clone())),
                    Place::Field { object, name } => match class.field_owner_name(object.ty, name) {
                        Some(owner) => fields.push((owner.to_string(), name.clone())),
                        // A field the analysis cannot place: every field of that name.
                        None => fields.extend(
                            class.types.iter().filter(|d| d.fields.iter().any(|f| &f.name == name)).map(|d| (d.name.clone(), name.clone())),
                        ),
                    },
                    Place::Static { class: c, name } => statics.push((*c, name.clone())),
                    Place::Index(array, _) => elements.push(array.ty.element()),
                },
                ExprKind::ArrayLit(items) if items.iter().any(|i| value(i)) => elements.push(x.ty.element()),
                ExprKind::InstanceOf { value: tested, binding: Some(name), .. } if value(tested) => {
                    vars.push((m, name.clone()))
                }
                ExprKind::Call { args, resolved: Some(r), .. }
                | ExprKind::New { args, ctor: Some(r), .. }
                | ExprKind::ThisCall { args, resolved: Some(r) }
                | ExprKind::SuperInit { args, resolved: Some(r) } => {
                    for (i, a) in args.iter().enumerate() {
                        if value(a) {
                            params.push((vec![*r], i));
                        }
                    }
                }
                ExprKind::MethodCall { args, resolved: Some(r), .. } => {
                    for (i, a) in args.iter().enumerate() {
                        if value(a) {
                            params.push((class.hierarchy.group(*r), i));
                        }
                    }
                }
                // A library function may store its arguments into the arrays it is given.
                ExprKind::Builtin(_, args) if args.iter().any(|a| value(a)) => {
                    elements.extend(args.iter().filter_map(|a| match a.ty {
                        Type::Array(element) => Some(*element),
                        _ => None,
                    }));
                }
                _ => {}
            };
            let mut on_stmt = |s: &Stmt| match s {
                Stmt::Local { name, init: Some(init), .. } if value(init) => declared.push((m, name.clone())),
                Stmt::ForEach { name, array, .. } => {
                    if let Type::Array(element) = array.ty
                        && v.elements.contains(element)
                    {
                        declared.push((m, name.clone()));
                    }
                }
                Stmt::Return(Some(e), _) if value(e) => returns.push(m),
                _ => {}
            };
            for s in &method.body {
                visit_stmt(s, &mut |e| visit_expr(e, &mut on_expr), &mut on_stmt);
            }
        }
        for (t, decl) in class.types.iter().enumerate() {
            for f in &decl.fields {
                if f.init.as_ref().is_some_and(|e| v.may_be_value(class, usize::MAX, e)) {
                    fields.push((decl.name.clone(), f.name.clone()));
                }
            }
            for f in &decl.statics {
                if f.init.as_ref().is_some_and(|e| v.may_be_value(class, usize::MAX, e)) {
                    statics.push((t, f.name.clone()));
                }
            }
        }
        for (methods, i) in params {
            for m in methods {
                if let Some(p) = class.methods[m].params.get(i) {
                    vars.push((m, p.name.clone()));
                }
            }
        }
        let returns: Vec<usize> = returns.into_iter().flat_map(|m| class.hierarchy.group(m)).collect();
        values.vars.extend(vars);
        values.vars.extend(declared);
        values.returns.extend(returns);
        values.fields.extend(fields);
        values.statics.extend(statics);
        values.elements.extend(elements);
        let after = (
            values.vars.len(),
            values.returns.len(),
            values.fields.len(),
            values.statics.len(),
            values.elements.len(),
        );
        if before == after {
            return values;
        }
    }
}

/// The variables of `method` that the analysis follows.
fn followed(method: &Method) -> HashSet<String> {
    let mut names: HashSet<String> = method.params.iter().map(|p| p.name.clone()).collect();
    let mut bindings = Vec::new();
    for s in &method.body {
        visit_stmt(
            s,
            &mut |e| {
                visit_expr(e, &mut |x| {
                    if let ExprKind::InstanceOf { binding: Some(name), .. } = &x.kind {
                        bindings.push(name.clone());
                    }
                })
            },
            &mut |s| match s {
                Stmt::Local { name, .. } | Stmt::ForEach { name, .. } => _ = names.insert(name.clone()),
                Stmt::Try { catches, .. } => names.extend(catches.iter().map(|c| c.name.clone())),
                _ => {}
            },
        );
    }
    names.extend(bindings);
    names
}
