//! Code generation for static initialization. A `static final` compile-time constant is a Rust
//! `const`, read without a cell. Other static fields are thread-local cells. A class whose static
//! initialization must run eagerly (see `analysis::statics`) has an `init` function that runs its
//! `<clinit>` once, and every static method, constructor and access from another class calls it
//! first, as Java initializes a class on first use.
//!
//! A program that starts threads has one copy of each static for the process, read and written
//! with the same text: a primitive is a `java::Atomic` (`X.get()`, `X.set(v)`, which acquire and
//! release), anything else a `java::Locked` (`X.with_borrow(..)`, `X.with_borrow_mut(..)`), inside
//! a `LazyLock` when its initializer is not a constant; an eager class's flag is a
//! `java::ClassInit`, which makes another thread wait while one runs the initialization.

use super::*;

/// The `init` function of a class, which runs the class's static initialization once; from
/// another package, with that package's path (`crate::a::b::init_point`).
pub(super) fn init_name(class: &str) -> String {
    let (path, local) = modules::split(class);
    path + &format!("init_{}", snake_case(&local.replace('.', "")))
}

/// The flag that records that a class's static initialization has started.
fn flag(class: &str) -> String {
    format!("{}__INITIALIZED", struct_name(class)).to_uppercase()
}

/// Whether a static field of this type is a `Cell`, read and written whole, rather than a
/// `RefCell`: a primitive value, which is `Copy` and is never null. A `Cell` needs no borrow
/// flag, so each access is one thread-local read or write.
pub(super) fn is_cell(ty: Type) -> bool {
    matches!(
        ty,
        Type::Int
            | Type::Long
            | Type::Double
            | Type::Byte
            | Type::Short
            | Type::Char
            | Type::Float
            | Type::Boolean
    )
}

impl<'a> Gen<'a> {
    /// The constants as `const` items, then the thread-local cells of the other static fields. An
    /// eager class's cells start at Java's default and its `init` function sets them; any other
    /// static's cell computes its initializer on first access.
    pub(super) fn statics(&mut self) {
        let class = self.class;
        if self.module_types().any(|t| t.statics.iter().any(|f| f.constant)) {
            self.gap();
        }
        for (decl, f) in self.module_types().flat_map(|t| t.statics.iter().map(move |f| (t, f))) {
            if !f.constant {
                continue;
            }
            let (ty, value) = match (&f.ty, &f.init) {
                (Type::String, Some(Expr { kind: ExprKind::Str(s), .. })) => {
                    ("&str".to_string(), rust_string(s))
                }
                (_, Some(init)) => (self.ty_name(f.ty), self.value(init, f.ty).text),
                (_, None) => unreachable!("a constant has a value"),
            };
            let cell = cell_name(&decl.name, &f.name);
            self.line(&format!("{}const {cell}: {ty} = {value};", self.vis()));
        }
        let here: Vec<usize> = (0..class.types.len())
            .filter(|&t| self.module_types().any(|d| std::ptr::eq(d, &class.types[t])))
            .collect();
        let eager: Vec<usize> = here.iter().copied().filter(|t| self.facts.eager_init.contains(t)).collect();
        let cells: Vec<(usize, &Field)> = here
            .iter()
            .map(|&t| (t, &class.types[t]))
            .flat_map(|(t, decl)| decl.statics.iter().filter(|f| !f.constant).map(move |f| (t, f)))
            .collect();
        if eager.is_empty() && cells.is_empty() {
            return;
        }
        self.gap();
        if self.facts.threaded {
            return self.process_statics(&eager, cells);
        }
        self.line("thread_local! {");
        self.indent += 1;
        for &t in &eager {
            self.line(&format!(
                "{}static {}: Cell<bool> = const {{ Cell::new(false) }};",
                self.vis(),
                flag(&class.types[t].name)
            ));
        }
        for (t, f) in cells {
            let (cell, ty, init, constant) = self.static_cell(t, f);
            let kind = if is_cell(f.ty) { "Cell" } else { "RefCell" };
            let init = if constant {
                format!("const {{ {kind}::new({init}) }}")
            } else {
                format!("{kind}::new({init})")
            };
            self.line(&format!("{}static {cell}: {kind}<{ty}> = {init};", self.vis()));
        }
        self.indent -= 1;
        self.line("}");
    }

    /// The cell of static field `f` of class `t`: its name, its Rust type, its initial value,
    /// and whether that value is a constant, which Rust evaluates at compile time. An eager
    /// class's cells start at Java's default.
    fn static_cell(&mut self, t: usize, f: &'a Field) -> (String, String, String, bool) {
        let stored = self.static_optional(t, &f.name);
        let init = match &f.init {
            Some(e) if !self.facts.eager_init.contains(&t) => self.field_value(e, f.ty, stored),
            _ => self.placeholder(f.ty, stored),
        };
        let cell = cell_name(&self.class.types[t].name, &f.name);
        let ty = self.field_ty_name(f.ty, stored);
        let constant = init == "None"
            || init == "String::new()"
            || init.parse::<f64>().is_ok()
            || init.trim_start_matches('-').chars().next().is_some_and(|c| c.is_ascii_digit())
                && init.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
            || matches!(init.as_str(), "true" | "false");
        (cell, ty, init, constant)
    }

    /// The statics of a program with threads, one for the process (see the module's
    /// documentation).
    fn process_statics(&mut self, eager: &[usize], cells: Vec<(usize, &'a Field)>) {
        for &t in eager {
            let init = self.runtime("ClassInit");
            let flag = flag(&self.class.types[t].name);
            self.line(&format!("{}static {flag}: {init} = {init}::new();", self.vis()));
        }
        for (t, f) in cells {
            let (cell, ty, init, constant) = self.static_cell(t, f);
            let kind = if is_cell(f.ty) { self.runtime("Atomic") } else { self.runtime("Locked") };
            let vis = self.vis();
            if constant {
                self.line(&format!("{vis}static {cell}: {kind}<{ty}> = {kind}::new({init});"));
            } else {
                self.line(&format!(
                    "{vis}static {cell}: LazyLock<{kind}<{ty}>> = LazyLock::new(|| {kind}::new({init}));"
                ));
            }
        }
    }

    /// At the start of an `init` function: returns if initialization has started, else records
    /// that it has, so that a use during initialization, from the same thread, sees the class as
    /// initialized, as Java does. With threads, another thread's initialization is waited for,
    /// and `init_done` ends the function.
    pub(super) fn init_guard(&mut self, class: &str) {
        if self.facts.threaded {
            self.line(&format!("if !{}.start() {{", flag(class)));
            self.line("    return;");
            self.line("}");
            return;
        }
        self.line(&format!("if {}.get() {{", flag(class)));
        self.line("    return;");
        self.line("}");
        self.line(&format!("{}.set(true);", flag(class)));
    }

    /// At the end of an `init` function in a program with threads: the initialization is done,
    /// for the threads that wait for it.
    pub(super) fn init_done(&mut self, class: &str) {
        self.line(&format!("{}.finish();", flag(class)));
    }

    /// The call that initializes the class of a static field or method before it is used, or
    /// `None` when neither the class nor a superclass is eager, or the code is in that class or
    /// a subclass of it, which is already initialized by then.
    pub(super) fn init_call(&mut self, class: usize) -> Option<String> {
        let current = self.class.methods[self.method].class;
        let init = self.facts.init_class(self.class, class)?;
        let initialized = current == init || self.class.hierarchy.ancestors(current).contains(&init);
        (!initialized).then(|| format!("{};", self.init_run(init)))
    }

    /// The call that runs the initialization of the class `init` (its `init` function), without
    /// the `;`. A class whose `<clinit>` can throw returns a `Result`, which is passed on from here
    /// (see `passed_on`).
    pub(super) fn init_run(&mut self, init: usize) -> String {
        let call = format!("{}()", init_name(&self.class.types[init].name));
        if crate::lower::rewritten_clinit(&self.class.types[init]) {
            self.passed_on(Code::atom(call))
        } else {
            call
        }
    }

    /// Whether the cell of static `name` of class `class` holds an `Option`: the static may be
    /// null, or it starts empty (`Facts::placeholder_statics`).
    pub(super) fn static_optional(&self, class: usize, name: &str) -> bool {
        let key = (class, name.to_string());
        self.facts.nullable_statics.contains(&key) || self.facts.placeholder_statics.contains(&key)
    }

    /// The `const` read of a constant static field, or `None` for any other static field.
    pub(super) fn constant_read(&self, class: usize, name: &str, ty: Type) -> Option<Code> {
        let field = self.class.types[class].statics.iter().find(|f| f.name == name)?;
        if !field.constant {
            return None;
        }
        let cell = cell_name(&self.class.types[class].name, name);
        Some(Code::atom(if ty == Type::String { format!("String::from({cell})") } else { cell }))
    }
}
