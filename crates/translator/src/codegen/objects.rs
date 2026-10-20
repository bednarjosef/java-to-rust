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
    Field { obj: String, field: String },
    Static { cell: String },
}

impl Slot {
    fn read(&self, ty: Type) -> String {
        let copy = !needs_clone(ty);
        match self {
            Slot::Field { obj, field } => {
                format!("{obj}.borrow().{field}{}", if copy { "" } else { ".clone()" })
            }
            Slot::Static { cell } => {
                format!("{cell}.with_borrow({})", if copy { "|v| *v" } else { "|v| v.clone()" })
            }
        }
    }

    fn write(&self, value: &str) -> String {
        match self {
            Slot::Field { obj, field } => format!("{obj}.borrow_mut().{field} = {value};"),
            Slot::Static { cell } => format!("{cell}.with_borrow_mut(|v| *v = {value});"),
        }
    }
}

/// Whether values of this type are owned heap data or shared objects, which are cloned when read.
pub(super) fn needs_clone(ty: Type) -> bool {
    matches!(ty, Type::String | Type::Class(_))
}

/// The Rust name of a class: its qualified Java name with the dots replaced.
pub(super) fn struct_name(name: &str) -> String {
    name.replace('.', "_")
}

/// The Rust type of an object of a class.
pub(super) fn class_type(name: &str) -> String {
    format!("Rc<RefCell<{}>>", struct_name(name))
}

/// The thread-local cell that holds a static field.
pub(super) fn cell_name(class: &str, field: &str) -> String {
    format!("{}_{}", struct_name(class), snake_case(field)).to_uppercase()
}

/// The value a field has before its constructor runs: Java's default, or a placeholder that the
/// initializers overwrite. Object fields have no placeholder, so lowering rejects them.
pub(super) fn default_value(ty: Type) -> &'static str {
    match ty {
        Type::Int | Type::Long => "0",
        Type::Double => "0.0",
        Type::Boolean => "false",
        Type::String => "String::new()",
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
            visit_stmt(s, &mut note, &mut |_| {});
        }
    }
    for f in class.types.iter().flat_map(|t| t.fields.iter().chain(&t.statics)) {
        if let Some(init) = &f.init {
            visit_expr(init, &mut note);
        }
    }
    live
}

/// The Rust type of a value of any type, including objects.
pub(super) fn rust_class_type(ty: Type) -> String {
    match ty {
        Type::Class(name) => class_type(name),
        _ => rust_type(ty).to_string(),
    }
}

/// The part of an overloaded method's name that a parameter type contributes.
pub(super) fn type_suffix(ty: Type) -> String {
    match ty {
        Type::Class(name) => struct_name(name).to_lowercase(),
        _ => rust_type(ty).to_lowercase(),
    }
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
        )
    });
    found
}

/// Whether the expressions that a statement evaluates itself borrow mutably; the expressions of
/// nested statements are checked when those are emitted.
pub(super) fn stmt_borrows_mut(stmt: &Stmt) -> bool {
    let exprs: Vec<&Expr> = match stmt {
        Stmt::Local { init, .. } => init.iter().collect(),
        Stmt::Expr(e) | Stmt::Return(Some(e), _) => vec![e],
        Stmt::If { cond, .. } | Stmt::While { cond, .. } | Stmt::DoWhile { cond, .. } => vec![cond],
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
    /// The text of an object used for a field access: `this`, a variable, or an expression.
    pub(super) fn object(&mut self, e: &'a Expr) -> String {
        match &e.kind {
            ExprKind::This => "this".to_string(),
            ExprKind::Var(name) => self.local(name),
            _ => self.expr(e).receiver(),
        }
    }

    /// `e` as the receiver of a method, borrowed; `this` is already a reference.
    pub(super) fn receiver_ref(&mut self, e: &'a Expr) -> String {
        match &e.kind {
            ExprKind::This => "this".to_string(),
            _ => format!("&{}", self.object(e)),
        }
    }

    /// The arguments of a call of method `index`: `&str` parameters are borrowed, others are values.
    pub(super) fn call_args(&mut self, index: usize, args: &'a [Expr]) -> Vec<String> {
        let params = &self.class.methods[index].params;
        args.iter()
            .zip(params)
            .enumerate()
            .map(|(i, (a, p))| {
                if self.facts.borrowed_params.contains(&(index, i)) {
                    self.str_ref(a)
                } else if self.guarded && matches!(a.kind, ExprKind::Field { .. }) {
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
        let mut all = vec![self.receiver_ref(receiver)];
        all.extend(self.call_args(index, args));
        Code::atom(format!("{}({})", self.qualified_fn(index), all.join(", ")))
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
            Place::Field { object, name } => {
                let obj = self.temp("object");
                let owned = match &object.kind {
                    ExprKind::This => "this.clone()".to_string(),
                    ExprKind::Var(v) => format!("{}.clone()", self.local(v)),
                    _ => self.expr(object).text,
                };
                lines.push(format!("let {obj} = {owned};"));
                Slot::Field { obj, field: snake_case(name) }
            }
            Place::Static { class, name } => {
                Slot::Static { cell: cell_name(&self.class.types[*class].name, name) }
            }
            Place::Var(_) => unreachable!("variables are assigned by `assignment`"),
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
                let v = self.value(value, ty).text;
                lines.push(format!("let {new} = {v};"));
                new.clone()
            }
            Update::Compound(_, value) if ty == Type::String => {
                let mut parts = vec![Part::Arg(Code::atom(slot.read(ty)))];
                self.parts(value, &mut parts);
                lines.push(format!("let {new} = {};", self.format("format!", parts).text));
                new.clone()
            }
            Update::Compound(op, value) => {
                lines.push(format!("let {old} = {};", slot.read(ty)));
                let rty = crate::types::binary_type(op, ty, value.ty).expect("checked");
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
            self.line(&format!("{}: {},", snake_case(&f.name), rust_class_type(f.ty)));
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
        self.gap();
        self.line(&format!("fn field_inits(this: &{}) {{", class_type(&class.types[index].name)));
        self.indent += 1;
        for f in inits {
            // The line writes a field, so a read in the value must end its `Ref` first.
            let outer = std::mem::replace(&mut self.guarded, true);
            let value = self.value(f.init.as_ref().expect("filtered"), f.ty).text;
            self.guarded = outer;
            self.line(&format!("this.borrow_mut().{} = {value};", snake_case(&f.name)));
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
                    Some(e) => self.value(e, f.ty).text,
                    None => default_value(f.ty).to_string(),
                };
                let cell = cell_name(&decl.name, &f.name);
                let ty = rust_class_type(f.ty);
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
        let decl = &class.types[m.class];
        let (delegate, rest) = match m.body.split_first() {
            Some((Stmt::Expr(Expr { kind: ExprKind::ThisCall { args, resolved }, .. }), rest)) => {
                (Some((args, resolved.expect("calls are resolved by the checker"))), rest)
            }
            _ => (None, &m.body[..]),
        };
        let initialize = delegate.is_none() && decl.fields.iter().any(|f| f.init.is_some());
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
                self.line(&format!("let obj = Rc::new(RefCell::new({name} {{ {} }}));", fields.join(", ")));
            }
        }
        if initialize || uses_this(rest) {
            self.line("let this = &obj;");
        }
        if initialize {
            self.line(&format!("{}::field_inits(this);", struct_name(&decl.name)));
        }
        self.stmts(rest, false);
        self.line("obj");
    }

    /// A blank line before an item, unless the output is empty or a line of an item is last.
    pub(super) fn gap(&mut self) {
        if self.out.ends_with("}\n") {
            self.out.push('\n');
        }
    }
}
