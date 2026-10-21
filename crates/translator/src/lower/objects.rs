//! Lowering of classes: the classes of a file, their fields, and the names, fields and `new`
//! expressions that refer to them. A class is identified by its qualified name as javac writes
//! it (`Point`, or `Outer.Inner` for a nested class).

use super::*;

/// A class declared in the file, in `Class::types` order.
pub(super) struct TypeInfo<'t> {
    pub name: String,
    pub node: Node<'t>,
}

/// A method or constructor of a class, in `Class::methods` order.
pub(super) struct MethodInfo<'t> {
    /// Index into the types of the class that declares it.
    pub owner: usize,
    pub node: Node<'t>,
    /// The method's name, `<init>` for a constructor.
    pub name: &'t str,
    /// The javac signature, such as `(int)void`.
    pub sig: &'t str,
}

impl<'t> Lowering<'t> {
    /// Collects every class of the file, nested static classes after their outer class, and every
    /// method and constructor, so that a use can refer to any of them before it is lowered.
    pub(super) fn declare(decls: &[Node<'t>]) -> Result<Self, Error> {
        let mut lowering = Lowering::empty();
        for decl in decls {
            lowering.declare_type(*decl, None)?;
        }
        Ok(lowering)
    }

    pub(super) fn empty() -> Self {
        Lowering {
            next_id: Cell::new(0),
            current: Cell::new(0),
            method_ctx: Cell::new(None),
            scope: Cell::new(None),
            in_ctor: Cell::new(false),
            ret: Cell::new(Type::Void),
            types: Vec::new(),
            methods: Vec::new(),
            reach: None,
            entry: None,
            object: None,
            real: Default::default(),
            failed: Default::default(),
        }
    }

    /// Declares a class, named `outer.Name` inside `outer` (a class or a package).
    pub(super) fn declare_type(&mut self, node: Node<'t>, outer: Option<&str>) -> Result<(), Error> {
        if !matches!(node.kind(), "CLASS" | "ENUM" | "INTERFACE") {
            return unsupported(node, &kind_words(node.kind()));
        }
        let simple = node.str("simpleName");
        let name = match outer {
            Some(outer) => format!("{outer}.{simple}"),
            None => simple.to_string(),
        };
        let index = self.types.len();
        self.types.push(TypeInfo { name: name.clone(), node });
        let members: Vec<Node<'t>> = node.list("members").filter(|m| self.keeps(*m, &name)).collect();
        for member in members {
            match member.kind() {
                "METHOD" => self.methods.push(MethodInfo {
                    owner: index,
                    node: member,
                    name: member.str("name"),
                    sig: member.symbol().map_or("", |s| s.ty),
                }),
                "VARIABLE" | "EMPTY_STATEMENT" => {}
                "ENUM" | "INTERFACE" => self.declare_type(member, Some(&name))?,
                "CLASS" if member.has_flag("STATIC") => self.declare_type(member, Some(&name))?,
                "CLASS" => return unsupported(member, "inner classes"),
                "BLOCK" if Self::is_static_block(member) => {}
                "BLOCK" => self.check_initializer_block(node, member)?,
                kind => return unsupported(member, &kind_words(kind)),
            }
        }
        Ok(())
    }

    /// The index of the class with this qualified name, if the file declares it.
    pub(super) fn type_index(&self, qualified: &str) -> Option<usize> {
        self.types.iter().position(|t| t.name == qualified)
    }

    /// The index of the method or constructor that a symbol names, in `Class::methods`.
    pub(super) fn method_index(&self, symbol: &Symbol) -> Option<usize> {
        self.methods.iter().position(|m| {
            self.types[m.owner].name == symbol.owner && m.name == symbol.name && m.sig == symbol.ty
        })
    }

    /// The declaration of a class: its instance fields and static fields, in source order.
    pub(super) fn type_decl(&self, index: usize) -> Result<TypeDecl, Error> {
        self.current.set(index);
        let node = self.types[index].node;
        let (mut fields, mut statics) = (Vec::new(), Vec::new());
        let name = &self.types[index].name;
        for member in node
            .list("members")
            .filter(|m| m.kind() == "VARIABLE" && !enums::is_enum_constant(*m) && self.keeps(*m, name))
        {
            let field = self.field(member)?;
            // A subclass's struct holds its superclass's fields as `base`.
            if !member.has_flag("STATIC")
                && field.name.eq_ignore_ascii_case("base")
                && self.superclass(index)?.is_some()
            {
                return unsupported(member, "a field named `base` in a subclass");
            }
            if member.has_flag("STATIC") { statics.push(field) } else { fields.push(field) }
        }
        // An enum's constants need its constructors, which `lower` lowers after this.
        let kind = match node.kind() {
            "ENUM" => TypeKind::Enum,
            "INTERFACE" => TypeKind::Interface,
            _ => TypeKind::Class,
        };
        Ok(TypeDecl {
            name: self.types[index].name.clone(),
            package: String::new(),
            kind,
            is_abstract: kind == TypeKind::Interface || node.has_flag("ABSTRACT"),
            extends: self.superclass(index)?,
            implements: self.interfaces(index)?,
            fields,
            statics,
            enum_constants: None,
            type_params: self.class_type_params(index),
        })
    }

    fn field(&self, node: Node<'t>) -> Result<Field, Error> {
        let ty = self.ty(node, node.symbol().map_or("", |s| s.ty))?;
        let init = node.get("initializer");
        let constant = Self::is_constant(node);
        let init = if constant {
            let text = node.symbol().and_then(|s| s.constant).unwrap_or_default();
            let kind = self.constant_literal(node, ty, text)?;
            Some(self.synth(kind, node.pos(), ty))
        } else {
            init.map(|i| Ok::<_, Error>(self.convert(self.expr(i)?, ty))).transpose()?
        };
        Ok(Field { name: node.str("name").to_string(), ty, init, constant, pos: node.pos() })
    }

    /// The place that `node` names when it is a field: `name`, `this.name`, `object.name` or
    /// `Class.name`. `None` for anything else, including fields of library classes.
    pub(super) fn field_place(&self, node: Node<'t>) -> Result<Option<Place>, Error> {
        let Some(symbol) = node.symbol().filter(|s| s.kind == "FIELD") else { return Ok(None) };
        let Some(class) = self.type_index(symbol.owner) else { return Ok(None) };
        let receiver = match node.kind() {
            "IDENTIFIER" => None,
            "MEMBER_SELECT" => Some(node.child("expression")?),
            _ => return Ok(None),
        };
        let name = symbol.name.to_string();
        if symbol.is_static {
            // Java evaluates an object in front of a static field and ignores it, even null;
            // one without effects is simply dropped.
            if let Some(r) = receiver
                && r.symbol().is_none_or(|s| !matches!(s.kind, "CLASS" | "INTERFACE" | "ENUM"))
                && has_effects(&self.expr(r)?)
            {
                return unsupported(node, "static fields through an expression with effects");
            }
            return Ok(Some(Place::Static { class, name }));
        }
        let object = match receiver {
            Some(r) => self.expr(r)?,
            None => self.this(node),
        };
        Ok(Some(Place::Field { object: Box::new(object), name }))
    }

    /// An identifier: a field, `this`, or a local variable or parameter.
    pub(super) fn name(&self, node: Node<'t>) -> Result<ExprKind, Error> {
        if node.str("name") == "this" {
            return Ok(ExprKind::This);
        }
        if let Some(constant) = node.symbol().and_then(|s| self.enum_constant(&s)) {
            return Ok(constant);
        }
        Ok(match self.field_place(node)? {
            Some(place) => read(place),
            None => ExprKind::Var(self.local(node)?),
        })
    }

    /// A select: a field of an object or of a class.
    pub(super) fn select(&self, node: Node<'t>) -> Result<ExprKind, Error> {
        if let Some(constant) = node.symbol().and_then(|s| self.enum_constant(&s)) {
            return Ok(constant);
        }
        match self.field_place(node)? {
            Some(place) => Ok(read(place)),
            None => unsupported(node, "fields"),
        }
    }

    /// `new C(args)`.
    pub(super) fn new_object(&self, node: Node<'t>) -> Result<ExprKind, Error> {
        if node.get("classBody").is_some() {
            return unsupported(node, "anonymous classes");
        }
        if node.get("enclosingExpression").is_some() {
            return unsupported(node, "qualified instance creation");
        }
        let Some(symbol) = node.symbol() else { return unsupported(node, "this `new`") };
        let Some(class) = self.type_index(symbol.owner) else {
            return unsupported(node, &format!("`new {}`", symbol.owner));
        };
        let args: Vec<Expr> = node.list("arguments").map(|a| self.expr(a)).collect::<Result<_, _>>()?;
        let args = self.packed_args(node, &symbol, args)?;
        let ctor = self.method_index(&symbol);
        let args = self.convert_args(ctor, args)?;
        Ok(ExprKind::New { class, args, ctor })
    }

    /// `this` of the class whose members are being lowered.
    pub(super) fn this(&self, node: Node<'t>) -> Expr {
        let ty = self.self_type(self.current.get());
        self.synth(ExprKind::This, node.pos(), ty)
    }

    /// An expression that javac did not write, such as the implicit `this` of a field.
    pub(super) fn synth(&self, kind: ExprKind, pos: Pos, ty: Type) -> Expr {
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        Expr { kind, pos, ty, id }
    }
}

/// The expression that reads a place.
pub(super) fn read(place: Place) -> ExprKind {
    match place {
        Place::Var(name) => ExprKind::Var(name),
        Place::Field { object, name } => ExprKind::Field { object, name },
        Place::Static { class, name } => ExprKind::Static { class, name },
        Place::Index(array, index) => ExprKind::Index(array, index),
    }
}
