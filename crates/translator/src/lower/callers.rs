//! The class that called a method, which code asks for with `Reflection.getCallerClass()` (the
//! JDK's `@CallerSensitive` methods, such as `ResourceBundle.getBundle`) or with a
//! `StackWalker`'s `getCallerClass()` (logging libraries naming a logger after the class that
//! makes it). A translated program has no stack to walk, but the translator knows which class
//! each call is in, so it passes that class to the method: a static method whose own body asks
//! for its caller gets a parameter more, `caller$class`, the question in its body is that
//! parameter, and each call of it passes a class literal of the class whose code makes the call
//! (for a lambda's code, the class that holds the lambda, whose method javac makes of it).
//!
//! Only that shape is supported, the one that `@CallerSensitive` guarantees: the question is asked
//! by the method that the caller calls, not in a lambda or a class inside it, and that method is
//! static, so that every call of it names it. A `StackWalker` must be made with
//! `RETAIN_CLASS_REFERENCE` where it is asked (`StackWalker.getInstance(RETAIN_CLASS_REFERENCE)
//! .getCallerClass()`), or be a static final field of the method's class made so. A method
//! reference to a method that walks the stack is rejected: Java's walker skips the hidden frame
//! of the reference's class and finds whoever calls the functional interface. (For
//! `Reflection.getCallerClass()`, Java binds the caller of a method reference to the class that
//! holds the reference, which the translation passes.) Every other way of walking the stack
//! (`StackWalker.walk` over its frames) is rejected as before.

use super::*;
use std::collections::HashMap;

/// The parameter that a method asking for its caller gets.
pub(super) const CALLER_PARAM: &str = "caller$class";

/// The methods that answer which class called the method that calls them, by class, name and
/// javac's signature (see `program::CALLER_QUERIES`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum CallerQuery {
    /// `jdk.internal.reflect.Reflection.getCallerClass()`.
    Reflection,
    /// `java.lang.StackWalker`'s `getCallerClass()`.
    Walker,
}

/// The question that a call with this symbol asks, if it is one.
fn query_of(owner: &str, name: &str, signature: &str) -> Option<CallerQuery> {
    match crate::program::caller_query(owner, name, signature)? {
        "java.lang.StackWalker" => Some(CallerQuery::Walker),
        _ => Some(CallerQuery::Reflection),
    }
}

/// The question asked by `node`, a tree, outside the lambdas and classes in it; `Walker` when it
/// asks both.
fn asked(node: &Value) -> Option<CallerQuery> {
    match node {
        Value::Array(items) => items.iter().filter_map(asked).max_by_key(|q| *q == CallerQuery::Walker),
        Value::Object(map) => {
            let kind = map.get("kind").and_then(Value::as_str).unwrap_or_default();
            // The code of a lambda, a local class or an anonymous class is a method of its own.
            if matches!(kind, "LAMBDA_EXPRESSION" | "CLASS" | "INTERFACE" | "ENUM" | "RECORD") {
                return None;
            }
            let own = (kind == "METHOD_INVOCATION").then(|| {
                let symbol = &node["symbol"];
                let text = |f: &str| symbol[f].as_str().unwrap_or_default();
                query_of(text("owner"), text("name"), text("type"))
            });
            let inner = map.iter().filter(|(key, _)| key.as_str() != "symbol").filter_map(|(key, v)| {
                if kind == "NEW_CLASS" && key == "classBody" { None } else { asked(v) }
            });
            own.flatten().into_iter().chain(inner).max_by_key(|q| *q == CallerQuery::Walker)
        }
        _ => None,
    }
}

impl<'t> Lowering<'t> {
    /// What method `m` asks about its caller in its own body, if anything.
    pub(super) fn caller_sensitive(&self, m: usize) -> Option<CallerQuery> {
        if let Some(known) = self.callers.borrow().get(&m) {
            return *known;
        }
        let info = &self.methods[m];
        let query = if self.lambda_of_method(m).is_some() || info.node.kind() != "METHOD" {
            None
        } else {
            info.node.get("body").and_then(|b| asked(b.0))
        };
        self.callers.borrow_mut().insert(m, query);
        query
    }

    /// The parameter `caller$class` of method `m` (`node`) when it asks for its caller; rejects a
    /// method that is not static, or is the program's `main`, which nothing in it calls.
    pub(super) fn caller_param(
        &self,
        m: usize,
        node: Node<'t>,
        is_main: bool,
    ) -> Result<Option<Param>, Error> {
        if self.caller_sensitive(m).is_none() {
            return Ok(None);
        }
        if !node.has_flag("STATIC") || is_main {
            let what = if is_main { "`main`" } else { "a method that is not static" };
            return unsupported(node, &format!("asking for the caller (`getCallerClass`) in {what}"));
        }
        Ok(Some(Param { name: CALLER_PARAM.to_string(), ty: Type::JavaClass }))
    }

    /// `Reflection.getCallerClass()` or `walker.getCallerClass()`: the parameter `caller$class` of
    /// the method that asks, where it is supported (see the module's comment). `None` for any
    /// other call; it runs before the arguments are lowered.
    pub(super) fn caller_class(
        &self,
        node: Node<'t>,
        symbol: &Symbol,
        select: Node<'t>,
    ) -> Result<Option<ExprKind>, Error> {
        let Some(query) = query_of(symbol.owner, symbol.name, symbol.ty) else { return Ok(None) };
        let what = match query {
            CallerQuery::Reflection => "`Reflection.getCallerClass`",
            CallerQuery::Walker => "`StackWalker.getCallerClass`",
        };
        let asking = self
            .method_ctx
            .get()
            .filter(|&m| self.caller_sensitive(m).is_some() && self.methods[m].owner == self.current.get());
        let Some(m) = asking else {
            return unsupported(node, &format!("{what} outside the body of a static method"));
        };
        if query == CallerQuery::Walker {
            let receiver = select.get("expression").filter(|_| select.kind() == "MEMBER_SELECT");
            if !receiver.is_some_and(|r| self.retains_classes(r, self.methods[m].owner)) {
                return unsupported(
                    node,
                    &format!("{what} of a walker not made here with `RETAIN_CLASS_REFERENCE`"),
                );
            }
        }
        Ok(Some(ExprKind::Var(CALLER_PARAM.to_string())))
    }

    /// Whether `walker` is `StackWalker.getInstance(RETAIN_CLASS_REFERENCE)`, or a static final
    /// field of class `owner` that is initialized with it (reading it has no effect then, as the
    /// class is initialized).
    fn retains_classes(&self, walker: Node<'t>, owner: usize) -> bool {
        let made = |call: Node<'t>| {
            call.kind() == "METHOD_INVOCATION"
                && call.symbol().is_some_and(|s| {
                    (s.owner, s.name, s.ty)
                        == (
                            "java.lang.StackWalker",
                            "getInstance",
                            "(java.lang.StackWalker.Option)java.lang.StackWalker",
                        )
                })
                && call.list("arguments").next().and_then(|a| a.symbol()).is_some_and(|s| {
                    (s.owner, s.name) == ("java.lang.StackWalker.Option", "RETAIN_CLASS_REFERENCE")
                })
        };
        if made(walker) {
            return true;
        }
        let Some(field) = walker.symbol().filter(|s| s.kind == "FIELD" && s.is_static) else { return false };
        if self.type_index(field.owner) != Some(owner) {
            return false;
        }
        self.types[owner].node.list("members").any(|member| {
            member.kind() == "VARIABLE"
                && member.str("name") == field.name
                && member.has_flag("FINAL")
                && member.get("initializer").is_some_and(made)
        })
    }

    /// The argument that a call of method `resolved`, which asks for its caller, passes: a class
    /// literal of the class whose code makes the call (`node`). A method reference to a method
    /// that walks the stack is rejected (see the module's comment).
    pub(super) fn caller_arg(&self, node: Node<'t>, resolved: Option<usize>) -> Result<Option<Expr>, Error> {
        let Some(query) = resolved.and_then(|m| self.caller_sensitive(m)) else { return Ok(None) };
        let current = self.current.get();
        let reference = self.lambda_of_type(current).is_some_and(|l| l.node.kind() == "MEMBER_REFERENCE");
        if reference && query == CallerQuery::Walker {
            return unsupported(
                node,
                "a method reference to a method that asks a `StackWalker` for its caller",
            );
        }
        let caller = self.self_type(self.lambda_holder(current));
        Ok(Some(self.synth(
            ExprKind::Builtin(Builtin::ClassLiteral(caller), Vec::new()),
            node.pos(),
            Type::JavaClass,
        )))
    }
}

/// The calls that ask for the caller, per method, which `caller_sensitive` finds once.
pub(super) type Callers = RefCell<HashMap<usize, Option<CallerQuery>>>;
