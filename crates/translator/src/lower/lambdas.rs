//! Lambdas and method references. Each one is a class of the program, as javac desugars it: a
//! synthetic concrete class that implements the functional interface it converts to, with a
//! constructor and one method, the interface's abstract method (javac's `descriptor`, which
//! `JavaTree` prints with the node). The lambda expression creates an object of the class and
//! converts it to the interface, so the object model sees a class like any other: it is a
//! variant of the interface's enum, a call of the interface's method dispatches to its body, and
//! the analyses read its body as a method of the override group.
//!
//! The method takes the descriptor's types after javac's substitution (`apply(Integer)` for a
//! `Function<Integer, Integer>`); for a generic interface, whose method is declared with the
//! erased types, that is an override with another signature, which gets its bridge as any other
//! (see `object::bridges`). A method reference's body is its one call: the referenced method on
//! the parameters (static), on the first parameter (unbound: `String::length`), on its receiver
//! (bound: `System.out::println`), or a `new` of the class or array. That call is lowered as if
//! javac had written it, from a tree made here, so that it gets every conversion and every
//! translation of a library method that a call in the program gets.
//!
//! The classes are declared before anything is lowered, with the members of the class whose code
//! holds them, and named `<Interface>LambdaIn<Method>`. They are nested classes like local ones
//! (`NestKind::Lambda`): what a lambda captures is found by `captures` as for a local class, held
//! in the fields of its object and passed to its constructor by the expression that creates it,
//! and `this`, or a member of the enclosing class named without it, is the enclosing instance,
//! held in the field `outer` (see `inner`). A bound method reference's receiver is its capture:
//! a local, `this`, or a final field read through it. Any other receiver (`a.b()::c`, a field
//! that is not final) is evaluated once, where the reference is created, as Java does: into a
//! synthetic local of the creating code, checked for null there, which the class captures (see
//! `plan_receivers`).

use super::inner::NestKind;
use super::*;
use serde_json::json;
use std::collections::HashSet;

/// A lambda or method reference of the code being lowered, and its synthetic class.
pub(super) struct LambdaInfo<'t> {
    pub node: Node<'t>,
    /// The synthetic class, an index into `types`.
    pub class: usize,
    /// Its constructor and its method, indexes into `methods`.
    pub ctor: usize,
    pub method: usize,
    /// The names of the method's parameters.
    pub params: Vec<&'t str>,
    /// The method's code: a lambda's body, or the call that a method reference makes, as a tree
    /// made here (see `reference_call`).
    pub code: Node<'t>,
    /// The member of a class whose code holds the lambda (a method, a field, an initializer).
    pub member: Node<'t>,
    /// For a method reference whose receiver is computed: the name of the synthetic local that
    /// holds the receiver's value, and the receiver.
    pub receiver: Option<(&'t str, Node<'t>)>,
}

/// The kinds of javac's symbols of local variables.
const LOCAL_KINDS: &[&str] =
    &["LOCAL_VARIABLE", "PARAMETER", "EXCEPTION_PARAMETER", "BINDING_VARIABLE", "RESOURCE_VARIABLE"];

impl<'t> Lowering<'t> {
    /// Declares the lambdas and method references in the code of `member`, a member of class
    /// `class` (`method`, the member's index into `methods` when it is a method), in the order of
    /// the source. The code of nested, local and anonymous classes is not the member's.
    pub(super) fn declare_lambdas(
        &mut self,
        member: Node<'t>,
        class: usize,
        method: Option<usize>,
    ) -> Result<(), Error> {
        let simple = |name: &str| name.rsplit('.').next().unwrap_or(name).to_string();
        let place = match method.map(|m| self.methods[m].name) {
            Some(name) if name != "<init>" => capitalized(name),
            _ => simple(&self.types[class].name),
        };
        let instance = !(member.has_flag("STATIC") || Self::is_static_block(member));
        // In the order of the source, which numbers the names; a lambda inside another comes after
        // it, so the class and method of its enclosing lambda are known.
        let mut found = Vec::new();
        find_lambdas(member.0, None, &mut found);
        found.sort_by_key(|(n, _)| (Node(n).pos().line, Node(n).pos().column));
        let mut declared: Vec<(&Value, usize)> = Vec::new();
        for (lambda, outer) in found {
            let enclosing = outer.and_then(|o| declared.iter().find(|(n, _)| std::ptr::eq(*n, o)));
            let (enclosing, method, instance) = match enclosing {
                Some(&(_, l)) => (self.lambdas[l].class, Some(self.lambdas[l].method), true),
                None => (class, method, instance),
            };
            self.declare_lambda(Node(lambda), member, enclosing, method, instance, &place)?;
            declared.push((lambda, self.lambdas.len() - 1));
        }
        Ok(())
    }

    /// Declares the class of one lambda, nested in class `class` and method `method`.
    fn declare_lambda(
        &mut self,
        node: Node<'t>,
        member: Node<'t>,
        class: usize,
        method: Option<usize>,
        instance: bool,
        place: &str,
    ) -> Result<(), Error> {
        let descriptor = node.child("descriptor")?;
        let target = descriptor.str("target");
        // A local interface's binary name has its number before its simple name.
        let interface = target.rsplit(['.', '$']).next().unwrap_or(target);
        let interface = interface.trim_start_matches(|c: char| c.is_ascii_digit());
        let base = format!("{}{interface}LambdaIn{place}", self.package_of(class));
        let mut name = base.clone();
        let mut n = 2;
        while self.types.iter().any(|t| t.name == name) {
            name = format!("{base}{n}");
            n += 1;
        }
        let (param_types, _) = signature(descriptor.str("substituted"));
        let (params, code) = if node.kind() == "LAMBDA_EXPRESSION" {
            (node.list("parameters").map(|p| p.str("name")).collect(), node.child("body")?)
        } else {
            let taken = names(&node.0["qualifierExpression"]);
            let params = param_names(param_types.len(), &taken);
            let call = self.reference_call(node, &param_types, &params, None)?;
            (params, Node(call))
        };
        let index = self.types.len();
        let nested = self.nested(NestKind::Lambda, class, method, instance, Some(node));
        self.types.push(TypeInfo { name: name.clone(), javac: name, node, nested: Some(nested) });
        let ctor = self.methods.len();
        self.methods.push(MethodInfo { owner: index, node, name: "<init>", sig: "()void" });
        self.methods.push(MethodInfo {
            owner: index,
            node,
            name: descriptor.str("name"),
            sig: descriptor.str("substituted"),
        });
        let lambda =
            LambdaInfo { node, class: index, ctor, method: ctor + 1, params, code, member, receiver: None };
        self.lambdas.push(lambda);
        Ok(())
    }

    /// Gives each method reference whose receiver is computed (see `computed_receiver`) a
    /// synthetic local for the receiver's value, which its call reads: the code that creates the
    /// reference evaluates the receiver into it, and the class captures it. Runs once every class
    /// is declared, which telling a final field needs. An error comes with the index of the class
    /// it is in.
    pub(super) fn plan_receivers(&mut self) -> Result<(), (usize, Error)> {
        let mut taken: Vec<(*const Value, HashSet<&'t str>)> = Vec::new();
        for i in 0..self.lambdas.len() {
            let l = &self.lambdas[i];
            let Some(qualifier) =
                computed_receiver(l.node.0, &|owner, name| self.is_final_field(owner, name))
            else {
                continue;
            };
            let (node, member, class) = (l.node, l.member, l.class);
            // A name that the member's code does not use, nor the reference's parameters.
            let at = match taken.iter().position(|(m, _)| std::ptr::eq(*m, member.0)) {
                Some(at) => at,
                None => {
                    taken.push((member.0, names(member.0)));
                    taken.len() - 1
                }
            };
            let names = &mut taken[at].1;
            let base = "receiver";
            let name = (1..)
                .map(|n| if n == 1 { base.to_string() } else { format!("{base}{n}") })
                .find(|n| !names.contains(n.as_str()) && !l.params.contains(&n.as_str()))
                .expect("a free name");
            let name: &'t str = Box::leak(name.into_boxed_str());
            names.insert(name);
            let descriptor = node.child("descriptor").map_err(|e| (class, e))?;
            let (param_types, _) = signature(descriptor.str("substituted"));
            let params = l.params.clone();
            let code =
                self.reference_call(node, &param_types, &params, Some(name)).map_err(|e| (class, e))?;
            let l = &mut self.lambdas[i];
            l.code = Node(code);
            l.receiver = Some((name, Node(qualifier)));
        }
        Ok(())
    }

    /// The synthetic local and the receiver of the method reference `node`, when its receiver is
    /// computed (see `plan_receivers`).
    pub(super) fn lambda_receiver(&self, node: Node<'t>) -> Option<(&'t str, Node<'t>)> {
        self.lambdas.iter().find(|l| std::ptr::eq(l.node.0, node.0)).and_then(|l| l.receiver)
    }

    /// The lambda whose class is `t`.
    pub(super) fn lambda_of_type(&self, t: usize) -> Option<&LambdaInfo<'t>> {
        self.lambdas.iter().find(|l| l.class == t)
    }

    /// The lambda whose constructor or method is `m`.
    pub(super) fn lambda_of_method(&self, m: usize) -> Option<&LambdaInfo<'t>> {
        self.lambdas.iter().find(|l| l.ctor == m || l.method == m)
    }

    /// The class of the lambda or method reference `node`.
    pub(super) fn lambda_class(&self, node: Node<'t>) -> Option<usize> {
        self.lambdas.iter().find(|l| std::ptr::eq(l.node.0, node.0)).map(|l| l.class)
    }

    /// The code of the class `t` when it is a lambda's: the names of its method's parameters,
    /// and its body.
    pub(super) fn lambda_code(&self, t: usize) -> Option<(Vec<&'t str>, Node<'t>)> {
        self.lambda_of_type(t).map(|l| (l.params.clone(), l.code))
    }

    /// The class whose code holds lambda class `t`, outside any lambda: whose object `this` is
    /// in the lambda.
    pub(super) fn lambda_holder(&self, t: usize) -> usize {
        match self.nesting(t) {
            Some(n) if n.kind == NestKind::Lambda => self.lambda_holder(n.enclosing),
            _ => t,
        }
    }

    /// `this` (`node`) in the code of a lambda: the object of the class that holds it; `None`
    /// outside lambdas.
    pub(super) fn lambda_this(&self, node: Node<'t>) -> Option<Result<Expr, Error>> {
        let current = self.current.get();
        self.lambda_of_type(current)?;
        Some(self.enclosing_instance(self.lambda_holder(current), node.pos()))
    }

    /// The declaration of a lambda's class: it implements the interface it converts to, and has
    /// the fields that hold its captures.
    pub(super) fn lambda_decl(&self, t: usize) -> Option<Result<TypeDecl, Error>> {
        let l = self.lambda_of_type(t)?;
        let decl = || {
            Ok(TypeDecl {
                name: self.types[t].name.clone(),
                package: String::new(),
                kind: TypeKind::Class,
                is_abstract: false,
                extends: None,
                // A census's stub of the class implements nothing.
                implements: match self.lambda_interface(l) {
                    Err(_) if self.stubbing.get() => Vec::new(),
                    interface => vec![interface?],
                },
                fields: self.synthetic_fields(t)?,
                statics: Vec::new(),
                enum_constants: None,
                type_params: Vec::new(),
                outer: self.nesting(t).filter(|n| n.outer).map(|n| n.enclosing),
                captures: self.captures_of(t)?,
                origin: self.origin(t),
                cloneable: false,
            })
        };
        self.current.set(t);
        Some(decl())
    }

    /// The functional interface a lambda converts to, an index into `types`.
    fn lambda_interface(&self, l: &LambdaInfo<'t>) -> Result<usize, Error> {
        let target = l.node.child("descriptor")?.str("target");
        match self.type_index(target) {
            Some(t) => Ok(t),
            None => unsupported(l.node, &format!("lambdas of `{target}`")),
        }
    }

    /// A lambda or method reference as an expression: a new object of its class, given what it
    /// captures, converted to the interface.
    pub(super) fn lambda(&self, node: Node<'t>) -> Result<ExprKind, Error> {
        let Some(l) = self.lambdas.iter().find(|l| std::ptr::eq(l.node.0, node.0)) else {
            return unsupported(node, "lambdas");
        };
        let ty = self.ty(node, node.java_type())?;
        let class_ty = Type::class(&self.types[l.class].name);
        let pos = node.pos();
        // A computed receiver is evaluated first, into its local, and checked for null.
        let receiver = match l.receiver {
            Some((name, qualifier)) => {
                let value = self.expr(qualifier)?;
                let capture = self.nesting(l.class).and_then(|n| n.captures.iter().find(|c| c.name == name));
                let local_ty = match capture {
                    Some(c) => self.capture_type(c)?,
                    None => value.ty,
                };
                let value = self.convert(value, local_ty);
                let var = || self.synth(ExprKind::Var(name.to_string()), pos, local_ty);
                let checked = self.synth(ExprKind::Builtin(Builtin::NullChecked, vec![var()]), pos, local_ty);
                let local = Stmt::Local { ty: local_ty, name: name.to_string(), init: Some(value), pos };
                Some(vec![local, Stmt::Expr(checked)])
            }
            None => None,
        };
        let args = self.with_synthetic(self.creation_args(node, l.class)?, Vec::new());
        let new = ExprKind::New { class: l.class, args, ctor: Some(l.ctor) };
        let created = ExprKind::Cast(ty, Box::new(self.synth(new, pos, class_ty)));
        let Some(mut block) = receiver else { return Ok(created) };
        block.push(Stmt::Yield(self.synth(created, pos, ty)));
        Ok(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body: block }],
        })
    }

    /// The constructor or method of a lambda's class, when `index` is one.
    pub(super) fn lambda_member(&self, index: usize) -> Option<Result<Method, Error>> {
        let l = self.lambda_of_method(index)?;
        let method = |name: &str, params, ret, body, overrides| Method {
            name: name.to_string(),
            params,
            ret,
            body,
            is_main: false,
            is_static: false,
            is_ctor: index == l.ctor,
            class: l.class,
            pos: l.node.pos(),
            is_abstract: false,
            is_synchronized: false,
            overrides,
            type_params: Vec::new(),
        };
        self.current.set(l.class);
        self.in_ctor.set(index == l.ctor);
        if index == l.ctor {
            // It sets the fields of what the lambda captures from its parameters.
            let ctor = || {
                let (first, last) = self.synthetic_params(l.class)?;
                let body = self.synthetic_statements(l.class)?;
                Ok(method("<init>", first.into_iter().chain(last).collect(), Type::Void, body, Vec::new()))
            };
            return Some(ctor());
        }
        Some(self.lambda_body(l).map(|(name, params, ret, body, overridden)| {
            method(name, params, ret, body, overridden.into_iter().collect())
        }))
    }

    /// The parts of a lambda's method: its name, parameters, return type, body and the abstract
    /// method it implements.
    #[allow(clippy::type_complexity)]
    fn lambda_body(
        &self,
        l: &LambdaInfo<'t>,
    ) -> Result<(&'t str, Vec<Param>, Type, Vec<Stmt>, Option<usize>), Error> {
        let node = l.node;
        let stub = self.stubbing.get();
        if let Some(sup) = uses_super(l.code.0).filter(|_| !stub) {
            return unsupported(Node(sup), "`super` in a lambda");
        }
        let descriptor = node.child("descriptor")?;
        let (param_types, ret) = signature(descriptor.str("substituted"));
        let ret = self.signature_type(node, ret)?;
        self.ret.set(ret);
        let params: Vec<Param> = if node.kind() == "LAMBDA_EXPRESSION" {
            node.list("parameters")
                .map(|p| {
                    Ok(Param {
                        name: p.str("name").to_string(),
                        ty: self.signature_type(p, p.symbol().map_or("", |s| s.ty))?,
                    })
                })
                .collect::<Result<_, Error>>()?
        } else {
            param_types
                .iter()
                .zip(&l.params)
                .map(|(ty, name)| Ok(Param { name: name.to_string(), ty: self.signature_type(node, ty)? }))
                .collect::<Result<_, Error>>()?
        };
        let body = match (node.kind(), node.str("bodyKind")) {
            _ if stub => self.never(l.class, node.pos()),
            ("LAMBDA_EXPRESSION", "STATEMENT") => self.block(l.code)?,
            _ => self.tail(self.expr(l.code)?, ret),
        };
        let overridden = self.method_index(&Symbol {
            kind: "METHOD",
            name: descriptor.str("name"),
            owner: descriptor.str("owner"),
            is_static: false,
            varargs: false,
            native: false,
            ty: descriptor.str("type"),
            constant: None,
        });
        Ok((descriptor.str("name"), params, ret, body, overridden))
    }

    /// Whether `owner.name` is a final field of the program.
    fn is_final_field(&self, owner: &str, name: &str) -> bool {
        self.type_index(owner).is_some_and(|t| {
            self.types[t]
                .node
                .list("members")
                .any(|m| m.kind() == "VARIABLE" && m.str("name") == name && m.has_flag("FINAL"))
        })
    }

    /// The body of a lambda whose body is the expression `e`: it returns its value, converted to
    /// the method's return type, or evaluates it when the method returns nothing.
    fn tail(&self, e: Expr, ret: Type) -> Vec<Stmt> {
        if ret == Type::Void {
            vec![Stmt::Expr(e)]
        } else {
            let pos = e.pos;
            vec![Stmt::Return(Some(self.convert(e, ret)), pos)]
        }
    }

    /// The call that a method reference makes, as javac's tree of it: the referenced method or
    /// constructor applied to the lambda's parameters `params` (of the descriptor's types).
    /// `receiver`, when set, names the local that holds a computed receiver's value, which the
    /// call reads in place of the receiver.
    fn reference_call(
        &self,
        node: Node<'t>,
        param_types: &[&str],
        params: &[&str],
        receiver: Option<&str>,
    ) -> Result<&'t Value, Error> {
        let at = |mut v: Value| {
            v["line"] = node.0["line"].clone();
            v["column"] = node.0["column"].clone();
            v
        };
        let param = |i: usize| {
            let (name, ty) = (params[i], param_types[i]);
            at(json!({"kind": "IDENTIFIER", "name": name, "javaType": ty,
                "symbol": {"kind": "PARAMETER", "name": name, "owner": "", "static": false, "type": ty}}))
        };
        let qualifier = node.child("qualifierExpression")?;
        let symbol = node.0["symbol"].clone();
        let (_, referent_ret) = signature(node.child("descriptor")?.str("referent"));
        let (_, descriptor_ret) = signature(node.child("descriptor")?.str("substituted"));
        let unbound = !symbol["static"].as_bool().unwrap_or(false)
            && qualifier
                .symbol()
                .is_some_and(|s| matches!(s.kind, "CLASS" | "INTERFACE" | "ENUM" | "RECORD"));
        let call = match node.str("mode") {
            "NEW" if qualifier.kind() == "ARRAY_TYPE" => at(json!({"kind": "NEW_ARRAY",
                "javaType": qualifier.java_type(), "type": qualifier.0["type"].clone(),
                "dimensions": [param(0)]})),
            "NEW" => {
                // The class the descriptor returns when it is the referenced one, with its type
                // arguments; the qualifier's type otherwise.
                let raw = |t: &str| t.split('<').next().unwrap_or_default().to_string();
                let created = if raw(descriptor_ret) == raw(qualifier.java_type()) {
                    descriptor_ret
                } else {
                    qualifier.java_type()
                };
                let args: Vec<Value> = (0..param_types.len()).map(param).collect();
                at(json!({"kind": "NEW_CLASS", "javaType": created, "symbol": symbol,
                    "identifier": qualifier.0.clone(), "arguments": args}))
            }
            _ => {
                let bound = match receiver {
                    Some(name) => {
                        let ty = qualifier.java_type();
                        at(json!({"kind": "IDENTIFIER", "name": name, "javaType": ty, "symbol":
                            {"kind": "LOCAL_VARIABLE", "name": name, "owner": "", "static": false, "type": ty}}))
                    }
                    None => qualifier.0.clone(),
                };
                let (receiver, first) = if unbound { (param(0), 1) } else { (bound, 0) };
                let args: Vec<Value> = (first..param_types.len()).map(param).collect();
                let select = at(json!({"kind": "MEMBER_SELECT", "identifier": node.str("name"),
                    "symbol": symbol, "expression": receiver}));
                at(json!({"kind": "METHOD_INVOCATION", "javaType": referent_ret, "symbol": symbol,
                    "methodSelect": select, "arguments": args}))
            }
        };
        Ok(Box::leak(Box::new(call)))
    }
}

/// The receiver of a bound method reference (`node`) that the reference cannot take as a
/// capture: one other than a string literal, a local, `this`, `System.out`, `System.err` or a
/// field that `final_field(owner, name)` says is final, read on `this` or a class. javac
/// evaluates the receiver once, where the reference is; the translation evaluates a capture's
/// value there too, and a final field gives the same value on every read. `None` for every
/// other node.
pub fn computed_receiver<'t>(node: &'t Value, final_field: &dyn Fn(&str, &str) -> bool) -> Option<&'t Value> {
    let node = Node(node);
    if node.kind() != "MEMBER_REFERENCE" || node.str("mode") != "INVOKE" {
        return None;
    }
    let qualifier = Node(&node.0["qualifierExpression"]);
    let is_type =
        |n: Node| n.symbol().is_some_and(|s| matches!(s.kind, "CLASS" | "INTERFACE" | "ENUM" | "RECORD"));
    let field = |n: Node| {
        n.symbol().filter(|s| s.kind == "FIELD").is_some_and(|s| {
            (s.owner == "java.lang.System" && matches!(s.name, "out" | "err")) || final_field(s.owner, s.name)
        })
    };
    let takes = match qualifier.kind() {
        "STRING_LITERAL" => true,
        _ if is_type(qualifier) => true,
        // `super::m` and `Outer.super::m` call the method on the enclosing object, as a lambda's
        // `super.m()` does.
        "IDENTIFIER" => {
            matches!(qualifier.str("name"), "this" | "super")
                || qualifier.symbol().is_some_and(|s| LOCAL_KINDS.contains(&s.kind))
                || field(qualifier)
        }
        // A class literal is a constant, and never null.
        "MEMBER_SELECT" if matches!(qualifier.str("identifier"), "super" | "class") => true,
        "MEMBER_SELECT" => {
            let on = Node(&qualifier.0["expression"]);
            field(qualifier) && ((on.kind() == "IDENTIFIER" && on.str("name") == "this") || is_type(on))
        }
        _ => false,
    };
    (!takes).then_some(qualifier.0)
}

/// The first `super` in a lambda's code, outside the classes it declares, that is not the
/// receiver of a method call (`super.m()`, which runs on the object of the class whose code holds
/// the lambda, as javac's synthetic method does) other than `super.clone()`.
fn uses_super(v: &Value) -> Option<&Value> {
    match v {
        Value::Object(map) => {
            let kind = map.get("kind").and_then(Value::as_str).unwrap_or_default();
            if crate::program::CLASS_KINDS.contains(&kind) {
                return None;
            }
            if kind == "IDENTIFIER" && map.get("name").and_then(Value::as_str) == Some("super") {
                return Some(v);
            }
            if kind == "MEMBER_SELECT"
                && Node(v).symbol().is_some_and(|s| s.kind == "METHOD" && s.name != "clone")
                && Node(&v["expression"]).kind() == "IDENTIFIER"
                && v["expression"]["name"] == "super"
            {
                return None;
            }
            map.iter().filter(|(k, _)| *k != "symbol" && *k != "classBody").find_map(|(_, c)| uses_super(c))
        }
        Value::Array(items) => items.iter().find_map(uses_super),
        _ => None,
    }
}

/// The lambdas and method references in `v`, each with the one it is inside (`outer`), if any.
/// The code of nested, local and anonymous classes is left out.
fn find_lambdas<'t>(v: &'t Value, outer: Option<&'t Value>, out: &mut Vec<(&'t Value, Option<&'t Value>)>) {
    match v {
        Value::Object(map) => {
            let kind = map.get("kind").and_then(Value::as_str).unwrap_or_default();
            if matches!(kind, "CLASS" | "INTERFACE" | "ENUM" | "RECORD") {
                return;
            }
            let mut inner = outer;
            if matches!(kind, "LAMBDA_EXPRESSION" | "MEMBER_REFERENCE") && map.contains_key("descriptor") {
                out.push((v, outer));
                inner = Some(v);
            }
            for (key, child) in map {
                if key != "classBody" {
                    find_lambdas(child, inner, out);
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| find_lambdas(item, outer, out)),
        _ => {}
    }
}

/// The names that a tree mentions.
fn names(v: &Value) -> HashSet<&str> {
    fn walk<'v>(v: &'v Value, out: &mut HashSet<&'v str>) {
        match v {
            Value::Object(map) => {
                if let Some(name) = map.get("name").and_then(Value::as_str) {
                    out.insert(name);
                }
                map.values().for_each(|child| walk(child, out));
            }
            Value::Array(items) => items.iter().for_each(|item| walk(item, out)),
            _ => {}
        }
    }
    let mut out = HashSet::new();
    walk(v, &mut out);
    out
}

/// The parameter types and the return type of javac's method type `(int,java.lang.String)int`.
fn signature(ty: &str) -> (Vec<&str>, &str) {
    let Some(close) = top_level_close(ty) else { return (Vec::new(), ty) };
    let params = &ty[1..close];
    let mut out = Vec::new();
    let (mut depth, mut start) = (0, 0);
    for (i, c) in params.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => depth -= 1,
            ',' if depth == 0 => {
                out.push(params[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    if !params.trim().is_empty() {
        out.push(params[start..].trim());
    }
    (out, &ty[close + 1..])
}

/// The index of the `)` that closes the parameters of a method type.
fn top_level_close(ty: &str) -> Option<usize> {
    if !ty.starts_with('(') {
        return None;
    }
    let mut depth = 0;
    for (i, c) in ty.char_indices() {
        match c {
            '(' | '<' => depth += 1,
            '>' => depth -= 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// The names of a method reference's `n` parameters: `a`, `b`, ..., skipping those that its
/// receiver mentions.
fn param_names(n: usize, taken: &HashSet<&str>) -> Vec<&'static str> {
    let mut out = Vec::new();
    let mut i = 0;
    while out.len() < n {
        let name = match i {
            0..26 => ((b'a' + i as u8) as char).to_string(),
            _ => format!("p{i}"),
        };
        if !taken.contains(name.as_str()) {
            out.push(&*Box::leak(name.into_boxed_str()));
        }
        i += 1;
    }
    out
}

/// `main` as `Main`.
fn capitalized(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}
