//! Enums whose constructors do more than set fields to pure values (see `enums`): a call, a loop,
//! an argument computed by a static method or a constant of another enum, as in the JDK's
//! `StreamOpFlag` (`DISTINCT(0, set(Type.SPLITERATOR).set(Type.STREAM))`). Their fields are
//! tables, built once at the enum's static initialization, as Java builds the constants then:
//! instance field `f` becomes the static array `fOfConstants`, indexed by ordinal; each
//! constructor becomes the static method `initShapeConstant(constantOrdinal, ..)` (for an enum `Shape`) that fills the
//! constant's row; the enum's `<clinit>` creates the tables and calls it for each constant, in
//! order, before its other static initializers; and every read or write of `e.f` becomes
//! `fOfConstants[e.ordinal()]`. The fields that a constant's body declares are tables too, which
//! the body's initializers fill right after the constant's constructor, as Java's constructor of
//! the body's class runs them after the enum's. An enum that fits the pure form keeps it, so its
//! fields stay `match`es on the constant.

use super::*;
use std::collections::HashSet;

/// The name of the static array that holds field `field` of every constant.
fn table(field: &str) -> String {
    format!("{field}OfConstants")
}

/// The field whose table is the static array `name`, if it is one.
pub(crate) fn tabled_field(name: &str) -> Option<&str> {
    name.strip_suffix("OfConstants").filter(|f| !f.is_empty())
}

/// For each method, the enums whose fields it reads or writes through a value of theirs, worked out
/// once for the method instead of once for every enum that takes the table form: on the Minecraft
/// server, rewriting every method for each such enum was a large part of lowering. A table form
/// only takes the accesses of its own enum out of other classes' methods, and the methods of its
/// enum are forgotten after it (`forget`), so what is kept stays true for the enums still to come.
#[derive(Default)]
pub(super) struct EnumFieldUses {
    of: Vec<Option<HashSet<usize>>>,
}

impl EnumFieldUses {
    /// Whether method `m` reads or writes a field through a value of enum `index`.
    fn uses(&mut self, lowering: &Lowering, methods: &[Method], m: usize, index: usize) -> bool {
        if self.of.len() <= m {
            self.of.resize(m + 1, None);
        }
        let uses = self.of[m].get_or_insert_with(|| lowering.field_uses(&methods[m])).contains(&index);
        debug_assert!(
            uses || !lowering.field_uses(&methods[m]).contains(&index),
            "a method changed unnoticed"
        );
        uses
    }

    /// Forgets what the methods of class `class` use, as they changed.
    pub(super) fn forget(&mut self, methods: &[Method], class: usize) {
        for (m, uses) in self.of.iter_mut().enumerate() {
            if methods.get(m).is_some_and(|method| method.class == class) {
                *uses = None;
            }
        }
    }
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
        bodies: Vec<Vec<Field>>,
        uses: &mut EnumFieldUses,
        clinit_ctors: &super::clinit_errors::ClinitCtors,
    ) -> Result<(), Error> {
        self.current.set(index);
        self.in_ctor.set(false);
        let pos = self.types[index].node.pos();
        let enum_ty = Type::enum_type(&self.types[index].name);
        let (builder, fields_builder) =
            (constant_builder(&self.types[index].name), fields_builder(&self.types[index].name));
        let fields = std::mem::take(&mut types[index].fields);
        let count = self.enum_constant_names(index).count();
        // The tables, at Java's default values. An enum-typed field that every constructor sets
        // holds a constant until then, so that its table's elements need not be `Option`s; one
        // that a constructor may leave unset starts as null.
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
                        ExprKind::NewArray(vec![self.synth(
                            ExprKind::Int(count as i32),
                            field.pos,
                            Type::Int,
                        )])
                    } else {
                        let first = || {
                            self.synth(ExprKind::EnumConstant { class: e, ordinal: 0 }, field.pos, field.ty)
                        };
                        ExprKind::ArrayLit((0..count).map(|_| first()).collect())
                    }
                }
                _ => ExprKind::NewArray(vec![self.synth(ExprKind::Int(count as i32), field.pos, Type::Int)]),
            };
            let init = self.synth(init, field.pos, array);
            let name = table(&field.name);
            let target = Place::Static { class: index, name: name.clone() };
            let assign = ExprKind::Assign { op: None, target, value: Box::new(init) };
            creates.push(Stmt::Expr(self.synth(assign, field.pos, array)));
            types[index].statics.push(Field {
                name,
                ty: array,
                init: None,
                constant: false,
                volatile: false,
                // Only the enum's `<clinit>` assigns the table, once; its rows are filled after.
                is_final: true,
                pos: field.pos,
            });
        }
        // Each field's type, a field of a constant's body's too.
        let has_fields: std::collections::HashMap<String, Type> =
            fields.iter().map(|f| (f.name.clone(), f.ty)).collect();
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
                jdk_code: false,
                field_update: None,
            });
            methods.len() - 1
        });
        let ctors: Vec<usize> =
            (0..methods.len()).filter(|&m| methods[m].class == index && methods[m].is_ctor).collect();
        // Every field access on a value of this enum goes through the tables.
        for m in 0..methods.len() {
            if !uses.uses(self, methods, m, index) {
                continue;
            }
            for stmt in &mut methods[m].body {
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
        // The constants, built in order before the rest of the static initialization, each
        // followed by the initializers of its body's fields, where `this` is the constant.
        let mut bodies = bodies.into_iter();
        for (ordinal, member) in
            self.types[index].node.list("members").filter(|m| enums::is_enum_constant(*m)).enumerate()
        {
            let body_fields = bodies.next().unwrap_or_default();
            let init = member.child("initializer")?;
            let args: Vec<Expr> = init.list("arguments").map(|a| self.expr(a)).collect::<Result<_, _>>()?;
            let Some(symbol) = init.symbol() else { return unsupported(member, "this enum constant") };
            let Some(ctor) = self.enum_constructor(index, &symbol) else {
                return unsupported(member, "this enum constant");
            };
            // A constructor's varargs, which a constant may pass none of.
            let args = self.packed_args_of(init, ctor, args)?;
            let mut args = self.convert_args(Some(ctor), args)?;
            args.insert(0, self.synth(ExprKind::Int(ordinal as i32), member.pos(), Type::Int));
            let call = ExprKind::Call { name: builder.clone(), args, resolved: Some(ctor) };
            creates.push(Stmt::Expr(self.synth(call, member.pos(), Type::Void)));
            for field in body_fields {
                let Some(value) = field.init else { continue };
                let (pos, ty) = (field.pos, field.ty);
                let this = self.synth(ExprKind::EnumConstant { class: index, ordinal }, pos, enum_ty);
                let target = Place::Field { object: Box::new(this), name: field.name };
                let mut init = Stmt::Expr(self.synth(
                    ExprKind::Assign { op: None, target, value: Box::new(value) },
                    pos,
                    ty,
                ));
                rewrite_stmt(&mut init, &mut |e| {
                    if matches!(e.kind, ExprKind::This) {
                        e.kind = ExprKind::EnumConstant { class: index, ordinal };
                    }
                });
                rewrite_stmt(&mut init, &mut |e| self.table_access(e, index, &has_fields));
                creates.push(init);
            }
        }
        // A `<clinit>` that keeps the state of its class's initialization runs its statements in
        // a `try` (`lower/clinit_errors.rs`), which the constants are built in too, so that their
        // failure is the class's.
        match methods.iter().position(|m| m.class == index && m.name == "<clinit>") {
            Some(clinit) if rewritten_clinit(&types[index]) => {
                let Some(Stmt::Try { body, .. }) = methods[clinit].body.last_mut() else {
                    return error(pos, "an enum's rewritten static initializer without its `try`");
                };
                creates.append(body);
                *body = creates;
            }
            Some(clinit) => {
                let rest = std::mem::take(&mut methods[clinit].body);
                creates.extend(rest);
                methods[clinit].body = creates;
            }
            None => {
                let mut clinit = Method {
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
                    jdk_code: false,
                    field_update: None,
                };
                self.guard_clinit(index, &mut clinit, &mut types[index], clinit_ctors);
                methods.push(clinit);
            }
        }
        Ok(())
    }

    /// `e.f` and `e.f = v` of a field `f` of enum `index` as `fOfConstants[e.ordinal()]`.
    /// The enums through a value of which `method` reads or writes a field, as `table_access`
    /// finds them.
    fn field_uses(&self, method: &Method) -> HashSet<usize> {
        let mut uses = HashSet::new();
        let mut note = |object: &Expr| {
            if let Type::Enum(n) = object.ty
                && let Some(e) = self.enum_index(n)
            {
                uses.insert(e);
            }
        };
        for stmt in &method.body {
            visit_stmt(
                stmt,
                &mut |e| {
                    visit_expr(e, &mut |x| match &x.kind {
                        ExprKind::Field { object, .. }
                        | ExprKind::Assign { target: Place::Field { object, .. }, .. }
                        | ExprKind::IncDec { target: Place::Field { object, .. }, .. } => note(object),
                        _ => {}
                    })
                },
                &mut |_| {},
            );
        }
        uses
    }

    fn table_access(&self, e: &mut Expr, index: usize, fields: &std::collections::HashMap<String, Type>) {
        let on_enum = |object: &Expr, name: &str| {
            matches!(object.ty, Type::Enum(n) if self.enum_index(n) == Some(index))
                && fields.contains_key(name)
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
                    let ty = fields[name.as_str()];
                    let object =
                        std::mem::replace(object.as_mut(), self.synth(ExprKind::Null, pos, Type::Null));
                    let (array, ordinal) = row(object, &name.clone(), ty);
                    *target = Place::Index(Box::new(array), Box::new(ordinal));
                }
            }
            _ => {}
        }
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
pub(super) fn rewrite_stmt(stmt: &mut Stmt, f: &mut dyn FnMut(&mut Expr)) {
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
