//! Enum constants with bodies (`PLUS { int apply(int a, int b) { return a + b; } }`). Each such
//! body is an anonymous subclass that overrides methods of the enum; an enum is a Rust `enum`, so
//! the overrides become arms of a `switch` on `this` in the enum's own method: the constants that
//! override it run their body, the others the enum's (an abstract method has none, as every
//! constant overrides it). A body may only override the enum's methods, or the methods of an
//! interface that the enum leaves to its constants: the enum gets a method of its own for each of
//! those (`body_methods`), whose body is the interface's default method.

use super::*;
use enums::{is_enum_constant, pure, subst};

/// A constant's ordinal, and the statements of its body's override of a method.
type Override = (usize, Vec<Stmt>);

/// The parameters of a javac signature, `(int)` of `(int)void`: a body's method overrides the
/// enum's of the same name and parameters, whatever type it narrows the result to.
fn params_of(sig: &str) -> &str {
    sig.find(')').map_or(sig, |end| &sig[..=end])
}

/// Whether a case's statements always leave it, so that it needs no `break`.
fn leaves(body: &[Stmt]) -> bool {
    matches!(body.last(), Some(Stmt::Return(..) | Stmt::Break | Stmt::Continue))
}

/// `body` as the statements of a case: a `break` ends it unless it leaves already.
fn case_body(mut body: Vec<Stmt>) -> Vec<Stmt> {
    if !leaves(&body) {
        body.push(Stmt::Break);
    }
    body
}

impl<'t> Lowering<'t> {
    /// Declares a method of enum `class` for each method of its constants' bodies that overrides
    /// none of the enum's own, as one that implements an interface's: the constants that do not
    /// override it run the interface's default method (`interface_default`), which Java requires
    /// when the interface's is abstract. It is declared with the first body's declaration, for its
    /// signature, but with the interface's result type where bodies narrow it differently
    /// (`body_returns`). A method that implements no interface's is a helper of its body, declared as a
    /// method of the enum with the body's code (`body_helpers`).
    pub(super) fn body_methods(&mut self, class: usize) {
        let constants: Vec<Node<'t>> =
            self.types[class].node.list("members").filter(|m| is_enum_constant(*m)).collect();
        for constant in constants {
            let Some(body) = constant.get("initializer").and_then(|i| i.get("classBody")) else { continue };
            for member in body.list("members").filter(|m| m.kind() == "METHOD" && m.str("name") != "<init>") {
                let (name, sig) = (member.str("name"), member.symbol().map_or("", |s| s.ty));
                let declared = self
                    .methods
                    .iter()
                    .position(|m| m.owner == class && m.name == name && params_of(m.sig) == params_of(sig));
                // `Object`'s and `Enum`'s methods are no interface's.
                let overrides = member.get("symbol").and_then(|s| s.0["overrides"].as_array());
                let interface_sig = overrides.into_iter().flatten().find_map(|o| match o["owner"].as_str() {
                    None | Some("java.lang.Object" | "java.lang.Enum") => None,
                    Some(_) => o["type"].as_str(),
                });
                let of_interface = interface_sig.is_some();
                let overrides_none = overrides.is_none_or(|o| o.is_empty());
                // Bodies that narrow the interface's result differently (`Double readNumber` and
                // `Number readNumber` of Gson's `ToNumberPolicy`) share one method, which returns
                // the interface's type.
                if let Some(first) = declared
                    && self.body_methods.contains(&first)
                    && self.methods[first].sig != sig
                    && let Some(interface_sig) = interface_sig
                {
                    let ret = &interface_sig[params_of(interface_sig).len()..];
                    self.body_returns.insert(first, ret);
                }
                if declared.is_some() || member.has_flag("STATIC") || !of_interface && !overrides_none {
                    continue;
                }
                self.methods.push(MethodInfo { owner: class, node: member, name, sig });
                if of_interface {
                    self.body_methods.insert(self.methods.len() - 1);
                } else {
                    // A method that overrides nothing, which only its body's code calls: a method
                    // of the enum, which those calls resolve to (`constant_body_call`).
                    self.body_helpers.insert(self.methods.len() - 1);
                }
            }
        }
    }

    /// javac's name of the result type of method `index`, whose return type node is `ret`: the
    /// declared one, but the interface's for a method of `body_methods` that the bodies narrow
    /// differently (`body_returns`).
    pub(super) fn return_java_type(&self, index: usize, ret: Node<'t>) -> &'t str {
        self.body_returns.get(&index).copied().unwrap_or_else(|| ret.java_type())
    }

    /// The method of `body_methods` of the enum named `e` that implements the interface's method
    /// `resolved`.
    pub(super) fn body_method(&self, e: &str, resolved: Option<usize>) -> Option<usize> {
        let resolved = resolved?;
        let e = self.enum_index(e)?;
        self.body_methods.iter().copied().find(|&m| {
            self.methods[m].owner == e
                && self.methods[m].name == self.methods[resolved].name
                && self.overrides(self.methods[m].node).contains(&resolved)
        })
    }

    /// The body of method `index` of `body_methods`: a call of the default method of an
    /// interface that it overrides, on `this`, with its parameters; nothing when the interface's
    /// method is abstract, as every constant overrides it then.
    pub(super) fn interface_default(
        &self,
        index: usize,
        params: &[Param],
        ret: Type,
    ) -> Result<Vec<Stmt>, Error> {
        let node = self.methods[index].node;
        let pos = node.pos();
        let default = self.overrides(node).into_iter().find(|&m| {
            let owner = self.methods[m].owner;
            self.types[owner].node.kind() == "INTERFACE" && self.methods[m].node.get("body").is_some()
        });
        let Some(default) = default else { return Ok(Vec::new()) };
        let owner = self.methods[default].owner;
        let this = self.synth(ExprKind::This, pos, Type::class(&self.types[owner].name));
        let args = params.iter().map(|p| self.synth(ExprKind::Var(p.name.clone()), pos, p.ty)).collect();
        let args = self.convert_args(Some(default), args)?;
        let call = ExprKind::MethodCall {
            receiver: Box::new(this),
            name: self.methods[default].name.to_string(),
            args,
            resolved: Some(default),
            is_super: true,
        };
        let declared = self.methods[default].node.child("returnType")?;
        let returned = self.signature_type(declared, declared.java_type())?;
        let call = self.synth(call, pos, returned);
        Ok(vec![if ret == Type::Void {
            Stmt::Expr(call)
        } else {
            Stmt::Return(Some(self.convert(call, ret)), pos)
        }])
    }

    /// The index of the enum constructor that the constructor of a constant's body calls: javac
    /// gives the anonymous class a constructor with the same parameters.
    pub(super) fn enum_constructor(&self, class: usize, symbol: &Symbol) -> Option<usize> {
        self.method_index(symbol).or_else(|| {
            self.methods.iter().position(|m| m.owner == class && m.name == "<init>" && m.sig == symbol.ty)
        })
    }

    /// A call, in the body of an enum constant, of a method of that body (javac's owner of it is
    /// the anonymous class, `""`): it overrides the enum's method, which is called instead and
    /// runs the override for this constant. `None` for any other call.
    pub(super) fn constant_body_call(
        &self,
        node: Node<'t>,
        symbol: &Symbol,
        receiver: Option<Node<'t>>,
        args: &mut Vec<Expr>,
    ) -> Result<Option<ExprKind>, Error> {
        let current = self.current.get();
        if !symbol.owner.is_empty()
            || symbol.is_static
            || self.enum_index(&self.types[current].name).is_none()
        {
            return Ok(None);
        }
        let Some(resolved) = self
            .methods
            .iter()
            .position(|m| m.owner == current && m.name == symbol.name && m.sig == symbol.ty)
        else {
            return Ok(None);
        };
        let receiver = match receiver {
            Some(r) => self.expr(r)?,
            None => self.this(node),
        };
        Ok(Some(ExprKind::MethodCall {
            receiver: Box::new(receiver),
            name: symbol.name.to_string(),
            // An argument converts to its parameter's type, as `range(date)` passes a class's
            // object for an interface.
            args: self.convert_args(Some(resolved), std::mem::take(args))?,
            resolved: Some(resolved),
            is_super: false,
        }))
    }

    /// The enum that a field read in an enum constant's body belongs to. javac gives such a field
    /// no owner, and the body's methods are lowered in the enum, so the enum is the current class
    /// or, in a lambda or a class in such a method, the innermost enum around it.
    pub(super) fn body_field_owner(&self, symbol: &Symbol) -> Option<usize> {
        if symbol.kind != "FIELD" || !symbol.owner.is_empty() {
            return None;
        }
        let at = self.enclosing_enum(self.current.get())?;
        self.enum_index(&self.types[at].name)
    }

    /// The instance fields that the bodies of the constants of enum `class` declare, by ordinal.
    /// Each is a field of the enum: the body's methods read it through `this`, and only its own
    /// constant has an initializer for it (see `body_field_value`).
    pub(super) fn constant_body_fields(&self, class: usize) -> Result<Vec<Vec<Field>>, Error> {
        self.current.set(class);
        self.in_ctor.set(false);
        let mut all = Vec::new();
        for constant in self.types[class].node.list("members").filter(|m| is_enum_constant(*m)) {
            let mut fields = Vec::new();
            let body = constant.get("initializer").and_then(|i| i.get("classBody"));
            for member in body.into_iter().flat_map(|b| b.list("members")).filter(|m| m.kind() == "VARIABLE")
            {
                if member.has_flag("STATIC") {
                    return unsupported(member, "static fields in an enum constant's body");
                }
                let ty = self.ty(member, member.symbol().map_or("", |s| s.ty))?;
                let init = member.get("initializer").map(|i| self.expr(i)).transpose()?;
                let init = init.map(|e| self.convert(e, ty));
                let name = member.str("name").to_string();
                fields.push(Field {
                    name,
                    ty,
                    init,
                    constant: false,
                    volatile: member.has_flag("VOLATILE"),
                    pos: member.pos(),
                });
            }
            all.push(fields);
        }
        Ok(all)
    }

    /// Adds the fields of the constants' bodies to the enum's `fields`, which hold the enum's own
    /// fields before. A name that two bodies declare is one field, which needs one type.
    pub(super) fn add_body_fields(
        &self,
        bodies: &[Vec<Field>],
        fields: &mut Vec<Field>,
    ) -> Result<(), Error> {
        let own = fields.len();
        for field in bodies.iter().flatten() {
            if fields[..own].iter().any(|f| f.name == field.name) {
                return error(
                    field.pos,
                    "an enum constant's body field with the name of a field of the enum",
                );
            }
            match fields[own..].iter().find(|f| f.name == field.name) {
                Some(other) if other.ty != field.ty => {
                    return error(
                        field.pos,
                        "enum constant bodies that declare fields of one name with different types",
                    );
                }
                Some(_) => {}
                None => fields.push(Field {
                    name: field.name.clone(),
                    ty: field.ty,
                    init: None,
                    constant: false,
                    volatile: field.volatile,
                    pos: field.pos,
                }),
            }
        }
        Ok(())
    }

    /// The value of an enum field that a constant's body declares, for the constant with ordinal
    /// `ordinal`: the body's initializer, which must be pure as the enum's are, or the zero of the
    /// type when this constant's body does not declare it (or gives no initializer). `None` for a
    /// field that no body declares.
    pub(super) fn body_field_value(
        &self,
        bodies: &[Vec<Field>],
        ordinal: usize,
        field: &Field,
    ) -> Option<Result<Expr, Error>> {
        if !bodies.iter().flatten().any(|b| b.name == field.name) {
            return None;
        }
        let declared = bodies[ordinal].iter().find(|b| b.name == field.name);
        Some(match declared.and_then(|b| b.init.as_ref()) {
            Some(init) if pure(init, false) => Ok(subst(init, &[])),
            Some(init) => error(init.pos, "enum field initializers that are not constants"),
            None => self.zero(field.ty, field.pos),
        })
    }

    /// The zero of a type of a field that an enum constant's body declares. A field of another
    /// type leaves the pure form, and the enum takes the table form, whose tables start out null.
    fn zero(&self, ty: Type, pos: Pos) -> Result<Expr, Error> {
        let kind = match ty {
            Type::Int => ExprKind::Int(0),
            Type::Long => ExprKind::Long(0),
            Type::Double => ExprKind::Double(0.0),
            Type::Float => ExprKind::Float(0.0),
            Type::Char => ExprKind::Char(0),
            Type::Boolean => ExprKind::Bool(false),
            Type::String => ExprKind::Str(String::new()),
            _ => return error(pos, "an enum constant's body field of this type in the pure form"),
        };
        Ok(self.synth(kind, pos, ty))
    }

    /// Declares the lambdas and method references in the fields and methods of the constants' bodies of enum
    /// `class` as those of the enum's method that each overrides, whose code it becomes
    /// (`constant_bodies`): `this` in them is the constant, and the override's parameters are
    /// locals of that method.
    pub(super) fn declare_body_lambdas(&mut self, class: usize) -> Result<(), Error> {
        let constants: Vec<Node<'t>> =
            self.types[class].node.list("members").filter(|m| is_enum_constant(*m)).collect();
        for constant in constants {
            let Some(body) = constant.get("initializer").and_then(|i| i.get("classBody")) else { continue };
            // The initializers of the body's fields, which are the enum's (`constant_body_fields`).
            for member in body.list("members").filter(|m| m.kind() == "VARIABLE" && !m.has_flag("STATIC")) {
                self.declare_lambdas(member, class, None)?;
            }
            for member in body.list("members").filter(|m| m.kind() == "METHOD" && m.str("name") != "<init>") {
                let (name, sig) = (member.str("name"), member.symbol().map_or("", |s| s.ty));
                // One that overrides none of the enum's methods is rejected by `constant_bodies`.
                let base =
                    self.methods.iter().position(|m| m.owner == class && m.name == name && m.sig == sig);
                if let Some(base) = base {
                    self.declare_lambdas(member, class, Some(base))?;
                }
            }
        }
        Ok(())
    }

    /// Moves the methods of the constants' bodies of enum `class` into the enum's methods, as arms
    /// of a switch on `this`.
    /// The methods of enum `class` that its constants' bodies override, whose `switch` over the
    /// constants `constant_bodies` makes.
    pub(super) fn overridden_by_bodies(&self, class: usize) -> Vec<usize> {
        let mut out = Vec::new();
        for constant in self.types[class].node.list("members").filter(|m| is_enum_constant(*m)) {
            let Some(body) = constant.get("initializer").and_then(|i| i.get("classBody")) else { continue };
            for member in body.list("members") {
                if member.kind() != "METHOD" || member.str("name") == "<init>" {
                    continue;
                }
                let sig = member.symbol().map_or("", |s| s.ty);
                let name = member.str("name");
                let base = self
                    .methods
                    .iter()
                    .position(|m| m.owner == class && m.name == name && params_of(m.sig) == params_of(sig));
                if let Some(base) = base.filter(|b| !self.body_helpers.contains(b) && !out.contains(b)) {
                    out.push(base);
                }
            }
        }
        out
    }

    pub(super) fn constant_bodies(&self, class: usize, methods: &mut [Method]) -> Result<(), Error> {
        self.current.set(class);
        self.in_ctor.set(false);
        let enum_name = self.types[class].name.as_str();
        let ty = Type::enum_type(enum_name);
        // For each method of the enum that a body overrides, the overrides.
        let mut overrides: Vec<(usize, Vec<Override>)> = Vec::new();
        let constants: Vec<Node<'t>> =
            self.types[class].node.list("members").filter(|m| is_enum_constant(*m)).collect();
        for (ordinal, constant) in constants.iter().enumerate() {
            let Some(body) = constant.get("initializer").and_then(|i| i.get("classBody")) else { continue };
            for member in body.list("members") {
                match member.kind() {
                    "METHOD" if member.str("name") == "<init>" => continue,
                    "METHOD" => {}
                    // The fields are lowered by `constant_body_fields`.
                    "EMPTY_STATEMENT" | "VARIABLE" => continue,
                    _ => return unsupported(member, "blocks and classes in an enum constant's body"),
                }
                let symbol = member.symbol();
                let sig = symbol.as_ref().map_or("", |s| s.ty);
                let name = member.str("name");
                let Some(base) = self
                    .methods
                    .iter()
                    .position(|m| m.owner == class && m.name == name && params_of(m.sig) == params_of(sig))
                else {
                    return unsupported(
                        member,
                        "a method of an enum constant's body that overrides none of the enum's",
                    );
                };
                // A helper is a method of its own; one of the same name and signature in another
                // body would be a second method that its calls cannot tell apart.
                if self.body_helpers.contains(&base) {
                    if !std::ptr::eq(self.methods[base].node.0, member.0) {
                        return unsupported(
                            member,
                            "methods of one name and signature that override nothing in two enum constants' bodies",
                        );
                    }
                    continue;
                }
                let Some(block) = member.get("body") else {
                    return unsupported(member, "methods without a body");
                };
                let mut stmts = Vec::new();
                // The override's parameters are the enum method's, by their names in the override.
                for (param, base_param) in member.list("parameters").zip(&methods[base].params) {
                    let param_name = param.str("name");
                    if param_name != base_param.name {
                        let value =
                            self.synth(ExprKind::Var(base_param.name.clone()), param.pos(), base_param.ty);
                        stmts.push(Stmt::Local {
                            ty: base_param.ty,
                            name: param_name.to_string(),
                            init: Some(value),
                            pos: param.pos(),
                        });
                    }
                }
                // The override's code is that of the enum's method, with its type variables and
                // its result.
                let outer = self.method_ctx.replace(Some(base));
                self.ret.set(methods[base].ret);
                let binary = body.get("symbol").and_then(|s| s.0["binary"].as_str().map(str::to_string));
                *self.body_class.borrow_mut() = binary;
                let lowered = self.block(block);
                self.body_class.borrow_mut().take();
                self.method_ctx.set(outer);
                stmts.extend(lowered?);
                match overrides.iter_mut().find(|(m, _)| *m == base) {
                    Some((_, arms)) => arms.push((ordinal, stmts)),
                    None => overrides.push((base, vec![(ordinal, stmts)])),
                }
            }
        }
        for (base, arms) in overrides {
            let method = &mut methods[base];
            let pos = method.pos;
            let mut cases: Vec<Case> = Vec::new();
            let every = arms.len() == constants.len();
            for (ordinal, stmts) in arms {
                let label = self.synth(ExprKind::EnumConstant { class, ordinal }, pos, ty);
                cases.push(Case { labels: vec![CaseLabel::Value(label)], body: case_body(stmts) });
            }
            if !every {
                let own = std::mem::take(&mut method.body);
                cases.push(Case { labels: vec![CaseLabel::Default], body: case_body(own) });
            }
            let selector = self.synth(ExprKind::This, pos, ty);
            method.body = vec![Stmt::Switch { selector, cases }];
        }
        Ok(())
    }
}
