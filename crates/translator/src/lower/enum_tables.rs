//! Enums whose constructors do more than set fields to pure values (see `enums`): a call, a loop,
//! an argument computed by a static method or a constant of another enum, as in the JDK's
//! `StreamOpFlag` (`DISTINCT(0, set(Type.SPLITERATOR).set(Type.STREAM))`). Their fields are
//! tables, built once at the enum's static initialization, as Java builds the constants then:
//! instance field `f` becomes the static array `fOfConstants`, indexed by ordinal; each
//! constructor becomes the static method `initShapeConstant(constantOrdinal, ..)` (for an enum `Shape`) that fills the
//! constant's row; the enum's `<clinit>` creates the tables and calls it for each constant, in
//! order, before its other static initializers; and every read or write of `e.f` becomes
//! `fOfConstants[e.ordinal()]`. An enum that fits the pure form keeps it, so its fields stay
//! `match`es on the constant.

use super::*;

/// The name of the static array that holds field `field` of every constant.
fn table(field: &str) -> String {
    format!("{field}OfConstants")
}

/// The field whose table is the static array `name`, if it is one.
pub(crate) fn tabled_field(name: &str) -> Option<&str> {
    name.strip_suffix("OfConstants").filter(|f| !f.is_empty())
}

/// The parameter of a constant builder that names the constant it builds.
pub(crate) const ORDINAL: &str = "constantOrdinal";

impl<'t> Lowering<'t> {
    /// Gives enum `index` the table form: its fields become static arrays, its constructors
    /// static methods, and its constants are built by its `<clinit>`. Every method of the program
    /// reads and writes the fields through the tables.
    pub(super) fn enum_tables(
        &self,
        index: usize,
        types: &mut [TypeDecl],
        methods: &mut Vec<Method>,
    ) -> Result<(), Error> {
        self.current.set(index);
        self.in_ctor.set(false);
        let pos = self.types[index].node.pos();
        let enum_ty = Type::enum_type(&self.types[index].name);
        let (builder, fields_builder) =
            (constant_builder(&self.types[index].name), fields_builder(&self.types[index].name));
        let fields = std::mem::take(&mut types[index].fields);
        let count = self.enum_constant_names(index).count();
        // The tables, at Java's default values: an enum-typed field holds a constant until its
        // constructor sets it, as the translation has no null enum.
        let mut creates = Vec::new();
        for field in &fields {
            let array = Type::array(field.ty);
            let init = match field.ty {
                Type::Enum(name) => {
                    let Some(e) =
                        self.enum_index(name).filter(|&e| self.enum_constant_names(e).next().is_some())
                    else {
                        return error(field.pos, "an enum field of an enum without constants");
                    };
                    if field.init.is_none() && !ctors_set(methods, index, &field.name) {
                        return error(
                            field.pos,
                            "an enum field of an enum that a constructor may leave unset",
                        );
                    }
                    let first =
                        || self.synth(ExprKind::EnumConstant { class: e, ordinal: 0 }, field.pos, field.ty);
                    ExprKind::ArrayLit((0..count).map(|_| first()).collect())
                }
                _ => ExprKind::NewArray(vec![self.synth(ExprKind::Int(count as i32), field.pos, Type::Int)]),
            };
            let init = self.synth(init, field.pos, array);
            let name = table(&field.name);
            let target = Place::Static { class: index, name: name.clone() };
            let assign = ExprKind::Assign { op: None, target, value: Box::new(init) };
            creates.push(Stmt::Expr(self.synth(assign, field.pos, array)));
            types[index].statics.push(Field { name, ty: array, init: None, constant: false, pos: field.pos });
        }
        let names: Vec<String> = fields.iter().map(|f| f.name.clone()).collect();
        let has_fields: std::collections::HashSet<&str> = names.iter().map(String::as_str).collect();
        // The field initializers, which every constructor that does not call `this(..)` runs
        // first: a static method of their own.
        let initializers: Vec<(String, Expr)> =
            fields.into_iter().filter_map(|f| f.init.map(|init| (f.name, init))).collect();
        let fields_init = (!initializers.is_empty()).then(|| {
            let body = initializers
                .into_iter()
                .map(|(name, value)| {
                    let ty = value.ty;
                    let this = self.synth(ExprKind::This, value.pos, enum_ty);
                    let target = Place::Field { object: Box::new(this), name };
                    let pos = value.pos;
                    Stmt::Expr(self.synth(
                        ExprKind::Assign { op: None, target, value: Box::new(value) },
                        pos,
                        ty,
                    ))
                })
                .collect();
            methods.push(Method {
                name: fields_builder.clone(),
                params: vec![Param { name: ORDINAL.to_string(), ty: Type::Int }],
                ret: Type::Void,
                body,
                is_main: false,
                is_static: true,
                is_ctor: false,
                class: index,
                pos,
                is_abstract: false,
                is_synchronized: false,
                overrides: Vec::new(),
                type_params: Vec::new(),
            });
            methods.len() - 1
        });
        let ctors: Vec<usize> =
            (0..methods.len()).filter(|&m| methods[m].class == index && methods[m].is_ctor).collect();
        // Every field access on a value of this enum goes through the tables.
        for method in methods.iter_mut() {
            for stmt in &mut method.body {
                rewrite_stmt(stmt, &mut |e| self.table_access(e, index, &has_fields));
            }
        }
        for &c in ctors.iter().chain(&fields_init) {
            let method = &mut methods[c];
            let starts_with_this_call = delegates(&method.body);
            if method.is_ctor {
                method.is_ctor = false;
                method.is_static = true;
                method.name = builder.clone();
                method.params.insert(0, Param { name: ORDINAL.to_string(), ty: Type::Int });
                if let (Some(f), false) = (fields_init, starts_with_this_call) {
                    let ordinal = self.synth(ExprKind::Var(ORDINAL.to_string()), method.pos, Type::Int);
                    let call = ExprKind::Call {
                        name: fields_builder.clone(),
                        args: vec![ordinal],
                        resolved: Some(f),
                    };
                    method.body.insert(0, Stmt::Expr(self.synth(call, method.pos, Type::Void)));
                }
            }
            for stmt in &mut method.body {
                rewrite_stmt(stmt, &mut |e| self.constant_init(e, &builder));
            }
            // Any other `this` is the constant being built: `values()[constantOrdinal]`.
            for stmt in &mut method.body {
                rewrite_stmt(stmt, &mut |e| {
                    if matches!(e.kind, ExprKind::This) {
                        let constants = (0..count)
                            .map(|ordinal| {
                                self.synth(ExprKind::EnumConstant { class: index, ordinal }, e.pos, enum_ty)
                            })
                            .collect();
                        let values = self.synth(ExprKind::ArrayLit(constants), e.pos, Type::array(enum_ty));
                        let ordinal = self.synth(ExprKind::Var(ORDINAL.to_string()), e.pos, Type::Int);
                        e.kind = ExprKind::Index(Box::new(values), Box::new(ordinal));
                    }
                });
            }
        }
        // The constants, built in order before the rest of the static initialization.
        for (ordinal, member) in
            self.types[index].node.list("members").filter(|m| enums::is_enum_constant(*m)).enumerate()
        {
            let init = member.child("initializer")?;
            let args: Vec<Expr> = init.list("arguments").map(|a| self.expr(a)).collect::<Result<_, _>>()?;
            let Some(ctor) = init.symbol().and_then(|s| self.enum_constructor(index, &s)) else {
                return unsupported(member, "this enum constant");
            };
            let mut args = self.convert_args(Some(ctor), args)?;
            args.insert(0, self.synth(ExprKind::Int(ordinal as i32), member.pos(), Type::Int));
            let call = ExprKind::Call { name: builder.clone(), args, resolved: Some(ctor) };
            creates.push(Stmt::Expr(self.synth(call, member.pos(), Type::Void)));
        }
        match methods.iter().position(|m| m.class == index && m.name == "<clinit>") {
            Some(clinit) => {
                let rest = std::mem::take(&mut methods[clinit].body);
                creates.extend(rest);
                methods[clinit].body = creates;
            }
            None => methods.push(Method {
                name: "<clinit>".to_string(),
                params: Vec::new(),
                ret: Type::Void,
                body: creates,
                is_main: false,
                is_static: true,
                is_ctor: false,
                class: index,
                pos,
                is_abstract: false,
                is_synchronized: false,
                overrides: Vec::new(),
                type_params: Vec::new(),
            }),
        }
        Ok(())
    }

    /// `e.f` and `e.f = v` of a field `f` of enum `index` as `fOfConstants[e.ordinal()]`.
    fn table_access(&self, e: &mut Expr, index: usize, fields: &std::collections::HashSet<&str>) {
        let on_enum = |object: &Expr, name: &str| {
            matches!(object.ty, Type::Enum(n) if self.enum_index(n) == Some(index)) && fields.contains(name)
        };
        let row = |object: Expr, name: &str, ty: Type| {
            let pos = object.pos;
            let array =
                self.synth(ExprKind::Static { class: index, name: table(name) }, pos, Type::array(ty));
            let ordinal = self.synth(ExprKind::Builtin(Builtin::EnumOrdinal, vec![object]), pos, Type::Int);
            (array, ordinal)
        };
        let (pos, ty) = (e.pos, e.ty);
        match &mut e.kind {
            ExprKind::Field { object, name } if on_enum(object, name) => {
                let object = std::mem::replace(object.as_mut(), self.synth(ExprKind::Null, pos, Type::Null));
                let (array, ordinal) = row(object, &name.clone(), ty);
                e.kind = ExprKind::Index(Box::new(array), Box::new(ordinal));
            }
            ExprKind::Assign { target, .. } | ExprKind::IncDec { target, .. } => {
                if let Place::Field { object, name } = target
                    && on_enum(object, name)
                {
                    let ty = self.enum_field_type(index, name);
                    let object =
                        std::mem::replace(object.as_mut(), self.synth(ExprKind::Null, pos, Type::Null));
                    let (array, ordinal) = row(object, &name.clone(), ty);
                    *target = Place::Index(Box::new(array), Box::new(ordinal));
                }
            }
            _ => {}
        }
    }

    /// The type of field `name` of enum `index`, from its declaration.
    fn enum_field_type(&self, index: usize, name: &str) -> Type {
        self.types[index]
            .node
            .list("members")
            .find(|m| m.kind() == "VARIABLE" && m.str("name") == name)
            .and_then(|m| self.ty(m, m.symbol().map_or("", |s| s.ty)).ok())
            .unwrap_or(Type::Int)
    }

    /// In a constructor become a constant builder: `this`'s ordinal as the `constantOrdinal`
    /// parameter, and `this(..)` as a call of the other builder.
    fn constant_init(&self, e: &mut Expr, builder: &str) {
        let pos = e.pos;
        match &mut e.kind {
            ExprKind::Builtin(Builtin::EnumOrdinal, args)
                if matches!(args[..], [Expr { kind: ExprKind::This, .. }]) =>
            {
                e.kind = ExprKind::Var(ORDINAL.to_string());
            }
            ExprKind::ThisCall { args, resolved } => {
                let mut args = std::mem::take(args);
                args.insert(0, self.synth(ExprKind::Var(ORDINAL.to_string()), pos, Type::Int));
                e.kind = ExprKind::Call { name: builder.to_string(), args, resolved: *resolved };
            }
            _ => {}
        }
    }
}

/// Whether every constructor of enum `index` begins by calling another (`this(..)`) or sets
/// field `name` in a statement of its own.
fn ctors_set(methods: &[Method], index: usize, name: &str) -> bool {
    methods.iter().filter(|m| m.class == index && m.is_ctor).all(|m| {
        m.body.iter().any(|s| match s {
            Stmt::Expr(Expr { kind: ExprKind::ThisCall { .. }, .. }) => true,
            Stmt::Expr(Expr {
                kind: ExprKind::Assign { op: None, target: Place::Field { object, name: n }, .. },
                ..
            }) => matches!(object.kind, ExprKind::This) && n == name,
            _ => false,
        })
    })
}

/// Calls `f` on every expression in `stmt`, inner expressions first.
fn rewrite_stmt(stmt: &mut Stmt, f: &mut dyn FnMut(&mut Expr)) {
    match stmt {
        Stmt::Local { init: Some(e), .. } | Stmt::Expr(e) | Stmt::Return(Some(e), _) => rewrite(e, f),
        Stmt::Yield(e) | Stmt::Throw(e) => rewrite(e, f),
        Stmt::If { cond, then, otherwise } => {
            rewrite(cond, f);
            rewrite_stmt(then, f);
            if let Some(o) = otherwise {
                rewrite_stmt(o, f);
            }
        }
        Stmt::While { cond, body } | Stmt::DoWhile { body, cond } => {
            rewrite(cond, f);
            rewrite_stmt(body, f);
        }
        Stmt::For { init, cond, update, body } => {
            init.iter_mut().for_each(|s| rewrite_stmt(s, f));
            if let Some(cond) = cond {
                rewrite(cond, f);
            }
            update.iter_mut().for_each(|e| rewrite(e, f));
            rewrite_stmt(body, f);
        }
        Stmt::ForEach { array, body, .. } => {
            rewrite(array, f);
            rewrite_stmt(body, f);
        }
        Stmt::Block(stmts) => stmts.iter_mut().for_each(|s| rewrite_stmt(s, f)),
        Stmt::Switch { selector, cases } => {
            rewrite(selector, f);
            rewrite_cases(cases, f);
        }
        Stmt::Labeled { body, .. } => rewrite_stmt(body, f),
        Stmt::Try { body, catches, finally } => {
            body.iter_mut().for_each(|s| rewrite_stmt(s, f));
            for catch in catches {
                catch.body.iter_mut().for_each(|s| rewrite_stmt(s, f));
            }
            finally.iter_mut().flatten().for_each(|s| rewrite_stmt(s, f));
        }
        Stmt::Local { init: None, .. }
        | Stmt::Return(None, _)
        | Stmt::Break
        | Stmt::Continue
        | Stmt::BreakLabel(_)
        | Stmt::ContinueLabel(_) => {}
    }
}

fn rewrite_cases(cases: &mut [Case], f: &mut dyn FnMut(&mut Expr)) {
    for case in cases {
        for label in &mut case.labels {
            if let CaseLabel::Value(e) = label {
                rewrite(e, f);
            }
        }
        case.body.iter_mut().for_each(|s| rewrite_stmt(s, f));
    }
}

/// Calls `f` on every expression inside `e`, then on `e`.
pub(super) fn rewrite(e: &mut Expr, f: &mut dyn FnMut(&mut Expr)) {
    match &mut e.kind {
        ExprKind::Unary(_, x)
        | ExprKind::Cast(_, x)
        | ExprKind::InstanceOf { value: x, .. }
        | ExprKind::Length(x)
        | ExprKind::Field { object: x, .. } => rewrite(x, f),
        ExprKind::Assign { target, value, .. } => {
            rewrite_place(target, f);
            rewrite(value, f);
        }
        ExprKind::IncDec { target, .. } => rewrite_place(target, f),
        ExprKind::Binary(_, l, r) | ExprKind::Index(l, r) => {
            rewrite(l, f);
            rewrite(r, f);
        }
        ExprKind::NewArray(items) | ExprKind::ArrayLit(items) => items.iter_mut().for_each(|a| rewrite(a, f)),
        ExprKind::Conditional(c, a, b) => {
            rewrite(c, f);
            rewrite(a, f);
            rewrite(b, f);
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            rewrite(receiver, f);
            args.iter_mut().for_each(|a| rewrite(a, f));
        }
        ExprKind::Call { args, .. }
        | ExprKind::Builtin(_, args)
        | ExprKind::New { args, .. }
        | ExprKind::ThisCall { args, .. }
        | ExprKind::SuperInit { args, .. } => args.iter_mut().for_each(|a| rewrite(a, f)),
        ExprKind::Switch { selector, cases } => {
            rewrite(selector, f);
            rewrite_cases(cases, f);
        }
        _ => {}
    }
    f(e);
}

fn rewrite_place(place: &mut Place, f: &mut dyn FnMut(&mut Expr)) {
    match place {
        Place::Index(array, index) => {
            rewrite(array, f);
            rewrite(index, f);
        }
        Place::Field { object, .. } => rewrite(object, f),
        Place::Var(_) | Place::Static { .. } => {}
    }
}
