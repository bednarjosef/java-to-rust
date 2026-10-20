//! Code generation for static initialization. A `static final` compile-time constant is a Rust
//! `const`, read without a cell. Other static fields are thread-local cells. A class whose static
//! initialization must run eagerly (see `analysis::statics`) has an `init` function that runs its
//! `<clinit>` once, and every static method, constructor and access from another class calls it
//! first, as Java initializes a class on first use.

use super::*;

/// The `init` function of a class, which runs the class's static initialization once.
pub(super) fn init_name(class: &str) -> String {
    format!("init_{}", snake_case(&struct_name(class)))
}

/// The thread-local flag that records that a class's static initialization has started.
fn flag(class: &str) -> String {
    format!("{}__INITIALIZED", struct_name(class)).to_uppercase()
}

impl<'a> Gen<'a> {
    /// The constants as `const` items, then the thread-local cells of the other static fields. An
    /// eager class's cells start at Java's default and its `init` function sets them; any other
    /// static's cell computes its initializer on first access.
    pub(super) fn statics(&mut self) {
        let class = self.class;
        if class.types.iter().any(|t| t.statics.iter().any(|f| f.constant)) {
            self.gap();
        }
        for (decl, f) in class.types.iter().flat_map(|t| t.statics.iter().map(move |f| (t, f))) {
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
            self.line(&format!("const {cell}: {ty} = {value};"));
        }
        let eager: Vec<usize> =
            (0..class.types.len()).filter(|t| self.facts.eager_init.contains(t)).collect();
        let cells: Vec<(usize, &Field)> = class
            .types
            .iter()
            .enumerate()
            .flat_map(|(t, decl)| decl.statics.iter().filter(|f| !f.constant).map(move |f| (t, f)))
            .collect();
        if eager.is_empty() && cells.is_empty() {
            return;
        }
        self.gap();
        self.line("thread_local! {");
        self.indent += 1;
        for &t in &eager {
            self.line(&format!("static {}: Cell<bool> = Cell::new(false);", flag(&class.types[t].name)));
        }
        for (t, f) in cells {
            let init = match &f.init {
                Some(e) if !self.facts.eager_init.contains(&t) => self.value(e, f.ty).text,
                _ => default_value(f.ty).to_string(),
            };
            let cell = cell_name(&class.types[t].name, &f.name);
            let ty = self.ty_name(f.ty);
            self.line(&format!("static {cell}: RefCell<{ty}> = RefCell::new({init});"));
        }
        self.indent -= 1;
        self.line("}");
    }

    /// At the start of an `init` function: returns if initialization has started, else records
    /// that it has, so that a use during initialization, from the same thread, sees the class as
    /// initialized, as Java does.
    pub(super) fn init_guard(&mut self, class: &str) {
        self.line(&format!("if {}.get() {{", flag(class)));
        self.line("    return;");
        self.line("}");
        self.line(&format!("{}.set(true);", flag(class)));
    }

    /// The call that initializes the class of a static field or method before it is used, or
    /// `None` when the class is not eager or the code is in that class, which is already
    /// initialized by then.
    pub(super) fn init_call(&self, class: usize) -> Option<String> {
        let current = self.class.methods[self.method].class;
        (self.facts.eager_init.contains(&class) && class != current)
            .then(|| format!("{}();", init_name(&self.class.types[class].name)))
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
