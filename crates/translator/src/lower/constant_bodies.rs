//! Enum constants with bodies (`PLUS { int apply(int a, int b) { return a + b; } }`). Each such
//! body is an anonymous subclass that overrides methods of the enum; an enum is a Rust `enum`, so
//! the overrides become arms of a `switch` on `this` in the enum's own method: the constants that
//! override it run their body, the others the enum's (an abstract method has none, as every
//! constant overrides it). A body may only override the enum's methods.

use super::*;
use enums::is_enum_constant;

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
        }))
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
                    "EMPTY_STATEMENT" => continue,
                    _ => return unsupported(member, "fields, blocks and classes in an enum constant's body"),
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
