//! Code generation for classes with state. A class is a struct, and an object is an
//! `Rc<RefCell<C>>` that every reference shares; a field access borrows it for one block, so
//! that no `RefCell` borrow outlives the expression that made it. Static fields live in
//! thread-local cells, which are initialized on first use.

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
    },
    Static {
        cell: String,
    },
}

impl Slot {
    fn read(&self, ty: Type) -> String {
        let copy = !needs_clone(ty);
        match self {
            Slot::Field { obj, field, plain: true } => {
                format!("{obj}.{field}{}", if copy { "" } else { ".clone()" })
            }
            Slot::Field { obj, field, plain: false } => {
                format!("{obj}.borrow().{field}{}", if copy { "" } else { ".clone()" })
            }
            Slot::Static { cell } => {
                format!("{cell}.with_borrow({})", if copy { "|v| *v" } else { "|v| v.clone()" })
            }
        }
    }

    fn write(&self, value: &str) -> String {
        match self {
            Slot::Field { obj, field, plain: true } => format!("{obj}.{field} = {value};"),
            Slot::Field { obj, field, plain: false } => format!("{obj}.borrow_mut().{field} = {value};"),
            Slot::Static { cell } => format!("{cell}.with_borrow_mut(|v| *v = {value});"),
        }
    }
}

/// Whether values of this type are owned heap data or shared arrays or objects, which are cloned
/// when read.
pub(super) fn needs_clone(ty: Type) -> bool {
    matches!(ty, Type::String | Type::Array(_) | Type::Class(_) | Type::Boxed(_))
}

/// The Rust name of a class: its qualified Java name with the dots replaced.
pub(super) fn struct_name(name: &str) -> String {
    name.replace('.', "_")
}

/// The Rust type of an object of a class. An immutable class's objects never change, so they need
/// no `RefCell` to be shared.
pub(super) fn class_type(name: &str, immutable: bool) -> String {
    if immutable {
        format!("Rc<{}>", struct_name(name))
    } else {
        format!("Rc<RefCell<{}>>", struct_name(name))
    }
}

/// The thread-local cell that holds a static field.
pub(super) fn cell_name(class: &str, field: &str) -> String {
    format!("{}_{}", struct_name(class), snake_case(field)).to_uppercase()
}

/// The value a field has before its constructor runs: Java's default, or a placeholder that the
/// initializers overwrite. Object fields have no placeholder, so lowering rejects them.
pub(super) fn default_value(ty: Type) -> &'static str {
    match ty {
        Type::Int | Type::Long | Type::Byte | Type::Short | Type::Char => "0",
        Type::Double | Type::Float => "0.0",
        Type::Boolean => "false",
        Type::String => "String::new()",
        Type::Boxed(_) => "None",
        _ => unreachable!("lowering rejects fields of type {ty}"),
    }
}

/// Which classes are ever instantiated. Only those get a struct and instance methods, so a class
/// with only static members is plain functions.
pub(super) fn live_types(class: &Class) -> Vec<bool> {
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

/// Whether an expression writes a field or calls a method or constructor, which may borrow an
/// object mutably while a field read of the same statement still holds its `Ref`.
pub(super) fn borrows_mut(e: &Expr) -> bool {
    let mut found = false;
    visit_expr(e, &mut |x| {
        found |= matches!(
            &x.kind,
            ExprKind::Assign { target: Place::Field { .. }, .. }
                | ExprKind::IncDec { target: Place::Field { .. }, .. }
                | ExprKind::Call { .. }
                | ExprKind::MethodCall { .. }
                | ExprKind::New { .. }
                | ExprKind::ThisCall { .. }
        ) || converts_object(x);
    });
    found
}

/// Whether an expression turns an object into text, which runs the object's `toString`.
fn converts_object(x: &Expr) -> bool {
    match &x.kind {
        ExprKind::Builtin(_, args) => args.iter().any(|a| matches!(a.ty, Type::Class(_))),
        ExprKind::Binary(_, l, r) => matches!(l.ty, Type::Class(_)) || matches!(r.ty, Type::Class(_)),
        _ => false,
    }
}

/// Whether the expressions that a statement evaluates itself borrow mutably; the expressions of
/// nested statements are checked when those are emitted.
pub(super) fn stmt_borrows_mut(stmt: &Stmt) -> bool {
    let exprs: Vec<&Expr> = match stmt {
        Stmt::Local { init, .. } => init.iter().collect(),
        Stmt::Expr(e) | Stmt::Return(Some(e), _) => vec![e],
        Stmt::If { cond, .. } | Stmt::While { cond, .. } | Stmt::DoWhile { cond, .. } => vec![cond],
        Stmt::ForEach { array, .. } => vec![array],
        Stmt::For { cond, update, .. } => cond.iter().chain(update.iter()).collect(),
        Stmt::Return(None, _) | Stmt::Break | Stmt::Continue | Stmt::Block(_) => Vec::new(),
    };
    exprs.into_iter().any(borrows_mut)
}

/// Whether some expression of the statements is `this`, so that a constructor must bind it.
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

impl<'a> Gen<'a> {
    /// The Rust type of an object of class `name`: a plain value for an owned class.
    pub(super) fn object_type(&self, name: &str) -> String {
        if self.facts.owned.contains(name) {
            struct_name(name)
        } else {
            class_type(name, self.facts.immutable.contains(name))
        }
    }

    /// Whether `e` is an object whose fields are read and written without a `RefCell`: an object of
    /// an immutable class, or of an owned class.
    pub(super) fn is_plain(&self, e: &Expr) -> bool {
        matches!(e.ty, Type::Class(name) if self.facts.immutable.contains(name) || self.facts.owned.contains(name))
    }

    /// The type of `this` in a method of class `name`: a reference to the object.
    pub(super) fn this_type(&self, name: &str) -> String {
        if self.facts.immutable.contains(name) {
            format!("&{}", struct_name(name))
        } else {
            format!("&{}", class_type(name, false))
        }
    }

    /// The name of `this` in the code being written: `self` in an owned class's method, else `this`.
    pub(super) fn this_name(&self) -> &'static str {
        if self.owned_self { "self" } else { "this" }
    }

    /// The text of an object used for a field access: `this`, a variable, or an expression.
    pub(super) fn object(&mut self, e: &'a Expr) -> String {
        match &e.kind {
            ExprKind::This => self.this_name().to_string(),
            ExprKind::Var(name) => self.local(name),
            _ => self.expr(e).receiver(),
        }
    }

    /// `e` as the receiver of a method, borrowed; `this` is already a reference.
    pub(super) fn receiver_ref(&mut self, e: &'a Expr) -> String {
        match &e.kind {
            ExprKind::This => self.this_name().to_string(),
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
                } else if self.facts.borrowed_optional_params.contains(&(index, i)) {
                    self.option_arg(a)
                } else if self.optional_var(index, p.ty, &p.name) {
                    self.optional_to(a, p.ty).text
                } else if let Some(&written) = self.facts.borrowed_arrays.get(&(index, i)) {
                    self.borrowed_arg(a, written)
                } else if self.guarded
                    && matches!(&a.kind, ExprKind::Field { object, .. } if !self.is_plain(object))
                {
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

    /// The Rust path of a method or constructor, such as `Point::new` or `Point::norm`.
    pub(super) fn qualified_fn(&self, index: usize) -> String {
        let owner = self.class.methods[index].class;
        format!("{}::{}", struct_name(&self.class.types[owner].name), self.fn_names[index])
    }

    /// A value read from a field of an object.
    pub(super) fn field_read(&mut self, object: &'a Expr, name: &str, ty: Type) -> Code {
        let obj = self.object(object);
        let clone = if needs_clone(ty) { ".clone()" } else { "" };
        if self.is_plain(object) {
            return Code::atom(format!("{obj}.{}{clone}", snake_case(name)));
        }
        let read = format!("{obj}.borrow().{}{clone}", snake_case(name));
        // The `Ref` lives to the end of the statement, so it is scoped with braces only when the
        // statement also borrows mutably; otherwise the braces would be noise.
        Code::atom(if self.guarded { format!("{{ {read} }}") } else { read })
    }

    /// A value read from a static field.
    pub(super) fn static_read(&self, class: usize, name: &str, ty: Type) -> Code {
        let cell = cell_name(&self.class.types[class].name, name);
        Code::atom(Slot::Static { cell }.read(ty))
    }

    /// `new C(args)`, which calls the constructor that the checker chose.
    pub(super) fn new_object(&mut self, ctor: usize, args: &'a [Expr]) -> Code {
        let args = self.call_args(ctor, args).join(", ");
        Code::atom(format!("{}({args})", self.qualified_fn(ctor)))
    }

    /// `receiver.name(args)`, a call of an instance method.
    pub(super) fn method_call(&mut self, receiver: &'a Expr, index: usize, args: &'a [Expr]) -> Code {
        if self.is_owned_method(index) {
            // An owned object is a value, so its method is called with method syntax: the receiver
            // is borrowed by the call itself, and `p.set(p.x + 1)` is accepted by two-phase borrows.
            let args = self.call_args(index, args).join(", ");
            let obj = self.object(receiver);
            return Code::atom(format!("{obj}.{}({args})", self.fn_names[index]));
        }
        let mut all = vec![self.receiver_ref(receiver)];
        all.extend(self.call_args(index, args));
        Code::atom(format!("{}({})", self.qualified_fn(index), all.join(", ")))
    }

    /// The text of an object, which Java gets from its class's `toString` (lowering rejects classes
    /// without one).
    pub(super) fn object_text(&mut self, e: &'a Expr) -> Code {
        let Type::Class(name) = e.ty else { unreachable!("an object has a class type") };
        let index = self
            .class
            .methods
            .iter()
            .position(|m| {
                self.class.types[m.class].name == name && m.name == "toString" && m.params.is_empty()
            })
            .expect("lowering checks that the class has toString");
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
        let slot = match place {
            Place::Field { object, name } if self.is_plain(object) => {
                // Only a constructor writes a field of an immutable class, through its own `this`;
                // an owned object's fields are written by its methods.
                Slot::Field { obj: self.object(object), field: snake_case(name), plain: true }
            }
            Place::Field { object, name } => {
                let obj = self.temp("object");
                let owned = match &object.kind {
                    ExprKind::This => "this.clone()".to_string(),
                    ExprKind::Var(v) => format!("{}.clone()", self.local(v)),
                    _ => self.expr(object).text,
                };
                lines.push(format!("let {obj} = {owned};"));
                Slot::Field { obj, field: snake_case(name), plain: false }
            }
            Place::Static { class, name } => {
                Slot::Static { cell: cell_name(&self.class.types[*class].name, name) }
            }
            Place::Var(_) => unreachable!("variables are assigned by `assignment`"),
            Place::Index(..) => unreachable!("array elements are assigned by `index_set`"),
        };
        let old = self.temp("old");
        let new = self.temp("value");
        // The right-hand side is computed on its own line, before the write, so only a call or a
        // field write inside it can conflict with a field read.
        let rhs = match &how {
            Update::Assign(value) | Update::Compound(_, value) => borrows_mut(value),
            Update::Step(_) => false,
        };
        let outer = std::mem::replace(&mut self.guarded, rhs);
        let result = match how {
            Update::Assign(value) => {
                let v = self.field_value(value, ty);
                lines.push(format!("let {new} = {v};"));
                new.clone()
            }
            Update::Compound(op, value) if matches!(ty, Type::Boxed(_)) => {
                lines.push(format!("let {old} = {};", slot.read(ty)));
                let right = self.operand(value);
                let stored = self.boxed_result(op, ty, Code::atom(old.clone()), true, right, id);
                lines.push(format!("let {new} = {};", stored.text));
                format!("{}({new})", self.runtime("non_null"))
            }
            Update::Step(increment) if matches!(ty, Type::Boxed(_)) => {
                lines.push(format!("let {old} = {};", slot.read(ty)));
                let op = if increment { BinOp::Add } else { BinOp::Sub };
                let one = Operand { code: Code::literal(Literal::Int(1)), ty: Type::Int };
                let stored = self.boxed_result(op, ty, Code::atom(old.clone()), true, one, id);
                lines.push(format!("let {new} = {};", stored.text));
                let value = if prefix { new.clone() } else { old.clone() };
                format!("{}({value})", self.runtime("non_null"))
            }
            Update::Compound(_, value) if ty == Type::String => {
                let mut parts = vec![Part::Arg(Code::atom(slot.read(ty)))];
                self.parts(value, &mut parts);
                lines.push(format!("let {new} = {};", self.format("format!", parts).text));
                new.clone()
            }
            Update::Compound(op, value) => {
                lines.push(format!("let {old} = {};", slot.read(ty)));
                let rty = crate::types::binary_type(op, ty, value.ty.unboxed()).expect("checked");
                let right = self.operand(value);
                let code = self.binary(op, Operand { code: Code::atom(old.clone()), ty }, right, rty, id);
                lines.push(format!("let {new} = {};", self.convert(code, rty, ty).text));
                new.clone()
            }
            Update::Step(increment) => {
                lines.push(format!("let {old} = {};", slot.read(ty)));
                let sign = if increment { "+" } else { "-" };
                let text = if ty == Type::Double {
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
        let stored = if needs_clone(ty) { format!("{new}.clone()") } else { new.clone() };
        lines.push(slot.write(&stored));
        Code::atom(format!("{{ {} {result} }}", lines.join(" ")))
    }

    /// The struct and `impl` of a class with state: its constructors and instance methods.
    pub(super) fn type_items(&mut self, index: usize) {
        let class = self.class;
        let decl = &class.types[index];
        let name = struct_name(&decl.name);
        self.gap();
        self.line(&format!("struct {name} {{"));
        self.indent += 1;
        for f in &decl.fields {
            let ty = self.field_ty_name(f.ty);
            self.line(&format!("{}: {ty},", snake_case(&f.name)));
        }
        self.indent -= 1;
        self.line("}");
        self.gap();
        self.line(&format!("impl {name} {{"));
        self.indent += 1;
        self.field_inits(index);
        for i in 0..class.methods.len() {
            if class.methods[i].class == index && !class.methods[i].is_static {
                self.gap();
                self.method(i);
            }
        }
        self.indent -= 1;
        self.line("}");
    }

    /// Runs the field initializers of an object, in source order, on `this`.
    fn field_inits(&mut self, index: usize) {
        let class = self.class;
        let inits: Vec<&'a Field> = class.types[index].fields.iter().filter(|f| f.init.is_some()).collect();
        if inits.is_empty() {
            return;
        }
        let name = &class.types[index].name;
        // Immutable and owned objects are built in place, so `this` is a `&mut` to the value.
        let by_value = self.facts.immutable.contains(name) || self.facts.owned.contains(name);
        self.owned_self = false;
        self.gap();
        let this = if by_value {
            format!("&mut {}", struct_name(name))
        } else {
            format!("&{}", class_type(name, false))
        };
        self.line(&format!("fn field_inits(this: {this}) {{"));
        self.indent += 1;
        for f in inits {
            // The line writes a field, so a read in the value must end its `Ref` first.
            let outer = std::mem::replace(&mut self.guarded, true);
            let value = self.field_value(f.init.as_ref().expect("filtered"), f.ty);
            self.guarded = outer;
            let field = snake_case(&f.name);
            if by_value {
                self.line(&format!("this.{field} = {value};"));
            } else {
                self.line(&format!("this.borrow_mut().{field} = {value};"));
            }
        }
        self.indent -= 1;
        self.line("}");
    }

    /// The thread-local cells of all static fields, initialized lazily on first use.
    pub(super) fn statics(&mut self) {
        let class = self.class;
        if class.types.iter().all(|t| t.statics.is_empty()) {
            return;
        }
        self.gap();
        self.line("thread_local! {");
        self.indent += 1;
        for decl in &class.types {
            for f in &decl.statics {
                let init = match &f.init {
                    Some(e) => self.field_value(e, f.ty),
                    None => default_value(f.ty).to_string(),
                };
                let cell = cell_name(&decl.name, &f.name);
                let ty = self.field_ty_name(f.ty);
                self.line(&format!("static {cell}: RefCell<{ty}> = RefCell::new({init});"));
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
        self.owned_self = false;
        if self.facts.owned.contains(&class.types[m.class].name) {
            return self.owned_constructor(m);
        }
        let decl = &class.types[m.class];
        let (delegate, rest) = match m.body.split_first() {
            Some((Stmt::Expr(Expr { kind: ExprKind::ThisCall { args, resolved }, .. }), rest)) => {
                (Some((args, resolved.expect("calls are resolved by the checker"))), rest)
            }
            _ => (None, &m.body[..]),
        };
        let initialize = delegate.is_none() && decl.fields.iter().any(|f| f.init.is_some());
        let immutable = self.facts.immutable.contains(&decl.name);
        let mutable = initialize || uses_this(rest);
        match delegate {
            Some((args, index)) => {
                let args = self.call_args(index, args).join(", ");
                self.line(&format!("let obj = {}({args});", self.qualified_fn(index)));
            }
            None => {
                let fields: Vec<String> = decl
                    .fields
                    .iter()
                    .map(|f| format!("{}: {}", snake_case(&f.name), default_value(f.ty)))
                    .collect();
                let name = struct_name(&decl.name);
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
            self.line(&format!("{}::field_inits(this);", struct_name(&decl.name)));
        }
        self.stmts(rest, false);
        self.line(if immutable { "Rc::new(obj)" } else { "obj" });
    }

    /// A blank line before an item, unless the output is empty or a line of an item is last.
    pub(super) fn gap(&mut self) {
        if self.out.ends_with("}\n") {
            self.out.push('\n');
        }
    }
}
