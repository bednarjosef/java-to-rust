//! Methods that lowering adds for the code it writes, so that no function of the translation is
//! too big for rustc and LLVM to compile in reasonable time: LLVM's time grows faster than a
//! function's size, and a function is compiled on one thread. A `switch` over thousands of cases
//! that lowering writes (the reflection table's natives, `lower/reflection.rs`, and the classes
//! that `Class.forName` of a computed name finds, `lower/by_name.rs`) is cut into methods of about
//! `CASES` cases each, which a `switch` on the label's range or length calls (`chunked_switch`);
//! code that many places would each spell out in full (that `Class.forName`) is one method that
//! each calls (`shared_method`).

use super::*;

/// About how many cases a method that `chunked_switch` adds has.
pub(super) const CASES: usize = 128;

/// Where a `switch` that `chunked_int_switch` or `chunked_string_switch` cuts is: the class
/// whose methods the parts are, the name that theirs start with, the parameters that the cases
/// read (the `switch` is over the first), and the type and position of the `switch`.
pub(super) struct Chunked<'a> {
    pub class: usize,
    pub base: &'a str,
    pub params: &'a [(String, Type)],
    pub ty: Type,
    pub pos: Pos,
}

/// A method that lowering adds (`add_method`): its class, its name (or for `shared_method`'s
/// `make` the name that its name starts with), parameters, result type, body and position.
pub(super) struct AddedMethod {
    pub class: usize,
    pub name: String,
    pub params: Vec<Param>,
    pub ret: Type,
    pub body: Vec<Stmt>,
    pub pos: Pos,
}

impl<'t> Lowering<'t> {
    /// Adds a static method of the class `class` named after `base`, with the parameters
    /// `params`, the result type `ret` and the body `body`, and gives its name, which no other
    /// added method and no method of the class has: a call of it by that name (`ExprKind::Call`
    /// with no method resolved) is resolved once the methods are added (`added_methods`).
    pub(super) fn add_method(
        &self,
        class: usize,
        base: &str,
        params: Vec<Param>,
        ret: Type,
        body: Vec<Stmt>,
        pos: Pos,
    ) -> String {
        let mut added = self.added_methods.borrow_mut();
        let name = (1..)
            .map(|n| if n == 1 { base.to_string() } else { format!("{base}{n}") })
            .find(|n| {
                !added.iter().any(|m| &m.name == n)
                    && !self.methods.iter().any(|m| m.owner == class && m.name == n.as_str())
            })
            .expect("a free name");
        added.push(AddedMethod { class, name: name.clone(), params, ret, body, pos });
        name
    }

    /// The method that `key` names, added once (`add_method`) as `make` gives it the first time
    /// it is asked for: its name.
    pub(super) fn shared_method(
        &self,
        key: &str,
        make: &mut dyn FnMut() -> Result<AddedMethod, Error>,
    ) -> Result<String, Error> {
        if let Some(name) = self.shared_methods.borrow().get(key) {
            return Ok(name.clone());
        }
        let m = make()?;
        let name = self.add_method(m.class, &m.name, m.params, m.ret, m.body, m.pos);
        self.shared_methods.borrow_mut().insert(key.to_string(), name.clone());
        Ok(name)
    }

    /// A call of the added method `name` (`add_method`) with `args`, whose result is of type `ty`.
    pub(super) fn added_call(&self, name: &str, args: Vec<Expr>, pos: Pos, ty: Type) -> Expr {
        self.synth(ExprKind::Call { name: name.to_string(), args, resolved: None }, pos, ty)
    }

    /// `switch (params[0]) { cases.. default -> otherwise }` (see `Chunked`) of type `ty`, whose
    /// cases are labelled with `int`s and read only `params`, with more than `CASES` cases cut
    /// into methods named after `base`: each has the cases whose labels divided by `step` are the
    /// same, about `CASES` of them, and `switch (params[0] / step)` calls the method of the
    /// labels' range, or runs `otherwise` for any other. Every label and every value of
    /// `params[0]` that a case is for must not be negative.
    pub(super) fn chunked_int_switch(
        &self,
        at: &Chunked,
        cases: Vec<(i32, Vec<Stmt>)>,
        step: i32,
        otherwise: &dyn Fn() -> Vec<Stmt>,
    ) -> Expr {
        let int = |i: i32| self.synth(ExprKind::Int(i), at.pos, Type::Int);
        let divided = || {
            let divided = ExprKind::Binary(BinOp::Div, Box::new(self.param(at, 0)), Box::new(int(step)));
            self.synth(divided, at.pos, Type::Int)
        };
        self.chunked_switch(at, cases, &|label| int(*label), &|label| label / step, &divided, otherwise)
    }

    /// `switch (params[0]) { cases.. default -> otherwise }` (see `Chunked`) over a `String`
    /// that is not null, whose cases are labelled with strings, with more than `CASES` cases cut
    /// into methods named after `base` by the labels' lengths, which `switch
    /// (params[0].length())` chooses among.
    pub(super) fn chunked_string_switch(
        &self,
        at: &Chunked,
        cases: Vec<(String, Vec<Stmt>)>,
        otherwise: &dyn Fn() -> Vec<Stmt>,
    ) -> Expr {
        let label = |label: &String| self.synth(ExprKind::Str(label.clone()), at.pos, Type::String);
        let length = |label: &String| label.encode_utf16().count() as i32;
        let selector = || {
            let length = ExprKind::Builtin(Builtin::StringLength, vec![self.param(at, 0)]);
            self.synth(length, at.pos, Type::Int)
        };
        self.chunked_switch(at, cases, &label, &length, &selector, otherwise)
    }

    /// Parameter `i` of `at`, as an expression.
    fn param(&self, at: &Chunked, i: usize) -> Expr {
        self.synth(ExprKind::Var(at.params[i].0.clone()), at.pos, at.params[i].1)
    }

    /// `switch (params[0]) { cases.. default -> otherwise }`, with more than `CASES` cases cut
    /// into methods by `chunk` of their labels, which `switch (selector) { chunk -> method(..) }`
    /// calls (`chunked_int_switch`, `chunked_string_switch`).
    fn chunked_switch<L>(
        &self,
        at: &Chunked,
        cases: Vec<(L, Vec<Stmt>)>,
        label: &dyn Fn(&L) -> Expr,
        chunk: &dyn Fn(&L) -> i32,
        selector: &dyn Fn() -> Expr,
        otherwise: &dyn Fn() -> Vec<Stmt>,
    ) -> Expr {
        let switch = |selector: Expr, cases: Vec<(Expr, Vec<Stmt>)>| {
            let mut cases: Vec<Case> = cases
                .into_iter()
                .map(|(label, body)| Case { labels: vec![CaseLabel::Value(label)], body })
                .collect();
            cases.push(Case { labels: vec![CaseLabel::Default], body: otherwise() });
            self.synth(ExprKind::Switch { selector: Box::new(selector), cases }, at.pos, at.ty)
        };
        if cases.len() <= CASES {
            return switch(self.param(at, 0), cases.into_iter().map(|(l, body)| (label(&l), body)).collect());
        }
        let mut chunks: std::collections::BTreeMap<i32, Vec<(Expr, Vec<Stmt>)>> = Default::default();
        for (l, body) in cases {
            chunks.entry(chunk(&l)).or_default().push((label(&l), body));
        }
        let mut calls = Vec::new();
        for (chunk, cases) in chunks {
            let body = vec![Stmt::Return(Some(switch(self.param(at, 0), cases)), at.pos)];
            let declared = at.params.iter().map(|(name, ty)| Param { name: name.clone(), ty: *ty }).collect();
            let name =
                self.add_method(at.class, &format!("{}Cases{chunk}", at.base), declared, at.ty, body, at.pos);
            let args = (0..at.params.len()).map(|i| self.param(at, i)).collect();
            let call = self.added_call(&name, args, at.pos, at.ty);
            calls.push((self.synth(ExprKind::Int(chunk), at.pos, Type::Int), vec![Stmt::Yield(call)]));
        }
        switch(selector(), calls)
    }

    /// The methods that lowering added (`add_method`), appended to `methods`, and the calls of
    /// them resolved.
    pub(super) fn added_methods(&self, methods: &mut Vec<Method>) {
        let added = self.added_methods.take();
        self.shared_methods.borrow_mut().clear();
        if added.is_empty() {
            return;
        }
        let first = methods.len();
        let index: std::collections::HashMap<String, usize> =
            added.iter().enumerate().map(|(i, m)| (m.name.clone(), first + i)).collect();
        methods.extend(added.into_iter().map(|m| Method {
            name: m.name,
            params: m.params,
            ret: m.ret,
            body: m.body,
            is_main: false,
            is_static: true,
            is_ctor: false,
            class: m.class,
            pos: m.pos,
            is_abstract: false,
            is_synchronized: false,
            overrides: Vec::new(),
            type_params: Vec::new(),
            jdk_code: false,
            field_update: None,
        }));
        for m in methods.iter_mut() {
            for s in &mut m.body {
                rewrite_stmt(s, &mut |e| {
                    if let ExprKind::Call { name, resolved: resolved @ None, .. } = &mut e.kind
                        && let Some(&i) = index.get(name.as_str())
                    {
                        *resolved = Some(i);
                    }
                });
            }
        }
    }
}
