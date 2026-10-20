//! Owned objects: a value of an owned class is built by its constructor as a local named `this`,
//! its methods take `&self` or `&mut self`, and a call on it uses method syntax.

use super::objects::*;
use super::*;

impl<'a> Gen<'a> {
    /// Whether method `index` is an instance method of an owned class.
    pub(super) fn is_owned_method(&self, index: usize) -> bool {
        let m = &self.class.methods[index];
        !m.is_static && self.facts.owned.contains(&self.class.types[m.class].name)
    }

    /// The body of a constructor of an owned class: the object is built as a plain value named
    /// `this`, its field initializers run, the rest of the body runs, and the value is returned.
    pub(super) fn owned_constructor(&mut self, m: &'a Method) {
        let class = self.class;
        let decl = &class.types[m.class];
        let initialize = decl.fields.iter().any(|f| f.init.is_some());
        let mutable = initialize || self.changes_owned(&m.body, &|e| matches!(e.kind, ExprKind::This));
        let fields: Vec<String> =
            decl.fields.iter().map(|f| format!("{}: {}", snake_case(&f.name), default_value(f.ty))).collect();
        let mut_kw = if mutable { "mut " } else { "" };
        self.line(&format!("let {mut_kw}this = {} {{ {} }};", struct_name(&decl.name), fields.join(", ")));
        if initialize {
            self.line(&format!("{}::field_inits(&mut this);", struct_name(&decl.name)));
        }
        self.stmts(&m.body, false);
        self.line("this");
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
