//! Code generation for static initialization. A `static final` compile-time constant is a Rust
//! `const`, read without a cell. In a program that starts no thread, every other static field is
//! a `java::Global` (or `java::LazyGlobal`, when its initializer is not a constant): a plain
//! `static` that holds a `Cell` or a `RefCell` and is written with the text of a thread-local
//! cell, which the compiler can keep in a register through a loop (see the runtime's
//! `statics.rs`); a `RefCell`'s value is read with `cloned()`, which tests its borrow flag
//! without writing it. A class whose static
//! initialization must run eagerly (see `analysis::statics`) has an `init` function that runs its
//! `<clinit>` once, and every static method, constructor and access from another class calls it
//! first, as Java initializes a class on first use. The `init` function is inlined and only tests
//! whether initialization has started; `init_..._slow`, out of line, runs it.
//!
//! In a program that starts threads, the statics that the started threads' code may touch
//! (`Facts::process_statics`, by class) have one copy for the process, read and written with the
//! same text: a primitive is a `java::Atomic` (`X.get()`, `X.set(v)`, which acquire and release),
//! anything else a `java::Locked` (`X.with_borrow(..)`, `X.with_borrow_mut(..)`), inside a
//! `LazyLock` when its initializer is not a constant; such an eager class's flag is a
//! `java::ClassInit`, which makes another thread wait while one runs the initialization. The other
//! classes' statics are thread-local cells, which only the main thread uses.
//!
//! An object, array or string static of the process that only its class's static initialization
//! assigns (`Facts::final_statics`) is a `java::Final` instead, with no `LazyLock` around it (it
//! builds a starting value that is not a constant itself, on its first read), which a read reaches
//! with no lock:
//! `X.get().clone()` as a value, and `(*X.get())`, a place, where the read is only borrowed (the
//! object of a call or a field access, an array that is indexed, a string that is lent). An eager
//! class's is read with `get_prepared` instead, with no test that it has a value: `main` prepares
//! it first (`prepared_static`); one of a class whose initialization nothing can observe, which
//! `main` runs first (`Facts::early_init`), with `get_settled`, a plain load. Such an
//! array that the program only indexes to read and measures (`Facts::frozen_statics`) is a
//! `java::Final<java::Frozen<T>>`, or `java::Frozen<T, N>` where every array it is given has the
//! constant length `N`: the new array is frozen as it is stored (`java::freeze`,
//! `java::freeze_fixed`), and its elements are read with no atomic load.

use super::*;

/// The `init` function of a class, which runs the class's static initialization once; from
/// another package, with that package's path (`crate::a::b::init_point`).
pub(super) fn init_name(class: &str) -> String {
    let (path, local) = modules::split(class);
    path + &format!("init_{}", snake_case(&modules::flat_name(class, local)))
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

/// The most statics in one `thread_local!`.
const THREAD_LOCALS_PER_MACRO: usize = 100;

impl<'a> Gen<'a> {
    /// The constants as `const` items, then the thread-local cells of the other static fields. An
    /// eager class's cells start at Java's default and its `init` function sets them; any other
    /// static's cell computes its initializer on first access.
    pub(super) fn statics(&mut self) {
        let class = self.class;
        if self.module_types().any(|t| t.statics.iter().any(|f| f.constant)) {
            self.gap();
        }
        for (t, f) in self
            .module_type_indexes()
            .into_iter()
            .flat_map(|t| class.types[t].statics.iter().map(move |f| (t, f)))
        {
            if !f.constant {
                continue;
            }
            let (ty, value) = match (&f.ty, &f.init) {
                (Type::String, Some(Expr { kind: ExprKind::Str(s), .. })) => {
                    ("&java::Wtf8".to_string(), format!("java::wtf8({})", rust_string(s)))
                }
                (_, Some(init)) => (self.ty_name(f.ty), self.value(init, f.ty).text),
                (_, None) => unreachable!("a constant has a value"),
            };
            let cell = self.cell_name(t, &f.name);
            self.line(&format!("{}const {cell}: {ty} = {value};", self.vis()));
        }
        let here: Vec<usize> = self.module_type_indexes();
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
        let process = |t: &usize| self.facts.process_statics(*t);
        let (process_eager, eager): (Vec<usize>, Vec<usize>) = eager.into_iter().partition(process);
        let (process_cells, cells): (Vec<_>, Vec<_>) = cells.into_iter().partition(|(t, _)| process(t));
        self.process_statics(&process_eager, process_cells);
        if !self.facts.threaded {
            self.globals(&eager, cells);
            return;
        }
        let mut items = Vec::new();
        for &t in &eager {
            items.push(format!(
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
            items.push(format!("{}static {cell}: {kind}<{ty}> = {init};", self.vis()));
        }
        // `thread_local!` expands one static per level of recursion, which Rust limits to 128,
        // so a module with more statics (`Character.UnicodeBlock` has hundreds) has several.
        for chunk in items.chunks(THREAD_LOCALS_PER_MACRO) {
            self.line("thread_local! {");
            self.indent += 1;
            for item in chunk {
                self.line(item);
            }
            self.indent -= 1;
            self.line("}");
        }
    }

    /// The statics of a program that starts no thread, each a `java::Global` (see the module's
    /// documentation): the flags of the eager classes `eager`, then the static fields `cells`.
    fn globals(&mut self, eager: &[usize], cells: Vec<(usize, &'a Field)>) {
        let (global, lazy) = (self.runtime("Global"), self.runtime("LazyGlobal"));
        let vis = self.vis();
        for &t in eager {
            let flag = flag(&self.class.types[t].name);
            self.line(&format!(
                "{vis}static {flag}: {global}<Cell<bool>> = {global}::new(Cell::new(false));"
            ));
        }
        for (t, f) in cells {
            let (cell, ty, init, constant) = self.static_cell(t, f);
            let kind = if is_cell(f.ty) { "Cell" } else { "RefCell" };
            if constant {
                self.line(&format!(
                    "{vis}static {cell}: {global}<{kind}<{ty}>> = {global}::new({kind}::new({init}));"
                ));
            } else {
                self.line(&format!(
                    "{vis}static {cell}: {lazy}<{kind}<{ty}>> = {lazy}::new(|| {kind}::new({init}));"
                ));
            }
        }
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
        let cell = self.cell_name(t, &f.name);
        let ty = self.field_ty_name(f.ty, stored);
        let constant = init == "None"
            || init == "java::JString::new()"
            || init.parse::<f64>().is_ok()
            || init.trim_start_matches('-').chars().next().is_some_and(|c| c.is_ascii_digit())
                && init.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
            || matches!(init.as_str(), "true" | "false");
        (cell, ty, init, constant)
    }

    /// The statics that are the process's, one copy each (see the module's
    /// documentation).
    fn process_statics(&mut self, eager: &[usize], cells: Vec<(usize, &'a Field)>) {
        for &t in eager {
            let init = self.runtime("ClassInit");
            let flag = flag(&self.class.types[t].name);
            self.line(&format!("{}static {flag}: {init} = {init}::new();", self.vis()));
        }
        for (t, f) in cells {
            let (cell, ty, init, constant) = self.static_cell(t, f);
            let fixed = !is_cell(f.ty) && self.final_static(t, &f.name);
            // An eager class's frozen array of a constant length starts with that many elements.
            let filled = match self.facts.frozen_lengths.get(&(t, f.name.clone())) {
                Some(n) if self.facts.eager_init.contains(&t) && self.frozen_static(t, &f.name) => self
                    .frozen_filler(t, &f.name)
                    .map(|zero| format!("{}({n}, {zero})", self.runtime("shared_filled"))),
                _ => None,
            };
            let init = filled.unwrap_or(init);
            let (ty, init) = match self.frozen_static(t, &f.name) {
                true => (self.frozen_ty(t, &f.name, f.ty), format!("{}({init})", self.freezer(t, &f.name))),
                false => (ty, init),
            };
            let kind = if is_cell(f.ty) {
                self.runtime("Atomic")
            } else if fixed {
                self.runtime("Final")
            } else {
                self.runtime("Locked")
            };
            let vis = self.vis();
            if fixed && !constant {
                self.line(&format!("{vis}static {cell}: {kind}<{ty}> = {kind}::lazy(|| {init});"));
            } else if constant {
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
    /// initialized, as Java does. When its statics are the process's, another thread's
    /// initialization is waited for, and `init_done` ends the function. A rewritten `<clinit>`
    /// keeps its own state, which is its guard (`lower/clinit_errors.rs`): when its statics are
    /// the process's, another thread's initialization is waited for all the same, and the run
    /// this thread starts is done at whichever return ends it, failed or not, and ready (the
    /// `ClassInit`'s state that the fast path tests alone) when the kept state says it succeeded.
    pub(super) fn init_guard(&mut self, t: usize) {
        let class = &self.class.types[t].name;
        if crate::lower::rewritten_clinit(&self.class.types[t]) {
            if self.facts.process_statics(t) {
                let kept = self.cell_name(t, crate::lower::STATE);
                self.line(&format!("let _run = {}.run(&{kept});", flag(class)));
            }
            return;
        }
        if self.facts.process_statics(t) {
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

    /// The `init` function of class `t`, named `name`, returning `ret`: inlined where it is called,
    /// it returns at once when the class's initialization has started (for the process's statics,
    /// when it is done), and otherwise calls `{name}_slow`, which runs the `<clinit>` and is kept
    /// out of line. A class is used far more often than it is initialized, and the test is one
    /// load of one flag: a loop that uses several classes tests each on every pass.
    pub(super) fn init_fast_path(&mut self, t: usize, name: &str, ret: &str) {
        let decl = &self.class.types[t];
        // A rewritten `<clinit>` records that it started before it runs; another thread waits
        // until it is done, and the `ClassInit` is ready once a run is done and succeeded, which
        // stands for its kept state being 1 as well.
        let started = if crate::lower::rewritten_clinit(decl) && self.facts.process_statics(t) {
            format!("{}.ready()", flag(&decl.name))
        } else if crate::lower::rewritten_clinit(decl) {
            format!("{}.get() == 1", self.cell_name(t, crate::lower::STATE))
        } else if self.facts.process_statics(t) {
            format!("{}.done()", flag(&decl.name))
        } else {
            format!("{}.get()", flag(&decl.name))
        };
        let done = if ret.is_empty() { "return;" } else { "return Ok(());" };
        let local = name.rsplit("::").next().unwrap_or(name);
        let vis = self.vis();
        self.line("#[inline]");
        self.line(&format!("{vis}fn {local}(){ret} {{"));
        self.line(&format!("    if {started} {{"));
        self.line(&format!("        {done}"));
        self.line("    }");
        self.line(&format!("    {local}_slow()"));
        self.line("}");
        self.gap();
        self.line("#[cold]");
        self.line("#[inline(never)]");
    }

    /// At the end of the `init` function of a class whose statics are the process's: the
    /// initialization is done, for the threads that wait for it. (A rewritten `<clinit>`'s run is
    /// done as it returns, see `init_guard`.)
    pub(super) fn init_done(&mut self, t: usize) {
        if crate::lower::rewritten_clinit(&self.class.types[t]) {
            return;
        }
        let class = &self.class.types[t].name;
        self.line(&format!("{}.finish();", flag(class)));
    }

    /// The call that initializes the class of a static field or method before it is used, or
    /// `None` when neither the class nor a superclass is eager, or the code is in that class or
    /// a subclass of it, which is already initialized by then.
    pub(super) fn init_call(&mut self, class: usize) -> Option<String> {
        let init = self.uninitialized(class)?;
        Some(format!("{};", self.init_run(init)))
    }

    /// The class whose initialization a use of class `class` runs here (`init_call`), if any.
    pub(super) fn uninitialized(&self, class: usize) -> Option<usize> {
        let current = self.class.methods[self.method].class;
        let init = self.facts.init_class(self.class, class)?;
        let initialized = current == init || self.class.hierarchy.ancestors(current).contains(&init);
        (!initialized).then_some(init)
    }

    /// The call that runs the initialization of the class `init` (its `init` function), without
    /// the `;`. A class whose `<clinit>` can throw returns a `Result`, which is passed on from here
    /// (see `passed_on`).
    pub(super) fn init_run(&mut self, init: usize) -> String {
        let call = format!("{}()", init_name(&self.class.types[init].name));
        if crate::lower::rewritten_clinit(&self.class.types[init]) {
            self.passed_on_thrown(Code::atom(call))
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

    /// Whether static `name` of class `class` is a `java::Final`: an object, array or string of
    /// the process's statics that only its class's static initialization assigns.
    pub(super) fn final_static(&self, class: usize, name: &str) -> bool {
        self.facts.process_statics(class)
            && self.facts.final_statics.contains(&(class, name.to_string()))
            && self.class.types[class].statics.iter().any(|f| f.name == name && !is_cell(f.ty))
    }

    /// Whether static `name` of class `class` is a `java::Final` that `main` prepares before any
    /// other code runs (`prepare_finals`), which a read reaches with no test (`get_prepared`):
    /// one of an eager class, whose starting value is Java's default, cheap to make. Any other
    /// starts with its initializer's value, which its first read makes (`get`), and one of a
    /// class that `main` initializes first (`Facts::early_init`) is settled, which `get` reads
    /// first.
    /// A frozen array of a constant length is the exception: its starting value needs that many
    /// elements, and an object that may not be null has no default to fill it with.
    pub(super) fn prepared_static(&self, class: usize, name: &str) -> bool {
        self.final_static(class, name)
            && self.facts.eager_init.contains(&class)
            && !self.facts.early_init.contains(&class)
            && (!self.frozen_static(class, name)
                || !self.facts.frozen_lengths.contains_key(&(class, name.to_string()))
                || self.frozen_filler(class, name).is_some())
    }

    /// The default element that fills the starting value of a frozen static array of a constant
    /// length: Java's default of a primitive or string, or `None` where the elements may be null.
    fn frozen_filler(&self, class: usize, name: &str) -> Option<&'static str> {
        let f = self.class.types[class].statics.iter().find(|f| f.name == name)?;
        let elem = f.ty.element();
        if self.nullable_element(elem) {
            return Some("None");
        }
        exceptions::default_value(elem)
    }

    /// The method of `java::Final` that reads static `name` of class `class`: `get_settled` for
    /// one of a class that `main` initializes first (`settled_static`), but in that class's own
    /// initialization, which runs before `main` checks it is settled.
    pub(super) fn final_get(&self, class: usize, name: &str) -> &'static str {
        let m = &self.class.methods[self.method];
        if self.settled_static(class, name) && !(m.class == class && m.name == "<clinit>") {
            "get_settled"
        } else if self.prepared_static(class, name) {
            "get_prepared"
        } else {
            "get"
        }
    }

    /// Whether static `name` of class `class` is a `java::Final` of a class whose static
    /// initialization `main` runs before anything else (`Facts::early_init`): its assignment
    /// settles it before any thread starts.
    fn settled_static(&self, class: usize, name: &str) -> bool {
        self.final_static(class, name) && self.facts.early_init.contains(&class)
    }

    /// At the start of `main`: prepares every `java::Final` that is read with no test
    /// (`prepared_static`), before any thread starts, then runs the static initialization of
    /// the classes whose initialization has no observable effect (`Facts::early_init`), which
    /// settles their statics before any thread starts too.
    pub(super) fn prepare_finals(&mut self) {
        let class = self.class;
        for (t, decl) in class.types.iter().enumerate() {
            for f in &decl.statics {
                if !f.constant && self.prepared_static(t, &f.name) {
                    let cell = self.cell_name(t, &f.name);
                    self.line(&format!("{cell}.prepare();"));
                }
            }
        }
        let mut early: Vec<usize> = self.facts.early_init.iter().copied().collect();
        early.sort_unstable();
        for &t in &early {
            let call = self.init_run(t);
            self.line(&format!("{call};"));
        }
        for t in early {
            for f in &class.types[t].statics {
                if !f.constant && self.settled_static(t, &f.name) {
                    let cell = self.cell_name(t, &f.name);
                    self.line(&format!("{cell}.check_settled();"));
                }
            }
        }
    }

    /// Whether static `name` of class `class` is a `java::Final` that holds a `java::Frozen`: a
    /// frozen array (`Facts::frozen_statics`) that is never null, whose reads see it made.
    pub(super) fn frozen_static(&self, class: usize, name: &str) -> bool {
        self.final_static(class, name)
            && self.facts.frozen_statics.contains(&(class, name.to_string()))
            && !self.static_optional(class, name)
    }

    /// The type of frozen static or field `name` of class `class`, an array of type `ty`:
    /// `java::Frozen<T>`, or `java::Frozen<T, N>` where its arrays have one constant length.
    pub(super) fn frozen_ty(&mut self, class: usize, name: &str, ty: Type) -> String {
        let elem = self.element_ty_name(ty.element());
        match self.facts.frozen_lengths.get(&(class, name.to_string())) {
            Some(n) => format!("{}<{elem}, {n}>", self.runtime("Frozen")),
            None => format!("{}<{elem}>", self.runtime("Frozen")),
        }
    }

    /// The runtime function that makes a new array into the `java::Frozen` of frozen static or
    /// field `name` of class `class`: `freeze`, or `freeze_fixed` for one of a constant length.
    pub(super) fn freezer(&mut self, class: usize, name: &str) -> String {
        match self.facts.frozen_lengths.contains_key(&(class, name.to_string())) {
            true => self.runtime("freeze_fixed"),
            false => self.runtime("freeze"),
        }
    }

    /// `code`, the new array `e`, frozen where a frozen field is given it (`Facts::frozen_news`).
    /// The field's type gives the length, where it has one.
    pub(super) fn frozen_new(&mut self, e: &Expr, code: Code) -> Code {
        let Some(&fixed) = self.facts.frozen_news.get(&e.id) else { return code };
        let freeze = self.runtime(if fixed { "freeze_fixed" } else { "freeze" });
        Code::atom(format!("{freeze}({})", code.text))
    }

    /// Whether `e` reads a frozen field (`Facts::frozen_fields`).
    pub(super) fn frozen_field_read(&self, e: &Expr) -> bool {
        let ExprKind::Field { object, name } = &e.kind else { return false };
        self.class
            .field_owner(object.ty, name)
            .is_some_and(|t| self.facts.frozen_fields.contains(&(t, name.clone())))
    }

    /// A read of a `java::Final` static (`e`) as the place that holds the value that the read
    /// would clone, after its class's initialization, for code that only borrows it:
    /// `(*X.get())` (or `get_prepared`, see `final_get`), or for a static that starts empty
    /// (`Facts::placeholder_statics`), read where it is set, `(*java::non_null(X.get().as_ref()))`.
    /// `None` for any other expression.
    pub(super) fn final_place(&mut self, e: &Expr) -> Option<String> {
        let ExprKind::Static { class, name } = &e.kind else { return None };
        if !self.final_static(*class, name) {
            return None;
        }
        let cell = self.cell_name(*class, name);
        let placeholder = self.facts.placeholder_statics.contains(&(*class, name.clone()))
            && !self.facts.early_static_reads.contains(&e.id);
        let get = self.final_get(*class, name);
        let value = match placeholder {
            true => format!("{}({cell}.{get}().as_ref())", self.runtime("non_null")),
            false => format!("{cell}.{get}()"),
        };
        Some(match self.init_call(*class) {
            Some(init) => format!("(*{{ {init} {value} }})"),
            None => format!("(*{value})"),
        })
    }

    /// A string static of a program without threads, borrowed where it is (`(*cell.borrow())`,
    /// its `Option` where it may be null) instead of copied, where the statement being written
    /// cannot change it (`static_steady`): reading a long string in a loop copied all of it at
    /// each read. The borrow is a `RefCell`'s, which ends with the statement.
    pub(super) fn lent_static(&mut self, e: &Expr) -> Option<String> {
        let ExprKind::Static { class, name } = &e.kind else { return None };
        if e.ty != Type::String
            || self.facts.threaded
            || super::shared_strings()
            || self.final_static(*class, name)
            || self.class.types[*class].statics.iter().any(|f| f.name == *name && f.constant)
            || self.facts.early_static_reads.contains(&e.id)
            || self.facts.placeholder_statics.contains(&(*class, name.clone()))
            || !self.static_steady(*class)
        {
            return None;
        }
        let cell = self.cell_name(*class, name);
        let borrow = format!("std::cell::RefCell::borrow(&{cell})");
        Some(match self.init_call(*class) {
            Some(init) => format!("(*{{ {init} {borrow} }})"),
            None => format!("(*{borrow})"),
        })
    }

    /// The `const` read of a constant static field, or `None` for any other static field.
    pub(super) fn constant_read(&mut self, class: usize, name: &str, ty: Type) -> Option<Code> {
        let field = self.class.types[class].statics.iter().find(|f| f.name == name)?;
        if !field.constant {
            return None;
        }
        let cell = self.cell_name(class, name);
        Some(Code::atom(if ty == Type::String { self.string_literal(&cell) } else { cell }))
    }
}
