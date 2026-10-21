//! Monitors (see docs/multi-threading.md, section 3, and `analysis::monitors`).
//!
//! Where the monitor of a `synchronized` block's lock is kept, the block holds it: the guard of
//! `java::enter` is the block's first local, and drops however the block ends.
//!
//! ```text
//! {
//!     let _monitor = java::enter(&lock);
//!     { .. }
//! }
//! ```
//!
//! A synchronized method's body starts with the guard of its object's monitor (`this`, a handle),
//! or of its class's (`java::enter_class(&C__MONITOR)`). Anywhere else the statement evaluates
//! the lock once and throws a `NullPointerException` when it is null, then the block runs.
//! `wait`, `notify`, `notifyAll` and `Thread.holdsLock` are the runtime's functions on the
//! object's handle.

use super::*;

impl<'a> Gen<'a> {
    /// Evaluates a lock and drops it, throwing if it is null.
    pub(super) fn null_check(&mut self, lock: &'a Expr) -> Code {
        if self.nullable(lock) {
            let option = self.option_ref(lock);
            return Code::atom(self.dereference(&[lock.id], &option));
        }
        // A lock that cannot be null throws nothing, wherever a `catch` or a `finally` would see it.
        self.dereferenced.insert(lock.id);
        let value = self.expr(lock).text;
        Code::atom(format!("{{ let _ = {value}; }}"))
    }

    /// A `synchronized` block whose monitor is kept: `stmt` is its lock (`MonitorEnter`) and
    /// `body` the block after it. `false`, with nothing written, when the monitor is not kept.
    pub(super) fn synchronized_block(&mut self, stmt: &'a Stmt, body: Option<&'a Stmt>, tail: bool) -> bool {
        let Stmt::Expr(Expr { kind: ExprKind::Builtin(Builtin::MonitorEnter, args), .. }) = stmt else {
            return false;
        };
        if !self.kept_lock(&args[0]) {
            return false;
        }
        let Some(Stmt::Block(stmts)) = body else {
            unreachable!("a `synchronized` lock comes before its block")
        };
        let enter = self.enter(&args[0]);
        self.line("{");
        self.indent += 1;
        self.line(&format!("let _monitor = {enter};"));
        self.stmts(stmts, tail);
        self.indent -= 1;
        self.line("}");
        true
    }

    /// The guard of a synchronized method's monitor, first in its body, where it is kept.
    pub(super) fn method_monitor(&mut self, m: usize) {
        let method = &self.class.methods[m];
        if !method.is_synchronized {
            return;
        }
        let enter = if method.is_static {
            if !self.facts.class_monitors.contains(&method.class) {
                return;
            }
            self.class_enter(method.class)
        } else {
            if !self.facts.kept_monitor_of(method.class) || self.this_form != ThisForm::Shared {
                return;
            }
            format!("{}({})", self.runtime("enter"), self.this_name())
        };
        self.line(&format!("let _monitor = {enter};"));
    }

    /// Whether the monitor of `lock` is kept. A constructor's object, which no other thread has
    /// yet, keeps none.
    fn kept_lock(&self, lock: &Expr) -> bool {
        match lock.kind {
            ExprKind::Builtin(Builtin::ClassLiteral(ty), _) => {
                self.class.class_index(ty).is_some_and(|t| self.facts.class_monitors.contains(&t))
            }
            ExprKind::This if self.this_form != ThisForm::Shared => false,
            _ => matches!(lock.ty, Type::Class(..)) && self.facts.kept_monitor(self.class, lock.ty),
        }
    }

    /// `java::enter(&o)`, or a class's `java::enter_class(&C__MONITOR)`.
    fn enter(&mut self, lock: &'a Expr) -> String {
        if let ExprKind::Builtin(Builtin::ClassLiteral(ty), _) = lock.kind {
            let t = self.class.class_index(ty).expect("a class monitor's class");
            return self.class_enter(t);
        }
        let handle = self.monitor_handle(lock);
        format!("{}({handle})", self.runtime("enter"))
    }

    fn class_enter(&mut self, t: usize) -> String {
        format!("{}(&{})", self.runtime("enter_class"), class_monitor(&self.class.types[t].name))
    }

    /// A reference to the handle of the object whose monitor is used, which throws a
    /// `NullPointerException` when it is null. A handle read through another object's borrow is
    /// taken out of it first (`&{ o.borrow().lock.clone() }`), so that no lock is held while the
    /// thread waits for the monitor.
    fn monitor_handle(&mut self, object: &'a Expr) -> String {
        let id = object.id;
        let e = self.peel(object);
        let nullable = matches!(e.ty, Type::Class(..)) && self.nullable(e);
        if self.reads_through_borrow(e) {
            let value = self.expr(e).text;
            if nullable {
                return self.dereference(&[id, e.id], &format!("{{ {value} }}.as_ref()"));
            }
            return format!("&{{ {value} }}");
        }
        if nullable {
            let option = format!("{}.as_ref()", self.object_place(e));
            return self.dereference(&[id, e.id], &option);
        }
        self.object_ref(e)
    }

    /// `o.wait(millis)`, `o.notify()`, `o.notifyAll()` and `Thread.holdsLock(o)`.
    pub(super) fn monitor_native(&mut self, native: Native, args: &'a [Expr], id: ExprId) -> Code {
        let handle = self.monitor_handle(&args[0]);
        match native {
            Native::MonitorWait => {
                let millis = self.value(&args[1], Type::Long).text;
                self.may_fail(id, "monitor_wait", "", &format!("{handle}, {millis}"))
            }
            Native::MonitorNotify { all } => {
                self.may_fail(id, "monitor_notify", "", &format!("{handle}, {all}"))
            }
            _ => Code::atom(format!("{}({handle})", self.runtime("holds_lock"))),
        }
    }

    /// The statics of the class monitors of this module's classes.
    pub(super) fn class_monitors(&mut self) {
        let class = self.class;
        let here: Vec<usize> = (0..class.types.len())
            .filter(|t| self.facts.class_monitors.contains(t))
            .filter(|&t| self.module_types().any(|d| std::ptr::eq(d, &class.types[t])))
            .collect();
        if here.is_empty() {
            return;
        }
        self.gap();
        let locked = self.runtime("Locked");
        for t in here {
            let name = class_monitor(&class.types[t].name);
            let local = name.rsplit("::").next().unwrap_or(&name).to_string();
            self.line(&format!("{}static {local}: {locked}<()> = {locked}::new(());", self.vis()));
        }
    }
}

/// The static that is the monitor of class `class`; from another package, with that package's
/// path.
fn class_monitor(class: &str) -> String {
    let (path, local) = modules::split(class);
    path + &format!("{}__MONITOR", local.replace('.', "")).to_uppercase()
}
