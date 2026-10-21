//! Lowering of static initialization: compile-time constants, which become literals, and the
//! static initializer of a class, which javac's `static` members make into a `<clinit>` method
//! that runs them in source order.

use super::*;

/// Parses javac's text for a constant value; text that does not parse is an unsupported constant.
fn parse_constant<T: std::str::FromStr>(node: Node, text: &str) -> Result<T, Error> {
    text.parse().or_else(|_| unsupported(node, "this constant"))
}

impl<'t> Lowering<'t> {
    /// The literal that a compile-time constant of type `ty` has, given javac's text for its value.
    pub(super) fn constant_literal(&self, node: Node<'t>, ty: Type, text: &str) -> Result<ExprKind, Error> {
        Ok(match ty {
            Type::Int => ExprKind::Int(parse_constant(node, text)?),
            Type::Long => ExprKind::Long(parse_constant(node, text)?),
            Type::Double => ExprKind::Double(parse_constant(node, text)?),
            Type::Float => ExprKind::Float(parse_constant(node, text)?),
            Type::Boolean => ExprKind::Bool(text == "true"),
            Type::Char => ExprKind::Char(parse_constant(node, text)?),
            Type::String => ExprKind::Str(text.to_string()),
            Type::Byte | Type::Short => {
                let int = self.synth(ExprKind::Int(parse_constant(node, text)?), node.pos(), Type::Int);
                ExprKind::Cast(ty, Box::new(int))
            }
            _ => return unsupported(node, "this constant"),
        })
    }

    /// Whether a static field's value is a compile-time constant, which is written as a `const`.
    pub(super) fn is_constant(node: Node) -> bool {
        node.symbol().is_some_and(|s| s.constant.is_some()) && node.has_flag("STATIC")
    }

    /// Whether a member is a `static { ... }` block, which javac marks with `static` rather than a
    /// modifier.
    pub(super) fn is_static_block(node: Node) -> bool {
        node.kind() == "BLOCK" && node.0["static"].as_bool() == Some(true)
    }

    /// Whether the class has static initialization that is not just constant fields: a static
    /// block, or a static field with an initializer that is not a constant.
    pub(super) fn has_static_init(&self, index: usize) -> bool {
        self.types[index].node.list("members").any(|m| match m.kind() {
            "BLOCK" => Self::is_static_block(m),
            "VARIABLE" => {
                m.has_flag("STATIC")
                    && m.get("initializer").is_some()
                    && !Self::is_constant(m)
                    && !enums::is_enum_constant(m)
                    && self.keeps(m, &self.types[index].javac)
                    && self.handle_declaration(m).is_none()
            }
            _ => false,
        })
    }

    /// The `<clinit>` method of a class: its static fields' initializers and static blocks, in
    /// source order, as assignments and blocks.
    pub(super) fn clinit(&self, index: usize) -> Result<Method, Error> {
        self.current.set(index);
        self.in_ctor.set(false);
        let node = self.types[index].node;
        let mut body = Vec::new();
        for member in node.list("members") {
            // Enum constants are values built by the enum's own code, not initializers.
            if (!member.has_flag("STATIC") && !Self::is_static_block(member))
                || enums::is_enum_constant(member)
            {
                continue;
            }
            match member.kind() {
                // In a census, a rejected block or initializer is left out.
                "BLOCK" => match self.block(member) {
                    Ok(block) => body.push(Stmt::Block(block)),
                    Err(e) => self.reject(index, e)?,
                },
                // A field that nothing reads, whose initializer does nothing else, is not kept.
                "VARIABLE"
                    if !Self::is_constant(member)
                        && self.keeps(member, &self.types[index].javac)
                        && self.handle_declaration(member).is_none() =>
                {
                    let Some(init) = member.get("initializer") else { continue };
                    let ty = match self.ty(member, member.symbol().map_or("", |s| s.ty)) {
                        Ok(ty) => ty,
                        Err(e) => {
                            self.reject(index, e)?;
                            continue;
                        }
                    };
                    let target = Place::Static { class: index, name: member.str("name").to_string() };
                    let value = match self.expr(init) {
                        Ok(value) => Box::new(self.convert(value, ty)),
                        Err(e) => {
                            self.reject(index, e)?;
                            continue;
                        }
                    };
                    let kind = ExprKind::Assign { op: None, target, value };
                    let stmt = self.synth(kind, member.pos(), ty);
                    body.push(Stmt::Expr(stmt));
                }
                _ => {}
            }
        }
        Ok(Method {
            name: "<clinit>".to_string(),
            params: Vec::new(),
            ret: Type::Void,
            body,
            is_main: false,
            is_static: true,
            is_ctor: false,
            class: index,
            pos: node.pos(),
            is_abstract: false,
            is_synchronized: false,
            overrides: Vec::new(),
            type_params: Vec::new(),
        })
    }
}
