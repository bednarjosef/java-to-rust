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
//! holds them, and named `<Interface>LambdaIn<Method>`. A lambda that captures a local variable
//! or the enclosing instance is rejected until captures exist.

use super::real_generics::Owner;
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
    /// The class whose code holds the lambda, and the method that does (a lambda's own method
    /// for a lambda inside a lambda); `None` in a field initializer or an initializer block.
    pub enclosing_class: usize,
    pub enclosing_method: Option<usize>,
}

/// The kinds of javac's symbols of local variables, which a lambda captures when it uses one
/// declared outside it.
const LOCAL_KINDS: &[&str] =
    &["LOCAL_VARIABLE", "PARAMETER", "EXCEPTION_PARAMETER", "BINDING_VARIABLE", "RESOURCE_VARIABLE"];

impl<'t> Lowering<'t> {
    /// Declares the lambdas and method references in the code of a member of class `class`
    /// (`method`, the member's index into `methods` when it is a method), in the order of the
    /// source. The code of nested, local and anonymous classes is not the member's.
    pub(super) fn declare_lambdas(&mut self, node: Node<'t>, class: usize, method: Option<usize>) {
        let simple = |name: &str| name.rsplit('.').next().unwrap_or(name).to_string();
        let place = match method.map(|m| self.methods[m].name) {
            Some(name) if name != "<init>" => capitalized(name),
            _ => simple(&self.types[class].name),
        };
        // In the order of the source, which numbers the names; a lambda inside another comes after
        // it, so its enclosing lambda's method is known.
        let mut found = Vec::new();
        find_lambdas(node.0, None, &mut found);
        found.sort_by_key(|(n, _)| (Node(n).pos().line, Node(n).pos().column));
        let mut methods: Vec<(&Value, usize)> = Vec::new();
        for (lambda, outer) in found {
            let enclosing = match outer {
                Some(o) => methods.iter().find(|(n, _)| std::ptr::eq(*n, o)).map(|&(_, m)| m),
                None => method,
            };
            let m = self.declare_lambda(Node(lambda), class, enclosing, &place);
            methods.push((lambda, m));
        }
    }

    /// Declares the class of one lambda; returns the index of its method.
    fn declare_lambda(&mut self, node: Node<'t>, class: usize, method: Option<usize>, place: &str) -> usize {
        let descriptor = Node(&node.0["descriptor"]);
        let target = descriptor.str("target");
        let interface = target.rsplit('.').next().unwrap_or(target);
        let package = self.package_of(class);
        let prefix = if package.is_empty() { String::new() } else { format!("{package}.") };
        let base = format!("{prefix}{interface}LambdaIn{place}");
        let mut name = base.clone();
        let mut n = 2;
        while self.types.iter().any(|t| t.name == name) {
            name = format!("{base}{n}");
            n += 1;
        }
        let index = self.types.len();
        self.types.push(TypeInfo { name, node });
        let ctor = self.methods.len();
        self.methods.push(MethodInfo { owner: index, node, name: "<init>", sig: "()void" });
        self.methods.push(MethodInfo {
            owner: index,
            node,
            name: descriptor.str("name"),
            sig: descriptor.str("substituted"),
        });
        self.lambdas.push(LambdaInfo {
            node,
            class: index,
            ctor,
            method: ctor + 1,
            enclosing_class: class,
            enclosing_method: method,
        });
        ctor + 1
    }

    /// The package of a class: its name without the classes it is nested in.
    fn package_of(&self, class: usize) -> String {
        let mut name = self.types[class].name.as_str();
        while let Some((outer, _)) = name.rsplit_once('.') {
            name = outer;
            if self.type_index(outer).is_none() {
                return outer.to_string();
            }
        }
        String::new()
    }

    /// The lambda whose class is `t`.
    pub(super) fn lambda_of_type(&self, t: usize) -> Option<&LambdaInfo<'t>> {
        self.lambdas.iter().find(|l| l.class == t)
    }

    /// The lambda whose constructor or method is `m`.
    fn lambda_of_method(&self, m: usize) -> Option<&LambdaInfo<'t>> {
        self.lambdas.iter().find(|l| l.ctor == m || l.method == m)
    }

    /// The class and method whose type variables are in scope in the code of class `class` and
    /// method `method`: those of the code that holds a lambda, for the lambda's own.
    pub(super) fn lambda_scope(&self, class: usize, method: Option<usize>) -> (usize, Option<usize>) {
        let lambda = method.and_then(|m| self.lambda_of_method(m)).or_else(|| self.lambda_of_type(class));
        match lambda {
            Some(l) => self.lambda_scope(l.enclosing_class, l.enclosing_method),
            None => (class, method),
        }
    }

    /// The declaration of a lambda's class: it implements the interface it converts to.
    pub(super) fn lambda_decl(&self, t: usize) -> Option<Result<TypeDecl, Error>> {
        let l = self.lambda_of_type(t)?;
        Some(self.lambda_interface(l).map(|interface| TypeDecl {
            name: self.types[t].name.clone(),
            package: String::new(),
            kind: TypeKind::Class,
            is_abstract: false,
            extends: None,
            implements: vec![interface],
            fields: Vec::new(),
            statics: Vec::new(),
            enum_constants: None,
            type_params: Vec::new(),
        }))
    }

    /// The functional interface a lambda converts to, an index into `types`.
    fn lambda_interface(&self, l: &LambdaInfo<'t>) -> Result<usize, Error> {
        let target = l.node.child("descriptor")?.str("target");
        match self.type_index(target) {
            Some(t) => Ok(t),
            None => unsupported(l.node, &format!("lambdas of `{target}`")),
        }
    }

    /// A lambda or method reference as an expression: a new object of its class, converted to
    /// the interface.
    pub(super) fn lambda(&self, node: Node<'t>) -> Result<ExprKind, Error> {
        let Some(l) = self.lambdas.iter().find(|l| std::ptr::eq(l.node.0, node.0)) else {
            return unsupported(node, "lambdas");
        };
        let ty = self.ty(node, node.java_type())?;
        let class_ty = Type::class(&self.types[l.class].name);
        let new = ExprKind::New { class: l.class, args: Vec::new(), ctor: Some(l.ctor) };
        Ok(ExprKind::Cast(ty, Box::new(self.synth(new, node.pos(), class_ty))))
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
            overrides,
            type_params: Vec::new(),
        };
        if index == l.ctor {
            return Some(Ok(method("<init>", Vec::new(), Type::Void, Vec::new(), Vec::new())));
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
        if let Some(pos) = self.captures(node) {
            return error(pos, "lambda captures not supported yet");
        }
        // The class and method that hold the lambda cannot be Rust generics: the lambda's class
        // is not one.
        if let (_, Some(m)) = self.lambda_scope(l.enclosing_class, l.enclosing_method)
            && self.is_real(Owner::Method(m))
        {
            self.fail(Owner::Method(m));
        }
        let (class, _) = self.lambda_scope(l.enclosing_class, None);
        if self.is_real(Owner::Class(class)) {
            self.fail(Owner::Class(class));
        }
        let descriptor = node.child("descriptor")?;
        let (param_types, ret) = signature(descriptor.str("substituted"));
        self.current.set(l.class);
        self.in_ctor.set(false);
        let ret = self.ty(node, ret)?;
        self.ret.set(ret);
        let params: Vec<Param> = if node.kind() == "LAMBDA_EXPRESSION" {
            node.list("parameters")
                .map(|p| {
                    Ok(Param {
                        name: p.str("name").to_string(),
                        ty: self.ty(p, p.symbol().map_or("", |s| s.ty))?,
                    })
                })
                .collect::<Result<_, Error>>()?
        } else {
            param_types
                .iter()
                .enumerate()
                .map(|(i, ty)| Ok(Param { name: param_name(i), ty: self.ty(node, ty)? }))
                .collect::<Result<_, Error>>()?
        };
        let body = match (node.kind(), node.str("bodyKind")) {
            ("LAMBDA_EXPRESSION", "STATEMENT") => self.block(node.child("body")?)?,
            ("LAMBDA_EXPRESSION", _) => self.tail(self.expr(node.child("body")?)?, ret),
            _ => {
                let call = self.reference_call(node, &param_types)?;
                self.tail(self.expr(Node(call))?, ret)
            }
        };
        let overridden = self.method_index(&Symbol {
            kind: "METHOD",
            name: descriptor.str("name"),
            owner: descriptor.str("owner"),
            is_static: false,
            varargs: false,
            ty: descriptor.str("type"),
            constant: None,
        });
        Ok((descriptor.str("name"), params, ret, body, overridden))
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
    /// constructor applied to the lambda's parameters `a`, `b`, ... (of the descriptor's types).
    fn reference_call(&self, node: Node<'t>, param_types: &[&str]) -> Result<&'t Value, Error> {
        let at = |mut v: Value| {
            v["line"] = node.0["line"].clone();
            v["column"] = node.0["column"].clone();
            v
        };
        let param = |i: usize| {
            let (name, ty) = (param_name(i), param_types[i]);
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
                let (receiver, first) = if unbound { (param(0), 1) } else { (qualifier.0.clone(), 0) };
                let args: Vec<Value> = (first..param_types.len()).map(param).collect();
                let select = at(json!({"kind": "MEMBER_SELECT", "identifier": node.str("name"),
                    "symbol": symbol, "expression": receiver}));
                at(json!({"kind": "METHOD_INVOCATION", "javaType": referent_ret, "symbol": symbol,
                    "methodSelect": select, "arguments": args}))
            }
        };
        Ok(Box::leak(Box::new(call)))
    }

    /// Where a lambda or method reference captures something (see `captured`), which is not
    /// supported yet. `None` when it captures nothing.
    fn captures(&self, node: Node<'t>) -> Option<Pos> {
        let constant_field = |owner: &str, name: &str| {
            self.type_index(owner).is_some_and(|t| {
                self.types[t].node.list("members").any(|m| {
                    m.kind() == "VARIABLE"
                        && m.str("name") == name
                        && m.has_flag("FINAL")
                        && m.has_flag("STATIC")
                })
            })
        };
        captured(node.0, &constant_field).map(|v| Node(v).pos())
    }
}

/// The node where a lambda uses a local variable declared outside it or the enclosing instance,
/// or where a method reference has a receiver that it evaluates once: a capture. `None` when it
/// captures nothing. `constant_field(owner, name)` tells a `static final` field, a receiver that
/// is the same object whenever it is evaluated, as a string literal, `System.out` and
/// `System.err` are.
pub fn captured<'t>(node: &'t Value, constant_field: &dyn Fn(&str, &str) -> bool) -> Option<&'t Value> {
    let node = Node(node);
    if node.kind() == "MEMBER_REFERENCE" && node.str("mode") == "INVOKE" {
        let qualifier = Node(&node.0["qualifierExpression"]);
        let is_type =
            qualifier.symbol().is_some_and(|s| matches!(s.kind, "CLASS" | "INTERFACE" | "ENUM" | "RECORD"));
        let constant = qualifier.kind() == "STRING_LITERAL"
            || qualifier.symbol().filter(|s| s.kind == "FIELD" && s.is_static).is_some_and(|s| {
                (s.owner == "java.lang.System" && matches!(s.name, "out" | "err"))
                    || constant_field(s.owner, s.name)
            });
        if !is_type && !constant {
            return Some(qualifier.0);
        }
    }
    let mut declared = HashSet::new();
    declarations(node.0, &mut declared);
    let mut found = None;
    uses_outside(node.0, &declared, &mut found);
    found
}

fn uses_outside<'t>(v: &'t Value, declared: &HashSet<&'t str>, found: &mut Option<&'t Value>) {
    if found.is_some() {
        return;
    }
    match v {
        Value::Object(map) => {
            let node = Node(v);
            let kind = node.kind();
            if matches!(kind, "CLASS" | "INTERFACE" | "ENUM" | "RECORD") {
                return;
            }
            let symbol = node.symbol();
            let captured = match kind {
                "IDENTIFIER" => {
                    let name = node.str("name");
                    matches!(name, "this" | "super")
                        || symbol.as_ref().is_some_and(|s| {
                            (LOCAL_KINDS.contains(&s.kind) && !declared.contains(name))
                                || (s.kind == "FIELD" && !s.is_static)
                        })
                }
                // An unqualified call of an instance method runs on the enclosing instance.
                "METHOD_INVOCATION" => {
                    let select = Node(&map["methodSelect"]);
                    select.kind() == "IDENTIFIER"
                        && select.symbol().is_some_and(|s| s.kind == "METHOD" && !s.is_static)
                }
                _ => false,
            };
            if captured {
                *found = Some(v);
                return;
            }
            for (key, child) in map {
                if key != "symbol" && key != "descriptor" && key != "classBody" {
                    uses_outside(child, declared, found);
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| uses_outside(item, declared, found)),
        _ => {}
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

/// The names of the variables declared in a lambda: its parameters and its locals.
fn declarations<'t>(v: &'t Value, out: &mut HashSet<&'t str>) {
    match v {
        Value::Object(map) => {
            if map.get("kind").and_then(Value::as_str) == Some("VARIABLE")
                && let Some(name) = map.get("name").and_then(Value::as_str)
            {
                out.insert(name);
            }
            map.values().for_each(|child| declarations(child, out));
        }
        Value::Array(items) => items.iter().for_each(|item| declarations(item, out)),
        _ => {}
    }
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

/// The name of a method reference's parameter `i`: `a`, `b`, ...
fn param_name(i: usize) -> String {
    match i {
        0..26 => ((b'a' + i as u8) as char).to_string(),
        _ => format!("p{i}"),
    }
}

/// `main` as `Main`.
fn capitalized(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}
