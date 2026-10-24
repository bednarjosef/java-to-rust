//! Classes initialized through a `Class` value: `MethodHandles.lookup().ensureInitialized(c)`
//! runs the static initialization of the class `c`, as `SharedSecrets.ensureClassInitialized`
//! does to have `Buffer`, `FileDescriptor` or `ResourceBundle` register its access object. The
//! JDK passes the class literal to a helper that passes its parameter on, so the classes are
//! those whose literals reachable code gives `ensureInitialized`, directly or as the argument of a
//! method's parameter that the method gives it (one call deep). They are reached as initialized
//! (`Reach::ensured`), and the lowering initializes the one the value names
//! (`lower::by_name`).

use super::{Item, MethodKey, Walk, strip_arguments, text};
use crate::value::Value;

/// Whether the call `node` is `Lookup.ensureInitialized(c)`.
pub fn is_ensure_initialized(node: &Value) -> bool {
    let symbol = &node["symbol"];
    text(node, "kind") == "METHOD_INVOCATION"
        && text(symbol, "owner") == "java.lang.invoke.MethodHandles.Lookup"
        && text(symbol, "name") == "ensureInitialized"
        && node["arguments"].as_array().is_some_and(|a| a.len() == 1)
}

/// The class that a class literal `X.class` names, for any other node `None`.
pub fn literal_class(node: &Value) -> Option<String> {
    (text(node, "kind") == "MEMBER_SELECT" && text(node, "identifier") == "class")
        .then(|| strip_arguments(text(&node["expression"], "javaType")))
        .filter(|class| !class.is_empty())
}

impl Walk<'_> {
    /// A call of reachable code: if it ensures that a class is initialized, that class is, and a
    /// class literal it passes to a method that ensures its parameter is initialized is too.
    pub(super) fn initialization(&mut self, call: &Value) {
        if text(call, "kind") != "METHOD_INVOCATION" {
            return;
        }
        let Some(arguments) = call["arguments"].as_array() else { return };
        if is_ensure_initialized(call) {
            let argument = &arguments[0];
            if let Some(class) = literal_class(argument) {
                self.ensure(class);
            } else if text(argument, "kind") == "IDENTIFIER"
                && text(&argument["symbol"], "kind") == "PARAMETER"
                && let Some(method) = self.scanning.clone()
                && let Some(index) = self.parameter_index(&method, text(argument, "name"))
            {
                for class in self.class_arguments.get(&(method.clone(), index)).cloned().unwrap_or_default() {
                    self.ensure(class);
                }
                self.initializing.insert((method, index));
            }
            return;
        }
        let symbol = &call["symbol"];
        let callee: MethodKey = (
            strip_arguments(text(symbol, "owner")),
            text(symbol, "name").into(),
            text(symbol, "type").into(),
        );
        for (index, argument) in arguments.iter().enumerate() {
            let Some(class) = literal_class(argument) else { continue };
            let key = (callee.clone(), index);
            if self.initializing.contains(&key) {
                self.ensure(class.clone());
            }
            self.class_arguments.entry(key).or_default().insert(class);
        }
    }

    /// The class `class` is initialized through a `Class` value.
    fn ensure(&mut self, class: String) {
        if self.reach.ensured.insert(class.clone()) {
            self.work.push(Item::Init(class));
        }
    }

    /// The position of the parameter `name` of the method `method`.
    pub(super) fn parameter_index(&self, method: &MethodKey, name: &str) -> Option<usize> {
        let node = self.methods.get(method)?;
        node["parameters"].as_array()?.iter().position(|p| text(p, "name") == name)
    }
}
