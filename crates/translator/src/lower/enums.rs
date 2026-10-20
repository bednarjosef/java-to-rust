//! Lowering of enums. An enum's values are a Rust enum with one variant per constant. Its fields
//! are final and set by its constructor, so each field is a function of the constant: the
//! constructor's arguments are pure expressions, which are computed wherever a field is read.
//! Anything outside that (a constructor with side effects, mutable fields, `null` enum values) is
//! rejected.

use super::*;

/// Whether a member is one of an enum's constants (`RED`, `PLANET(3.0)`).
pub(super) fn is_enum_constant(node: Node) -> bool {
    node.symbol().is_some_and(|s| s.kind == "ENUM_CONSTANT")
}

/// Whether an expression is pure and cheap to repeat: literals, arithmetic and conditionals on
/// them, `Math` functions, and, when `params` is set, the parameters of a constructor. Division is
/// not pure because it throws on zero, which would then happen at every read instead of once.
pub(super) fn pure(e: &Expr, params: bool) -> bool {
    match &e.kind {
        ExprKind::Int(_)
        | ExprKind::Long(_)
        | ExprKind::Double(_)
        | ExprKind::Char(_)
        | ExprKind::Float(_)
        | ExprKind::Bool(_)
        | ExprKind::Str(_) => true,
        ExprKind::Var(_) => params,
        ExprKind::Unary(_, x) | ExprKind::Cast(_, x) => pure(x, params),
        ExprKind::Binary(op, a, b) => {
            !matches!(op, BinOp::Div | BinOp::Rem) && pure(a, params) && pure(b, params)
        }
        ExprKind::Conditional(c, a, b) => pure(c, params) && pure(a, params) && pure(b, params),
        ExprKind::Builtin(
            Builtin::MathAbs | Builtin::MathMax | Builtin::MathMin | Builtin::MathSqrt | Builtin::MathPow,
            args,
        ) => args.iter().all(|a| pure(a, params)),
        _ => false,
    }
}

/// A copy of a pure expression with each parameter replaced by its argument. An argument whose
/// type differs from the parameter's (javac does not show the widening) gets a cast to it. The
/// copies keep the ids of the originals, which only makes the facts about them weaker.
fn subst(e: &Expr, args: &[(&str, &Expr)]) -> Expr {
    let kind = match &e.kind {
        ExprKind::Var(name) => match args.iter().find(|(p, _)| *p == name.as_str()) {
            Some((_, arg)) if arg.ty == e.ty => return subst(arg, &[]),
            Some((_, arg)) => {
                let cast = ExprKind::Cast(e.ty, Box::new(subst(arg, &[])));
                return Expr { kind: cast, pos: e.pos, ty: e.ty, id: e.id };
            }
            None => ExprKind::Var(name.clone()),
        },
        ExprKind::Int(v) => ExprKind::Int(*v),
        ExprKind::Long(v) => ExprKind::Long(*v),
        ExprKind::Double(v) => ExprKind::Double(*v),
        ExprKind::Char(v) => ExprKind::Char(*v),
        ExprKind::Float(v) => ExprKind::Float(*v),
        ExprKind::Bool(v) => ExprKind::Bool(*v),
        ExprKind::Str(s) => ExprKind::Str(s.clone()),
        ExprKind::Unary(op, x) => ExprKind::Unary(*op, Box::new(subst(x, args))),
        ExprKind::Binary(op, a, b) => {
            ExprKind::Binary(*op, Box::new(subst(a, args)), Box::new(subst(b, args)))
        }
        ExprKind::Cast(ty, x) => ExprKind::Cast(*ty, Box::new(subst(x, args))),
        ExprKind::Conditional(c, a, b) => {
            ExprKind::Conditional(Box::new(subst(c, args)), Box::new(subst(a, args)), Box::new(subst(b, args)))
        }
        ExprKind::Builtin(builtin, xs) => ExprKind::Builtin(*builtin, xs.iter().map(|x| subst(x, args)).collect()),
        _ => unreachable!("`pure` accepts only the forms that `subst` copies"),
    };
    Expr { kind, pos: e.pos, ty: e.ty, id: e.id }
}

impl<'t> Lowering<'t> {
    /// The index of the enum with this qualified name, if the file declares one.
    pub(super) fn enum_index(&self, qualified: &str) -> Option<usize> {
        self.type_index(qualified).filter(|&t| self.types[t].node.kind() == "ENUM")
    }

    /// The names of an enum's constants, in declaration order (their ordinals).
    fn enum_constant_names(&self, class: usize) -> impl Iterator<Item = &'t str> {
        self.types[class].node.list("members").filter(|m| is_enum_constant(*m)).map(|m| m.str("name"))
    }

    /// The value of the enum constant that a symbol names, if it is one.
    pub(super) fn enum_constant(&self, symbol: &Symbol) -> Option<ExprKind> {
        if symbol.kind != "ENUM_CONSTANT" {
            return None;
        }
        let class = self.enum_index(symbol.owner)?;
        let ordinal = self.enum_constant_names(class).position(|name| name == symbol.name)?;
        Some(ExprKind::EnumConstant { class, ordinal })
    }

    /// The constants of an enum. Each has the value of every instance field: the constructor's
    /// assignment to that field with the parameters replaced by the constant's arguments, which
    /// must be pure.
    pub(super) fn enum_constants(
        &self,
        class: usize,
        fields: &[Field],
        methods: &[Method],
    ) -> Result<Vec<EnumConstant>, Error> {
        self.current.set(class);
        self.in_ctor.set(false);
        let mut constants = Vec::new();
        for member in self.types[class].node.list("members").filter(|m| is_enum_constant(*m)) {
            let init = member.child("initializer")?;
            if init.get("classBody").is_some() {
                return unsupported(member, "enum constants with bodies");
            }
            let args: Vec<Expr> = init.list("arguments").map(|a| self.expr(a)).collect::<Result<_, _>>()?;
            if let Some(arg) = args.iter().find(|a| !pure(a, false)) {
                return error(arg.pos, "enum constant arguments that are not constants");
            }
            let ctor = init.symbol().and_then(|s| self.method_index(&s)).map(|c| &methods[c]);
            let params = ctor.map_or(Vec::new(), |c| c.params.iter().map(|p| p.name.as_str()).collect());
            let bound: Vec<(&str, &Expr)> = params.into_iter().zip(&args).collect();
            let mut values = Vec::new();
            for field in fields {
                let assigned = ctor.and_then(|c| {
                    c.body.iter().find_map(|stmt| match stmt {
                        Stmt::Expr(Expr {
                            kind: ExprKind::Assign { op: None, target: Place::Field { name, .. }, value },
                            ..
                        }) if *name == field.name => Some(value.as_ref()),
                        _ => None,
                    })
                });
                let Some(value) = assigned else {
                    return error(member.pos(), "enum constructors that do not set every field");
                };
                values.push(subst(value, &bound));
            }
            constants.push(EnumConstant { name: member.str("name").to_string(), values });
        }
        Ok(constants)
    }

    /// Checks an enum's members and constructors against what the translation supports.
    pub(super) fn check_enum(&self, index: usize, methods: &[Method]) -> Result<(), Error> {
        let node = self.types[index].node;
        let method_names: Vec<&str> = node.list("members").filter(|m| m.kind() == "METHOD").map(|m| m.str("name")).collect();
        for member in node.list("members") {
            match member.kind() {
                "VARIABLE" if is_enum_constant(member) => {}
                "VARIABLE" if member.has_flag("STATIC") => {}
                "VARIABLE" if member.get("initializer").is_some() => {
                    return unsupported(member, "enum fields with initializers");
                }
                "VARIABLE" if !member.has_flag("FINAL") => {
                    return unsupported(member, "enum fields that are not final");
                }
                "VARIABLE" if method_names.contains(&member.str("name")) => {
                    return unsupported(member, "an enum field and method with the same name");
                }
                "METHOD" if member.str("name") == "toString" => {
                    return unsupported(member, "toString in an enum");
                }
                _ => {}
            }
        }
        for m in methods.iter().filter(|m| m.class == index && m.is_ctor) {
            for stmt in &m.body {
                match stmt {
                    Stmt::Expr(Expr {
                        kind: ExprKind::Assign { op: None, target: Place::Field { object, .. }, value },
                        ..
                    }) if matches!(object.kind, ExprKind::This) && pure(value, true) => {}
                    _ => return error(m.pos, "enum constructors other than field assignments"),
                }
            }
        }
        Ok(())
    }

    /// A call of an enum's methods: `values()` and `valueOf(s)` of the enum, and the methods that
    /// `java.lang.Enum` declares. `None` for any other call.
    pub(super) fn enum_call(
        &self,
        node: Node<'t>,
        symbol: &Symbol,
        receiver: Option<Node<'t>>,
        args: &mut Vec<Expr>,
    ) -> Result<Option<ExprKind>, Error> {
        if let Some(class) = self.enum_index(symbol.owner) {
            if symbol.is_static && symbol.name == "values" {
                let ty = Type::enum_type(&self.types[class].name);
                let values = (0..self.enum_constant_names(class).count())
                    .map(|ordinal| self.synth(ExprKind::EnumConstant { class, ordinal }, node.pos(), ty))
                    .collect();
                return Ok(Some(ExprKind::ArrayLit(values)));
            }
            if symbol.is_static && symbol.name == "valueOf" {
                return Ok(Some(ExprKind::Builtin(Builtin::EnumValueOf, std::mem::take(args))));
            }
        }
        let builtin = match (symbol.owner, symbol.name) {
            ("java.lang.Enum", "ordinal") => Builtin::EnumOrdinal,
            ("java.lang.Enum", "name" | "toString") => Builtin::EnumName,
            ("java.lang.Enum", "compareTo") => Builtin::EnumCompareTo,
            _ => return Ok(None),
        };
        let receiver = match receiver {
            Some(r) => self.expr(r)?,
            None => self.this(node),
        };
        args.insert(0, receiver);
        Ok(Some(ExprKind::Builtin(builtin, std::mem::take(args))))
    }
}

/// Whether `value` is the `null` literal written where an enum of type `ty` is expected.
/// Whether the instance field `name` of class `index` is set before any read: every constructor
/// begins with `this.name = value`, where `value` does not read the field, and no initializer of
/// the class reads it. Then the field is never `null` where Java could see it.
fn set_before_use(class: &Class, index: usize, name: &str) -> bool {
    let reads = |e: &Expr| {
        let mut read = false;
        visit_expr(e, &mut |x| {
            read |= matches!(&x.kind, ExprKind::Field { name: n, .. } if n == name);
        });
        read
    };
    let decl = &class.types[index];
    if decl.fields.iter().filter_map(|f| f.init.as_ref()).any(|init| {
        let mut mentioned = false;
        visit_expr(init, &mut |x| mentioned |= matches!(&x.kind, ExprKind::Var(n) if n == name));
        mentioned || reads(init)
    }) {
        return false;
    }
    let ctors: Vec<&Method> = class.methods.iter().filter(|m| m.class == index && m.is_ctor).collect();
    !ctors.is_empty()
        && ctors.iter().all(|m| {
            matches!(
                m.body.first(),
                Some(Stmt::Expr(Expr {
                    kind: ExprKind::Assign {
                        op: None,
                        target: Place::Field { object, name: n },
                        value,
                    },
                    ..
                })) if matches!(object.kind, ExprKind::This) && n == name && !reads(value)
            )
        })
}

fn null_for(value: &Expr, ty: Type) -> bool {
    matches!(value.kind, ExprKind::Null) && matches!(ty, Type::Enum(_))
}

/// Whether an expression gives `null` to an enum: in an assignment, a conditional, a cast, an
/// array, or an argument of a call.
fn null_enum_in(e: &Expr, class: &Class) -> bool {
    match &e.kind {
        ExprKind::Binary(BinOp::Eq | BinOp::Ne, a, b) => null_for(a, b.ty) || null_for(b, a.ty),
        ExprKind::Assign { value, .. } => null_for(value, e.ty),
        ExprKind::Conditional(_, a, b) => {
            matches!(e.ty, Type::Enum(_)) && (matches!(a.kind, ExprKind::Null) || matches!(b.kind, ExprKind::Null))
        }
        ExprKind::Cast(ty, x) => null_for(x, *ty),
        ExprKind::ArrayLit(elements) => {
            matches!(e.ty, Type::Array(el) if matches!(el, Type::Enum(_)))
                && elements.iter().any(|x| matches!(x.kind, ExprKind::Null))
        }
        ExprKind::NewArray(_) => matches!(e.ty, Type::Array(el) if matches!(el, Type::Enum(_))),
        ExprKind::Call { args, resolved: Some(r), .. }
        | ExprKind::MethodCall { args, resolved: Some(r), .. }
        | ExprKind::New { args, ctor: Some(r), .. }
        | ExprKind::ThisCall { args, resolved: Some(r) } => {
            args.iter().zip(&class.methods[*r].params).any(|(a, p)| null_for(a, p.ty))
        }
        _ => false,
    }
}

/// Rejects `null` where an enum value is expected. Java gives an enum-typed field `null` until it
/// is set, so an instance field without an initializer is allowed only when every constructor
/// sets it first (see `set_before_use`), and a static field must have an initializer.
pub(super) fn check_nulls(class: &Class) -> Result<(), Error> {
    let found: Cell<Option<Pos>> = Cell::new(None);
    let record = |pos: Pos| {
        if found.get().is_none() {
            found.set(Some(pos));
        }
    };
    for (t, decl) in class.types.iter().enumerate() {
        for f in decl.fields.iter().chain(&decl.statics) {
            if let Some(init) = &f.init {
                if null_for(init, f.ty) {
                    record(init.pos);
                }
            }
        }
        for f in decl.fields.iter().filter(|f| f.init.is_none() && matches!(f.ty, Type::Enum(_))) {
            if !set_before_use(class, t, &f.name) {
                record(f.pos);
            }
        }
        for f in decl.statics.iter().filter(|f| f.init.is_none() && matches!(f.ty, Type::Enum(_))) {
            record(f.pos);
        }
    }
    for m in &class.methods {
        for stmt in &m.body {
            visit_stmt(
                stmt,
                &mut |e| visit_expr(e, &mut |x| if null_enum_in(x, class) { record(x.pos) }),
                &mut |s| match s {
                    Stmt::Local { ty, init: Some(init), pos, .. } if null_for(init, *ty) => record(*pos),
                    Stmt::Return(Some(e), _) if null_for(e, m.ret) => record(e.pos),
                    _ => {}
                },
            );
        }
    }
    match found.get() {
        Some(pos) => error(pos, "null enums not supported yet"),
        None => Ok(()),
    }
}
