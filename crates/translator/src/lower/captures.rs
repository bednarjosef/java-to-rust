//! Captures: what the code of each inner, local and anonymous class takes from the code around
//! it. javac's tree carries no capture list, so it is computed here, on that tree, before anything
//! is lowered (`plan_captures`):
//!
//! - a local variable, parameter or pattern variable that the class's code reads but does not
//!   declare is captured, found with a stack of the scopes of the locals the code declares;
//! - a local class created by the code, or declared in it, needs its own captures there: those
//!   that are locals of the creating code are passed from them, the others are captured by the
//!   creating class too (a capture of a capture, as javac does), and a local class extending
//!   another passes it the superclass's captures;
//! - the class needs its enclosing instance when its code uses a member of an enclosing class
//!   (a field or method named without `this`, or `Outer.this`), or creates an object of an inner
//!   or local class whose enclosing instance it must pass.
//!
//! These depend on each other between classes, so they are computed to a fixpoint. Lowering then
//! reads an identifier that reads a captured local as the field that holds it (the constructor's
//! parameter, in a constructor), and a member of an enclosing class through the chain of `outer`
//! fields (see `inner`).

use super::inner::{Captured, OUTER};
use super::*;
use std::collections::{HashMap, HashSet};

/// The symbol kinds of local variables.
const LOCAL_KINDS: &[&str] =
    &["LOCAL_VARIABLE", "PARAMETER", "EXCEPTION_PARAMETER", "BINDING_VARIABLE", "RESOURCE_VARIABLE"];

/// What the code of one class takes from around it, from one pass over it.
#[derive(Default)]
struct Found<'t> {
    /// The captured locals, in order, each with an identifier that reads it.
    free: Vec<(&'t str, Node<'t>)>,
    /// The classes whose instances the code needs besides its own: enclosing classes.
    needs: Vec<usize>,
    /// The identifiers that read a captured local.
    refs: Vec<*const Value>,
    /// The `new` expressions (by address) and the captures of the class they create that are
    /// captures of this class there, not locals.
    sites: Vec<(*const Value, &'t str)>,
}

struct Walk<'l, 't> {
    lowering: &'l Lowering<'t>,
    /// The class whose code is walked.
    class: usize,
    /// The locals in scope, innermost scope last.
    scopes: Vec<Vec<&'t str>>,
    found: Found<'t>,
}

impl<'t> Lowering<'t> {
    /// Computes what each inner, local and anonymous class captures, and whether it holds its
    /// enclosing instance (see the module's comment). An error comes with the index of the class
    /// it is in.
    pub(super) fn plan_captures(&mut self) -> Result<(), (usize, Error)> {
        let nested: Vec<usize> = (0..self.types.len()).filter(|&t| self.types[t].nested.is_some()).collect();
        loop {
            let mut found = Vec::new();
            for &t in &nested {
                let mut walk = Walk { lowering: self, class: t, scopes: Vec::new(), found: Found::default() };
                walk.class_code().map_err(|e| (t, e))?;
                found.push(walk.found);
            }
            let mut changed = false;
            let mut refs = HashSet::new();
            let mut sites = HashSet::new();
            for (&t, found) in nested.iter().zip(found) {
                let instance = self.types[t].nested.as_ref().is_some_and(|n| n.instance);
                if !found.needs.is_empty() && !instance {
                    return unsupported(self.types[t].node, "an enclosing instance in static code")
                        .map_err(|e| (t, e));
                }
                let outer = !found.needs.is_empty();
                let nested = self.types[t].nested.as_mut().expect("a nested class");
                let names: Vec<&str> = nested.captures.iter().map(|c| c.name).collect();
                let now: Vec<&str> = found.free.iter().map(|(n, _)| *n).collect();
                changed |= nested.outer != outer
                    || nested.needs != found.needs
                    || names.iter().collect::<HashSet<_>>() != now.iter().collect();
                nested.outer = outer;
                nested.needs = found.needs;
                nested.captures = found
                    .free
                    .into_iter()
                    .map(|(name, node)| Captured {
                        name,
                        node,
                        field: name.to_string(),
                        inherited: false,
                        param: name.to_string(),
                    })
                    .collect();
                refs.extend(found.refs);
                sites.extend(found.sites.into_iter().map(|(at, name)| (at, name.to_string())));
            }
            if !changed {
                self.capture_refs = refs;
                self.capture_sites = sites;
                break;
            }
        }
        // Superclasses first, so that a subclass finds the fields they declare.
        let mut order = nested.clone();
        order.sort_by_key(|&t| self.depth(t));
        for t in order {
            self.name_captures(t).map_err(|e| (t, e))?;
        }
        Ok(())
    }

    /// How many superclasses class `t` has in the program.
    fn depth(&self, t: usize) -> usize {
        self.superclass(t).ok().flatten().map_or(0, |s| 1 + self.depth(s))
    }

    /// The fields and parameters that hold the captures of class `t` and its enclosing instance.
    fn name_captures(&mut self, t: usize) -> Result<(), Error> {
        let superclass = self.superclass(t)?.filter(|&s| self.types[s].nested.is_some());
        let enclosing = self.types[t].nested.as_ref().map(|n| n.enclosing);
        let inherited_outer = superclass.and_then(|s| self.nesting(s)).is_some_and(|s| s.outer);
        let outer = self.nesting(t).is_some_and(|n| n.outer);
        if outer
            && inherited_outer
            && superclass.and_then(|s| self.nesting(s)).map(|s| s.enclosing) != enclosing
        {
            return unsupported(self.types[t].node, "a nested class extending one nested in another class");
        }
        let fields: HashSet<String> = std::iter::once(t)
            .chain(std::iter::successors(self.superclass(t)?, |&s| self.superclass(s).ok().flatten()))
            .flat_map(|c| self.declared_fields(c))
            .collect();
        if outer && fields.contains(OUTER) {
            return unsupported(self.types[t].node, "a field named `outer` in a nested class");
        }
        let declared = self.constructor_names(t);
        let from_super: HashMap<&str, String> = superclass
            .and_then(|s| self.nesting(s))
            .map(|s| s.captures.iter().map(|c| (c.name, c.field.clone())).collect())
            .unwrap_or_default();
        let nested = self.types[t].nested.as_mut().expect("a nested class");
        nested.own_outer = outer && !inherited_outer;
        let mut taken: HashSet<String> = fields.clone();
        let mut params: HashSet<String> = declared;
        params.insert(OUTER.to_string());
        for c in &mut nested.captures {
            match from_super.get(c.name) {
                Some(field) => {
                    c.field = field.clone();
                    c.inherited = true;
                }
                None => {
                    while taken.contains(&c.field) || c.field == OUTER {
                        c.field.push('_');
                    }
                    taken.insert(c.field.clone());
                }
            }
            while params.contains(&c.param) {
                c.param.push('_');
            }
            params.insert(c.param.clone());
        }
        Ok(())
    }

    /// The names of the instance fields that class `t` declares.
    fn declared_fields(&self, t: usize) -> Vec<String> {
        self.types[t]
            .node
            .list("members")
            .filter(|m| m.kind() == "VARIABLE" && !m.has_flag("STATIC"))
            .map(|m| m.str("name").to_string())
            .collect()
    }

    /// The names that the constructors, field initializers and initializer blocks of class `t`
    /// declare, which the synthetic parameters must not hide.
    fn constructor_names(&self, t: usize) -> HashSet<String> {
        let mut out = HashSet::new();
        for member in self.types[t].node.list("members") {
            let code = match member.kind() {
                "METHOD" if member.str("name") == "<init>" => member,
                "VARIABLE" if !member.has_flag("STATIC") => match member.get("initializer") {
                    Some(init) => init,
                    None => continue,
                },
                "BLOCK" => member,
                _ => continue,
            };
            declared_names(code, &mut out);
        }
        out
    }

    /// What an identifier that javac resolves to a local variable reads: the field (or, in a
    /// constructor, the parameter) that holds it when it is a captured local; `None` for a local
    /// of the code itself.
    pub(super) fn capture_read(&self, node: Node<'t>) -> Result<Option<Expr>, Error> {
        if !self.capture_refs.contains(&(node.0 as *const Value)) {
            return Ok(None);
        }
        let name = node.str("name");
        let ty = self.ty(node, node.symbol().map_or("", |s| s.ty))?;
        self.capture_value(node.pos(), name, ty).map(Some)
    }

    /// The value of the captured local `name` in the code of the current class.
    fn capture_value(&self, pos: Pos, name: &str, ty: Type) -> Result<Expr, Error> {
        let current = self.current.get();
        let Some(c) = self.nesting(current).and_then(|n| n.captures.iter().find(|c| c.name == name)) else {
            return error(pos, format!("`{name}` is not captured here"));
        };
        if self.in_constructor() {
            return Ok(self.synth(ExprKind::Var(c.param.clone()), pos, ty));
        }
        let this = self.synth(ExprKind::This, pos, self.self_type(current));
        let kind = ExprKind::Field { object: Box::new(this), name: c.field.clone() };
        Ok(self.synth(kind, pos, ty))
    }

    /// The value of the local `name` that the class created by the `new` expression `node`
    /// captures: the local of the creating code, or its own capture of it.
    pub(super) fn captured_at(&self, node: Node<'t>, name: &str, ty: Type) -> Result<Expr, Error> {
        if self.capture_sites.contains(&(node.0 as *const Value, name.to_string())) {
            return self.capture_value(node.pos(), name, ty);
        }
        Ok(self.synth(ExprKind::Var(name.to_string()), node.pos(), ty))
    }

    /// Whether the code being lowered is a constructor of the current class, where the synthetic
    /// parameters hold the captures and the enclosing instance.
    fn in_constructor(&self) -> bool {
        self.method_ctx
            .get()
            .is_some_and(|m| self.methods[m].name == "<init>" && self.methods[m].owner == self.current.get())
    }

    /// The instance of class `target` (or of a subclass) that the code of the current class
    /// reaches: `this`, or the chain of `outer` fields to the enclosing class that is one.
    pub(super) fn enclosing_instance(&self, target: usize, pos: Pos) -> Result<Expr, Error> {
        let mut class = self.current.get();
        let mut value = self.synth(ExprKind::This, pos, self.self_type(class));
        let mut first = true;
        while !self.is_subclass(class, target) {
            let Some(nested) = self.nesting(class).filter(|n| n.outer) else {
                return error(pos, "no enclosing instance of this class here");
            };
            let enclosing = nested.enclosing;
            let ty = self.self_type(enclosing);
            value = if first && self.in_constructor() {
                self.synth(ExprKind::Var(OUTER.to_string()), pos, ty)
            } else {
                let kind = ExprKind::Field { object: Box::new(value), name: OUTER.to_string() };
                self.synth(kind, pos, ty)
            };
            class = enclosing;
            first = false;
        }
        Ok(value)
    }

    /// `this` for a member that `node` names without an object: the current class's, or that of
    /// the enclosing class that has the member.
    pub(super) fn implicit_this(&self, node: Node<'t>) -> Option<Result<Expr, Error>> {
        let symbol = node.symbol().filter(|s| !s.is_static && matches!(s.kind, "FIELD" | "METHOD"))?;
        let owner = self.type_index(symbol.owner)?;
        if self.is_subclass(self.current.get(), owner) {
            return None;
        }
        Some(self.enclosing_instance(owner, node.pos()))
    }

    /// `Outer.this` (`node`): the instance of the enclosing class `Outer`.
    pub(super) fn qualified_this(&self, node: Node<'t>) -> Option<Result<Expr, Error>> {
        if node.str("identifier") != "this" {
            return None;
        }
        let class = node.get("expression")?.symbol().and_then(|s| self.type_index(raw(s.ty)))?;
        Some(self.enclosing_instance(class, node.pos()))
    }
}

/// A class's name without its type arguments.
fn raw(name: &str) -> &str {
    name.split('<').next().unwrap_or(name)
}

/// Adds the names that the code declares (locals, parameters, catch and pattern variables).
fn declared_names(node: Node, out: &mut HashSet<String>) {
    match node.0 {
        Value::Object(fields) => {
            if matches!(node.kind(), "VARIABLE") {
                out.insert(node.str("name").to_string());
            }
            if crate::program::CLASS_KINDS.contains(&node.kind()) {
                return;
            }
            for (field, child) in fields {
                if field != "symbol" {
                    declared_names(Node(child), out);
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| declared_names(Node(item), out)),
        _ => {}
    }
}

impl<'t> Walk<'_, 't> {
    /// The code of the class: its methods, field initializers and initializer blocks, and what
    /// its superclass's constructor needs passed.
    fn class_code(&mut self) -> Result<(), Error> {
        let l = self.lowering;
        let info = &l.types[self.class];
        if let Some(s) = l.superclass(self.class)?
            && let Some(nested) = l.nesting(s)
        {
            for c in &nested.captures {
                self.free(c.name, c.node);
            }
            if nested.outer {
                self.need(nested.enclosing);
            }
        }
        for member in info.node.list("members").filter(|m| l.keeps(*m, &info.javac)) {
            match member.kind() {
                "METHOD" => {
                    self.scopes = vec![member.list("parameters").map(|p| p.str("name")).collect()];
                    if let Some(body) = member.get("body") {
                        self.walk(body)?;
                    }
                }
                "VARIABLE" if !enums::is_enum_constant(member) => {
                    self.scopes = vec![Vec::new()];
                    if let Some(init) = member.get("initializer") {
                        self.walk(init)?;
                    }
                }
                "BLOCK" => {
                    self.scopes = vec![Vec::new()];
                    self.walk(member)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn in_scope(&self, name: &str) -> bool {
        self.scopes.iter().any(|s| s.contains(&name))
    }

    fn declare(&mut self, name: &'t str) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.push(name);
        }
    }

    fn free(&mut self, name: &'t str, node: Node<'t>) {
        if !self.found.free.iter().any(|(n, _)| *n == name) {
            self.found.free.push((name, node));
        }
    }

    /// The code needs the instance of class `class`: its own, or an enclosing one.
    fn need(&mut self, class: usize) {
        if !self.lowering.is_subclass(self.class, class) && !self.found.needs.contains(&class) {
            self.found.needs.push(class);
        }
    }

    /// The code declares or creates an object of the nested class `class` at `site`: its
    /// captures that are not locals here are captured here too, and the instances it needs.
    fn uses_class(&mut self, class: usize, site: Option<Node<'t>>, declared_here: bool) -> Result<(), Error> {
        let l = self.lowering;
        let Some(nested) = l.nesting(class) else { return Ok(()) };
        for c in &nested.captures {
            if !self.in_scope(c.name) {
                self.free(c.name, c.node);
                if let Some(site) = site {
                    self.found.sites.push((site.0 as *const Value, c.name));
                }
            } else if !declared_here && let Some(site) = site {
                return unsupported(
                    site,
                    &format!("creating a local class where a local hides its capture `{}`", c.name),
                );
            }
        }
        if declared_here {
            // Declared in this code: its enclosing instance is this class's object, and what it
            // needs beyond that is needed here.
            for &n in &nested.needs {
                self.need(n);
            }
        } else if nested.outer {
            self.need(nested.enclosing);
        }
        Ok(())
    }

    fn walk(&mut self, node: Node<'t>) -> Result<(), Error> {
        match node.0 {
            Value::Array(items) => {
                for item in items {
                    self.walk(Node(item))?;
                }
                return Ok(());
            }
            Value::Object(_) => {}
            _ => return Ok(()),
        }
        let l = self.lowering;
        match node.kind() {
            "BLOCK" | "FOR_LOOP" | "SWITCH" | "SWITCH_EXPRESSION" | "CATCH" | "TRY" | "LAMBDA_EXPRESSION" => {
                self.scopes.push(Vec::new());
                if node.kind() == "CATCH" {
                    let name = node.get("parameter").map_or("", |p| p.str("name"));
                    self.declare(name);
                }
                if node.kind() == "LAMBDA_EXPRESSION" {
                    for p in node.list("parameters") {
                        self.declare(p.str("name"));
                    }
                }
                self.children(node)?;
                self.scopes.pop();
            }
            "ENHANCED_FOR_LOOP" => {
                if let Some(e) = node.get("expression") {
                    self.walk(e)?;
                }
                self.scopes.push(vec![node.get("variable").map_or("", |v| v.str("name"))]);
                if let Some(s) = node.get("statement") {
                    self.walk(s)?;
                }
                self.scopes.pop();
            }
            "VARIABLE" => {
                if let Some(init) = node.get("initializer") {
                    self.walk(init)?;
                }
                self.declare(node.str("name"));
            }
            "BINDING_PATTERN" => {
                if let Some(v) = node.get("variable") {
                    self.declare(v.str("name"));
                }
            }
            kind if crate::program::CLASS_KINDS.contains(&kind) => {
                if let Some(class) = l.type_index(&crate::program::class_name(node.0)) {
                    self.uses_class(class, None, true)?;
                }
            }
            "NEW_CLASS" => {
                for field in ["enclosingExpression", "arguments"] {
                    if let Some(child) = node.get(field) {
                        self.walk(child)?;
                    }
                }
                if let Some(body) = node.get("classBody") {
                    if let Some(class) = l.type_index(&crate::program::class_name(body.0)) {
                        self.uses_class(class, Some(node), true)?;
                    }
                } else if let Some(class) = node.symbol().and_then(|s| l.type_index(s.owner)) {
                    let declared_here = l.nesting(class).is_some_and(|n| n.enclosing == self.class);
                    let explicit = node.get("enclosingExpression").is_some();
                    let Some(nested) = l.nesting(class) else { return Ok(()) };
                    if nested.kind == inner::NestKind::Inner {
                        if nested.outer && !explicit {
                            self.need(nested.enclosing);
                        }
                    } else {
                        self.uses_class(class, Some(node), declared_here)?;
                    }
                }
            }
            "IDENTIFIER" => {
                let Some(symbol) = node.symbol() else { return Ok(()) };
                let name = node.str("name");
                if LOCAL_KINDS.contains(&symbol.kind) {
                    if !self.in_scope(name) {
                        self.free(name, node);
                        self.found.refs.push(node.0 as *const Value);
                    }
                } else if matches!(symbol.kind, "FIELD" | "METHOD")
                    && !symbol.is_static
                    && !matches!(name, "this" | "super")
                    && let Some(owner) = l.type_index(symbol.owner)
                {
                    self.need(owner);
                }
            }
            "MEMBER_SELECT" => {
                let class = node
                    .get("expression")
                    .and_then(|e| e.symbol())
                    .filter(|s| crate::program::CLASS_KINDS.contains(&s.kind));
                match (node.str("identifier"), class.and_then(|s| l.type_index(raw(s.ty)))) {
                    ("this", Some(class)) => self.need(class),
                    ("super", Some(class))
                        if l.types[class].node.kind() != "INTERFACE" && class != self.class =>
                    {
                        return unsupported(node, "`Outer.super`");
                    }
                    _ => self.children(node)?,
                }
            }
            _ => self.children(node)?,
        }
        Ok(())
    }

    /// The children of `node`, in the order the code runs where it declares locals that later
    /// children read (the tree's fields come in alphabetical order).
    fn children(&mut self, node: Node<'t>) -> Result<(), Error> {
        let Value::Object(fields) = node.0 else { return Ok(()) };
        let first: &[&str] = match node.kind() {
            "FOR_LOOP" => &["initializer", "condition", "update"],
            "TRY" => &["resources"],
            "CASE" => &["labels", "expressions", "guard"],
            _ => &[],
        };
        for field in first {
            if let Some(child) = node.get(field) {
                self.walk(child)?;
            }
        }
        for (field, child) in fields {
            if field != "symbol" && !first.contains(&field.as_str()) {
                self.walk(Node(child))?;
            }
        }
        Ok(())
    }
}
