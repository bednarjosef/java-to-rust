//! `sun.misc.Unsafe`, the class of unsafe operations that libraries use (module
//! `jdk.unsupported`). Its methods that read, write or atomically update a field or an element at
//! an offset forward to `jdk.internal.misc.Unsafe`'s after a warning
//! (`program::sun_misc_forwarded` names them):
//!
//! ```java
//! public int getInt(Object o, long offset) {
//!     beforeMemoryAccess();
//!     return theInternalUnsafe.getInt(o, offset);
//! }
//! ```
//!
//! On the first call of any of them in a run, `beforeMemoryAccess()` prints four lines on the
//! JVM's first standard error, naming the method, the class that called it, which it finds by
//! walking the stack (`StackWalker.walk`), and the place that class was loaded from:
//!
//! ```text
//! WARNING: A terminally deprecated method in sun.misc.Unsafe has been called
//! WARNING: sun.misc.Unsafe::objectFieldOffset has been called by Main (file:/home/me/classes/)
//! WARNING: Please consider reporting this to the maintainers of class Main
//! WARNING: sun.misc.Unsafe::objectFieldOffset will be removed in a future release
//! ```
//!
//! A translation has no stack to walk, but the caller is the class whose code makes the call, as
//! for `getCallerClass` (`callers.rs`); and a translation runs as `java Main` does from the folder
//! of the program's classes (its class path is `.`, see `system.rs`), so the place is the working
//! directory, as the class path's `file:` URL (the runtime's `java::memory_access_warning`).
//!
//! So a call of such a method is the access of `jdk.internal.misc.Unsafe`'s method at the same
//! offset (`handles.rs`), after `Builtin::MemoryAccessWarning` of the receiver and the caller,
//! and the methods' bodies are not translated. `objectFieldOffset(f)` makes a field handle, as
//! `jdk.internal.misc.Unsafe`'s does; its definition, which is left out, leaves the warning in
//! its place. Java evaluates the arguments before the method warns, and the warning comes first
//! here, so every argument but the offset must be one whose evaluation does nothing: a literal,
//! `this`, a local or parameter, or a field of the class whose code makes the call. A method
//! reference to such a method is rejected: Java's walker skips the frame of the reference's
//! hidden class and names whoever calls the functional interface.

use super::*;
use crate::program::SUN_MISC_UNSAFE;

/// `sun.misc.Unsafe.objectFieldOffset(Field)`, of the field that `Class.getDeclaredField` finds.
pub(super) const SUN_OFFSET_OF_FIELD: (&str, &str, &str) =
    (SUN_MISC_UNSAFE, "objectFieldOffset", "(java.lang.reflect.Field)long");

impl<'t> Lowering<'t> {
    /// `access`, the access that the call `node` of the method `method` of `sun.misc.Unsafe`
    /// forwards to, after the method's warning (see the module's comment).
    pub(super) fn after_memory_access_warning(
        &self,
        node: Node<'t>,
        method: &str,
        access: ExprKind,
    ) -> Result<ExprKind, Error> {
        for (i, arg) in node.list("arguments").enumerate() {
            if i != 1 && !self.quiet_argument(arg) {
                return unsupported(
                    arg,
                    &format!("an argument of `sun.misc.Unsafe.{method}` that may have an effect"),
                );
            }
        }
        let warning = self.memory_access_warning(node, method)?;
        let (pos, ty) = (node.pos(), self.ty(node, node.java_type())?);
        let access = self.synth(access, pos, ty);
        let body = vec![
            Stmt::Expr(warning),
            if ty == Type::Void { Stmt::Expr(access) } else { Stmt::Yield(access) },
        ];
        Ok(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body }],
        })
    }

    /// The warning of `sun.misc.Unsafe`'s `objectFieldOffset` that defines the field handle
    /// `member` of class `owner`, whose definition is left out; `None` for a handle that
    /// `jdk.internal.misc.Unsafe` defines. Java throws for a record's field and for a field that
    /// is not there, which is rejected.
    pub(super) fn handle_warning(&self, owner: usize, member: Node<'t>) -> Result<Option<Expr>, Error> {
        let Some(call) = self.sun_offset_call(owner, member) else { return Ok(None) };
        let Some((class, field)) = self.handle_declaration(owner, member) else { return Ok(None) };
        let decl = self.types[class].node;
        if decl.kind() == "RECORD" {
            return unsupported(call, "`sun.misc.Unsafe.objectFieldOffset` of a record's field");
        }
        let declared = decl
            .list("members")
            .any(|m| m.kind() == "VARIABLE" && m.str("name") == field && !m.has_flag("STATIC"));
        if !declared {
            return unsupported(call, &format!("the offset of `{field}`, which is not a field of the class"));
        }
        self.memory_access_warning(call, "objectFieldOffset").map(Some)
    }

    /// Whether the value of the static field `member` of class `owner` is a call of
    /// `sun.misc.Unsafe`'s `objectFieldOffset`, which warns.
    pub(super) fn made_by_sun_unsafe(&self, owner: usize, member: Node<'t>) -> bool {
        self.sun_offset_call(owner, member).is_some()
    }

    /// The value of the static field `member` of class `owner`, when it is a call of
    /// `sun.misc.Unsafe`'s `objectFieldOffset`.
    fn sun_offset_call(&self, owner: usize, member: Node<'t>) -> Option<Node<'t>> {
        let call = handles::plain(self.static_value(owner, member)?.0);
        let symbol = call.symbol().filter(|_| call.kind() == "METHOD_INVOCATION")?;
        ((symbol.owner, symbol.name, symbol.ty) == SUN_OFFSET_OF_FIELD).then_some(call)
    }

    /// `Builtin::MemoryAccessWarning` of the receiver of `call`, a call of the method `method`
    /// of `sun.misc.Unsafe`, and of the class whose code makes it.
    fn memory_access_warning(&self, call: Node<'t>, method: &str) -> Result<Expr, Error> {
        let select = call.child("methodSelect")?;
        let Some(receiver) = select.get("expression").filter(|_| select.kind() == "MEMBER_SELECT") else {
            return unsupported(call, &format!("`sun.misc.Unsafe.{method}` without a receiver"));
        };
        let current = self.current.get();
        if self.lambda_of_type(current).is_some_and(|l| l.node.kind() == "MEMBER_REFERENCE") {
            return unsupported(call, &format!("a method reference to `sun.misc.Unsafe.{method}`"));
        }
        let pos = call.pos();
        let caller = self.self_type(self.lambda_holder(current));
        let caller =
            self.synth(ExprKind::Builtin(Builtin::ClassLiteral(caller), Vec::new()), pos, Type::JavaClass);
        let warning = Builtin::MemoryAccessWarning(interned_name(method));
        Ok(self.synth(ExprKind::Builtin(warning, vec![self.expr(receiver)?, caller]), pos, Type::Void))
    }

    /// Whether evaluating the argument `node` does nothing that could be seen: a literal, `this`,
    /// a local or parameter, or a field of the class whose code is being lowered.
    fn quiet_argument(&self, node: Node<'t>) -> bool {
        let node = handles::plain(node);
        if node.kind().ends_with("_LITERAL") {
            return true;
        }
        if node.kind() != "IDENTIFIER" {
            return false;
        }
        node.str("name") == "this"
            || node.symbol().is_some_and(|s| match s.kind {
                "FIELD" => self.type_index(s.owner) == Some(self.current.get()),
                kind => kind != "METHOD",
            })
    }
}
