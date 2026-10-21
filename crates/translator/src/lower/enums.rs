//! Lowering of enums. An enum's values are a Rust enum with one variant per constant. Its fields
//! are set only by its initializers and constructor, so each field is a function of the constant:
//! the constructor's arguments and the initializers are pure expressions, which are computed
//! wherever a field is read. Anything outside that (a constructor with side effects, a field
//! assigned elsewhere, `null` enum values) is rejected.

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
        ExprKind::Conditional(c, a, b) => ExprKind::Conditional(
            Box::new(subst(c, args)),
            Box::new(subst(a, args)),
            Box::new(subst(b, args)),
        ),
        ExprKind::Builtin(builtin, xs) => {
            ExprKind::Builtin(*builtin, xs.iter().map(|x| subst(x, args)).collect())
        }
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
    pub(super) fn enum_constant_names(&self, class: usize) -> impl Iterator<Item = &'t str> {
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
            let args: Vec<Expr> = init.list("arguments").map(|a| self.expr(a)).collect::<Result<_, _>>()?;
            if let Some(arg) = args.iter().find(|a| !pure(a, false)) {
                return error(arg.pos, "enum constant arguments that are not constants");
            }
            let ctor = init.symbol().and_then(|s| self.enum_constructor(class, &s)).map(|c| &methods[c]);
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
                // The constructor runs after the initializer, so its value is the one that stays.
                let value = match (assigned, &field.init) {
                    (Some(value), _) => subst(value, &bound),
                    (None, Some(init)) if pure(init, false) => subst(init, &[]),
                    (None, Some(init)) => {
                        return error(init.pos, "enum field initializers that are not constants");
                    }
                    (None, None) => {
                        return error(member.pos(), "enum constructors that do not set every field");
                    }
                };
                values.push(value);
            }
            constants.push(EnumConstant { name: member.str("name").to_string(), values });
        }
        Ok(constants)
    }

    /// Checks an enum's members and constructors against what the translation supports.
    pub(super) fn check_enum(&self, index: usize, methods: &[Method]) -> Result<(), Error> {
        let ty = Type::enum_type(&self.types[index].name);
        // A field that is not final is still one value per constant if only the constructor sets it.
        for m in methods.iter().filter(|m| !(m.is_ctor && m.class == index)) {
            let mut assigned = None;
            for stmt in &m.body {
                visit_stmt(
                    stmt,
                    &mut |e| {
                        visit_expr(e, &mut |x| match &x.kind {
                            ExprKind::Assign { target: Place::Field { object, .. }, .. }
                            | ExprKind::IncDec { target: Place::Field { object, .. }, .. }
                                if object.ty == ty =>
                            {
                                assigned = assigned.or(Some(x.pos));
                            }
                            _ => {}
                        })
                    },
                    &mut |_| {},
                );
            }
            if let Some(pos) = assigned {
                return error(pos, "assigning an enum field outside its constructor not supported yet");
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
        if symbol.owner != "java.lang.Enum" {
            return Ok(None);
        }
        let receiver = match receiver {
            Some(r) => self.expr(r)?,
            None => self.this(node),
        };
        // An `Enum` whose enum the type does not say is an `Object` or an `Enum` (see `ty`);
        // its `toString` is the one of its enum.
        let erased_ty = receiver.ty;
        let erased = Self::is_object(erased_ty) || matches!(erased_ty, Type::Class(name, _) if name == ENUM);
        let builtin = match symbol.name {
            "ordinal" => Builtin::EnumOrdinal,
            "toString" if erased => {
                let receiver = self.convert(receiver, Type::class(OBJECT));
                return Ok(Some(ExprKind::Builtin(Builtin::ObjectText, vec![receiver])));
            }
            "name" | "toString" => Builtin::EnumName,
            "compareTo" => Builtin::EnumCompareTo,
            // The class of an enum's constant is its enum's, a constant body's too.
            "getDeclaringClass" => return Ok(Some(ExprKind::Builtin(Builtin::GetClass, vec![receiver]))),
            "equals" | "hashCode" if erased => {
                let at = self.object.expect("an `Object` has its methods");
                let (resolved, args) = match symbol.name {
                    "equals" => (at.equals, args.drain(..).map(|a| self.convert(a, Type::class(OBJECT))).collect()),
                    _ => (at.hash_code, Vec::new()),
                };
                let name = symbol.name.to_string();
                let receiver = Box::new(self.convert(receiver, Type::class(OBJECT)));
                return Ok(Some(ExprKind::MethodCall { receiver, name, args, resolved: Some(resolved), is_super: false }));
            }
            _ => return Ok(None),
        };
        if erased {
            for a in args.iter_mut() {
                let value = std::mem::replace(a, self.synth(ExprKind::Null, node.pos(), Type::Null));
                *a = self.convert(value, erased_ty);
            }
        }
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
    // The constructor's first statements set fields of `this` to values that call nothing (and
    // so cannot read the field either), and one of them sets this field.
    let calls = |e: &Expr| {
        let mut call = false;
        visit_expr(e, &mut |x| {
            call |=
                matches!(x.kind, ExprKind::MethodCall { .. } | ExprKind::Call { .. } | ExprKind::New { .. });
        });
        call
    };
    !ctors.is_empty()
        && ctors.iter().all(|m| {
            for stmt in &m.body {
                let Stmt::Expr(Expr {
                    kind: ExprKind::Assign { op: None, target: Place::Field { object, name: n }, value },
                    ..
                }) = stmt
                else {
                    return false;
                };
                if !matches!(object.kind, ExprKind::This) || reads(value) || calls(value) {
                    return false;
                }
                if n == name {
                    return true;
                }
            }
            false
        })
}

/// Rejects `null` where an enum value is expected, except in a nullable enum local (see
/// `enum_nulls`). Java gives an enum-typed field `null` until it is set, so an instance field
/// without an initializer is allowed only when every constructor sets it first (see
/// `set_before_use`), and a static field must have an initializer.
pub(super) fn check_nulls(class: &Class) -> Result<(), Error> {
    let found: Cell<Option<Pos>> = Cell::new(None);
    let record = |pos: Pos| {
        if found.get().is_none() {
            found.set(Some(pos));
        }
    };
    for (t, decl) in class.types.iter().enumerate() {
        for f in decl.fields.iter().chain(&decl.statics) {
            if let Some(init) = &f.init
                && matches!(init.kind, ExprKind::Null)
                && matches!(f.ty, Type::Enum(_))
            {
                record(init.pos);
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
    for m in 0..class.methods.len() {
        if let Some(pos) = enum_nulls::escape(class, m) {
            record(pos);
        }
    }
    match found.get() {
        Some(pos) => error(pos, "null enums not supported yet"),
        None => Ok(()),
    }
}
