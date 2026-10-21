//! for-each over an `Iterable`: `for (T x : c) body` on an object of the program is the loop
//! that Java runs, written out:
//!
//! ```text
//! { Iterator it = c.iterator(); while (it.hasNext()) { T x = (T) it.next(); body } }
//! ```
//!
//! The calls are ordinary calls of the program's methods (the JDK's `Iterable` and `Iterator`
//! in a whole program, the translator's own declarations of them in a file), so what an iterator
//! does, throwing `ConcurrentModificationException` or `NoSuchElementException` included, is
//! what its translated code does. A label on the loop goes on the `while`.

use super::*;
use std::collections::{HashSet, VecDeque};

impl<'t> Lowering<'t> {
    /// `for (T x : collection) body` (`node`) when `collection` is an object of the program, with
    /// the loop's `label` if it has one.
    pub(super) fn for_each_iterable(
        &self,
        node: Node<'t>,
        collection: Expr,
        label: Option<&str>,
    ) -> Result<Stmt, Error> {
        let pos = node.pos();
        let rejected = || unsupported(node, "for-each over collections");
        let Some(iterator) = self.no_arg_method(collection.ty, "iterator") else { return rejected() };
        let start = self.call_of(collection, iterator, pos)?;
        let it_ty = start.ty;
        let (Some(has_next), Some(next)) =
            (self.no_arg_method(it_ty, "hasNext"), self.no_arg_method(it_ty, "next"))
        else {
            return rejected();
        };
        let it = self.fresh_name("it");
        let var = || self.synth(ExprKind::Var(it.clone()), pos, it_ty);
        let cond = self.call_of(var(), has_next, pos)?;
        let element = self.call_of(var(), next, pos)?;
        let variable = node.child("variable")?;
        let ty = self.ty(variable, variable.symbol().map_or("", |s| s.ty))?;
        // An element of a primitive type is unboxed from its box, as javac casts to it.
        let target = if ty.is_numeric() || ty == Type::Boolean { Type::boxed(ty) } else { ty };
        let element = if element.ty == target || self.has_param(element.ty) {
            element
        } else {
            self.synth(ExprKind::Cast(target, Box::new(element)), pos, target)
        };
        let declared = Stmt::Local {
            ty,
            name: variable.str("name").to_string(),
            init: Some(element),
            pos: variable.pos(),
        };
        let body = vec![declared, self.single_stmt(node.child("statement")?)?];
        let mut looped = Stmt::While { cond, body: Box::new(Stmt::Block(body)) };
        if let Some(label) = label {
            looped = Stmt::Labeled { label: label.to_string(), body: Box::new(looped) };
        }
        let start = Stmt::Local { ty: it_ty, name: it, init: Some(start), pos };
        Ok(Stmt::Block(vec![start, looped]))
    }

    /// `String.join(delimiter, elements)` over an `Iterable` of strings of the program (`node`
    /// is the call, `elements` the lowered collection): the iteration is the collection's own,
    /// written out as a for-each is, in a block that yields the joined text.
    ///
    /// ```text
    /// switch (0) { default -> { String d = delimiter; Iterator it = elements.iterator();
    ///     String joined = ""; boolean first = true;
    ///     while (it.hasNext()) {
    ///         String e = (String) it.next();
    ///         if (first) { first = false; joined = "" + e; } else { joined = joined + d + e; } }
    ///     yield joined; } }
    /// ```
    pub(super) fn join_iterable(
        &self,
        node: Node<'t>,
        delimiter: Expr,
        elements: Expr,
    ) -> Result<ExprKind, Error> {
        let pos = node.pos();
        let rejected = || unsupported(node, "`String.join` of this collection");
        let Some(iterator) = self.no_arg_method(elements.ty, "iterator") else { return rejected() };
        let start = self.call_of(elements, iterator, pos)?;
        let it_ty = start.ty;
        let (Some(has_next), Some(next)) =
            (self.no_arg_method(it_ty, "hasNext"), self.no_arg_method(it_ty, "next"))
        else {
            return rejected();
        };
        let [d, it, e, joined, first] =
            ["delimiter", "it", "element", "joined", "first"].map(|n| self.fresh_name(n));
        let var = |name: &str, ty: Type| self.synth(ExprKind::Var(name.to_string()), pos, ty);
        let local = |name: &str, ty: Type, init: Expr| Stmt::Local {
            ty,
            name: name.to_string(),
            init: Some(init),
            pos,
        };
        let assign = |name: &str, ty: Type, value: Expr| {
            let kind =
                ExprKind::Assign { op: None, target: Place::Var(name.to_string()), value: Box::new(value) };
            Stmt::Expr(self.synth(kind, pos, ty))
        };
        let concat = |l: Expr, r: Expr| {
            self.synth(ExprKind::Binary(BinOp::Add, Box::new(l), Box::new(r)), pos, Type::String)
        };
        let boolean = |b: bool| self.synth(ExprKind::Bool(b), pos, Type::Boolean);
        // The elements are `CharSequence`s, and the only ones that can be `Object` values are
        // strings, so the erased element is cast to a string.
        let element = self.call_of(var(&it, it_ty), next, pos)?;
        let element = match element.ty {
            Type::String => element,
            _ => self.synth(ExprKind::Cast(Type::String, Box::new(element)), pos, Type::String),
        };
        let first_text = assign(
            &joined,
            Type::String,
            concat(self.synth(ExprKind::Str(String::new()), pos, Type::String), var(&e, Type::String)),
        );
        let later_text = assign(
            &joined,
            Type::String,
            concat(concat(var(&joined, Type::String), var(&d, Type::String)), var(&e, Type::String)),
        );
        let body = Stmt::If {
            cond: var(&first, Type::Boolean),
            then: Box::new(Stmt::Block(vec![assign(&first, Type::Boolean, boolean(false)), first_text])),
            otherwise: Some(Box::new(Stmt::Block(vec![later_text]))),
        };
        let looped = Stmt::While {
            cond: self.call_of(var(&it, it_ty), has_next, pos)?,
            body: Box::new(Stmt::Block(vec![local(&e, Type::String, element), body])),
        };
        let block = vec![
            local(&d, Type::String, delimiter),
            local(&it, it_ty, start),
            local(&joined, Type::String, self.synth(ExprKind::Str(String::new()), pos, Type::String)),
            local(&first, Type::Boolean, boolean(true)),
            looped,
            Stmt::Yield(var(&joined, Type::String)),
        ];
        let selector = self.synth(ExprKind::Int(0), pos, Type::Int);
        Ok(ExprKind::Switch {
            selector: Box::new(selector),
            cases: vec![Case { labels: vec![CaseLabel::Default], body: block }],
        })
    }

    /// The method without parameters named `name` that a call on a value of type `ty` runs, as
    /// javac resolves it: declared by its class or the nearest supertype.
    fn no_arg_method(&self, ty: Type, name: &str) -> Option<usize> {
        let Type::Class(class, _) = ty else { return None };
        let mut queue = VecDeque::from([self.type_index(class)?]);
        let mut seen = HashSet::new();
        while let Some(t) = queue.pop_front() {
            if !seen.insert(t) {
                continue;
            }
            let found = self
                .methods
                .iter()
                .position(|m| m.owner == t && m.name == name && m.node.list("parameters").next().is_none());
            if found.is_some() {
                return found;
            }
            queue.extend(self.superclass(t).ok().flatten());
            queue.extend(self.interfaces(t).unwrap_or_default());
        }
        None
    }

    /// `receiver.m()` for method `m` without parameters, of the type it is declared with.
    fn call_of(&self, receiver: Expr, m: usize, pos: Pos) -> Result<Expr, Error> {
        let info = &self.methods[m];
        if self.is_real(real_generics::Owner::Class(info.owner)) {
            self.fail(real_generics::Owner::Class(info.owner));
        }
        let ret = info.node.child("returnType")?;
        let ty = self.declared_type(ret, ret.java_type(), info.owner, Some(m))?;
        let kind = ExprKind::MethodCall {
            receiver: Box::new(receiver),
            name: info.name.to_string(),
            args: Vec::new(),
            resolved: Some(m),
            is_super: false,
        };
        Ok(self.synth(kind, pos, ty))
    }

    /// A local's name, `base` or with `_` appended, that no variable of the current class's code
    /// has.
    pub(super) fn fresh_name(&self, base: &str) -> String {
        let mut taken = HashSet::new();
        for member in self.types[self.current.get()].node.list("members") {
            captures::declared_names(member, &mut taken);
        }
        let mut name = base.to_string();
        while taken.contains(&name) {
            name.push('_');
        }
        name
    }
}
