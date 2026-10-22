//! Lowering of enums. An enum's values are a Rust enum with one variant per constant. Its fields
//! are set only by its initializers and constructor, so each field is a function of the constant:
//! the constructor's arguments and the initializers are pure expressions, which are computed
//! wherever a field is read. Anything outside that (a constructor with side effects, a field
//! assigned elsewhere, `null` enum values) is rejected.

use super::*;
use std::collections::HashMap;

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
pub(super) fn subst(e: &Expr, args: &[(&str, &Expr)]) -> Expr {
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
        bodies: &[Vec<Field>],
    ) -> Result<Vec<EnumConstant>, Error> {
        self.current.set(class);
        self.in_ctor.set(false);
        let mut constants = Vec::new();
        for (ordinal, member) in
            self.types[class].node.list("members").filter(|m| is_enum_constant(*m)).enumerate()
        {
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
                if let Some(value) = self.body_field_value(bodies, ordinal, field) {
                    values.push(value?);
                    continue;
                }
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
    pub(super) fn check_enum(
        &self,
        index: usize,
        methods: &[Method],
        writes: &mut EnumFieldWrites,
    ) -> Result<(), Error> {
        // A census's stub of a constructor sets no field: the enum keeps its fields in tables.
        let stubs = self.stubbed.borrow();
        if let Some(&stub) =
            stubs.iter().find(|&&m| m < methods.len() && methods[m].is_ctor && methods[m].class == index)
        {
            return error(methods[stub].pos, "a stub of an enum's constructor");
        }
        let ty = Type::enum_type(&self.types[index].name);
        // A field that is not final is still one value per constant if only the constructor sets it.
        for m in (0..methods.len()).filter(|&m| !(methods[m].is_ctor && methods[m].class == index)) {
            if let Some(pos) = writes.first(methods, m, ty) {
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
    /// `Enum.valueOf(c, name)`, the constant `name` of the enum whose class is `c`: a test of
    /// `c` against each enum of the program, with that enum's `valueOf`, and for any other class
    /// the JDK's failure (`NotEnumClass`). Both arguments are evaluated first, as for the call.
    fn value_of_class(&self, node: Node<'t>, args: Vec<Expr>) -> Result<ExprKind, Error> {
        let pos = node.pos();
        let ty = self.ty(node, node.java_type())?;
        let mut args = args.into_iter();
        let (class, name) = (args.next().expect("two arguments"), args.next().expect("two arguments"));
        let (class_var, name_var) = (self.fresh_name("enum_class"), self.fresh_name("name"));
        let class_read = || self.synth(ExprKind::Var(class_var.clone()), pos, Type::JavaClass);
        let class_name = self.synth(
            ExprKind::Builtin(Builtin::ClassMethod(ClassOp::GetName), vec![class_read()]),
            pos,
            Type::String,
        );
        let mut chain = ExprKind::Builtin(Builtin::NotEnumClass, vec![class_name]);
        // A class of a known enum (`Class<Color>`) is that enum's.
        let candidates = (0..self.types.len()).rev().filter(|&t| {
            self.types[t].node.kind() == "ENUM"
                && !matches!(ty, Type::Enum(name) if name != self.types[t].name)
        });
        for e in candidates {
            let enum_ty = Type::enum_type(&self.types[e].name);
            let literal = self.synth(
                ExprKind::Builtin(Builtin::ClassLiteral(enum_ty), Vec::new()),
                pos,
                Type::JavaClass,
            );
            let test = ExprKind::Binary(BinOp::Eq, Box::new(class_read()), Box::new(literal));
            let name = self.synth(ExprKind::Var(name_var.clone()), pos, Type::String);
            let constant = self.synth(ExprKind::Builtin(Builtin::EnumValueOf, vec![name]), pos, enum_ty);
            let rest = self.synth(chain, pos, ty);
            chain = ExprKind::Conditional(
                Box::new(self.synth(test, pos, Type::Boolean)),
                Box::new(self.convert(constant, ty)),
                Box::new(rest),
            );
        }
        let block = vec![
            Stmt::Local { ty: Type::JavaClass, name: class_var, init: Some(class), pos },
            Stmt::Local {
                ty: Type::String,
                name: name_var,
                init: Some(self.convert(name, Type::String)),
                pos,
            },
            Stmt::Yield(self.synth(chain, pos, ty)),
        ];
        Ok(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body: block }],
        })
    }

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
                // The constants as `Object`s where the enum's arrays are `Object[]`s (`covariant`).
                let element = self.covariant_array(Type::array(ty)).element();
                let values = (0..self.enum_constant_names(class).count())
                    .map(|ordinal| {
                        let constant = self.synth(ExprKind::EnumConstant { class, ordinal }, node.pos(), ty);
                        self.convert(constant, element)
                    })
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
        if symbol.is_static && symbol.name == "valueOf" && args.len() == 2 {
            return Ok(Some(self.value_of_class(node, std::mem::take(args))?));
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
            // `Enum`'s `equals` and `hashCode` are `Object`'s identity ones (and final), so a
            // constant of a known enum is compared and hashed as an `Object` too.
            "equals" | "hashCode" if erased || self.object.is_some() => {
                let at = self.object.expect("an `Object` has its methods");
                let (resolved, args) = match symbol.name {
                    "equals" => {
                        (at.equals, args.drain(..).map(|a| self.convert(a, Type::class(OBJECT))).collect())
                    }
                    _ => (at.hash_code, Vec::new()),
                };
                let name = symbol.name.to_string();
                let receiver = Box::new(self.convert(receiver, Type::class(OBJECT)));
                return Ok(Some(ExprKind::MethodCall {
                    receiver,
                    name,
                    args,
                    resolved: Some(resolved),
                    is_super: false,
                }));
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

/// Whether the constructor `m`, with the constructors it calls first, cannot let the object it
/// builds out before it returns: it uses `this` only to read and set its fields, calls no method
/// of an object and creates none. Then no method of a subclass runs on the object while it is
/// built, to read a field that the subclass's constructor has not set yet.
fn keeps_this(class: &Class, m: usize, depth: usize) -> bool {
    if depth > 32 {
        return false;
    }
    let (mut kept, mut this, mut fields) = (true, 0, 0);
    let mut chained = Vec::new();
    let of_this = |object: &Expr| matches!(object.kind, ExprKind::This);
    for stmt in &class.methods[m].body {
        let mut each = |e: &Expr| match &e.kind {
            ExprKind::This => this += 1,
            ExprKind::Field { object, .. } if of_this(object) => fields += 1,
            ExprKind::Assign { target: Place::Field { object, .. }, .. }
            | ExprKind::IncDec { target: Place::Field { object, .. }, .. }
                if of_this(object) =>
            {
                fields += 1
            }
            ExprKind::MethodCall { .. } | ExprKind::New { .. } => kept = false,
            ExprKind::SuperInit { resolved, .. } | ExprKind::ThisCall { resolved, .. } => {
                chained.push(*resolved)
            }
            _ => {}
        };
        visit_stmt(stmt, &mut |top| visit_expr(top, &mut each), &mut |_| {});
    }
    kept && this == fields && chained.into_iter().all(|r| r.is_some_and(|r| keeps_this(class, r, depth + 1)))
}

/// Whether `value` is the `null` literal written where an enum of type `ty` is expected.
/// Whether the instance field `name` of class `index` is set before any read: every constructor
/// begins with `this.name = value`, where `value` does not read the field, after at most a
/// `super(..)` that cannot let the object out (`keeps_this`), and no initializer of the class
/// reads it. Then the field is never `null` where Java could see it.
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
            for (i, stmt) in m.body.iter().enumerate() {
                if i == 0
                    && let Stmt::Expr(Expr { kind: ExprKind::SuperInit { args, resolved: Some(r) }, .. }) =
                        stmt
                    && !args.iter().any(|a| reads(a))
                    && keeps_this(class, *r, 0)
                {
                    continue;
                }
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

/// Rejects `null` where an enum value is expected, except in the places that may hold it (see
/// `enum_nulls`). Java gives an enum-typed field `null` until it is set: an instance field that
/// starts as `null` (it has no initializer, or `null`), unless every constructor sets it first
/// (see `set_before_use`) or it holds a captured local, is such a place, an `Option` that starts
/// as `None`. A static field must have an initializer that is not `null`.
pub(super) fn check_nulls(class: &Class) -> Result<(), Error> {
    let found: Cell<Option<Pos>> = Cell::new(None);
    let record = |pos: Pos| {
        if found.get().is_none() {
            found.set(Some(pos));
        }
    };
    let mut starting_null = Vec::new();
    for (t, decl) in class.types.iter().enumerate() {
        for f in decl.statics.iter().filter(|f| matches!(f.ty, Type::Enum(_))) {
            match &f.init {
                Some(init) if matches!(init.kind, ExprKind::Null) => record(init.pos),
                Some(_) => {}
                None => record(f.pos),
            }
        }
        // A captured local's field is set from the constructor's parameter before anything runs.
        let captured = |name: &str| decl.captures.iter().any(|c| c.field == name);
        for f in decl.fields.iter().filter(|f| matches!(f.ty, Type::Enum(_))) {
            let starts_null = match &f.init {
                Some(init) => matches!(init.kind, ExprKind::Null),
                None => !captured(&f.name) && !set_before_use(class, t, &f.name),
            };
            if starts_null {
                starting_null.push((decl.name.clone(), f.name.clone()));
            }
        }
    }
    let places = enum_nulls::places(class, starting_null);
    for m in 0..class.methods.len() {
        if let Some(pos) = enum_nulls::escape(class, m, &places) {
            record(pos);
        }
    }
    match found.get() {
        Some(pos) => error(pos, "null enums not supported yet"),
        None => Ok(()),
    }
}

/// For each method, where it first assigns a field through a value of each enum type, worked out
/// once for the method instead of once for every enum that `check_enum` checks.
#[derive(Default)]
pub(super) struct EnumFieldWrites {
    of: Vec<Option<HashMap<Type, Pos>>>,
}

impl EnumFieldWrites {
    /// Where method `m` first assigns a field through a value of type `ty`.
    fn first(&mut self, methods: &[Method], m: usize, ty: Type) -> Option<Pos> {
        if self.of.len() <= m {
            self.of.resize(m + 1, None);
        }
        let pos = self.of[m].get_or_insert_with(|| field_writes(&methods[m])).get(&ty).copied();
        debug_assert_eq!(pos, field_writes(&methods[m]).get(&ty).copied(), "a method changed unnoticed");
        pos
    }

    /// Forgets what the methods of class `class` assign, as they changed.
    pub(super) fn forget(&mut self, methods: &[Method], class: usize) {
        for (m, writes) in self.of.iter_mut().enumerate() {
            if methods.get(m).is_some_and(|method| method.class == class) {
                *writes = None;
            }
        }
    }
}

/// Where `method` first assigns a field through a value of each enum type.
fn field_writes(method: &Method) -> HashMap<Type, Pos> {
    let mut first = HashMap::new();
    for stmt in &method.body {
        visit_stmt(
            stmt,
            &mut |e| {
                visit_expr(e, &mut |x| match &x.kind {
                    ExprKind::Assign { target: Place::Field { object, .. }, .. }
                    | ExprKind::IncDec { target: Place::Field { object, .. }, .. }
                        if matches!(object.ty, Type::Enum(_)) =>
                    {
                        first.entry(object.ty).or_insert(x.pos);
                    }
                    _ => {}
                })
            },
            &mut |_| {},
        );
    }
    first
}
