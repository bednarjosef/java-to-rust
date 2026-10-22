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
            object_arrays: Default::default(),
            field_updates: Default::default(),
            static_qualifiers: Default::default(),
            calls: Default::default(),
            covariant: Default::default(),
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
            library: Vec::new(),
            census: None,
            catch_panics: false,
            stubbing: Cell::new(false),
            stubbed: Default::default(),
            body_methods: Default::default(),
            annotation_types: Default::default(),
            rejected_classes: Default::default(),
            callers: Default::default(),
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
                // Does nothing when the program runs (see `annotation_types`).
                "ANNOTATION_TYPE" => {
                    self.annotation_types.insert(crate::program::class_name(member.0));
                }
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
        if node.kind() == "ENUM" {
            self.body_methods(index);
            self.declare_body_lambdas(index)?;
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
        // A census's rejected class keeps its fields, without their initializers.
        let rejected = self.rejected_classes.borrow().contains_key(&index);
        let javac = &self.types[index].javac;
        for member in node
            .list("members")
            .filter(|m| m.kind() == "VARIABLE" && !enums::is_enum_constant(*m) && self.keeps(*m, javac))
            .filter(|m| !self.is_handle(index, *m))
        {
            // Moved initializers run in the constructors (see `inner`).
            let field = self.field(member, !rejected && !(moved && !member.has_flag("STATIC")))?;
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
            cloneable: self.is_cloneable(index),
        })
    }

    /// A field, with its initializer when `with_init`.
    fn field(&self, node: Node<'t>, with_init: bool) -> Result<Field, Error> {
        let ty = self.signature_type(node, node.symbol().map_or("", |s| s.ty))?;
        let init = node.get("initializer").filter(|_| with_init);
        let constant = Self::is_constant(node);
        let init = if constant {
            let text = node.symbol().and_then(|s| s.constant).unwrap_or_default();
            let kind = self.constant_literal(node, ty, text)?;
            Some(self.synth(kind, node.pos(), ty))
        } else {
            match init.map(|i| self.expr(i)).transpose() {
                Ok(init) => init.map(|i| self.convert(i, ty)),
                // In a census, a rejected initializer leaves the field at its default value.
                Err(e) => {
                    self.reject(self.current.get(), e)?;
                    None
                }
            }
        };
        Ok(Field { name: node.str("name").to_string(), ty, init, constant, pos: node.pos() })
    }

    /// The place that `node` names when it is a field: `name`, `this.name`, `object.name` or
    /// `Class.name`. `None` for anything else, including fields of library classes.
    pub(super) fn field_place(&self, node: Node<'t>) -> Result<Option<Place>, Error> {
        let Some(symbol) = node.symbol().filter(|s| s.kind == "FIELD") else { return Ok(None) };
        // `System.in`, `System.out` and `System.err` as values: the translator's objects, which
        // a translation with the JDK's sources has (`program::standard_stream_field`).
        if symbol.owner == "java.lang.System"
            && let Some((holder, field)) = crate::program::standard_stream_field(symbol.name)
            && let Some(class) = self.type_index(holder)
        {
            return Ok(Some(Place::Static { class, name: field.to_string() }));
        }
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
            self.check_handle_use(node)?;
            // Java evaluates an object in front of a static field and ignores it, even null;
            // one without effects is simply dropped.
            if let Some(r) = receiver
                && r.symbol().is_none_or(|s| !matches!(s.kind, "CLASS" | "INTERFACE" | "ENUM" | "RECORD"))
                && has_effects(&self.expr(r)?)
            {
                return unsupported(node, "static fields through an expression with effects");
            }
            self.note_static_qualifier(node, receiver, symbol.owner, &name);
            return Ok(Some(Place::Static { class, name }));
        }
        let object = match receiver {
            Some(r) => self.expr(r)?,
            None => self.this(node),
        };
        Ok(Some(Place::Field { object: Box::new(object), name }))
    }

    /// What the class files name (`Class::class_file`): the static fields' classes noted while
    /// lowering, and the fields of the enclosing instances of the classes `types` declares.
    /// javac names that field after the enclosing class's own depth of enclosing instances
    /// (`Lower.outerThisName`).
    pub(super) fn class_file_names(&self, types: &[TypeDecl]) -> ClassFileNames {
        let mut outer_fields = std::collections::HashMap::new();
        for (t, info) in self.types.iter().enumerate() {
            let Some(nested) = &info.nested else { continue };
            if nested.kind == super::inner::NestKind::Lambda || types.get(t).is_none_or(|d| d.outer.is_none())
            {
                continue;
            }
            let mut depth = 0;
            let mut at = nested.enclosing;
            let known = loop {
                match &self.types[at].nested {
                    None => break true,
                    Some(n) if n.kind == super::inner::NestKind::Lambda => break false,
                    Some(n) if n.instance => {
                        depth += 1;
                        at = n.enclosing;
                    }
                    Some(_) => break true,
                }
            };
            if known {
                outer_fields.insert(types[t].name.clone(), format!("this${depth}"));
            }
        }
        ClassFileNames {
            static_qualifiers: self.static_qualifiers.take(),
            outer_fields,
            calls: self.calls.take(),
        }
    }

    /// Notes the method that the call `node` names in the class file (`Class::class_file`):
    /// javac names it by the class that the call names it in, `C.m()`, or by the erasure of the
    /// receiver's static type (`Gen.binaryQualifier`), except an instance method of `Object`,
    /// with the erasures of its parameters' types. An unqualified call names it by the class
    /// whose code calls it only when that class declares it; elsewhere javac's `Lower` decides,
    /// by rules not followed here.
    pub(super) fn note_call(&self, node: Node<'t>, symbol: &Symbol, select: Node<'t>) {
        if symbol.kind != "METHOD" {
            return;
        }
        let class = if symbol.owner == "java.lang.Object" && !symbol.is_static {
            Some(symbol.owner.to_string())
        } else {
            match select.kind() {
                "MEMBER_SELECT" => select.get("expression").and_then(|r| {
                    let receiver = r.java_type();
                    let receiver = receiver.split('<').next().unwrap_or(receiver);
                    // A class, not `super`, a type variable, a capture or an intersection.
                    let named = r.kind() != "IDENTIFIER" || r.str("name") != "super";
                    let class = receiver.contains('.')
                        && !receiver.contains([' ', '&', '?'])
                        && !receiver.starts_with("capture")
                        && !receiver.ends_with(']');
                    (named && class).then(|| receiver.to_string())
                }),
                "IDENTIFIER" => {
                    (self.types[self.current.get()].javac == symbol.owner).then(|| symbol.owner.to_string())
                }
                _ => None,
            }
        };
        // A parameter of a type variable's type is of its erasure, which its declaration tells.
        let callee = self.methods.iter().position(|m| {
            self.types[m.owner].javac == symbol.owner && m.name == symbol.name && m.sig == symbol.ty
        });
        let param = |ty: &str| {
            erased(ty).or_else(|| {
                let m = callee?;
                let var = ty.trim_end_matches("...").trim_end_matches("[]");
                let dims = ty[var.len()..].matches("[]").count() + usize::from(ty.ends_with("..."));
                let erasure = self.var_erasure_name(var, self.methods[m].owner, Some(m))?;
                Some(erasure + &"[]".repeat(dims))
            })
        };
        let params =
            method_params(symbol.ty).and_then(|p| p.iter().map(|t| param(t)).collect::<Option<Vec<_>>>());
        let named = class.zip(params).map(|(class, params)| CallName {
            class,
            name: symbol.name.to_string(),
            params,
        });
        let mut calls = self.calls.borrow_mut();
        let key = (self.types[self.current.get()].name.clone(), node.pos(), symbol.name.to_string());
        match calls.get(&key) {
            Some(known) if *known != named => {
                calls.insert(key, None);
            }
            Some(_) => {}
            None => {
                calls.insert(key, named);
            }
        }
    }

    /// Notes the class that names the static field `name` of `owner` that `node` reads, in the
    /// class file, where that is certain (`Class::static_qualifiers`): the class of `C.name`, as
    /// javac names it (`Gen.binaryQualifier`), or `owner` when the code of `owner` itself reads
    /// `name`. Elsewhere javac's `Lower` decides, by rules not followed here.
    fn note_static_qualifier(&self, node: Node<'t>, receiver: Option<Node<'t>>, owner: &str, name: &str) {
        let qualifier = match receiver {
            Some(r) => match r.symbol() {
                Some(s) if matches!(s.kind, "CLASS" | "INTERFACE" | "ENUM" | "RECORD") => Some(r.java_type()),
                _ => None,
            },
            None => (self.types[self.current.get()].javac == owner).then_some(owner),
        };
        let qualifier = qualifier.map(|q| q.split('<').next().unwrap_or(q).to_string());
        let mut qualifiers = self.static_qualifiers.borrow_mut();
        let key = (self.types[self.current.get()].name.clone(), node.pos(), name.to_string());
        match qualifiers.get(&key) {
            Some(known) if *known != qualifier => {
                qualifiers.insert(key, None);
            }
            Some(_) => {}
            None => {
                qualifiers.insert(key, qualifier);
            }
        }
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
            // A compile-time constant of a library class that a static import names
            // (`REF_invokeInterface` of `MethodHandleInfo`) is its value, as for a select.
            None if let Some(symbol) = node.symbol().filter(|s| s.kind == "FIELD" && s.is_static)
                && let Some(text) = symbol.constant =>
            {
                let ty = self.signature_type(node, symbol.ty)?;
                self.constant_literal(node, ty, text)?
            }
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
            None if node.symbol().is_some_and(|s| {
                s.owner == "java.lang.System" && crate::program::standard_stream_field(s.name).is_some()
            }) =>
            {
                let name = node.str("identifier");
                let message = format!(
                    "`System.{name}` as a value needs the JDK's sources: translate the program with \
                     `--main` and `--library`"
                );
                error(node.pos(), message)
            }
            // Another compile-time constant of a library class (`Double.PRECISION`) is its value,
            // as javac would have written it.
            None if let Some(symbol) = node.symbol().filter(|s| s.kind == "FIELD" && s.is_static)
                && let Some(text) = symbol.constant =>
            {
                let ty = self.signature_type(node, symbol.ty)?;
                self.constant_literal(node, ty, text)
            }
            // `Integer.TYPE` is the class of `int`, the same object as `int.class`.
            None if let Some(symbol) = node.symbol().filter(|s| s.kind == "FIELD" && s.is_static)
                && let Some(primitive) = super::boxing::primitive_class_field(symbol.owner, symbol.name) =>
            {
                Ok(ExprKind::Builtin(Builtin::ClassLiteral(primitive), Vec::new()))
            }
            None if let Some(owner) =
                node.symbol().map(|s| s.owner).filter(|o| self.annotation_types.contains(*o)) =>
            {
                unsupported(node, &format!("a field of the annotation type `{owner}`"))
            }
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
        let enclosing = self.anonymous_enclosing(node, class)?;
        let declared = node.list("arguments").map(|a| self.expr(a));
        let args: Vec<Expr> = enclosing.map(Ok).into_iter().chain(declared).collect::<Result<_, _>>()?;
        let args = self.packed_args(node, &symbol, args)?;
        self.check_standard_member(node, &symbol)?;
        let ctor = self.method_index(&symbol);
        // A class that a whole program does not translate is only a type there, without
        // constructors (`program::reach::stubs`): no object of it can be made.
        if ctor.is_none()
            && let Some(feature) = crate::program::untranslated(symbol.owner)
        {
            return unsupported(node, &format!("`new {}` ({feature})", symbol.owner));
        }
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

/// The parameters' types of javac's type of a method, `(int,java.util.List<T>)void` or
/// `<R>(java.util.function.Function<? super java.lang.String,? extends R>)R`.
fn method_params(ty: &str) -> Option<Vec<&str>> {
    let mut depth = 0;
    let mut start = None;
    let mut params = Vec::new();
    for (i, c) in ty.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => depth -= 1,
            '(' if depth == 0 && start.is_none() => start = Some(i + 1),
            ',' if depth == 0 && start.is_some() => {
                params.push(&ty[start?..i]);
                start = Some(i + 1);
            }
            ')' if depth == 0 => {
                let from = start?;
                if from < i {
                    params.push(&ty[from..i]);
                }
                return Some(params);
            }
            _ => {}
        }
    }
    None
}

/// The erasure of a parameter's type in javac's text: `java.util.List<T>` is `java.util.List`,
/// `T...` an array. `None` for a type variable, whose erasure the text does not tell.
fn erased(ty: &str) -> Option<String> {
    let (ty, varargs) = match ty.strip_suffix("...") {
        Some(element) => (element, true),
        None => (ty, false),
    };
    let base = ty.split('<').next().unwrap_or(ty).trim_end_matches("[]");
    let rest = &ty[base.len()..];
    let rest = rest.rsplit_once('>').map_or(rest, |(_, after)| after);
    let dims = rest.matches("[]").count() + usize::from(varargs);
    let primitive =
        matches!(base, "int" | "long" | "double" | "float" | "short" | "byte" | "char" | "boolean");
    (primitive || base.contains('.')).then(|| base.to_string() + &"[]".repeat(dims))
}
