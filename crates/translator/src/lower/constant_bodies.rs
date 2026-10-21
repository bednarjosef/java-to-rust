//! Enum constants with bodies (`PLUS { int apply(int a, int b) { return a + b; } }`). Each such
//! body is an anonymous subclass that overrides methods of the enum; an enum is a Rust `enum`, so
//! the overrides become arms of a `switch` on `this` in the enum's own method: the constants that
//! override it run their body, the others the enum's (an abstract method has none, as every
//! constant overrides it). A body may only override the enum's methods.

use super::*;
use enums::{is_enum_constant, pure, subst};

/// A constant's ordinal, and the statements of its body's override of a method.
type Override = (usize, Vec<Stmt>);

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
            args: std::mem::take(args),
            resolved: Some(resolved),
            is_super: false,
        }))
    }

    /// The enum that a field read in an enum constant's body belongs to. javac gives such a field
    /// no owner, and the body's methods are lowered in the enum, so the enum is the current class.
    pub(super) fn body_field_owner(&self, symbol: &Symbol) -> Option<usize> {
        if symbol.kind != "FIELD" || !symbol.owner.is_empty() {
            return None;
        }
        self.enum_index(&self.types[self.current.get()].name)
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
                if !matches!(
                    ty,
                    Type::Int
                        | Type::Long
                        | Type::Double
                        | Type::Float
                        | Type::Char
                        | Type::Boolean
                        | Type::String
                ) {
                    return unsupported(member, "fields of this type in an enum constant's body");
                }
                let init = member.get("initializer").map(|i| self.expr(i)).transpose()?;
                let init = init.map(|e| self.convert(e, ty));
                let name = member.str("name").to_string();
                fields.push(Field { name, ty, init, constant: false, pos: member.pos() });
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
            None => Ok(self.zero(field.ty, field.pos)),
        })
    }

    /// The zero of a type of a field that an enum constant's body declares.
    fn zero(&self, ty: Type, pos: Pos) -> Expr {
        let kind = match ty {
            Type::Long => ExprKind::Long(0),
            Type::Double => ExprKind::Double(0.0),
            Type::Float => ExprKind::Float(0.0),
            Type::Char => ExprKind::Char(0),
            Type::Boolean => ExprKind::Bool(false),
            Type::String => ExprKind::Str(String::new()),
            _ => ExprKind::Int(0),
        };
        self.synth(kind, pos, ty)
    }

    /// Moves the methods of the constants' bodies of enum `class` into the enum's methods, as arms
    /// of a switch on `this`.
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
                let Some(base) =
                    self.methods.iter().position(|m| m.owner == class && m.name == name && m.sig == sig)
                else {
                    return unsupported(
                        member,
                        "a method of an enum constant's body that overrides none of the enum's",
                    );
                };
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
                stmts.extend(self.block(block)?);
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
