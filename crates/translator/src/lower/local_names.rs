//! Distinct names for a method's locals of different types. Java lets two locals whose scopes do
//! not overlap share a name (`for (Box s : boxes)` and later `for (String s : names)`), but the
//! analyses record what they find about a local by its method and name (`Facts::nullable_vars`),
//! so a later local of another reference type (whose nullness may differ) is renamed, with every
//! use of it: `s2`. Locals of primitive types, and catch parameters, which are never null, keep
//! their names.
//!
//! Java forbids a local to shadow another one in scope, so a use refers to the declaration of its
//! name that comes last before it in the order of the source, which is the order of this walk.

use super::enum_tables::rewrite;
use crate::ast::*;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

/// Renames the locals of `method` that share a name with a parameter or an earlier local of
/// another type, and returns each new name with the Java name it stands for.
pub(super) fn distinct_locals(method: &mut Method) -> Vec<(String, String)> {
    // Every declaration, by name, with its types in order of appearance.
    let mut found: HashMap<String, Vec<Type>> = HashMap::new();
    for p in &method.params {
        found.entry(p.name.clone()).or_default().push(p.ty);
    }
    // The declarations in the order they appear, and every name the method uses.
    let declarations = RefCell::new(Vec::new());
    let mut names: HashSet<String> = found.keys().cloned().collect();
    for stmt in &method.body {
        visit_stmt(
            stmt,
            &mut |e| match &e.kind {
                ExprKind::InstanceOf { binding: Some(b), ty, .. } => {
                    declarations.borrow_mut().push((b.clone(), *ty))
                }
                ExprKind::Var(v) => {
                    names.insert(v.clone());
                }
                _ => {}
            },
            &mut |s| match s {
                Stmt::Local { ty, name, .. } | Stmt::ForEach { ty, name, .. } => {
                    declarations.borrow_mut().push((name.clone(), *ty))
                }
                Stmt::Try { catches, .. } => {
                    declarations.borrow_mut().extend(catches.iter().map(|c| (c.name.clone(), CAUGHT)));
                }
                _ => {}
            },
        );
    }
    for (name, ty) in declarations.into_inner() {
        found.entry(name).or_default().push(ty);
    }
    names.extend(found.keys().cloned());
    // The names declared with more than one type: each type after the first gets a new name.
    let mut renames: HashMap<(String, Type), String> = HashMap::new();
    for (name, types) in &found {
        let mut distinct: Vec<Type> = Vec::new();
        for &ty in types {
            // Only a string, an array, an object or an enum can be null.
            if crate::analysis::nullable_type(ty) && !distinct.contains(&ty) {
                distinct.push(ty);
            }
        }
        for &ty in distinct.iter().skip(1) {
            let fresh =
                (2..).map(|k| format!("{name}{k}")).find(|n| !names.contains(n)).expect("a free name");
            names.insert(fresh.clone());
            renames.insert((name.clone(), ty), fresh);
        }
    }
    if renames.is_empty() {
        return Vec::new();
    }
    // A declaration that the walk does not see, one inside a switch expression's cases, would
    // leave its uses with the wrong name: such a name is left as it is.
    let mut outside: HashMap<String, usize> = HashMap::new();
    for stmt in &method.body {
        visit_stmt_outside_switches(stmt, &mut |s| {
            for name in stmt_declarations(s) {
                *outside.entry(name.to_string()).or_default() += 1;
            }
        });
    }
    let mut all: HashMap<String, usize> = HashMap::new();
    for stmt in &method.body {
        visit_stmt(stmt, &mut |_| {}, &mut |s| {
            for name in stmt_declarations(s) {
                *all.entry(name.to_string()).or_default() += 1;
            }
        });
    }
    renames.retain(|(name, _), _| outside.get(name) == all.get(name));
    if renames.is_empty() {
        return Vec::new();
    }
    let mut walk = Walk { renames, current: HashMap::new() };
    for p in &method.params {
        walk.declare(&p.name, p.ty);
    }
    for stmt in &mut method.body {
        walk.stmt(stmt);
    }
    walk.renames.into_iter().map(|((name, _), fresh)| (fresh, name)).collect()
}

/// The names that statement `s` itself declares.
fn stmt_declarations(s: &Stmt) -> Vec<&str> {
    match s {
        Stmt::Local { name, .. } | Stmt::ForEach { name, .. } => vec![name.as_str()],
        Stmt::Try { catches, .. } => catches.iter().map(|c| c.name.as_str()).collect(),
        _ => Vec::new(),
    }
}

/// The type that stands for every catch parameter's: one is never null, so catch parameters of
/// different classes need no names of their own.
const CAUGHT: Type = Type::Class("java.lang.Throwable", &[]);

struct Walk {
    /// The new name of each name declared with each type.
    renames: HashMap<(String, Type), String>,
    /// The name each Java name stands for now, where it is renamed.
    current: HashMap<String, String>,
}

impl Walk {
    /// Declares `name` with type `ty`, and returns the name it gets.
    fn declare(&mut self, name: &str, ty: Type) -> Option<String> {
        match self.renames.get(&(name.to_string(), ty)) {
            Some(fresh) => {
                self.current.insert(name.to_string(), fresh.clone());
                Some(fresh.clone())
            }
            None => {
                self.current.remove(name);
                None
            }
        }
    }

    fn rename(&self, name: &mut String) {
        if let Some(fresh) = self.current.get(name.as_str()) {
            *name = fresh.clone();
        }
    }

    fn expr(&mut self, e: &mut Expr) {
        rewrite(e, &mut |x| match &mut x.kind {
            ExprKind::Var(v) => self.rename(v),
            ExprKind::Assign { target: Place::Var(v), .. }
            | ExprKind::IncDec { target: Place::Var(v), .. } => self.rename(v),
            ExprKind::InstanceOf { binding: Some(b), ty, .. } => {
                if let Some(fresh) = self.declare(b, *ty) {
                    *b = fresh;
                }
            }
            _ => {}
        });
    }

    fn stmt(&mut self, s: &mut Stmt) {
        match s {
            Stmt::Local { ty, name, init, .. } => {
                if let Some(init) = init {
                    self.expr(init);
                }
                if let Some(fresh) = self.declare(name, *ty) {
                    *name = fresh;
                }
            }
            Stmt::ForEach { ty, name, array, body } => {
                self.expr(array);
                if let Some(fresh) = self.declare(name, *ty) {
                    *name = fresh;
                }
                self.stmt(body);
            }
            Stmt::Expr(e) | Stmt::Return(Some(e), _) | Stmt::Yield(e) | Stmt::Throw(e) => self.expr(e),
            Stmt::If { cond, then, otherwise } => {
                self.expr(cond);
                self.stmt(then);
                if let Some(o) = otherwise {
                    self.stmt(o);
                }
            }
            Stmt::While { cond, body } => {
                self.expr(cond);
                self.stmt(body);
            }
            Stmt::DoWhile { body, cond } => {
                self.stmt(body);
                self.expr(cond);
            }
            Stmt::For { init, cond, update, body } => {
                init.iter_mut().for_each(|s| self.stmt(s));
                if let Some(c) = cond {
                    self.expr(c);
                }
                update.iter_mut().for_each(|e| self.expr(e));
                self.stmt(body);
            }
            Stmt::Block(stmts) => stmts.iter_mut().for_each(|s| self.stmt(s)),
            Stmt::Switch { selector, cases } => {
                self.expr(selector);
                for case in cases {
                    for label in &mut case.labels {
                        if let CaseLabel::Value(e) = label {
                            self.expr(e);
                        }
                    }
                    case.body.iter_mut().for_each(|s| self.stmt(s));
                }
            }
            Stmt::Labeled { body, .. } => self.stmt(body),
            Stmt::Try { body, catches, finally } => {
                body.iter_mut().for_each(|s| self.stmt(s));
                for c in catches {
                    if let Some(fresh) = self.declare(&c.name, CAUGHT) {
                        c.name = fresh;
                    }
                    c.body.iter_mut().for_each(|s| self.stmt(s));
                }
                finally.iter_mut().flatten().for_each(|s| self.stmt(s));
            }
            Stmt::Return(None, _)
            | Stmt::Break
            | Stmt::Continue
            | Stmt::BreakLabel(_)
            | Stmt::ContinueLabel(_) => {}
        }
    }
}
