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
//! WARNING: sun.misc.Unsafe::objectFieldOffset has been called by Main (file:/work/classes/)
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
//!
//! Its methods of raw memory, which reach memory outside the heap by address (`allocateMemory`,
//! `getInt(long)`, `copyMemory`, `invokeCleaner`, `program::sun_misc_memory` names them), and
//! `addressSize` and `pageSize`, which do not warn, are calls of the methods of the same names
//! of `jdk.internal.misc.TranslatedUnsafeMemory` (`java/overlay`), which do what
//! `sun.misc.Unsafe`'s do after the warning: call `jdk.internal.misc.Unsafe`'s, which the
//! translation has (`docs/natives-and-input.md`). Such a call evaluates its arguments into locals,
//! then checks the receiver and warns, as Java does, so its arguments may do anything;
//! `invokeCleaner` checks that its buffer is direct before it warns, as `sun.misc.Unsafe`'s does.
//! `arrayBaseOffset` and `arrayIndexScale` of a class literal are the constants of
//! `jdk.internal.misc.Unsafe`'s (`natives.rs`), the first after the warning.
//!
//! The class's own initialization is translated, as a program that reads its `theUnsafe` by
//! reflection runs it; a patch (`java/patches/sun/misc/Unsafe.java`) leaves out the reflection
//! that finds the warning's flag and the filter that hides `getUnsafe` from reflection, which the
//! reflection table leaves out instead (`reflection.rs`).

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

    /// A call `node` of one of `sun.misc.Unsafe`'s methods that reach memory by address or tell
    /// its layout (`program::sun_misc_memory`): the receiver and arguments evaluated, the
    /// method's warning, then the call of the translator's method that does what the method does
    /// after it (`program::TRANSLATED_UNSAFE_MEMORY`). An argument whose evaluation may do
    /// something is kept in a local first, so it is evaluated before the warning, as in Java.
    /// `None` for any other call.
    pub(super) fn sun_memory_call(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
    ) -> Result<Option<ExprKind>, Error> {
        let Some(warns) = crate::program::sun_misc_memory(symbol.owner, symbol.name, symbol.ty) else {
            return Ok(None);
        };
        let resolve = |name: &str, signature: &str| {
            self.methods.iter().position(|m| {
                self.types[m.owner].javac == crate::program::TRANSLATED_UNSAFE_MEMORY
                    && m.name == name
                    && m.sig == signature
            })
        };
        let method = symbol.name;
        let Some(resolved) = resolve(method, symbol.ty) else {
            return unsupported(node, &format!("`sun.misc.Unsafe.{method}` outside a whole program"));
        };
        let (pos, ret) = (node.pos(), self.ty(node, node.java_type())?);
        let select = node.child("methodSelect")?;
        let Some(receiver) = select.get("expression").filter(|_| select.kind() == "MEMBER_SELECT") else {
            return unsupported(node, &format!("`sun.misc.Unsafe.{method}` without a receiver"));
        };
        let cleaner = method == "invokeCleaner";
        if cleaner && !self.quiet_argument(receiver) {
            return unsupported(
                node,
                "`sun.misc.Unsafe.invokeCleaner` of a receiver that may have an effect",
            );
        }
        let mut block = Vec::new();
        let mut kept = Vec::new();
        let params = lambdas::signature(symbol.ty).0;
        for (i, (arg, param)) in node.list("arguments").zip(params).enumerate() {
            let ty = self.ty(arg, param)?;
            if self.quiet_argument(arg) {
                kept.push((Err(arg), ty));
                continue;
            }
            let name = self.fresh_name(&format!("memory_arg{i}"));
            let value = self.convert(self.expr(arg)?, ty);
            block.push(Stmt::Local { ty, name: name.clone(), init: Some(value), pos });
            kept.push((Ok(name), ty));
        }
        let args = || -> Result<Vec<Expr>, Error> {
            kept.iter()
                .map(|(arg, ty)| match arg {
                    Ok(name) => Ok(self.synth(ExprKind::Var(name.clone()), pos, *ty)),
                    Err(node) => Ok(self.convert(self.expr(*node)?, *ty)),
                })
                .collect()
        };
        // `invokeCleaner` checks its buffer before it warns, once the call has found its receiver.
        if cleaner {
            block.push(Stmt::Expr(self.receiver_checked(receiver, method, pos)?));
            let (check, signature) = crate::program::CLEANER_CHECK;
            let call = ExprKind::Call {
                name: check.to_string(),
                args: args()?,
                resolved: resolve(check, signature),
            };
            block.push(Stmt::Expr(self.synth(call, pos, Type::Void)));
        }
        if warns {
            block.push(Stmt::Expr(self.memory_access_warning(node, method)?));
        } else {
            block.push(Stmt::Expr(self.receiver_checked(receiver, method, pos)?));
        }
        let call = self.synth(
            ExprKind::Call { name: method.to_string(), args: args()?, resolved: Some(resolved) },
            pos,
            ret,
        );
        block.push(if ret == Type::Void { Stmt::Expr(call) } else { Stmt::Yield(call) });
        Ok(Some(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body: block }],
        }))
    }

    /// The receiver `receiver` of a call of the method `method`, checked for null as the call
    /// checks it.
    fn receiver_checked(&self, receiver: Node<'t>, method: &str, pos: Pos) -> Result<Expr, Error> {
        let value = self.expr(receiver)?;
        let ty = value.ty;
        let checked = Builtin::NullChecked(Some(interned_name(method)));
        Ok(self.synth(ExprKind::Builtin(checked, vec![value]), pos, ty))
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
        if node.kind().ends_with("_LITERAL")
            || (node.kind() == "MEMBER_SELECT" && node.str("identifier") == "class")
        {
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
