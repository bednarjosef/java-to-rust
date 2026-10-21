//! Code generation for classes with state. A class is a struct, and an object is an
//! `Rc<RefCell<C>>` that every reference shares; a field access borrows it for one block, so
//! that no `RefCell` borrow outlives the expression that made it. Static fields live in
//! thread-local cells, which are initialized on first use.

use super::statics::is_cell;
use super::*;

/// What an assignment, `++` or `--` does to a field or static field.
pub(super) enum Update<'a> {
    Assign(&'a Expr),
    Compound(BinOp, &'a Expr),
    Step(bool),
}

/// A field or static field that an update reads and writes.
enum Slot {
    /// A field of an object; `plain` when the object is an immutable class's `this` in its constructor.
    Field {
        obj: String,
        field: String,
        plain: bool,
        /// Whether the object is a value of an enum, whose fields are borrowed with `base()`.
        on_enum: bool,
    },
    /// A static field: a `Cell` when `plain`, else a `RefCell`.
    Static { cell: String, plain: bool },
}

impl Slot {
    fn read(&self, clone: bool) -> String {
        let copy = !clone;
        match self {
            Slot::Field { obj, field, plain: true, .. } => {
                format!("{obj}.{field}{}", if copy { "" } else { ".clone()" })
            }
            Slot::Field { obj, field, plain: false, on_enum } => {
                let borrow = if *on_enum { ".base()" } else { ".borrow()" };
                format!("{obj}{borrow}.{field}{}", if copy { "" } else { ".clone()" })
            }
            Slot::Static { cell, plain: true } => format!("{cell}.get()"),
            Slot::Static { cell, plain: false } => {
                format!("{cell}.with_borrow({})", if copy { "|v| *v" } else { "|v| v.clone()" })
            }
        }
    }

    fn write(&self, value: &str) -> String {
        match self {
            Slot::Field { obj, field, plain: true, .. } => format!("{obj}.{field} = {value};"),
            Slot::Field { obj, field, plain: false, on_enum } => {
                let borrow = if *on_enum { ".base_mut()" } else { ".borrow_mut()" };
                format!("{obj}{borrow}.{field} = {value};")
            }
            Slot::Static { cell, plain: true } => format!("{cell}.set({value});"),
            Slot::Static { cell, plain: false } => format!("{cell}.with_borrow_mut(|v| *v = {value});"),
        }
    }
}

/// Whether values of this type are owned heap data or shared arrays or objects, which are cloned
/// when read.
pub(super) fn needs_clone(ty: Type) -> bool {
    matches!(ty, Type::String | Type::StringBuilder | Type::Array(_) | Type::Class(..))
}

/// The Rust name of a class: its name in its package without the dots (`Outer.Inner` is
/// `OuterInner`), after the path of its
/// module when that is not the module being written (see `modules`).
pub(super) fn struct_name(name: &str) -> String {
    // A type variable of a Rust generic is the Rust type parameter.
    if let Some((_, var)) = name.rsplit_once('#') {
        return var.to_string();
    }
    let (path, local) = modules::split(name);
    path + &local.replace('.', "")
}

/// The thread-local cell that holds a static field.
pub(super) fn cell_name(class: &str, field: &str) -> String {
    let (path, local) = modules::split(class);
    path + &format!("{}_{}", local.replace('.', "_"), snake_case(field)).to_uppercase()
}

/// The value a field has before its constructor runs: Java's default, or a placeholder that the
/// initializers overwrite. Object fields have no placeholder, so lowering rejects them, and it
/// rejects string and array fields without an initializer, which Java sets to `null`.
pub(super) fn default_value(ty: Type) -> &'static str {
    match ty {
        Type::Array(_) => "java::from_vec(Vec::new())",
        Type::Int | Type::Long | Type::Byte | Type::Short | Type::Char => "0",
        Type::Double | Type::Float => "0.0",
        Type::Boolean => "false",
        Type::String => "String::new()",
        Type::StringBuilder => "java::StringBuilder::new()",
        Type::Boxed(_) | Type::Class(..) => "None",
        _ => unreachable!("lowering rejects fields of type {ty}"),
    }
}

/// Which classes get a struct and instance methods: the classes that are ever instantiated, and
/// the superclasses whose fields struct they embed. A class with only static members is plain
/// functions.
pub(super) fn live_types(class: &Class, facts: &Facts) -> Vec<bool> {
    let mut live = instantiated(class);
    // The runtime creates the exceptions it raises where the program sees them.
    for &c in &facts.raised_classes {
        live[c] = true;
    }
    // A class that only ever types a null reference still needs its struct for the type.
    for t in mentioned(class) {
        live[t] |= class.hierarchy.is_concrete(t) && !class.hierarchy.has_subtypes(t);
    }
    for t in 0..live.len() {
        if live[t] {
            for a in class.hierarchy.ancestors(t) {
                live[a] |= facts.fields_structs.contains(&a);
            }
        }
    }
    live
}

/// Which classes' instance methods can run: those of a class that the program creates objects of,
/// or that the runtime raises, and of its superclasses. A class of the program that is neither
/// still has its methods emitted, as its type may be named, but nothing calls them.
pub(super) fn running_types(class: &Class, facts: &Facts) -> Vec<bool> {
    let mut created = instantiated(class);
    for &c in &facts.raised_classes {
        created[c] = true;
    }
    let mut runs: Vec<bool> = class
        .types
        .iter()
        .map(|t| {
            t.kind != TypeKind::Class
                || t.enum_constants.is_some()
                || !matches!(t.origin, Origin::Member | Origin::Local { .. })
        })
        .collect();
    for t in (0..created.len()).filter(|&t| created[t]) {
        runs[t] = true;
        for a in class.hierarchy.ancestors(t) {
            runs[a] = true;
        }
    }
    runs
}

/// The methods that a `super.m(...)` call names.
pub(super) fn super_targets(class: &Class) -> HashSet<usize> {
    let mut out = HashSet::new();
    for m in &class.methods {
        for s in &m.body {
            visit_stmt(
                s,
                &mut |e| {
                    visit_expr(e, &mut |x| {
                        if let ExprKind::MethodCall { resolved: Some(r), is_super: true, .. } = &x.kind {
                            out.insert(*r);
                        }
                    })
                },
                &mut |_| {},
            );
        }
    }
    out
}

/// The classes that a field, parameter, result, local or expression has as its type.
fn mentioned(class: &Class) -> HashSet<usize> {
    let mut out = HashSet::new();
    let mut note = |mut ty: Type| {
        while let Type::Array(element) = ty {
            ty = *element;
        }
        if let Type::Class(name, _) = ty
            && let Some(t) = class.type_index(name)
        {
            out.insert(t);
        }
    };
    for f in class.types.iter().flat_map(|t| t.fields.iter().chain(&t.statics)) {
        note(f.ty);
    }
    for m in &class.methods {
        note(m.ret);
        m.params.iter().for_each(|p| note(p.ty));
        let (mut types, mut locals) = (Vec::new(), Vec::new());
        for s in &m.body {
            visit_stmt(s, &mut |e| visit_expr(e, &mut |x| types.push(x.ty)), &mut |s| {
                if let Stmt::Local { ty, .. } = s {
                    locals.push(*ty);
                }
            });
        }
        types.into_iter().chain(locals).for_each(&mut note);
    }
    out
}

/// Which classes are ever instantiated with `new`.
fn instantiated(class: &Class) -> Vec<bool> {
    let mut live = vec![false; class.types.len()];
    let mut note = |e: &Expr| {
        if let ExprKind::New { class: c, .. } = &e.kind {
            live[*c] = true;
        }
    };
    for m in &class.methods {
        for s in &m.body {
            visit_stmt(s, &mut |e| visit_expr(e, &mut note), &mut |_| {});
        }
    }
    for f in class.types.iter().flat_map(|t| t.fields.iter().chain(&t.statics)) {
        if let Some(init) = &f.init {
            visit_expr(init, &mut note);
        }
    }
    live
}

/// The expressions that a statement evaluates itself; the expressions of nested statements are
/// emitted as statements of their own.
fn own_exprs(stmt: &Stmt) -> Vec<&Expr> {
    match stmt {
        Stmt::Local { init, .. } => init.iter().collect(),
        Stmt::Expr(e) | Stmt::Return(Some(e), _) => vec![e],
        Stmt::If { cond, .. } | Stmt::While { cond, .. } | Stmt::DoWhile { cond, .. } => vec![cond],
        Stmt::ForEach { array, .. } => vec![array],
        Stmt::For { cond, update, .. } => cond.iter().chain(update.iter()).collect(),
        Stmt::Switch { selector: e, .. } | Stmt::Yield(e) => vec![e],
        Stmt::Return(None, _)
        | Stmt::Break
        | Stmt::Continue
        | Stmt::Block(_)
        | Stmt::Labeled { .. }
        | Stmt::BreakLabel(_)
        | Stmt::ContinueLabel(_)
        | Stmt::Try { .. } => Vec::new(),
        Stmt::Throw(e) => vec![e],
    }
}

/// Whether an expression turns an object into text, which runs the object's `toString`; the
/// object, if so.
fn converted_objects(x: &Expr) -> Vec<&Expr> {
    let is_object = |a: &&Expr| matches!(a.ty, Type::Class(..));
    match &x.kind {
        ExprKind::Builtin(Builtin::Print | Builtin::Println, args) => args.iter().filter(is_object).collect(),
        ExprKind::Binary(BinOp::Add, l, r) if x.ty == Type::String => {
            [&**l, &**r].into_iter().filter(is_object).collect()
        }
        // `x.toString()` of a type that declares none, such as an interface, is `Object`'s.
        ExprKind::Builtin(Builtin::ObjectText, args) => args.iter().filter(is_object).collect(),
        _ => Vec::new(),
    }
}

/// Whether some expression of the statements is `this`, so that a constructor must bind it.
/// A constructor's statements without a `return;` that ends them: the constructor's own end
/// gives back the object.
pub(super) fn without_final_return(stmts: &[Stmt]) -> &[Stmt] {
    match stmts.split_last() {
        Some((Stmt::Return(None, _), rest)) => rest,
        _ => stmts,
    }
}

fn uses_this(stmts: &[Stmt]) -> bool {
    let mut found = false;
    for stmt in stmts {
        visit_stmt(
            stmt,
            &mut |e| visit_expr(e, &mut |x| found |= matches!(x.kind, ExprKind::This)),
            &mut |_| {},
        );
    }
    found
}

/// Whether two expressions are the same `this` or local.
fn same_object(a: &Expr, b: &Expr) -> bool {
    match (&a.kind, &b.kind) {
        (ExprKind::This, ExprKind::This) => true,
        (ExprKind::Var(x), ExprKind::Var(y)) => x == y,
        _ => false,
    }
}

/// Whether an expression writes a field of `object` or calls a method that changes it.
fn changes_object(e: &Expr, object: &Expr, mutating: &HashSet<usize>) -> bool {
    let mut found = false;
    visit_expr(e, &mut |x| {
        found |= match &x.kind {
            ExprKind::Assign { target: Place::Field { object: o, .. }, .. }
            | ExprKind::IncDec { target: Place::Field { object: o, .. }, .. } => same_object(o, object),
            ExprKind::MethodCall { receiver, resolved: Some(j), .. } => {
                same_object(receiver, object) && mutating.contains(j)
            }
            _ => false,
        }
    });
    found
}

/// Whether an expression reads a static field.
fn reads_static(e: &Expr) -> bool {
    let mut found = false;
    visit_expr(e, &mut |x| found |= matches!(x.kind, ExprKind::Static { .. }));
    found
}

/// Whether an expression reads a field of an object of class `name`, or of a class of its class
/// tree, or turns one into text.
pub(super) fn reads_class(class: &Class, e: &Expr, name: &str) -> bool {
    let root = class.type_index(name).map(|t| class.hierarchy.root(t));
    let of_class = |x: &Expr| matches!(x.ty, Type::Class(c, _) if c == name || class.type_index(c).map(|t| class.hierarchy.root(t)) == root);
    let mut found = false;
    visit_expr(e, &mut |x| {
        found |= match &x.kind {
            ExprKind::Field { object, .. } => of_class(object),
            ExprKind::Builtin(_, args) => args.iter().any(of_class),
            ExprKind::Binary(_, l, r) => of_class(l) || of_class(r),
            _ => false,
        }
    });
    found
}

impl<'a> Gen<'a> {
    /// The concrete class whose objects represent the values of class type `t`: `t` itself, or
    /// the one class under it whose objects there are.
    pub(super) fn rep_class(&self, t: usize) -> usize {
        match hierarchy::rep(self.class, self.facts, t) {
            hierarchy::Rep::Object(c) => c,
            hierarchy::Rep::Enum(t) => t,
        }
    }

    /// The qualified name of the class whose objects represent the values of class type `name`.
    fn rep_name<'n>(&self, name: &'n str) -> &'n str
    where
        'a: 'n,
    {
        match self.class.type_index(name) {
            Some(t) => &self.class.types[self.rep_class(t)].name,
            None => name,
        }
    }

    /// The Rust name of the struct that holds the fields of an object of class `t`: the class's
    /// name, or `<Name>Fields` for a class with subclasses, whose struct they embed.
    pub(super) fn struct_of(&self, t: usize) -> String {
        let name = struct_name(&self.class.types[t].name);
        if self.facts.fields_structs.contains(&t) { name + "Fields" } else { name }
    }

    /// The Rust type of an object of class `name`: a plain value for an owned class.
    pub(super) fn object_type(&self, name: &str) -> String {
        self.object_type_with(name, "")
    }

    /// The type parameters of class `t` as a Rust generic: `<T: Clone, U: Clone>` to declare them
    /// and `<T, U>` to use them; empty for any other class.
    pub(super) fn type_params(&self, params: &[Type]) -> (String, String) {
        if params.is_empty() {
            return (String::new(), String::new());
        }
        let names: Vec<String> = params.iter().map(|p| struct_name(&p.to_string())).collect();
        let bounded: Vec<String> = names.iter().map(|n| format!("{n}: Clone")).collect();
        (format!("<{}>", bounded.join(", ")), format!("<{}>", names.join(", ")))
    }

    /// Whether `ty` is a type variable of a Rust generic.
    pub(super) fn is_type_param(&self, ty: Type) -> bool {
        self.class.class_index(ty).is_some_and(|t| self.class.types[t].kind == TypeKind::Param)
    }

    /// The struct of class `t` with its own type parameters, as its `impl` names it.
    pub(super) fn generic_struct(&self, t: usize) -> String {
        let decl = &self.class.types[self.rep_class(t)];
        self.struct_of(t) + &self.type_params(&decl.type_params).1
    }

    /// `object_type` of a generic class with its type arguments, `<i32>`.
    pub(super) fn object_type_with(&self, name: &str, args: &str) -> String {
        if let Some(t) = self.class.type_index(name)
            && self.class.types[t].kind == TypeKind::Param
        {
            return struct_name(name);
        }
        if let Some(t) = self.class.type_index(name)
            && let hierarchy::Rep::Enum(t) = hierarchy::rep(self.class, self.facts, t)
        {
            self.need(Need::Enum(t));
            return struct_name(&self.class.types[t].name);
        }
        let name = self.rep_name(name);
        let structure = match self.class.type_index(name) {
            Some(t) => self.struct_of(t),
            None => struct_name(name),
        } + args;
        if self.facts.owned.contains(name) {
            structure
        } else if self.facts.immutable.contains(name) {
            format!("Rc<{structure}>")
        } else {
            format!("Rc<RefCell<{structure}>>")
        }
    }

    /// Whether `e` is an object whose fields are read and written without a `RefCell`: an object of
    /// an immutable class or of an owned class, or `this` when it is borrowed or being built.
    pub(super) fn is_plain(&self, e: &Expr) -> bool {
        if self.enum_of(e).is_some() {
            return false;
        }
        if matches!(e.kind, ExprKind::This) && self.this_form != ThisForm::Shared {
            return true;
        }
        if matches!(e.ty, Type::Enum(_)) {
            return true;
        }
        matches!(e.ty, Type::Class(name, _) if {
            let name = self.rep_name(name);
            self.facts.immutable.contains(name) || self.facts.owned.contains(name)
        })
    }

    /// The type of `this` in the method being written: a reference to the object.
    pub(super) fn this_type(&self) -> String {
        let structure = self.generic_struct(self.this_struct);
        if self.facts.immutable.contains(&self.class.types[self.this_struct].name) {
            format!("&{structure}")
        } else {
            format!("&Rc<RefCell<{structure}>>")
        }
    }

    /// The class whose struct an object expression is: `this`'s in the code being written, else
    /// the class whose objects represent the expression's type.
    pub(super) fn struct_class(&self, object: &Expr) -> Option<usize> {
        if matches!(object.kind, ExprKind::This) {
            return Some(self.this_struct);
        }
        self.class.class_index(object.ty).map(|t| self.rep_class(t))
    }

    /// The path of field `name` from the struct of `object`: `name`, or through the fields
    /// structs of superclasses that the struct embeds, `base.name`.
    pub(super) fn field_path(&self, object: &Expr, name: &str) -> String {
        if let Some(t) = self.enum_of(object) {
            return self.enum_field_path(t, object.ty, name);
        }
        let field = snake_case(name);
        let (Some(s), Some(owner)) = (self.struct_class(object), self.class.field_owner(object.ty, name))
        else {
            return field;
        };
        hierarchy::base_path(self.facts, self.class, s, owner) + &field
    }

    /// The method that a call of `resolved` on `receiver` runs: the one it names for `super.m()`,
    /// else the implementation of the class whose objects the receiver is.
    pub(super) fn dispatch(&self, receiver: &Expr, resolved: usize, is_super: bool) -> usize {
        // A call on an enum is its dispatcher's (see `dispatch`), which counts as the method it names.
        if is_super || self.class.methods[resolved].is_static || self.enum_of(receiver).is_some() {
            return resolved;
        }
        // A method on a fields struct calls only methods that nothing overrides.
        if matches!(receiver.kind, ExprKind::This) && matches!(self.homes[self.method], Home::Fields(_)) {
            return resolved;
        }
        match self.class.class_index(receiver.ty) {
            Some(t) => self.class.hierarchy.implementation(resolved, self.rep_class(t)).unwrap_or(resolved),
            None => resolved,
        }
    }

    /// `e` without the conversions to a superclass type that change nothing: values of both types
    /// are objects of the same class.
    pub(super) fn peel(&self, e: &'a Expr) -> &'a Expr {
        match &e.kind {
            ExprKind::Cast(Type::Class(to, _), inner) if matches!(inner.ty, Type::Class(..)) => {
                let same = match (self.class.type_index(to), self.class.class_index(inner.ty)) {
                    (Some(a), Some(b)) => self.rep_class(a) == self.rep_class(b),
                    _ => false,
                };
                if same { self.peel(inner) } else { e }
            }
            _ => e,
        }
    }

    /// The method that a call expression runs (see `dispatch`).
    pub(super) fn target(&self, x: &Expr) -> Option<usize> {
        match &x.kind {
            ExprKind::MethodCall { receiver, resolved: Some(r), is_super, .. } => {
                Some(self.dispatch(receiver, *r, *is_super))
            }
            _ => None,
        }
    }

    /// The path from the struct of `receiver` to the struct whose `impl` has method `index`:
    /// `base.` for a method of a superclass's fields struct, else empty.
    fn method_path(&self, receiver: &Expr, index: usize) -> String {
        match (self.homes[index], self.struct_class(receiver)) {
            (Home::Fields(d), Some(s)) => hierarchy::base_path(self.facts, self.class, s, d),
            _ => String::new(),
        }
    }

    /// The name of `this` in the code being written: `self` in a method that borrows its object,
    /// else `this`.
    pub(super) fn this_name(&self) -> &'static str {
        if matches!(self.this_form, ThisForm::SelfRef | ThisForm::Enum) { "self" } else { "this" }
    }

    /// Whether the constructors of class `name` build a plain value, which `new` then shares.
    pub(super) fn builds_plain(&self, name: &str) -> bool {
        self.facts.owned.contains(name) || self.facts.plain_constructors.contains(name)
    }

    /// Whether the field initializers of class `t` are written into each constructor's struct
    /// literal, rather than run by a `field_inits` method that every constructor calls.
    pub(super) fn inline_inits(&self, t: usize) -> bool {
        crate::analysis::inline_initializers(self.class, t, self.facts)
    }

    /// Whether the field `name` of class `owner` can hold null, so that it is an `Option`.
    pub(super) fn field_nullable(&self, owner: &str, name: &str) -> bool {
        self.facts.nullable_fields.contains(&(owner.to_string(), name.to_string()))
    }

    /// Whether a field or static field can hold null.
    pub(super) fn place_nullable(&self, place: &Place) -> bool {
        match place {
            // The field may be declared by a superclass of the object's type.
            Place::Field { object, name } => self.facts.field_nullable(object.ty, name),
            Place::Static { class, name } => self.static_optional(*class, name),
            Place::Var(_) | Place::Index(..) => false,
        }
    }

    /// The text of an object used for a field access or a call: `this`, a variable, or an
    /// expression. Dereferencing a reference that may be null throws `NullPointerException`; the
    /// result is then a `&Rc<..>` when the reference is a place, else the `Rc` itself.
    pub(super) fn object(&mut self, e: &'a Expr) -> String {
        let id = e.id;
        let e = self.peel(e);
        let (text, place) = self.object_parts(e);
        if !(matches!(e.ty, Type::Class(..)) && self.nullable(e)) {
            return text;
        }
        let option = if place { format!("{text}.as_ref()") } else { text };
        self.dereference(&[id, e.id], &option)
    }

    /// `java::non_null(option)`, for the object (expression `ids`, before and after `peel`) that a
    /// call or a field access dereferences; where a `catch` or a `finally` may see its
    /// `NullPointerException` (`Facts::null_checked`), the checked form, whose failure goes on
    /// from here as a thrown exception does.
    pub(super) fn dereference(&mut self, ids: &[ExprId], option: &str) -> String {
        let Some(&id) = ids.iter().find(|id| self.facts.null_checked.contains_key(id)) else {
            return format!("{}({option})", self.runtime("non_null"));
        };
        self.dereferenced.insert(id);
        let call = format!("{}({option})", self.runtime("try_non_null"));
        self.passed_on(Code::atom(call))
    }

    /// Rejects the program when an object of `Facts::null_checked` is dereferenced by code that
    /// does not write the checked form, where its `NullPointerException` would end the program
    /// instead of reaching the `catch` or the `finally`.
    pub(super) fn check_dereferences(&mut self) {
        // The objects in the bodies that were not written, which nothing calls or which never run.
        let mut unwritten = HashSet::new();
        let methods = self.class.methods.iter().enumerate().filter(|(i, _)| !self.written.contains(i));
        for stmt in methods.flat_map(|(_, m)| &m.body) {
            visit_stmt(
                stmt,
                &mut |e| {
                    visit_expr(e, &mut |x| {
                        unwritten.insert(x.id);
                    })
                },
                &mut |_| {},
            );
        }
        let missed = self
            .facts
            .null_checked
            .iter()
            .filter(|(id, _)| !self.dereferenced.contains(id) && !unwritten.contains(id));
        if let Some(pos) = missed.map(|(_, &pos)| pos).min_by_key(|p| (p.line, p.column)) {
            let message = "catching the `NullPointerException` of this dereference not supported yet";
            self.rejected.get_or_insert(Error { pos, message: message.to_string() });
        }
    }

    /// The text of a reference as it is stored, without a clone: `this`, a variable, a field, or
    /// an expression. A reference that may be null is an `Option` here.
    pub(super) fn object_place(&mut self, e: &'a Expr) -> String {
        self.object_parts(e).0
    }

    /// `object_place`, and whether the text is a place that can be borrowed rather than a value.
    fn object_parts(&mut self, e: &'a Expr) -> (String, bool) {
        let e = self.peel(e);
        match &e.kind {
            ExprKind::This => (self.this_name().to_string(), true),
            ExprKind::Var(name) => (self.local(name), true),
            ExprKind::Field { object, name }
                if matches!(object.kind, ExprKind::This) && self.field_locals.contains_key(name) =>
            {
                (self.field_locals[name].clone(), true)
            }
            ExprKind::Field { object, name } if !matches!(object.ty, Type::Enum(_)) => {
                let field = self.field_path(object, name);
                let obj = self.object(object);
                if let Some(t) = self.enum_of(object) {
                    let borrow = self.enum_borrow(t, false);
                    if self.guarded && !self.immutable_enum(t) {
                        (format!("{{ {obj}{borrow}.{field}.clone() }}"), true)
                    } else {
                        (format!("{obj}{borrow}.{field}"), true)
                    }
                } else if self.is_plain(object) {
                    (format!("{obj}.{field}"), true)
                } else if self.guarded {
                    // A borrow that the statement must not hold is ended inside braces, which
                    // move the value out, so it is cloned.
                    (format!("{{ {obj}.borrow().{field}.clone() }}"), true)
                } else {
                    (format!("{obj}.borrow().{field}"), true)
                }
            }
            // An element of a plain array is a place too.
            ExprKind::Index(..) => {
                let text = self.expr(e).receiver();
                match text.strip_suffix(".clone()") {
                    Some(place) => (place.to_string(), true),
                    None => (text, false),
                }
            }
            _ => (self.expr(e).receiver(), false),
        }
    }

    /// A non-null object as an `Rc` of its own, which holds no borrow: null throws
    /// `NullPointerException`.
    pub(super) fn object_handle(&mut self, e: &'a Expr) -> String {
        let id = e.id;
        let e = self.peel(e);
        let text = match &e.kind {
            ExprKind::This => format!("{}.clone()", self.this_name()),
            ExprKind::Var(v) => format!("{}.clone()", self.local(v)),
            _ => self.expr(e).text,
        };
        if matches!(e.ty, Type::Class(..)) && self.nullable(e) {
            self.dereference(&[id, e.id], &text)
        } else {
            text
        }
    }

    /// An object reference as an `Option<&Rc<..>>`: `None` for null.
    fn object_option(&mut self, e: &'a Expr) -> String {
        let e = self.peel(e);
        match &e.kind {
            ExprKind::Null => "None".to_string(),
            _ if self.nullable(e) => format!("{}.as_ref()", self.object_place(e)),
            _ => format!("Some({})", self.object_ref(e)),
        }
    }

    /// A non-null object reference as a `&Rc<..>`. A variable narrowed by a null check already is.
    pub(super) fn object_ref(&mut self, e: &'a Expr) -> String {
        let e = self.peel(e);
        match &e.kind {
            // A method that uses `this` as a value has it as a `&Rc<..>`.
            ExprKind::This => self.this_name().to_string(),
            ExprKind::Var(name) if self.borrowed.contains(name) => self.local(name),
            _ => format!("&{}", self.object_place(e)),
        }
    }

    /// `a == b` and `a != b` on objects: whether they are the same object, as `Rc::ptr_eq`, or
    /// `java::same` where either may be null.
    pub(super) fn identity(&mut self, op: BinOp, l: &'a Expr, r: &'a Expr) -> Code {
        if self.enum_of(l).is_some() || self.enum_of(r).is_some() {
            return self.enum_identity(op, l, r);
        }
        let same = if self.nullable(l) || self.nullable(r) {
            let (a, b) = (self.object_option(l), self.object_option(r));
            format!("{}({a}, {b})", self.runtime("same"))
        } else {
            let (a, b) = (self.object_ref(l), self.object_ref(r));
            format!("Rc::ptr_eq({a}, {b})")
        };
        if op == BinOp::Ne { Code::new(format!("!{same}"), UNARY) } else { Code::atom(same) }
    }

    /// The text of a non-null object of `e`'s class, reached through `handle`, a `&Rc<..>`: what
    /// `object_text` gives for an object that may be null once it is not.
    pub(super) fn handle_text(&mut self, handle: &str, e: &'a Expr) -> String {
        if let Some(t) = self.enum_of(e) {
            return self.enum_text(t, handle.to_string());
        }
        let Some(index) = self.to_string_index(e) else {
            let c = self.struct_class(e).expect("an object has a class type");
            let hash = match self.hash_index(c) {
                Some(i) => self.handle_call(handle, e, i),
                None => format!("{}({})", self.runtime("address_hash"), self.address(c, handle)),
            };
            return self.default_text(c, hash);
        };
        if self.displays(index) {
            let obj = self.handle_object(handle, e, index);
            if self.lazy_text && !(self.printing && !self.quiet_text(e, 0)) {
                return obj;
            }
            return format!("{obj}.to_string()");
        }
        self.handle_call(handle, e, index)
    }

    /// The object of `e`'s class reached through `handle`, borrowed for method `index` to be
    /// called on it.
    fn handle_object(&self, handle: &str, e: &Expr, index: usize) -> String {
        match () {
            _ if self.is_plain(e) => handle.to_string(),
            _ if self.facts.mutating.contains(&index) => format!("{handle}.borrow_mut()"),
            _ => format!("{handle}.borrow()"),
        }
    }

    /// A call of method `index`, without arguments, on the object of `e`'s class that `handle`
    /// reaches.
    fn handle_call(&mut self, handle: &str, e: &'a Expr, index: usize) -> String {
        let obj = self.handle_object(handle, e, index);
        if self.facts.self_methods.contains(&index) {
            format!("{obj}.{}{}()", self.method_path(e, index), self.fn_names[index])
        } else {
            format!("{}({handle})", self.qualified_fn(index))
        }
    }

    /// `e` as the receiver of a method, borrowed; `this` is already a reference.
    pub(super) fn receiver_ref(&mut self, e: &'a Expr) -> String {
        let e = self.peel(e);
        match &e.kind {
            ExprKind::This => self.this_name().to_string(),
            // A variable narrowed by a null check is a reference already.
            ExprKind::Var(name) if matches!(e.ty, Type::Class(..)) && self.borrowed.contains(name) => {
                self.local(name)
            }
            ExprKind::Var(_) | ExprKind::Field { .. }
                if matches!(e.ty, Type::Class(..)) && self.nullable(e) =>
            {
                let (_, place) = self.object_parts(e);
                let text = self.object(e);
                if place { text } else { format!("&{text}") }
            }
            _ => format!("&{}", self.object(e)),
        }
    }

    /// The arguments of a call of method `index`: borrowed parameters get borrows, optional ones
    /// `Option`s, and others values.
    pub(super) fn call_args(&mut self, index: usize, args: &'a [Expr]) -> Vec<String> {
        let params = &self.class.methods[index].params;
        args.iter()
            .zip(params)
            .enumerate()
            .map(|(i, (a, p))| {
                if self.facts.borrowed_params.contains(&(index, i)) {
                    self.str_ref(a)
                } else if self.facts.borrowed_objects.contains(&(index, i)) {
                    self.object_ref(a)
                } else if self.facts.borrowed_optional_params.contains(&(index, i)) {
                    self.option_arg(a)
                } else if self.optional_var(index, p.ty, &p.name) {
                    self.optional_to(a, p.ty).text
                } else if let Some(&written) = self.facts.borrowed_arrays.get(&(index, i)) {
                    self.borrowed_arg(a, written)
                } else if self.guarded && self.holds_borrow(a) {
                    // A lone `{ x }` argument draws rustc's unused_braces lint; a `let` keeps the
                    // scope of the `Ref` short while the call runs.
                    let outer = std::mem::replace(&mut self.guarded, false);
                    let read = self.value(a, p.ty).text;
                    self.guarded = outer;
                    format!("{{ let value = {read}; value }}")
                } else {
                    self.value(a, p.ty).text
                }
            })
            .collect()
    }

    /// Whether a statement's own expressions may borrow an object mutably while another borrow made
    /// by the statement is still alive, so that every borrow must end where it is made.
    pub(super) fn stmt_conflicts(&self, stmt: &Stmt) -> bool {
        self.conflicts(&own_exprs(stmt)) || matches!(stmt, Stmt::Expr(e) if self.reassigns_through_borrow(e))
    }

    /// Whether evaluating `e` borrows a shared object through a `RefCell`.
    pub(super) fn reads_through_borrow(&self, e: &Expr) -> bool {
        let mut found = false;
        visit_expr(e, &mut |x| found |= self.holds_borrow(x));
        found
    }

    /// Whether an assignment to a local reads the local's old object through a `RefCell` borrow,
    /// as `t = t.left` does: the borrow would outlive the assignment that drops the old object.
    pub(super) fn reassigns_through_borrow(&self, e: &Expr) -> bool {
        // The place is a local, or a field of `this` (`head = head.next` in a method of a list).
        let (name, field) = match &e.kind {
            ExprKind::Assign { target: Place::Var(name), .. } => (name, false),
            ExprKind::Assign { target: Place::Field { object, name }, .. }
                if matches!(object.kind, ExprKind::This) =>
            {
                (name, true)
            }
            _ => return false,
        };
        let ExprKind::Assign { value, .. } = &e.kind else { unreachable!("matched above") };
        let mut found = false;
        visit_expr(value, &mut |x| {
            if self.holds_borrow(x) {
                visit_expr(x, &mut |y| {
                    found |= match &y.kind {
                        ExprKind::Var(v) => !field && v == name,
                        ExprKind::Field { object, name: f } => {
                            field && f == name && matches!(object.kind, ExprKind::This)
                        }
                        _ => false,
                    }
                });
            }
        });
        found
    }

    /// Whether the borrows that evaluating the expressions makes may conflict: a `RefCell` borrow
    /// lives to the end of the statement, so two borrows conflict when one of them is mutable.
    pub(super) fn conflicts(&self, exprs: &[&Expr]) -> bool {
        let (mut borrows, mut mutable) = (0, 0);
        for e in exprs {
            visit_expr(e, &mut |x| {
                let (b, m) = self.borrows_here(x);
                borrows += b;
                mutable += m;
            });
        }
        mutable > 0 && borrows > 1
    }

    /// The borrows of a shared object that one expression node makes (not counting its operands),
    /// and how many of them may be mutable. A method or constructor that reaches objects other
    /// than its own object may borrow any of them.
    fn borrows_here(&self, x: &Expr) -> (usize, usize) {
        let call = |index: usize, receiver: Option<&Expr>| {
            let mut counts = (0, 0);
            let shared = receiver.is_some_and(|r| !self.is_plain(r));
            let writes = usize::from(self.facts.writers.contains(&index));
            if receiver.is_some() && !self.facts.self_methods.contains(&index) {
                counts = (1, writes);
            } else if shared {
                counts = (1, usize::from(self.facts.mutating.contains(&index)));
            }
            if !self.facts.isolated.contains(&index) {
                counts = (counts.0 + 1, counts.1 + writes);
            }
            counts
        };
        let mut counts = match &x.kind {
            ExprKind::Field { object, .. } => (usize::from(!self.is_plain(object)), 0),
            ExprKind::Assign { target: Place::Field { object, .. }, .. }
            | ExprKind::IncDec { target: Place::Field { object, .. }, .. } => {
                let shared = usize::from(!self.is_plain(object));
                (shared, shared)
            }
            ExprKind::MethodCall { receiver, resolved: Some(_), .. } => {
                call(self.target(x).expect("a resolved call"), Some(receiver))
            }
            ExprKind::Call { resolved: Some(index), .. }
            | ExprKind::New { ctor: Some(index), .. }
            | ExprKind::ThisCall { resolved: Some(index), .. }
            | ExprKind::SuperInit { resolved: Some(index), .. } => call(*index, None),
            _ => (0, 0),
        };
        for object in converted_objects(x) {
            if let Some(t) = self.enum_of(object) {
                counts = (counts.0 + 1, counts.1 + usize::from(self.enum_text_writes(t)));
                continue;
            }
            if let Some(index) = self.text_index(object) {
                let (b, m) = call(index, Some(object));
                counts = (counts.0 + b, counts.1 + m);
            }
        }
        counts
    }

    /// The index of the `toString` method of an object's class, if the class has one.
    pub(super) fn to_string_index(&self, e: &Expr) -> Option<usize> {
        let t = self.struct_class(e).expect("an object has a class type");
        hierarchy::to_string_impl(self.class, t)
    }

    /// The method of the program that turning an object into text runs: its class's `toString`,
    /// or the `hashCode` that `Object`'s calls.
    fn text_index(&self, e: &Expr) -> Option<usize> {
        let t = self.struct_class(e).expect("an object has a class type");
        self.to_string_index(e).or_else(|| self.hash_index(t))
    }

    /// Whether `a` reads a shared object through a `RefCell` borrow that would live to the end of
    /// the statement: a field read, or a call of a method that borrows its object.
    fn holds_borrow(&self, a: &Expr) -> bool {
        match &a.kind {
            ExprKind::Field { object, .. } => match self.enum_of(object) {
                Some(t) => !self.immutable_enum(t),
                None => !self.is_plain(object),
            },
            ExprKind::MethodCall { receiver, .. } => {
                self.enum_of(receiver).is_none()
                    && self.target(a).is_some_and(|index| self.facts.self_methods.contains(&index))
                    && !self.is_plain(receiver)
            }
            _ => false,
        }
    }

    /// The Rust path of a method or constructor, such as `Point::new` or `Point::norm`: a function
    /// of the struct whose `impl` has it.
    pub(super) fn qualified_fn(&self, index: usize) -> String {
        let owner = match self.homes[index] {
            Home::Enum(t) => {
                return format!("{}::{}", struct_name(&self.class.types[t].name), self.fn_names[index]);
            }
            Home::Class(c) | Home::Fields(c) => c,
            Home::Free | Home::Nowhere => self.class.methods[index].class,
        };
        format!("{}::{}", self.struct_of(owner), self.fn_names[index])
    }

    /// A value read from a field of an object.
    pub(super) fn field_read(&mut self, object: &'a Expr, name: &str, ty: Type) -> Code {
        if matches!(object.ty, Type::Enum(_)) {
            return self.enum_field(object, name);
        }
        let clone = if self.clones(ty) { ".clone()" } else { "" };
        if matches!(object.kind, ExprKind::This)
            && let Some(local) = self.field_locals.get(name)
        {
            return Code::atom(format!("{local}{clone}"));
        }
        let obj = self.object(object);
        if self.is_plain(object) {
            return Code::atom(format!("{obj}.{}{clone}", self.field_path(object, name)));
        }
        let borrow = match self.enum_of(object) {
            Some(t) if self.immutable_enum(t) => {
                let borrow = self.enum_borrow(t, false);
                return Code::atom(format!("{obj}{borrow}.{}{clone}", self.field_path(object, name)));
            }
            Some(t) => self.enum_borrow(t, false),
            None => ".borrow()",
        };
        let read = format!("{obj}{borrow}.{}{clone}", self.field_path(object, name));
        // The `Ref` lives to the end of the statement, so it is scoped with braces only when the
        // statement also borrows mutably; otherwise the braces would be noise.
        Code::atom(if self.guarded { format!("{{ {read} }}") } else { read })
    }

    /// A value read from a static field.
    pub(super) fn static_read(&mut self, class: usize, name: &str, ty: Type) -> Code {
        if let Some(constant) = self.constant_read(class, name, ty) {
            return constant;
        }
        let cell = cell_name(&self.class.types[class].name, name);
        let read = Slot::Static { cell, plain: is_cell(ty) }.read(self.clones(ty));
        // A static that starts empty holds its value once its class is initialized.
        let read = if self.facts.placeholder_statics.contains(&(class, name.to_string())) {
            format!("{}({read})", self.runtime("non_null"))
        } else {
            read
        };
        match self.init_call(class) {
            Some(init) => Code::atom(format!("{{ {init} {read} }}")),
            None => Code::atom(read),
        }
    }

    /// `new C(args)` of type `ty`, which calls the constructor that the checker chose. A
    /// constructor that builds a plain value is wrapped here, unless the class is owned.
    pub(super) fn new_object(&mut self, ctor: usize, args: &'a [Expr], ty: Type) -> Code {
        let c = self.class.methods[ctor].class;
        let rep = self.enum_for(c);
        let turbofish = self.constructor_turbofish(ctor, args, ty);
        self.build_object(ctor, args, rep, &turbofish)
    }

    /// `new_object`, as a value of the enum of type `rep` if not `None` (an object of a class
    /// with subclasses is a value of its class's enum; an upcast makes it one of a superclass's).
    pub(super) fn new_object_as(&mut self, ctor: usize, args: &'a [Expr], rep: Option<usize>) -> Code {
        self.build_object(ctor, args, rep, "")
    }

    /// The type arguments of `new C<..>(args)` of a Rust generic, `::<String>`, where the
    /// arguments do not decide them all: a type parameter that no parameter of the constructor
    /// has as its type, or one that only gets `null` (`new Box<>(null)` is `Box::new(None)`).
    /// Empty where rustc infers them.
    fn constructor_turbofish(&mut self, ctor: usize, args: &'a [Expr], ty: Type) -> String {
        let c = self.class.methods[ctor].class;
        let params = &self.class.types[c].type_params;
        let Type::Class(_, targs) = ty else { return String::new() };
        if params.is_empty() || targs.len() != params.len() {
            return String::new();
        }
        let decided = params.iter().all(|&p| {
            self.class.methods[ctor].params.iter().zip(args).any(|(q, a)| q.ty == p && !is_null(a))
        });
        if decided {
            return String::new();
        }
        let names: Vec<String> = targs.iter().map(|&a| self.ty_name(a)).collect();
        format!("::<{}>", names.join(", "))
    }

    fn build_object(&mut self, ctor: usize, args: &'a [Expr], rep: Option<usize>, turbofish: &str) -> Code {
        // Java initializes the class when `new` runs, before the arguments, which may see what
        // the initialization does; the constructor initializes it again, which does nothing.
        let init = self.init_call(self.class.methods[ctor].class).filter(|_| args.iter().any(sees_statics));
        let args = self.call_args(ctor, args).join(", ");
        let path = self.qualified_fn(ctor);
        let path = match path.rsplit_once("::") {
            Some((owner, name)) if !turbofish.is_empty() => format!("{owner}{turbofish}::{name}"),
            _ => path,
        };
        let call = format!("{path}({args})");
        let call = self.raising(Code::atom(call), ctor).text;
        let call = match init {
            Some(init) => format!("{{ {init} {call} }}"),
            None => call,
        };
        let c = self.class.methods[ctor].class;
        let name = &self.class.types[c].name;
        let object = if self.facts.owned.contains(name) || !self.builds_plain(name) {
            call
        } else if self.facts.immutable.contains(name) {
            format!("Rc::new({call})")
        } else {
            format!("Rc::new(RefCell::new({call}))")
        };
        Code::atom(match rep {
            Some(t) => format!("{}({object})", self.variant(t, c)),
            None => object,
        })
    }

    /// `receiver.name(args)`, a call of an instance method. A method that borrows its object is
    /// called with method syntax, on a shared object through `borrow()` or `borrow_mut()`.
    pub(super) fn method_call(&mut self, receiver: &'a Expr, index: usize, args: &'a [Expr]) -> Code {
        if let Some(code) = self.inherited_call(receiver, index, args) {
            return code;
        }
        if !self.facts.self_methods.contains(&index) {
            let mut all = vec![self.receiver_ref(receiver)];
            all.extend(self.call_args(index, args));
            return Code::atom(format!("{}({})", self.qualified_fn(index), all.join(", ")));
        }
        let mutating = self.facts.mutating.contains(&index);
        if self.is_plain(receiver) {
            // The call borrows a plain object itself, and two-phase borrows accept `p.set(p.x + 1)`;
            // an argument that changes the object must run before the call borrows it, whether the
            // call borrows it mutably or not.
            if args.iter().any(|a| changes_object(a, receiver, &self.facts.mutating)) {
                return self.bound_call(receiver, index, args, "");
            }
            let args = self.call_args(index, args).join(", ");
            let obj = self.object(receiver);
            let path = self.method_path(receiver, index);
            return Code::atom(format!("{obj}.{path}{}({args})", self.call_name(index)));
        }
        let borrow = if mutating { ".borrow_mut()" } else { ".borrow()" };
        let Type::Class(class, _) = receiver.ty else { unreachable!("a receiver has a class type") };
        // The receiver is borrowed before the arguments run, so arguments that may reach an object
        // of the class run first.
        if args.iter().any(|a| has_effects(a) || reads_class(self.class, a, class)) {
            return self.bound_call(receiver, index, args, borrow);
        }
        let obj = self.object(receiver);
        let args = self.call_args(index, args).join(", ");
        let path = self.method_path(receiver, index);
        let call = format!("{obj}{borrow}.{path}{}({args})", self.call_name(index));
        Code::atom(if self.guarded { format!("{{ {call} }}") } else { call })
    }

    /// The name by which method `index` is called on its object: a `toString` that is the class's
    /// `Display` (see `displays`) is `to_string`, as `super.toString()` calls it.
    fn call_name(&self, index: usize) -> &str {
        if self.displays(index) { "to_string" } else { &self.fn_names[index] }
    }

    /// A method call whose arguments are bound before the call borrows its object, in Java's order:
    /// the receiver, the arguments, then the call.
    fn bound_call(&mut self, receiver: &'a Expr, index: usize, args: &'a [Expr], borrow: &str) -> Code {
        let outer = std::mem::replace(&mut self.guarded, false);
        let mut lines = Vec::new();
        let mut names: Vec<String> = Vec::new();
        let obj = match &receiver.kind {
            ExprKind::This | ExprKind::Var(_) => self.object(receiver),
            _ => {
                let temp = self.temp("object");
                let text = self.object_handle(receiver);
                lines.push(format!("let {temp} = {text};"));
                names.push(temp.clone());
                temp
            }
        };
        let texts = self.call_args(index, args);
        let mut bound = Vec::new();
        for (text, p) in texts.into_iter().zip(&self.class.methods[index].params) {
            let mut temp = self.temp(&snake_case(&p.name));
            while names.contains(&temp) {
                temp.push('_');
            }
            lines.push(format!("let {temp} = {text};"));
            names.push(temp.clone());
            bound.push(temp);
        }
        self.guarded = outer;
        let path = self.method_path(receiver, index);
        lines.push(format!("{obj}{borrow}.{path}{}({})", self.call_name(index), bound.join(", ")));
        Code::atom(format!("{{ {} }}", lines.join(" ")))
    }

    /// The text of an object, which Java gets from its class's `toString` (lowering rejects classes
    /// without one).
    pub(super) fn object_text(&mut self, e: &'a Expr) -> Code {
        if let Some(t) = self.enum_of(e) {
            let obj = self.object(e);
            return Code::atom(self.enum_text(t, obj));
        }
        let Some(index) = self.to_string_index(e) else {
            let c = self.struct_class(e).expect("an object has a class type");
            let hash = match self.hash_index(c) {
                Some(i) => self.method_call(e, i, &[]).text,
                None => {
                    let address = self.receiver_address(e, c);
                    format!("{}({address})", self.runtime("address_hash"))
                }
            };
            return Code::atom(self.default_text(c, hash));
        };
        if !self.displays(index) {
            return self.text_call(e, index);
        }
        if !self.lazy_text || (self.printing && !self.quiet_text(e, 0)) {
            return self.method_call(e, index, &[]);
        }
        // The object is formatted through its `Display`.
        let obj = self.object(e);
        Code::atom(if self.is_plain(e) { obj } else { format!("{obj}.borrow()") })
    }

    /// A call of a `toString` that is a plain method.
    fn text_call(&mut self, e: &'a Expr, index: usize) -> Code {
        // A `toString` that the object's class inherits from a class whose enum has it.
        if let Some(code) = self.inherited_call(e, index, &[]) {
            return code;
        }
        if self.facts.self_methods.contains(&index) {
            return self.method_call(e, index, &[]);
        }
        Code::atom(format!("{}({})", self.qualified_fn(index), self.receiver_ref(e)))
    }

    /// A name for a temporary that no Java local or function of this method uses.
    pub(super) fn temp(&self, base: &str) -> String {
        let taken = |n: &str| self.locals.values().any(|v| v == n) || self.fn_names.iter().any(|f| f == n);
        let mut name = base.to_string();
        let mut i = 1;
        while taken(&name) {
            i += 1;
            name = format!("{base}_{i}");
        }
        name
    }

    /// A field assignment, `++` or `--` evaluated for its effect, written in place: `p.x = v`,
    /// `self.count += 1`, `obj.borrow_mut().total += n`. `None` where Java's order of evaluation
    /// needs the general form of `update`: a compound assignment reads the field before its right
    /// operand runs, so that operand must have no effects.
    pub(super) fn field_effect(&mut self, e: &'a Expr) -> Option<String> {
        let (target, how) = match &e.kind {
            ExprKind::Assign { op: None, target, value } => (target, Update::Assign(value)),
            ExprKind::Assign { op: Some(op), target, value } => (target, Update::Compound(*op, value)),
            ExprKind::IncDec { target, increment, .. } => (target, Update::Step(*increment)),
            _ => return None,
        };
        let Place::Field { object, name } = target else { return None };
        // Java takes the object before the value, which Rust computes first: another object
        // expression is only written in place when neither has effects.
        let stable = matches!(object.kind, ExprKind::This | ExprKind::Var(_));
        let pure_value = match how {
            Update::Assign(value) | Update::Compound(_, value) => !has_effects(value),
            Update::Step(_) => true,
        };
        if !stable && (has_effects(object) || !pure_value) {
            return None;
        }
        let ty = e.ty;
        // A boxed field is updated through its value, in the general form.
        if matches!(ty, Type::Boxed(_)) && !matches!(how, Update::Assign(_)) {
            return None;
        }
        if let Update::Compound(_, value) = how
            && (ty == Type::String || has_effects(value))
        {
            return None;
        }
        let nullable = self.place_nullable(target);
        let obj = self.object(object);
        let field = self.field_path(object, name);
        if self.is_plain(object) {
            let place = format!("{obj}.{field}");
            return Some(match how {
                Update::Assign(value) => format!("{place} = {}", self.field_value(value, ty, nullable)),
                Update::Compound(op, value) => self.compound(op, &place, ty, value, e.id),
                Update::Step(increment) => self.step_place(place, ty, increment, e.id),
            });
        }
        // The right operand runs before the field is borrowed, and the borrow of every field it
        // reads ends first.
        let borrow = match self.enum_of(object) {
            Some(t) => self.enum_borrow(t, true),
            None => ".borrow_mut()",
        };
        let place = format!("{obj}{borrow}.{field}");
        let outer = std::mem::replace(&mut self.guarded, true);
        let Type::Class(class, _) = object.ty else { unreachable!("a field of an object") };
        let text = match how {
            Update::Assign(value) => Some(format!("{place} = {}", self.field_value(value, ty, nullable))),
            Update::Compound(op, value) if let Some(symbol) = self.in_place_op(op, ty, value, e.id) => {
                let v = self.value(value, if op.is_shift() { value.ty } else { ty });
                Some(format!("{place} {symbol} {}", v.text))
            }
            Update::Step(increment) if ty.is_floating() || self.no_overflow(e.id) => {
                let one = if ty.is_floating() { "1.0" } else { "1" };
                Some(format!("{place} {} {one}", if increment { "+=" } else { "-=" }))
            }
            // Wrapping needs the old value, read through the same borrow; the right operand must
            // not borrow an object of the class meanwhile.
            Update::Compound(_, value) if reads_class(self.class, value, class) => None,
            Update::Compound(..) | Update::Step(_) => {
                let local = self.temp(&snake_case(name));
                let marked = format!("({local})");
                let step = match how {
                    Update::Compound(op, value) => self.compound(op, &marked, ty, value, e.id),
                    Update::Step(increment) => self.step_place(marked.clone(), ty, increment, e.id),
                    Update::Assign(_) => unreachable!("handled above"),
                };
                let step = step.replacen(&format!("{marked} ="), &format!("*{local} ="), 1);
                // A method call dereferences by itself.
                let step = step.replace(&format!("{marked}."), &format!("{local}."));
                let step = step.replace(&marked, &format!("(*{local})"));
                // An object computed by an expression is a temporary that a `let` would not keep
                // alive while the field is borrowed: it gets a local of its own.
                let (bind, place) = if stable {
                    (String::new(), place)
                } else {
                    let handle = self.temp("object");
                    let value = self.object_handle(object);
                    (format!("let {handle} = {value}; "), format!("{handle}{borrow}.{field}"))
                };
                Some(format!("{{ {bind}let {local} = &mut {place}; {step}; }}"))
            }
        };
        self.guarded = outer;
        text
    }

    /// An assignment, `++` or `--` to a static field evaluated for its effect, as statements:
    /// `COUNT.set(0)`, `COUNT.with_borrow_mut(|v| *v += 1)`. `None` where the general form of
    /// `update` is needed: a compound assignment whose right operand has effects or reads a
    /// static, which it would do while the cell is borrowed.
    pub(super) fn static_effect(&mut self, e: &'a Expr) -> Option<Vec<String>> {
        let (target, how) = match &e.kind {
            ExprKind::Assign { op: None, target, value } => (target, Update::Assign(value)),
            ExprKind::Assign { op: Some(op), target, value } => (target, Update::Compound(*op, value)),
            ExprKind::IncDec { target, increment, .. } => (target, Update::Step(*increment)),
            _ => return None,
        };
        let Place::Static { class, name } = target else { return None };
        let ty = e.ty;
        if matches!(ty, Type::Boxed(_)) && !matches!(how, Update::Assign(_)) {
            return None;
        }
        if let Update::Compound(_, value) = how
            && ty != Type::String
            && (has_effects(value) || reads_static(value))
        {
            return None;
        }
        let cell = cell_name(&self.class.types[*class].name, name);
        let init = self.init_call(*class);
        let mut lines = Vec::new();
        match how {
            Update::Assign(value) => {
                let nullable = self.place_nullable(target);
                let v = self.field_value(value, ty, nullable);
                // Java computes the value before it initializes the class to write it.
                match init {
                    Some(init) => {
                        let temp = self.temp("value");
                        lines.push(format!("let {temp} = {v};"));
                        lines.push(init);
                        lines.push(format!("{cell}.set({temp});"));
                    }
                    None => lines.push(format!("{cell}.set({v});")),
                }
            }
            // The old string and then the right operand are formatted, in Java's order.
            Update::Compound(_, value) if ty == Type::String => {
                lines.extend(init);
                let old = Slot::Static { cell: cell.clone(), plain: false }.read(true);
                // A static that may be null shows as `null` and is written as `Some`.
                let nullable = self.place_nullable(target);
                let old =
                    if nullable { format!("{old}.unwrap_or_else(|| String::from(\"null\"))") } else { old };
                let mut parts = vec![Part::Arg(Code::atom(old))];
                self.parts(value, &mut parts);
                let text = self.format("format!", parts).text;
                let text = if nullable { format!("Some({text})") } else { text };
                lines.push(format!("{cell}.set({text});"));
            }
            Update::Compound(..) | Update::Step(_) => {
                lines.extend(init);
                // The closure's parameter must not hide a local that the right operand reads.
                let v = self.temp("v");
                let marked = format!("({v})");
                let step = match how {
                    Update::Compound(op, value) => self.compound(op, &marked, ty, value, e.id),
                    Update::Step(increment) => self.step_place(marked.clone(), ty, increment, e.id),
                    Update::Assign(_) => unreachable!("handled above"),
                };
                let step = step.replacen(&format!("{marked} ="), &format!("*{v} ="), 1);
                let step = step.replacen(&format!("{marked} "), &format!("*{v} "), 1);
                let step = step
                    .replace(&format!("{marked}."), &format!("{v}."))
                    .replace(&marked, &format!("(*{v})"));
                if is_cell(ty) {
                    // A `Cell` holds a copy: the step works on a local, which is then stored.
                    let value = self.temp("value");
                    lines.push(format!("let mut {value} = {cell}.get();"));
                    lines.push(format!("{{ let {v} = &mut {value}; {step}; }}"));
                    lines.push(format!("{cell}.set({value});"));
                } else {
                    lines.push(format!("{cell}.with_borrow_mut(|{v}| {step});"));
                }
            }
        }
        Some(lines)
    }

    /// An assignment, `++` or `--` to a field or a static field, as a block that yields the value
    /// of the expression. Java's order is kept: the object, the old value, the right operand, then
    /// the write. No `RefCell` borrow is held while the right operand runs.
    pub(super) fn update(
        &mut self,
        place: &'a Place,
        ty: Type,
        how: Update<'a>,
        id: ExprId,
        prefix: bool,
    ) -> Code {
        let mut lines = Vec::new();
        // Java initializes the class of a static before it reads the static, but after the value
        // of an assignment is computed, when it writes.
        let init = match place {
            Place::Static { class, .. } => self.init_call(*class),
            _ => None,
        };
        let assigns = matches!(how, Update::Assign(_));
        if !assigns {
            lines.extend(init.clone());
        }
        let slot = match place {
            Place::Field { object, name } if self.is_plain(object) => {
                // Only a constructor writes a field of an immutable class, through its own `this`;
                // an owned object's fields are written by its methods.
                Slot::Field {
                    obj: self.object(object),
                    field: self.field_path(object, name),
                    plain: true,
                    on_enum: false,
                }
            }
            Place::Field { object, name } => {
                let obj = self.temp("object");
                let owned = self.object_handle(object);
                lines.push(format!("let {obj} = {owned};"));
                let on_enum = match self.enum_of(object) {
                    Some(t) => {
                        self.need(Need::Base(t));
                        self.need(Need::BaseMut(t));
                        true
                    }
                    None => false,
                };
                Slot::Field { obj, field: self.field_path(object, name), plain: false, on_enum }
            }
            Place::Static { class, name } => {
                Slot::Static { cell: cell_name(&self.class.types[*class].name, name), plain: is_cell(ty) }
            }
            Place::Var(_) => unreachable!("variables are assigned by `assignment`"),
            Place::Index(..) => unreachable!("array elements are assigned by `index_set`"),
        };
        let old = self.temp("old");
        let new = self.temp("value");
        // The right-hand side is computed on its own line, before the write, so only a call or a
        // field write inside it can conflict with a field read.
        let rhs = match &how {
            Update::Assign(value) | Update::Compound(_, value) => self.conflicts(&[value]),
            Update::Step(_) => false,
        };
        let outer = std::mem::replace(&mut self.guarded, rhs);
        let mut wrap = false;
        let result = match how {
            Update::Assign(value) if !matches!(ty, Type::Boxed(_)) && self.place_nullable(place) => {
                // The assignment's value is an `Option` only where the assigned value may be null.
                let v =
                    if self.nullable(value) { self.optional(value).text } else { self.value(value, ty).text };
                lines.push(format!("let {new} = {v};"));
                wrap = !self.nullable(value);
                new.clone()
            }
            Update::Assign(value) => {
                let v = self.field_value(value, ty, false);
                lines.push(format!("let {new} = {v};"));
                new.clone()
            }
            Update::Compound(op, value) if matches!(ty, Type::Boxed(_)) => {
                lines.push(format!("let {old} = {};", slot.read(self.clones(ty))));
                let right = self.operand(value);
                let stored = self.boxed_result(op, ty, Code::atom(old.clone()), true, right, id);
                lines.push(format!("let {new} = {};", stored.text));
                format!("{}({new})", self.runtime("non_null"))
            }
            Update::Step(increment) if matches!(ty, Type::Boxed(_)) => {
                lines.push(format!("let {old} = {};", slot.read(self.clones(ty))));
                let op = if increment { BinOp::Add } else { BinOp::Sub };
                let one = Operand { code: Code::literal(Literal::Int(1)), ty: Type::Int };
                let stored = self.boxed_result(op, ty, Code::atom(old.clone()), true, one, id);
                lines.push(format!("let {new} = {};", stored.text));
                let value = if prefix { new.clone() } else { old.clone() };
                format!("{}({value})", self.runtime("non_null"))
            }
            Update::Compound(_, value) if ty == Type::String => {
                let old = slot.read(self.clones(ty));
                // A field that may be null is an `Option`, which shows as `null` and is written
                // as `Some`: the text is never null.
                let old = if self.place_nullable(place) {
                    wrap = true;
                    format!("{old}.unwrap_or_else(|| String::from(\"null\"))")
                } else {
                    old
                };
                let mut parts = vec![Part::Arg(Code::atom(old))];
                self.parts(value, &mut parts);
                lines.push(format!("let {new} = {};", self.format("format!", parts).text));
                new.clone()
            }
            Update::Compound(op, value) => {
                lines.push(format!("let {old} = {};", slot.read(self.clones(ty))));
                let rty = crate::types::binary_type(op, ty, value.ty.unboxed()).expect("checked");
                let right = self.operand(value);
                let code = self.binary(op, Operand { code: Code::atom(old.clone()), ty }, right, rty, id);
                lines.push(format!("let {new} = {};", self.convert(code, rty, ty).text));
                new.clone()
            }
            Update::Step(increment) => {
                lines.push(format!("let {old} = {};", slot.read(self.clones(ty))));
                let sign = if increment { "+" } else { "-" };
                let text = if ty.is_floating() {
                    format!("{old} {sign} 1.0")
                } else if self.no_overflow(id) {
                    format!("{old} {sign} 1")
                } else {
                    format!("{old}.wrapping_{}(1)", if increment { "add" } else { "sub" })
                };
                lines.push(format!("let {new} = {text};"));
                if prefix { new.clone() } else { old.clone() }
            }
        };
        self.guarded = outer;
        let stored = if self.clones(ty) { format!("{new}.clone()") } else { new.clone() };
        let stored = if wrap { format!("Some({stored})") } else { stored };
        if assigns {
            lines.extend(init);
        }
        lines.push(slot.write(&stored));
        Code::atom(format!("{{ {} {result} }}", lines.join(" ")))
    }

    /// The struct and `impl` of a class with state: its constructors and instance methods.
    /// The struct and `impl` of a class with state: its constructors and the instance methods
    /// emitted there, its own and those it inherits (see `analysis::hierarchy::home`). A class with
    /// subclasses has a fields struct, `<Name>Fields`, which the structs of its subclasses embed as
    /// `base`.
    pub(super) fn type_items(&mut self, index: usize) {
        let class = self.class;
        let decl = &class.types[index];
        let (params, args) = self.type_params(&decl.type_params);
        let name = self.struct_of(index);
        self.gap();
        self.line(&format!("{}struct {name}{args} {{", self.vis()));
        self.indent += 1;
        if let Some(base) = hierarchy::embedded(self.facts, class, index) {
            self.line(&format!("{}base: {},", self.vis(), self.struct_of(base)));
        }
        for f in &decl.fields {
            let ty = self.field_ty_name(f.ty, self.field_nullable(&decl.name, &f.name));
            self.line(&format!("{}{}: {ty},", self.vis(), snake_case(&f.name)));
        }
        self.indent -= 1;
        self.line("}");
        self.gap();
        self.line(&format!("impl{params} {name}{args} {{"));
        self.indent += 1;
        self.this_struct = index;
        self.field_inits(index);
        let mut display = None;
        // Its own constructors and methods first, then those it inherits.
        let mut order: Vec<usize> = (0..class.methods.len()).collect();
        order.sort_by_key(|&i| class.methods[i].class != index);
        for i in order {
            if self.emitted_in(i, index) {
                if self.displays(i) {
                    display = Some(i);
                    continue;
                }
                self.gap();
                self.method(i);
            }
        }
        self.indent -= 1;
        self.line("}");
        // `toString` is how the object is displayed.
        if let Some(i) = display {
            self.gap();
            self.line(&format!("impl{params} fmt::Display for {name}{args} {{"));
            self.indent += 1;
            self.method(i);
            self.indent -= 1;
            self.line("}");
        } else if let Some(i) = hierarchy::to_string_impl(class, index)
            && let Home::Fields(d) = self.homes[i]
            && d != index
            && class.hierarchy.is_concrete(index)
            && self.displays(i)
        {
            // An inherited `toString` displays the superclass's fields struct.
            let path = hierarchy::base_path(self.facts, class, index, d);
            self.gap();
            self.line(&format!("impl fmt::Display for {name} {{"));
            self.indent += 1;
            self.line("fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {");
            self.indent += 1;
            self.line(&format!("fmt::Display::fmt(&self.{}, f)", path.trim_end_matches('.')));
            self.indent -= 1;
            self.line("}");
            self.indent -= 1;
            self.line("}");
        }
    }

    /// Whether method `i` is emitted in the `impl` of the struct of class `t`: it lives there, and
    /// some object runs it (a superclass's method that every subclass overrides runs only through
    /// `super`).
    fn emitted_in(&self, i: usize, t: usize) -> bool {
        let m = &self.class.methods[i];
        if m.is_static {
            return false;
        }
        let h = &self.class.hierarchy;
        let runs = |c: usize| h.implementation(i, c) == Some(i) || self.super_targets.contains(&i);
        match self.homes[i] {
            _ if m.is_ctor => m.class == t,
            Home::Class(c) => c == t && (m.class == c || runs(c)),
            Home::Fields(d) => {
                d == t && (h.concretes(d).into_iter().any(runs) || self.super_targets.contains(&i))
            }
            _ => false,
        }
    }

    /// Whether method `index` is a `toString` that becomes the class's `Display` implementation:
    /// it borrows its object without changing it and never returns null, and it is what an object
    /// of the struct it is emitted in runs.
    pub(super) fn displays(&self, index: usize) -> bool {
        let m = &self.class.methods[index];
        let runs = match self.homes[index] {
            Home::Class(c) => hierarchy::to_string_impl(self.class, c) == Some(index),
            Home::Fields(_) => true,
            _ => false,
        };
        m.name == "toString"
            && m.params.is_empty()
            && !m.is_static
            && m.ret == Type::String
            && runs
            && self.facts.self_methods.contains(&index)
            && !self.facts.mutating.contains(&index)
            && !self.facts.nullable_returns.contains(&index)
    }

    /// Whether displaying an object of `e`'s class can neither print nor throw, so that printing
    /// may format it in the middle of the printed line: its `toString` returns a concatenation of
    /// constants, variables and fields that cannot throw.
    fn quiet_text(&self, e: &Expr, depth: usize) -> bool {
        if self.enum_of(e).is_some() {
            return false;
        }
        let Some(index) = self.to_string_index(e) else { return false };
        let m = &self.class.methods[index];
        depth < 4
            && self.displays(index)
            && matches!(&m.body[..], [Stmt::Return(Some(value), _)] if self.quiet(value, depth))
    }

    fn quiet(&self, e: &Expr, depth: usize) -> bool {
        match &e.kind {
            ExprKind::Int(_)
            | ExprKind::Long(_)
            | ExprKind::Double(_)
            | ExprKind::Float(_)
            | ExprKind::Char(_)
            | ExprKind::Bool(_)
            | ExprKind::Str(_)
            | ExprKind::Var(_) => !matches!(e.ty, Type::Class(..) | Type::Array(_)),
            ExprKind::Field { object, .. } => {
                matches!(object.kind, ExprKind::This)
                    && match e.ty {
                        Type::Class(..) => self.quiet_text(e, depth + 1),
                        Type::Array(_) => false,
                        _ => true,
                    }
            }
            ExprKind::Binary(op, l, r) => {
                !matches!(op, BinOp::Div | BinOp::Rem)
                    && [l, r].iter().all(|x| match x.ty {
                        Type::Class(..) if matches!(x.kind, ExprKind::This) => false,
                        _ => self.quiet(x, depth),
                    })
            }
            ExprKind::Unary(_, x) | ExprKind::Cast(_, x) | ExprKind::InstanceOf { value: x, .. } => {
                self.quiet(x, depth)
            }
            ExprKind::Conditional(c, a, b) => [c, a, b].iter().all(|x| self.quiet(x, depth)),
            _ => false,
        }
    }

    /// Runs the field initializers of an object, in source order, on `this`.
    fn field_inits(&mut self, index: usize) {
        let class = self.class;
        let inits: Vec<&'a Field> = class.types[index].fields.iter().filter(|f| f.init.is_some()).collect();
        if inits.is_empty() || self.inline_inits(index) {
            return;
        }
        let name = &class.types[index].name;
        // A plain value is built in place, so its initializers borrow it as `&mut self`; an
        // immutable object is built in place too.
        let plain = self.builds_plain(name);
        let by_value = plain || self.facts.immutable.contains(name);
        self.this_form = if plain { ThisForm::SelfRef } else { ThisForm::Shared };
        self.gap();
        if plain {
            self.line("fn field_inits(&mut self) {");
        } else if by_value {
            self.line(&format!("fn field_inits(this: &mut {}) {{", self.generic_struct(index)));
        } else {
            self.line(&format!("fn field_inits(this: &Rc<RefCell<{}>>) {{", self.generic_struct(index)));
        }
        self.indent += 1;
        for f in inits {
            // The line writes a field, so a read in the value must end its `Ref` first.
            let outer = std::mem::replace(&mut self.guarded, true);
            let nullable = self.field_nullable(name, &f.name);
            let value = self.field_value(f.init.as_ref().expect("filtered"), f.ty, nullable);
            self.guarded = outer;
            let field = snake_case(&f.name);
            if by_value {
                self.line(&format!("{}.{field} = {value};", self.this_name()));
            } else {
                self.line(&format!("this.borrow_mut().{field} = {value};"));
            }
        }
        self.indent -= 1;
        self.line("}");
    }

    /// The body of a constructor, which builds the object and returns it. `this(...)` builds it by
    /// calling another constructor; otherwise every field gets its default and the initializers run
    /// before the rest of the body.
    pub(super) fn constructor(&mut self, m: &'a Method) {
        let class = self.class;
        self.this_form = ThisForm::Shared;
        if self.builds_plain(&class.types[m.class].name) {
            return self.plain_constructor(m);
        }
        let decl = &class.types[m.class];
        let (delegate, rest) = match m.body.split_first() {
            Some((Stmt::Expr(Expr { kind: ExprKind::ThisCall { args, resolved }, .. }), rest)) => {
                (Some((args, resolved.expect("calls are resolved by the checker"))), rest)
            }
            _ => (None, &m.body[..]),
        };
        let (base, rest) = self.super_init(m.class, rest);
        let initialize = delegate.is_none() && decl.fields.iter().any(|f| f.init.is_some());
        let immutable = self.facts.immutable.contains(&decl.name);
        let mutable = initialize || uses_this(rest);
        match delegate {
            Some((args, index)) => {
                let args = self.call_args(index, args).join(", ");
                let call = Code::atom(format!("{}({args})", self.qualified_fn(index)));
                let call = self.raising(call, index).text;
                self.line(&format!("let obj = {call};"));
            }
            None => {
                let mut fields: Vec<String> = base.into_iter().map(|b| format!("base: {b}")).collect();
                fields.extend(decl.fields.iter().map(|f| {
                    format!(
                        "{}: {}",
                        snake_case(&f.name),
                        self.placeholder(f.ty, self.field_nullable(&decl.name, &f.name))
                    )
                }));
                let name = self.struct_of(m.class);
                let fields = fields.join(", ");
                if immutable {
                    let mut_kw = if mutable { "mut " } else { "" };
                    self.line(&format!("let {mut_kw}obj = {name} {{ {fields} }};"));
                } else {
                    self.line(&format!("let obj = Rc::new(RefCell::new({name} {{ {fields} }}));"));
                }
            }
        }
        // An immutable object is built in place and shared only once it is complete.
        let this = if immutable { "&mut obj" } else { "&obj" };
        if mutable {
            self.line(&format!("let this = {this};"));
        }
        if initialize {
            self.line(&format!("{}::field_inits(this);", self.struct_of(m.class)));
        }
        let result = if immutable { "Rc::new(obj)" } else { "obj" };
        self.ctor_result = Some(result);
        self.stmts(without_final_return(rest), false);
        self.ctor_result = None;
        let result = self.ok(result.to_string());
        self.line(&result);
    }

    /// The `super(...)` that starts the statements of a constructor of class `t`, if one does: the
    /// call of the superclass's constructor that builds the fields struct `t` embeds as `base`,
    /// and the statements after it. A superclass without a fields struct has a constructor that
    /// does nothing; only the arguments' effects are written, as statements.
    pub(super) fn super_init(&mut self, t: usize, stmts: &'a [Stmt]) -> (Option<String>, &'a [Stmt]) {
        let Some((Stmt::Expr(Expr { kind: ExprKind::SuperInit { args, resolved }, .. }), rest)) =
            stmts.split_first()
        else {
            return (None, stmts);
        };
        match (hierarchy::embedded(self.facts, self.class, t), resolved) {
            (Some(_), Some(ctor)) => {
                let args = self.call_args(*ctor, args).join(", ");
                let call = Code::atom(format!("{}({args})", self.qualified_fn(*ctor)));
                (Some(self.raising(call, *ctor).text), rest)
            }
            _ => {
                for a in args.iter().filter(|a| has_effects(a)) {
                    let text = self.expr(a).text;
                    self.line(&format!("{text};"));
                }
                (None, rest)
            }
        }
    }

    /// A blank line before an item, unless the output is empty or a line of an item is last.
    pub(super) fn gap(&mut self) {
        if self.out.ends_with("}\n") || self.out.ends_with(";\n") {
            self.out.push('\n');
        }
    }
}

/// Whether evaluating `e` may see the static state of a class: it reads or writes a static
/// field, or calls or creates something that may.
fn sees_statics(e: &Expr) -> bool {
    let mut found = false;
    visit_expr(e, &mut |x| {
        found |= matches!(
            x.kind,
            ExprKind::Static { .. }
                | ExprKind::Call { .. }
                | ExprKind::MethodCall { .. }
                | ExprKind::New { .. }
                | ExprKind::Assign { target: Place::Static { .. }, .. }
                | ExprKind::IncDec { target: Place::Static { .. }, .. }
        )
    });
    found
}

/// Whether `e` is `null`, converted or not.
fn is_null(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Null => true,
        ExprKind::Cast(_, inner) => is_null(inner),
        _ => false,
    }
}
