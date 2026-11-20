//! Code generation for static initialization. A `static final` compile-time constant is a Rust
//! `const`, read without a cell. In a program that starts no thread, every other static field is
//! a `java::Global` (or `java::LazyGlobal`, when its initializer is not a constant): a plain
//! `static` that holds a `Cell` or a `RefCell` and is written with the text of a thread-local
//! cell, which the compiler can keep in a register through a loop (see the runtime's
//! `statics.rs`); a `RefCell`'s value is read with `cloned()`, which tests its borrow flag
//! without writing it. A class whose static
//! initialization must run eagerly (see `analysis::statics`) has an `init` function that runs its
//! `<clinit>` once, and every static method, constructor and access from another class calls it
//! first, as Java initializes a class on first use, unless the class is known to be initialized
//! there (`uninitialized`). The `init` function is inlined and only tests whether initialization
//! has started; `init_..._slow`, out of line, runs it. A static method or constructor that starts
//! with the call is two functions (`split_entry`), so that a caller that knows the class is
//! initialized calls the one without it; where callers of only one kind exist, the written
//! program has one function again (`join_unused_entry_splits`).
//!
//! In a program that starts threads, the statics that the started threads' code may touch
//! (`Facts::process_statics`, by class) have one copy for the process: a primitive is a
//! `java::Atomic` (with the text of a thread-local `Cell`) and anything else a
//! `java::SharedField`, inside a `LazyLock` when its initializer is not a constant, both read and
//! written by value with no lock (`X.get()`, `X.set(v)`, which acquire and release;
//! `get_volatile` and `set_volatile`, sequentially consistent, for a `volatile` static); such an
//! eager class's flag is a `java::ClassInit`, which makes another thread wait while one runs the
//! initialization. The other classes' statics are thread-local cells, which only the main thread
//! uses.
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

/// What the name of a static method's or constructor's function that has no initialization
/// check ends with, after the name of the one that has (`Gen::split_entry`). A Java name in snake
/// case has no `__` but where it had `$$`.
pub(super) const INITIALIZED: &str = "__initialized";

/// The line that `Gen::entry_check_fn` writes among the attributes of the function of a split
/// method that runs the check, for `join_unused_entry_splits` to find it, which takes it away.
pub(super) const ENTRY_CHECK_MARKER: &str = "#[cfg_attr(any(), entry_check)]";

/// Undoes the split of a static method or constructor (`Gen::split_entry`) where one of its two
/// functions is not called. Where no code but the function with the check calls the one with the
/// code, the two are joined back into one function of the method's name, which runs the check and
/// then the code; where no code calls the function with the check (a constructor that only
/// subclasses' `super(..)` calls), that function goes. A split is only worth its second function
/// where callers of both kinds exist; each function is one more for the compiler to hold, and the
/// Minecraft server's header crate, where most splits had callers of one kind, took a fifth more
/// functions and more memory than rustc had. A function is found uncalled when its name is in the
/// program once besides the call in the function with the check (`count_names`): a name shared
/// with functions of other modules counts their uses too, which keeps the split.
pub(super) fn join_unused_entry_splits(tree: &mut ModuleTree) {
    let uses: HashMap<String, usize> = {
        let mut names: HashMap<&str, usize> = HashMap::new();
        for item in tree.modules.iter().flat_map(|m| &m.items).filter(|i| i.text.contains(ENTRY_CHECK_MARKER))
        {
            let lines: Vec<&str> = item.text.lines().collect();
            for at in 0..lines.len() {
                if let Some(split) = EntrySplit::at(&lines, at) {
                    for name in [split.inner, split.name] {
                        let next = names.len();
                        names.entry(name).or_insert(next);
                    }
                }
            }
        }
        if names.is_empty() {
            return;
        }
        // Every text of the program: a `use` line may name a function too.
        let texts: Vec<&str> = [tree.header.as_str(), tree.footer.as_str()]
            .into_iter()
            .chain(tree.modules.iter().flat_map(|m| m.uses.iter().map(String::as_str)))
            .chain(tree.modules.iter().flat_map(|m| &m.items).map(|i| i.text.as_str()))
            .collect();
        let counts = count_names(&texts, &names);
        names.into_iter().map(|(name, at)| (name.to_string(), counts[at])).collect()
    };
    let (mut kept, mut joined_splits, mut unchecked) = (0, 0, 0);
    for module in &mut tree.modules {
        for item in &mut module.items {
            if !item.text.contains(ENTRY_CHECK_MARKER) {
                continue;
            }
            let lines: Vec<&str> = item.text.lines().collect();
            let mut joined = String::with_capacity(item.text.len());
            let mut at = 0;
            while at < lines.len() {
                // The split starts on the line before its marker.
                let split = EntrySplit::at(&lines, at + 1);
                let is_split = split.is_some();
                if let Some(split) = &split
                    && uses.get(split.name) == Some(&1)
                {
                    // The function with the check goes; the one with the code stays as it is.
                    at = split.code_at;
                    unchecked += 1;
                    continue;
                }
                if let Some(split) = split
                    && uses.get(split.inner) == Some(&2)
                {
                    // The function with the check goes; the one with the code takes its name and
                    // runs the check first.
                    for line in &lines[split.code_at..split.code_fn] {
                        joined.push_str(line);
                        joined.push('\n');
                    }
                    let signature = lines[split.code_fn].replacen(
                        &format!("fn {}(", split.inner),
                        &format!("fn {}(", split.name),
                        1,
                    );
                    joined.push_str(&signature);
                    joined.push('\n');
                    joined.push_str(split.check);
                    joined.push('\n');
                    at = split.code_fn + 1;
                    joined_splits += 1;
                    continue;
                }
                kept += usize::from(is_split);
                // A split that stays loses its marker, which has done its work.
                if lines[at].trim() != ENTRY_CHECK_MARKER {
                    joined.push_str(lines[at]);
                    joined.push('\n');
                }
                at += 1;
            }
            if !item.text.ends_with('\n') {
                joined.pop();
            }
            item.text = joined;
        }
    }
    if crate::timings::enabled() {
        eprintln!(
            "timing: codegen: entry splits: {joined_splits} joined, {unchecked} called only without the check, {kept} kept"
        );
    }
}

/// A split method's two functions as `Gen::entry_check_fn` and `Gen::write_method` write them, by
/// lines: `#[inline]`, the marker, `#[allow(dead_code)]`, the signature, the check, the call of
/// the function with the code and the closing brace; then, after at most one attribute, the
/// signature of the function with the code.
struct EntrySplit<'a> {
    /// The name of the function with the check, the method's.
    name: &'a str,
    /// The name of the function with the code.
    inner: &'a str,
    /// The line of the check, as indented in the function with the check, as in the other.
    check: &'a str,
    /// The first line after the function with the check.
    code_at: usize,
    /// The line of the signature of the function with the code.
    code_fn: usize,
}

impl<'a> EntrySplit<'a> {
    /// The split whose marker is line `at` of `lines`.
    fn at(lines: &[&'a str], at: usize) -> Option<Self> {
        if lines.get(at).is_none_or(|l| l.trim() != ENTRY_CHECK_MARKER)
            || at == 0
            || lines[at - 1].trim() != "#[inline]"
        {
            return None;
        }
        let [_, signature, check, call, close, ..] = lines.get(at + 1..)? else { return None };
        let name = fn_name(signature)?;
        let call = call.trim();
        let inner = call.strip_prefix("Self::").unwrap_or(call);
        let inner = &inner[..inner.find('(')?];
        if close.trim() != "}"
            || !(inner.ends_with(INITIALIZED) || inner.contains(&format!("{INITIALIZED}_")))
        {
            return None;
        }
        let code_at = at + 6;
        let code_fn =
            (code_at..(code_at + 2).min(lines.len())).find(|&l| fn_name(lines[l]) == Some(inner))?;
        Some(EntrySplit { name, inner, check, code_at, code_fn })
    }
}

/// The name of the function whose signature starts line `line`.
fn fn_name(line: &str) -> Option<&str> {
    let after = &line[line.find("fn ")? + 3..];
    Some(&after[..after.find('(')?])
}

/// How often each of `names` (by its place in the counts) is in `texts` as a whole word, counted
/// on several threads: the Minecraft server's program is hundreds of megabytes of text. A text is
/// cut into pieces at line breaks, which no word spans, so that the crate root's few large items
/// are shared among the threads.
fn count_names(texts: &[&str], names: &HashMap<&str, usize>) -> Vec<usize> {
    const PIECE: usize = 1 << 22;
    let mut pieces = Vec::new();
    for text in texts {
        let mut rest = *text;
        while rest.len() > PIECE {
            // A line break is one byte, which no character's encoding has inside it.
            let cut = rest.as_bytes()[PIECE..]
                .iter()
                .position(|&b| b == b'\n')
                .map_or(rest.len(), |at| PIECE + at + 1);
            let (piece, after) = rest.split_at(cut);
            pieces.push(piece);
            rest = after;
        }
        pieces.push(rest);
    }
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).clamp(1, 8);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let count = || {
        let mut counts = vec![0; names.len()];
        let word = |b: &u8| b.is_ascii_alphanumeric() || *b == b'_';
        while let Some(text) = pieces.get(next.fetch_add(1, std::sync::atomic::Ordering::Relaxed)) {
            let bytes = text.as_bytes();
            let mut at = 0;
            while at < bytes.len() {
                if !word(&bytes[at]) {
                    at += 1;
                    continue;
                }
                let end = bytes[at..].iter().position(|b| !word(b)).map_or(bytes.len(), |p| at + p);
                if let Some(&n) = names.get(&text[at..end]) {
                    counts[n] += 1;
                }
                at = end;
            }
        }
        counts
    };
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads).map(|_| scope.spawn(count)).collect();
        let mut total = vec![0; names.len()];
        for handle in handles {
            let counts = handle.join().expect("counting names does not panic");
            total.iter_mut().zip(counts).for_each(|(t, c)| *t += c);
        }
        total
    })
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
                self.runtime("SharedField")
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
    /// a subclass of it, which is already initialized by then, or `main` initializes the class
    /// first (`Facts::initialized_first`).
    pub(super) fn init_call(&mut self, class: usize) -> Option<String> {
        let init = self.uninitialized(class)?;
        Some(format!("{};", self.init_run(init)))
    }

    /// The class whose initialization a use of class `class` runs here (`init_call`), if any:
    /// none in that class or a subclass of it, nor after a statement of the same method that
    /// surely ran it or the initialization of a subclass of it (`known_init`, `implies`), nor
    /// after an operand of the same expression that surely ran it before this code runs
    /// (`after_operand`), nor for a class that `main` initializes first
    /// (`Facts::initialized_first`).
    pub(super) fn uninitialized(&self, class: usize) -> Option<usize> {
        let current = self.class.methods[self.method].class;
        let init = self.facts.init_class(self.class, class)?;
        let initialized = current == init
            || self.class.hierarchy.ancestors(current).contains(&init)
            || self.facts.init_class(self.class, current).is_some_and(|c| self.implies(c, init))
            || self.known_init.iter().any(|&(m, known)| m == self.method && self.implies(known, init))
            || self.facts.initialized_first(init);
        (!initialized).then_some(init)
    }

    /// Whether a passed check of eager class `known` means that eager class `init` is
    /// initialized too: it is that class, or one that the `<clinit>` of `known` initializes
    /// before its own code (its superclass's, and the superinterfaces with code that are eager,
    /// see `write_method`), or one that those imply in turn. Once `known` has started, those
    /// initializations have run or are running on this thread, where a check of theirs would
    /// pass as well.
    fn implies(&self, known: usize, init: usize) -> bool {
        if known == init {
            return true;
        }
        let superclass =
            self.class.hierarchy.superclass(known).and_then(|s| self.facts.init_class(self.class, s));
        superclass.is_some_and(|s| self.implies(s, init))
            || (self.facts.eager_init.contains(&init)
                && super::default_superinterfaces(self.class, known).contains(&init))
    }

    /// Whether static method or constructor `index` is written as two functions (`write_method`):
    /// the one of its name, which runs its class's initialization check and calls the other,
    /// named with `INITIALIZED` after it, which holds the code. A call where the class is known
    /// to be initialized (`uninitialized`) calls the second (`entry_suffix`): a `super(..)` from a
    /// subclass's constructor, a call from the class's own code, or one after a statement that
    /// surely initialized it. Those are the methods that start with that check (`entry_init`),
    /// but `main`, an enum's builder of constants and a generic method, whose type arguments a
    /// call of the second would have to name.
    pub(super) fn split_entry(&self, index: usize) -> bool {
        let m = &self.class.methods[index];
        self.facts.entry_init.contains(&index)
            && (m.is_static || m.is_ctor)
            && !m.is_main
            && m.type_params.is_empty()
            && !self.class.is_initializer(index)
            && self.facts.init_class(self.class, m.class).is_some()
    }

    /// What a call of static method or constructor `index` with arguments `args` written here
    /// adds to its function's name: `INITIALIZED`, calling the code without the check, where the
    /// method is split (`split_entry`) and its class is known to be initialized here, or by an
    /// argument, which runs before the call; else nothing.
    pub(super) fn entry_suffix(&self, index: usize, args: &[Expr]) -> &'static str {
        let class = self.class.methods[index].class;
        let known = || match self.uninitialized(class) {
            None => true,
            Some(init) => {
                let mut found = Vec::new();
                args.iter().for_each(|a| self.surely_initializes(a, &mut found));
                found.into_iter().any(|known| self.implies(known, init))
            }
        };
        if self.split_entry(index) && known() { INITIALIZED } else { "" }
    }

    /// Marks the classes that operand `done` surely initialized, once it completed normally, as
    /// known to be initialized (`known_init`), for the operands written after it that run only
    /// after it (`surely_initializes`): the later arguments of a call (`call_args`) and the right
    /// operand of a binary operator. Rust runs a call's arguments and an operator's operands in
    /// the order they are written, which is Java's. The caller forgets them (`truncate`) once it
    /// has written those operands, as the code around it may run before `done`.
    pub(super) fn after_operand(&mut self, done: &Expr) {
        let mut found = Vec::new();
        self.surely_initializes(done, &mut found);
        let method = self.method;
        self.known_init.extend(found.into_iter().map(|c| (method, c)));
    }

    /// `write(self)`, which writes code as lines ahead of the statement being written (a
    /// native's arguments computed into temporaries first, `hoisted_ints`): that code runs before
    /// the operands written ahead of it as well, so it is written knowing no class initialized.
    pub(super) fn ahead_of_statement<T>(&mut self, write: impl FnOnce(&mut Self) -> T) -> T {
        let known = std::mem::take(&mut self.known_init);
        let written = write(self);
        self.known_init = known;
        written
    }

    /// The classes whose initialization statement `stmt` has run once it completes normally,
    /// through a check written in this method (`init_call`): one for a read or write of a static
    /// that is not a constant, an enum constant, or `Class.forName`, where the statement surely
    /// evaluates it; or through the check a called function starts with: a call or `new` of a
    /// static method or constructor that has one (`entry_call`). Once the check passed, the class
    /// is initialized, or this thread is running its initialization, and a second check would
    /// pass too. Anything inside a lambda, past `&&`, `||` or `?`, or in a switch's cases, which
    /// may not run, is left out.
    pub(super) fn initialized_by(&self, stmt: &Stmt) -> Vec<usize> {
        let mut found = Vec::new();
        match stmt {
            Stmt::Local { init: Some(e), .. } | Stmt::Expr(e) | Stmt::If { cond: e, .. } => {
                self.surely_initializes(e, &mut found)
            }
            _ => {}
        }
        found
    }

    fn surely_initializes(&self, e: &Expr, found: &mut Vec<usize>) {
        let mut class_of = |class: usize| found.extend(self.facts.init_class(self.class, class));
        match &e.kind {
            ExprKind::Static { class, name } => {
                let constant = self.class.types[*class].statics.iter().any(|f| f.name == *name && f.constant);
                if !constant {
                    class_of(*class);
                }
            }
            ExprKind::EnumConstant { class, .. } => class_of(*class),
            ExprKind::Builtin(Builtin::InitializeClass { class }, _) => class_of(*class),
            ExprKind::Assign { target, value, .. } => {
                if let Place::Static { class, .. } = target {
                    class_of(*class);
                }
                self.place_surely_initializes(target, found);
                self.surely_initializes(value, found);
            }
            ExprKind::IncDec { target, .. } => {
                if let Place::Static { class, .. } = target {
                    class_of(*class);
                }
                self.place_surely_initializes(target, found);
            }
            ExprKind::Binary(BinOp::And | BinOp::Or, left, _) => self.surely_initializes(left, found),
            ExprKind::Binary(_, left, right) | ExprKind::Index(left, right) => {
                self.surely_initializes(left, found);
                self.surely_initializes(right, found);
            }
            ExprKind::Unary(_, value)
            | ExprKind::Cast(_, value)
            | ExprKind::InstanceOf { value, .. }
            | ExprKind::Length(value)
            | ExprKind::Conditional(value, ..)
            | ExprKind::Field { object: value, .. }
            | ExprKind::Switch { selector: value, .. } => self.surely_initializes(value, found),
            ExprKind::NewArray(values)
            | ExprKind::ArrayLit(values)
            | ExprKind::Call { args: values, .. }
            | ExprKind::New { args: values, .. }
            | ExprKind::ThisCall { args: values, .. }
            | ExprKind::SuperInit { args: values, .. } => {
                values.iter().for_each(|v| self.surely_initializes(v, found))
            }
            ExprKind::MethodCall { receiver, args, .. } => {
                self.surely_initializes(receiver, found);
                args.iter().for_each(|v| self.surely_initializes(v, found));
            }
            _ => {}
        }
        // A call of a static method or constructor that starts with its class's check, written as
        // a call of its function (`entry_calls`), has run the check once it returns.
        if let ExprKind::Call { resolved: Some(m), .. }
        | ExprKind::MethodCall { resolved: Some(m), .. }
        | ExprKind::New { ctor: Some(m), .. } = &e.kind
            && self.entry_calls.contains(&e.id)
        {
            found.extend(self.facts.init_class(self.class, self.class.methods[*m].class));
        }
    }

    /// Records that call or `new` `e` of method `index` is written as a call of its function, so
    /// that a statement that surely runs it initializes the class of a method that starts with
    /// its class's check (`surely_initializes`); a `new` of a lambda that becomes a Rust closure,
    /// say, calls no constructor.
    pub(super) fn entry_call(&mut self, e: ExprId, index: usize) {
        let m = &self.class.methods[index];
        if self.facts.entry_init.contains(&index) && (m.is_static || m.is_ctor) {
            self.entry_calls.insert(e);
        }
    }

    fn place_surely_initializes(&self, place: &Place, found: &mut Vec<usize>) {
        match place {
            Place::Index(array, index) => {
                self.surely_initializes(array, found);
                self.surely_initializes(index, found);
            }
            Place::Field { object, .. } => self.surely_initializes(object, found),
            Place::Var(_) | Place::Static { .. } => {}
        }
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
