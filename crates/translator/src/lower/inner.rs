//! Inner, local and anonymous classes, lowered as javac desugars them: an object of such a class
//! holds its enclosing instance in a field `outer` and each local variable that its code reads
//! from the enclosing method in a field of its own, and every constructor takes them as
//! parameters (the enclosing instance first, the captured locals last) and stores them before
//! anything else runs. A `new` passes them: the enclosing instance is `this`, the explicit one of
//! `o.new Inner()`, or `this.outer` of the code creating the object, and a captured local is the
//! local itself or, where the creating code is itself in such a class, its own field. So the rest
//! of the translation sees ordinary classes with fields, parameters and arguments. What each class
//! captures is found by `captures`.
//!
//! The declaration of such a class is named after its place: a local class `Labeler` in `Main` is
//! `Main.Labeler` (`MainLabeler` in Rust), with the method appended where two local classes of a
//! class share a name; an anonymous class is `<Supertype>In<Method>` (`ComparatorInMain`),
//! numbered where that is taken.

use super::*;
use std::collections::HashMap;

/// How a class is nested in code that has variables of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NestKind {
    /// A member class that is not static.
    Inner,
    /// A class declared in a block.
    Local,
    /// `new T(..) { .. }`.
    Anonymous,
    /// The class of a lambda or method reference (see `lambdas`).
    Lambda,
}

pub(super) struct Nested<'t> {
    pub kind: NestKind,
    /// The class whose code declares this one: the outer class of an inner class.
    pub enclosing: usize,
    /// The method whose body declares a local or anonymous class; `None` in a field initializer
    /// or an initializer block, and for an inner class.
    pub method: Option<usize>,
    /// Whether the declaring code has an enclosing instance: always for an inner class; for a
    /// local or anonymous class, unless it is declared in static code.
    pub instance: bool,
    /// The `new` that declares an anonymous class.
    pub new: Option<Node<'t>>,
    /// The locals of the enclosing code that the class captures (see `captures`).
    pub captures: Vec<Captured<'t>>,
    /// The enclosing classes whose instances the code uses (see `captures`).
    pub needs: Vec<usize>,
    /// Whether the objects hold the enclosing instance, which their code uses.
    pub outer: bool,
    /// Whether the class declares the field `outer` itself, rather than inheriting it from a
    /// superclass nested in the same class.
    pub own_outer: bool,
    /// The field that holds the enclosing instance: `outer`, or `outer_` (and so on) where a
    /// superclass nested in another class already holds its own enclosing instance as `outer`.
    pub outer_field: String,
}

/// A local variable that a class captures.
pub(super) struct Captured<'t> {
    pub name: &'t str,
    /// An identifier that reads it, whose symbol gives its type.
    pub node: Node<'t>,
    /// The field that holds it: declared by the class, or by a superclass that captures it too.
    pub field: String,
    /// Whether a superclass declares the field.
    pub inherited: bool,
    /// The constructors' parameter that passes it.
    pub param: String,
}

/// The constructors' parameter that passes the enclosing instance.
pub(crate) const OUTER: &str = "outer";

impl<'t> Lowering<'t> {
    /// Declares the inner (non-static member) class `member` of class `outer`.
    pub(super) fn declare_inner(&mut self, outer: usize, member: Node<'t>) -> Result<(), Error> {
        // A class in a local class has javac's binary name, with `$` before its simple name.
        let separator = if self.types[outer].javac.contains('$') { '$' } else { '.' };
        let javac = format!("{}{separator}{}", self.types[outer].javac, member.str("simpleName"));
        let nested = self.nested(NestKind::Inner, outer, None, true, None);
        self.types.push(TypeInfo { name: javac.clone(), javac, node: member, nested: Some(nested) });
        self.declare_members(self.types.len() - 1)
    }

    pub(super) fn nested(
        &self,
        kind: NestKind,
        enclosing: usize,
        method: Option<usize>,
        instance: bool,
        new: Option<Node<'t>>,
    ) -> Nested<'t> {
        Nested {
            kind,
            enclosing,
            method,
            instance,
            new,
            captures: Vec::new(),
            needs: Vec::new(),
            outer: false,
            own_outer: false,
            outer_field: OUTER.to_string(),
        }
    }

    /// Declares the local and anonymous classes in the code of `member`, a member of class
    /// `class`: a method's body (`method`), a field's initializer or an initializer block.
    pub(super) fn declare_local_classes(
        &mut self,
        class: usize,
        member: Node<'t>,
        method: Option<usize>,
    ) -> Result<(), Error> {
        let instance = !(member.has_flag("STATIC") || Self::is_static_block(member));
        let found = self.scans.clone().local_classes(member, || {
            let mut found = Vec::new();
            let code = ["body", "initializer", "statements"].into_iter().filter_map(|f| member.get(f));
            for node in code {
                classes_in(node, &mut found);
            }
            found
        });
        for &(decl, new) in found.iter() {
            let javac = crate::program::class_name(decl.0);
            if self.reach.is_some_and(|r| !r.classes.contains(&javac)) {
                continue;
            }
            let kind = if new.is_some() { NestKind::Anonymous } else { NestKind::Local };
            if kind == NestKind::Local && decl.kind() != "CLASS" {
                return unsupported(decl, "local interfaces, enums and records");
            }
            let nested = self.nested(kind, class, method, instance, new);
            self.types.push(TypeInfo { name: javac.clone(), javac, node: decl, nested: Some(nested) });
            self.declare_members(self.types.len() - 1)?;
        }
        Ok(())
    }

    /// Names the declarations of the local and anonymous classes after their place (see the
    /// module's comment), once every class is declared.
    pub(super) fn name_nested(&mut self) {
        // A local class's name can say the name of the class it is in, which may be hoisted.
        self.name_hoisted();
        // How many names and javac names of classes there are with the dots left out, which a new
        // name must not repeat. Looking through every class for each one took a large part of the
        // Minecraft server's lowering.
        let flat = |name: &str| name.replace('.', "");
        // And the same as they are, for `package_of`, which `type_index` would answer by indexing
        // every class again after each new name.
        let mut flat_names: HashMap<String, usize> = HashMap::new();
        let mut names: HashMap<String, usize> = HashMap::new();
        for info in &self.types {
            *flat_names.entry(flat(&info.name)).or_default() += 1;
            *flat_names.entry(flat(&info.javac)).or_default() += 1;
            *names.entry(info.name.clone()).or_default() += 1;
            *names.entry(info.javac.clone()).or_default() += 1;
        }
        fn recount(counts: &mut HashMap<String, usize>, old: String, new: String) {
            if let Some(count) = counts.get_mut(&old) {
                *count -= 1;
            }
            *counts.entry(new).or_default() += 1;
        }
        let rename = |types: &mut [TypeInfo], flat_names: &mut _, names: &mut _, t: usize, name: String| {
            let old = std::mem::replace(&mut types[t].name, name.clone());
            recount(flat_names, flat(&old), flat(&name));
            recount(names, old, name);
        };
        for t in 0..self.types.len() {
            // A member of a local or anonymous class is declared after it, so its name is final.
            if let Some(&(_, outer)) = self.member_outers.iter().find(|(member, _)| *member == t) {
                let name = format!("{}.{}", self.types[outer].name, self.types[t].node.str("simpleName"));
                rename(&mut self.types, &mut flat_names, &mut names, t, name);
                continue;
            }
            let Some(nested) = &self.types[t].nested else { continue };
            let place = self.place_name(nested);
            let base = match nested.kind {
                NestKind::Inner | NestKind::Lambda => continue,
                NestKind::Local => {
                    format!("{}.{}", self.types[nested.enclosing].name, self.types[t].node.str("simpleName"))
                }
                NestKind::Anonymous => {
                    let new = nested.new.expect("an anonymous class has its `new`");
                    let supertype = new.get("identifier").map_or("", |i| i.java_type());
                    let supertype = super::captures::raw(supertype);
                    // A local class's binary name has its number before its simple name.
                    let simple = supertype.rsplit(['.', '$']).next().unwrap_or_default();
                    let simple = simple.trim_start_matches(|c: char| c.is_ascii_digit());
                    let package = self.package_with(t, |outer| names.get(outer).is_some_and(|&n| n > 0));
                    format!("{package}{simple}In{place}")
                }
            };
            // Whether another class has the name or javac name, with the dots left out: those of
            // the class itself are not counted.
            let own = [flat(&self.types[t].name), flat(&self.types[t].javac)];
            let taken = |name: &str, _: &Self| {
                let name = flat(name);
                let count = flat_names.get(&name).copied().unwrap_or(0);
                count > own.iter().filter(|o| **o == name).count()
            };
            let mut name = base.clone();
            if taken(&name, self) && self.types[t].nested.as_ref().is_some_and(|n| n.kind == NestKind::Local)
            {
                name = format!("{base}In{place}");
            }
            let first = name.clone();
            let mut n = 2;
            while taken(&name, self) {
                name = format!("{first}{n}");
                n += 1;
            }
            rename(&mut self.types, &mut flat_names, &mut names, t, name);
        }
        self.indices.get_mut().forget_types();
    }

    /// The place of a local or anonymous class, as a part of its name: its method (`Main` for
    /// `main`, `New` for a constructor), the field it initializes, or `Init` for a block.
    fn place_name(&self, nested: &Nested<'t>) -> String {
        let name = match nested.method {
            Some(m) if self.methods[m].name == "<init>" => "new",
            Some(m) => self.methods[m].name,
            None => "init",
        };
        let mut chars = name.chars();
        chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
    }

    /// The package prefix of class `t`'s name (`com.example.`), from its top-level class.
    pub(super) fn package_of(&self, t: usize) -> String {
        self.package_with(t, |outer| self.type_index(outer).is_some())
    }

    /// `package_of`, with `declared` telling whether a class has a name or javac name.
    fn package_with(&self, t: usize, declared: impl Fn(&str) -> bool) -> String {
        let mut top = t;
        while let Some(n) = &self.types[top].nested {
            top = n.enclosing;
        }
        let name = &self.types[top].javac;
        // A static member class is named `Outer.Inner`; its top-level class is the longest
        // prefix that is a declared class.
        let mut at = name.as_str();
        while let Some((outer, _)) = at.rsplit_once('.') {
            if !declared(outer) {
                return format!("{outer}.");
            }
            at = outer;
        }
        String::new()
    }

    /// The nesting of class `t`, if it is an inner, local or anonymous class.
    pub(super) fn nesting(&self, t: usize) -> Option<&Nested<'t>> {
        self.types[t].nested.as_ref()
    }

    /// The captures of class `t`, as the declaration lists them.
    pub(super) fn captures_of(&self, t: usize) -> Result<Vec<Capture>, Error> {
        let Some(nested) = self.nesting(t) else { return Ok(Vec::new()) };
        let receiver = self.lambda_of_type(t).and_then(|l| l.receiver).map(|(name, _)| name);
        nested
            .captures
            .iter()
            .map(|c| {
                Ok(Capture {
                    name: c.name.to_string(),
                    field: c.field.clone(),
                    ty: self.capture_type(c)?,
                    checked: receiver == Some(c.name),
                })
            })
            .collect()
    }

    /// Where class `t` is declared.
    pub(super) fn origin(&self, t: usize) -> Origin {
        if self.plain_object == Some(t) {
            return Origin::Object;
        }
        let binary = self.types[t].javac.clone();
        match self.nesting(t) {
            Some(n) if n.kind == NestKind::Local => Origin::Local { method: n.method, binary },
            Some(n) if n.kind == NestKind::Anonymous => Origin::Anonymous { method: n.method, binary },
            Some(n) if n.kind == NestKind::Lambda => {
                // Named after the binary name of the class that holds it: `p.Outer$Inner$$Lambda`.
                let holder = self.lambda_holder(t);
                let holder = self.binary_name(holder).unwrap_or_else(|| self.types[holder].javac.clone());
                let binary = format!("{holder}$$Lambda");
                Origin::Lambda { method: n.method, binary }
            }
            // A hoisted type's method is not kept: nothing reads it.
            None if self.is_hoisted(t) => Origin::Local { method: None, binary },
            _ => Origin::Member,
        }
    }

    /// The synthetic fields of class `t`, first in its struct: the enclosing instance and the
    /// captured locals that it declares itself.
    pub(super) fn synthetic_fields(&self, t: usize) -> Result<Vec<Field>, Error> {
        let Some(nested) = self.nesting(t) else { return Ok(Vec::new()) };
        let pos = self.types[t].node.pos();
        let mut out = Vec::new();
        if nested.own_outer {
            let ty = self.self_type(nested.enclosing);
            out.push(Field {
                name: nested.outer_field.clone(),
                ty,
                init: None,
                constant: false,
                volatile: false,
                pos,
            });
        }
        for c in nested.captures.iter().filter(|c| !c.inherited) {
            let ty = self.capture_type(c)?;
            out.push(Field { name: c.field.clone(), ty, init: None, constant: false, volatile: false, pos });
        }
        Ok(out)
    }

    /// The type of a captured local, from an identifier that reads it.
    pub(super) fn capture_type(&self, c: &Captured<'t>) -> Result<Type, Error> {
        self.signature_type(c.node, c.node.symbol().map_or("", |s| s.ty))
    }

    /// Whether the instance field initializers of class `t` run as statements of its
    /// constructors, after the synthetic fields are set: those they read would not be set yet
    /// where the initializers run otherwise.
    pub(super) fn moves_initializers(&self, t: usize) -> bool {
        self.nesting(t).is_some_and(|n| n.outer || !n.captures.is_empty())
    }

    /// The synthetic parameters of the constructors of class `t`: the enclosing instance, before
    /// the declared ones, and the captured locals, after them.
    pub(super) fn synthetic_params(&self, t: usize) -> Result<(Vec<Param>, Vec<Param>), Error> {
        let Some(nested) = self.nesting(t) else { return Ok((Vec::new(), Vec::new())) };
        let first = if nested.outer {
            vec![Param { name: OUTER.to_string(), ty: self.self_type(nested.enclosing) }]
        } else {
            Vec::new()
        };
        let last = nested
            .captures
            .iter()
            .map(|c| Ok(Param { name: c.param.clone(), ty: self.capture_type(c)? }))
            .collect::<Result<_, Error>>()?;
        Ok((first, last))
    }

    /// The statements that start the body of a constructor of class `t` that does not call
    /// another one: the synthetic fields set from the parameters, then the field initializers when
    /// they move here (`moves_initializers`), in order, which read those fields. Those written
    /// after an initializer block run with the blocks (`run_initializer_blocks`).
    pub(super) fn synthetic_statements(&self, t: usize) -> Result<Vec<Stmt>, Error> {
        let Some(nested) = self.nesting(t) else { return Ok(Vec::new()) };
        let pos = self.types[t].node.pos();
        let this = self.synth(ExprKind::This, pos, self.self_type(t));
        let assign = |name: &str, value: Expr| {
            let ty = value.ty;
            let target = Place::Field { object: Box::new(self.copy_this(&this)), name: name.to_string() };
            let kind = ExprKind::Assign { op: None, target, value: Box::new(value) };
            Stmt::Expr(self.synth(kind, pos, ty))
        };
        let mut out = Vec::new();
        if nested.own_outer {
            let ty = self.self_type(nested.enclosing);
            out.push(assign(&nested.outer_field, self.synth(ExprKind::Var(OUTER.to_string()), pos, ty)));
        }
        for c in nested.captures.iter().filter(|c| !c.inherited) {
            let ty = self.capture_type(c)?;
            out.push(assign(&c.field, self.synth(ExprKind::Var(c.param.clone()), pos, ty)));
        }
        if self.moves_initializers(t) {
            let node = self.types[t].node;
            let javac = &self.types[t].javac;
            for member in node.list("members").filter(|m| {
                m.kind() == "VARIABLE"
                    && !m.has_flag("STATIC")
                    && m.get("initializer").is_some()
                    && self.keeps(*m, javac)
                    && !self.runs_after_a_block(node, *m)
            }) {
                let ty = self.ty(member, member.symbol().map_or("", |s| s.ty))?;
                let initializing = self.initializing.replace(true);
                let value = member.child("initializer").and_then(|i| self.expr(i));
                self.initializing.set(initializing);
                out.push(assign(member.str("name"), self.convert(value?, ty)));
            }
        }
        Ok(out)
    }

    /// Puts the synthetic statements of class `owner` at the start of a constructor's `body`,
    /// after its `super(..)` and before the initializer blocks, unless it first calls another
    /// constructor, which sets them. javac sets the synthetic fields before `super(..)`; here the
    /// superclass's constructor runs first, as it builds the `base` of the struct.
    pub(super) fn start_constructor(&self, owner: usize, body: &mut Vec<Stmt>) -> Result<(), Error> {
        if delegates(body) {
            return Ok(());
        }
        let at = explicit_call(body).map_or(0, |i| i + 1);
        let statements = self.synthetic_statements(owner)?;
        body.splice(at..at, statements);
        Ok(())
    }

    /// A fresh `this` like `this`.
    fn copy_this(&self, this: &Expr) -> Expr {
        self.synth(ExprKind::This, this.pos, this.ty)
    }

    /// The synthetic arguments of a call of a constructor of class `t` from a constructor of the
    /// same class (`this(..)`) or of a subclass (`super(..)`): the enclosing instance, and each
    /// captured local, from the calling constructor's own parameters.
    pub(super) fn passed_on(&self, t: usize, pos: Pos) -> Result<(Vec<Expr>, Vec<Expr>), Error> {
        self.passed_on_from(t, pos, None)
    }

    /// `passed_on`, with the enclosing instance of class `t` given as `outer` (`o.super(..)`)
    /// rather than found from the current class.
    pub(super) fn passed_on_from(
        &self,
        t: usize,
        pos: Pos,
        outer: Option<Expr>,
    ) -> Result<(Vec<Expr>, Vec<Expr>), Error> {
        let Some(nested) = self.nesting(t) else { return Ok((Vec::new(), Vec::new())) };
        let first = match outer {
            Some(value) => vec![value],
            None if nested.outer => vec![self.enclosing_instance_of(nested.enclosing, pos)?],
            None => Vec::new(),
        };
        let current = self.current.get();
        let mut last = Vec::new();
        for c in &nested.captures {
            let ty = self.capture_type(c)?;
            let Some(mine) = self.nesting(current).and_then(|n| n.captures.iter().find(|m| m.name == c.name))
            else {
                return error(pos, "a constructor that does not capture what its superclass captures");
            };
            last.push(self.synth(ExprKind::Var(mine.param.clone()), pos, ty));
        }
        Ok((first, last))
    }

    /// The synthetic arguments of `new C(..)` (`node`) for class `t`: the enclosing instance,
    /// explicit (`o.new Inner()`) or the creating code's, and the captured locals, read where the
    /// object is created.
    pub(super) fn creation_args(&self, node: Node<'t>, t: usize) -> Result<(Vec<Expr>, Vec<Expr>), Error> {
        let explicit = node.get("enclosingExpression");
        let Some(nested) = self.nesting(t) else {
            return match explicit {
                Some(e) => unsupported(e, "an enclosing instance for this class"),
                None => Ok((Vec::new(), Vec::new())),
            };
        };
        let pos = node.pos();
        // In `o.new Inner() { .. }`, `o` is the superclass's enclosing instance, a declared
        // argument (see `anonymous_enclosing`).
        let first = match explicit.filter(|_| nested.kind != NestKind::Anonymous) {
            Some(e) => {
                let value = self.expr(e)?;
                if !nested.outer {
                    if has_effects(&value) {
                        return unsupported(e, "an enclosing instance with effects that is not used");
                    }
                    Vec::new()
                } else {
                    vec![self.convert(value, self.self_type(nested.enclosing))]
                }
            }
            None if nested.outer => vec![self.enclosing_instance_of(nested.enclosing, pos)?],
            None => Vec::new(),
        };
        let mut last = Vec::new();
        for c in &nested.captures {
            let ty = self.capture_type(c)?;
            last.push(self.captured_at(node, c.name, ty)?);
        }
        Ok((first, last))
    }

    /// The explicit enclosing instance of `o.new Inner() { .. }` (`node`), which creates an object
    /// of the anonymous class `t`: javac declares it as the first parameter of the anonymous
    /// class's constructor, which passes it on as `o.super(..)`, so it comes before the declared
    /// arguments. `None` for any other `new`.
    pub(super) fn anonymous_enclosing(&self, node: Node<'t>, t: usize) -> Result<Option<Expr>, Error> {
        let Some(e) = node.get("enclosingExpression") else { return Ok(None) };
        if self.nesting(t).is_none_or(|n| n.kind != NestKind::Anonymous) {
            return Ok(None);
        }
        let value = self.expr(e)?;
        match self.superclass(t)?.and_then(|s| self.nesting(s)) {
            Some(s) => Ok(Some(self.convert(value, self.self_type(s.enclosing)))),
            None => unsupported(e, "an enclosing instance for this class"),
        }
    }

    /// `args` with the synthetic arguments around them.
    pub(super) fn with_synthetic(&self, (first, last): (Vec<Expr>, Vec<Expr>), args: Vec<Expr>) -> Vec<Expr> {
        first.into_iter().chain(args).chain(last).collect()
    }
}

/// The local class declarations and anonymous classes in a tree of code, each with its `new` when
/// it is anonymous, outside the bodies of the classes found.
fn classes_in<'t>(node: Node<'t>, out: &mut Vec<(Node<'t>, Option<Node<'t>>)>) {
    match node.0 {
        crate::value::Value::Object(fields) => {
            let kind = node.kind();
            if crate::program::CLASS_KINDS.contains(&kind) {
                out.push((node, None));
                return;
            }
            if kind == "NEW_CLASS"
                && let Some(body) = node.get("classBody")
            {
                for field in ["enclosingExpression", "arguments"] {
                    if let Some(child) = node.get(field) {
                        classes_in(child, out);
                    }
                }
                out.push((body, Some(node)));
                return;
            }
            for (field, child) in fields {
                if field != "symbol" {
                    classes_in(Node(child), out);
                }
            }
        }
        crate::value::Value::Array(items) => items.iter().for_each(|item| classes_in(Node(item), out)),
        _ => {}
    }
}
