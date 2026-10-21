//! Lowering of classes: the classes of a file, their fields, and the names, fields and `new`
//! expressions that refer to them. A class is identified by its qualified name as javac writes
//! it (`Point`, or `Outer.Inner` for a nested class).

use super::inner::NestKind;
use super::*;

/// A class declared in the file, in `Class::types` order.
pub(super) struct TypeInfo<'t> {
    /// The name of the declaration, which its `Type`s carry: javac's qualified name, or for a
    /// local or anonymous class one that names it after its place (see `inner`).
    pub name: String,
    /// The name that javac's symbols and types give the class: `name`, or the binary name of a
    /// local or anonymous class (`Main$1Labeler`).
    pub javac: String,
    pub node: Node<'t>,
    /// How an inner, local or anonymous class is nested; `None` for any other class.
    pub nested: Option<super::inner::Nested<'t>>,
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
            standard: exceptions::Standard::default(),
            closes: Default::default(),
            object_arrays: Default::default(),
            object: None,
            plain_object: None,
            real: Default::default(),
            failed: Default::default(),
            aliases: Default::default(),
            functional: Default::default(),
            lambdas: Vec::new(),
            capture_refs: Default::default(),
            capture_sites: Default::default(),
            member_outers: Vec::new(),
        }
    }

    /// Declares a class, named `outer.Name` inside `outer` (a class or a package).
    pub(super) fn declare_type(&mut self, node: Node<'t>, outer: Option<&str>) -> Result<(), Error> {
        if !matches!(node.kind(), "CLASS" | "ENUM" | "INTERFACE" | "RECORD") {
            return unsupported(node, &kind_words(node.kind()));
        }
        let simple = node.str("simpleName");
        let name = match outer {
            Some(outer) => format!("{outer}.{simple}"),
            None => simple.to_string(),
        };
        // A local record, enum or interface moved to the top level (see `javac::records`) keeps
        // the binary name that javac's symbols give it.
        let binary = crate::program::class_name(node.0);
        let javac = if binary.contains('$') { binary } else { name.clone() };
        let index = self.types.len();
        self.types.push(TypeInfo { name, javac, node, nested: None });
        self.declare_members(index)
    }

    /// Declares the members of the class `index`: its methods and constructors, its nested
    /// classes, and the local and anonymous classes in its code.
    pub(super) fn declare_members(&mut self, index: usize) -> Result<(), Error> {
        let (node, name) = (self.types[index].node, self.types[index].javac.clone());
        let in_local = self.types[index].nested.as_ref().is_some_and(|n| n.kind != NestKind::Inner);
        let members: Vec<Node<'t>> = node.list("members").filter(|m| self.keeps(*m, &name)).collect();
        for member in members {
            match member.kind() {
                "METHOD" => {
                    self.methods.push(MethodInfo {
                        owner: index,
                        node: member,
                        name: member.str("name"),
                        sig: member.symbol().map_or("", |s| s.ty),
                    });
                    let method = self.methods.len() - 1;
                    self.declare_local_classes(index, member, Some(method))?;
                    self.declare_lambdas(member, index, Some(method))?;
                }
                "VARIABLE" if enums::is_enum_constant(member) => self.declare_lambdas(member, index, None)?,
                "VARIABLE" => {
                    self.declare_local_classes(index, member, None)?;
                    self.declare_lambdas(member, index, None)?;
                }
                "EMPTY_STATEMENT" => {}
                "CLASS" | "ENUM" | "INTERFACE" | "RECORD" if in_local => {
                    // Named after its outer class once that is named (see `inner::name_nested`).
                    self.member_outers.push((self.types.len(), index));
                    if member.kind() == "CLASS" && !member.has_flag("STATIC") {
                        self.declare_inner(index, member)?;
                    } else {
                        self.declare_type(member, Some(&name))?;
                    }
                }
                "ENUM" | "INTERFACE" | "RECORD" => self.declare_type(member, Some(&name))?,
                "CLASS" if member.has_flag("STATIC") => self.declare_type(member, Some(&name))?,
                "CLASS" => self.declare_inner(index, member)?,
                "BLOCK" => {
                    if !Self::is_static_block(member) {
                        self.check_initializer_block(node, member)?;
                    }
                    self.declare_local_classes(index, member, None)?;
                    self.declare_lambdas(member, index, None)?;
                }
                kind => return unsupported(member, &kind_words(kind)),
            }
        }
        Ok(())
    }

    /// The index of the class with this name, if the file declares it: the name of its
    /// declaration, or the name javac gives it.
    pub(super) fn type_index(&self, qualified: &str) -> Option<usize> {
        self.types.iter().position(|t| t.name == qualified || t.javac == qualified)
    }

    /// The index of the method or constructor that a symbol names, in `Class::methods`.
    pub(super) fn method_index(&self, symbol: &Symbol) -> Option<usize> {
        self.methods.iter().position(|m| {
            self.types[m.owner].javac == symbol.owner && m.name == symbol.name && m.sig == symbol.ty
        })
    }

    /// The declaration of a class: its instance fields and static fields, in source order.
    pub(super) fn type_decl(&self, index: usize) -> Result<TypeDecl, Error> {
        if let Some(decl) = self.lambda_decl(index) {
            return decl;
        }
        self.current.set(index);
        let node = self.types[index].node;
        // The enclosing instance and the captured locals come first, set before anything runs.
        let (mut fields, mut statics) = (self.synthetic_fields(index)?, Vec::new());
        let name = &self.types[index].name;
        let moved = self.moves_initializers(index);
        let javac = &self.types[index].javac;
        for member in node
            .list("members")
            .filter(|m| m.kind() == "VARIABLE" && !enums::is_enum_constant(*m) && self.keeps(*m, javac))
        {
            // Moved initializers run in the constructors (see `inner`).
            let field = self.field(member, !(moved && !member.has_flag("STATIC")))?;
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
        // A standard exception class has objects only if the program creates them.
        let unused_standard = self.standard.has(name) && !self.standard.instantiated.contains(name);
        Ok(TypeDecl {
            name: self.types[index].name.clone(),
            package: String::new(),
            kind,
            is_abstract: kind == TypeKind::Interface || node.has_flag("ABSTRACT") || unused_standard,
            extends: self.superclass(index)?,
            implements: self.interfaces(index)?,
            fields,
            statics,
            enum_constants: None,
            type_params: self.class_type_params(index),
            outer: self.nesting(index).filter(|n| n.outer).map(|n| n.enclosing),
            captures: self.captures_of(index)?,
            origin: self.origin(index),
        })
    }

    /// A field, with its initializer when `with_init`.
    fn field(&self, node: Node<'t>, with_init: bool) -> Result<Field, Error> {
        let ty = self.ty(node, node.symbol().map_or("", |s| s.ty))?;
        let init = node.get("initializer").filter(|_| with_init);
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
        let Some(class) = self.type_index(symbol.owner).or_else(|| self.body_field_owner(&symbol)) else {
            return Ok(None);
        };
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
                && r.symbol().is_none_or(|s| !matches!(s.kind, "CLASS" | "INTERFACE" | "ENUM" | "RECORD"))
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
            // In a lambda, the object of the code that holds it.
            if let Some(holder) = self.lambda_this(node) {
                return Ok(holder?.kind);
            }
            return Ok(ExprKind::This);
        }
        if let Some(constant) = node.symbol().and_then(|s| self.enum_constant(&s)) {
            return Ok(constant);
        }
        if let Some(captured) = self.capture_read(node)? {
            return Ok(captured.kind);
        }
        Ok(match self.field_place(node)? {
            Some(place) => read(place),
            None => ExprKind::Var(self.local(node)?),
        })
    }

    /// A select: a field of an object or of a class, or `Outer.this`.
    pub(super) fn select(&self, node: Node<'t>) -> Result<ExprKind, Error> {
        if let Some(constant) = node.symbol().and_then(|s| self.enum_constant(&s)) {
            return Ok(constant);
        }
        if let Some(outer) = self.qualified_this(node) {
            return Ok(outer?.kind);
        }
        match self.field_place(node)? {
            Some(place) => Ok(read(place)),
            None => unsupported(node, "fields"),
        }
    }

    /// `new C(args)`.
    pub(super) fn new_object(&self, node: Node<'t>) -> Result<ExprKind, Error> {
        let Some(symbol) = node.symbol() else { return unsupported(node, "this `new`") };
        if let Some(object) = self.new_plain_object(node) {
            return Ok(object);
        }
        // An anonymous class's `new` creates an object of the class its body declares.
        let class = match node.get("classBody") {
            Some(body) => self.type_index(&crate::program::class_name(body.0)),
            None => self.type_index(symbol.owner),
        };
        let Some(class) = class else {
            return unsupported(node, &format!("`new {}`", symbol.owner));
        };
        let args: Vec<Expr> = node.list("arguments").map(|a| self.expr(a)).collect::<Result<_, _>>()?;
        let args = self.packed_args(node, &symbol, args)?;
        self.check_standard_member(node, &symbol)?;
        let ctor = self.method_index(&symbol);
        let args = self.convert_generic_args(node, ctor, args)?;
        let args = self.with_synthetic(self.creation_args(node, class)?, args);
        Ok(ExprKind::New { class, args, ctor })
    }

    /// `this` for the member that `node` names without an object: of the class whose members are
    /// being lowered, or of the enclosing class that has the member (see `captures`).
    pub(super) fn this(&self, node: Node<'t>) -> Expr {
        if let Some(Ok(outer)) = self.implicit_this(node) {
            return outer;
        }
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
