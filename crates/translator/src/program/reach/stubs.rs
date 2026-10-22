//! Classes that a whole program does not translate (`UNTRANSLATED`), used as types. No
//! translated code can create an object of one, so in a translated program a variable of such a
//! type is only ever null, an instance method called on it can only throw
//! `NullPointerException`, and `instanceof` of it is false: exactly what Java does with that null.
//! Such a class is declared as a stub (`Reach::stubs`): a class without constructors or fields,
//! with the instance methods that reachable code calls on it, whose bodies never run
//! (`Reach::dormant`). `TreeMap.buildFromSorted` takes the `ObjectInputStream` that
//! deserialization reads a map from, and every call that copies a sorted map passes null.
//!
//! A static method, a constructor or a field of such a class stays out of reach, and a generic
//! or array type of one is not a stub: what uses them is rejected as before. A class nested in
//! one (`ObjectInputStream.GetField`) is a stub of its own, without its outer class.

use super::{MethodKey, Walk, provided, strip_arguments, text, untranslated};
use crate::value::Value;

impl Walk<'_> {
    /// The declared type of a variable, `ty`, which is a stub when it is an untranslated class.
    pub(super) fn stub_type(&mut self, ty: &str) {
        if is_stub(ty) {
            self.reach.stubs.entry(ty.to_string()).or_default();
        }
    }

    /// A method that reachable code calls, `symbol`: an instance method of a stub is declared
    /// with it.
    pub(super) fn stub_method(&mut self, symbol: &Value) {
        let owner = strip_arguments(text(symbol, "owner"));
        let name = text(symbol, "name");
        let is_static = symbol["static"].as_bool() == Some(true);
        if is_static || name == "<init>" || !is_stub(&owner) {
            return;
        }
        let key: MethodKey = (owner.clone(), name.to_string(), text(symbol, "type").to_string());
        self.reach.stubs.entry(owner).or_default().insert((key.1.clone(), key.2.clone()));
        self.reach.methods.insert(key.clone());
        self.reach.dormant.insert(key);
    }
}

/// Whether the type `ty` is a class that is not translated and can be a stub: not generic, not
/// an array, and in a package, at its top level or nested in another class
/// (`ObjectInputStream.GetField`, which `readObject` methods declare).
pub(super) fn is_stub(ty: &str) -> bool {
    let mut parts = ty.split('.');
    let in_package = parts.next().is_some_and(|first| first.starts_with(|c: char| c.is_lowercase()))
        && parts.any(|part| !part.starts_with(|c: char| c.is_lowercase()));
    !ty.contains(['<', '[', '?', ' ']) && in_package && untranslated(ty).is_some() && !provided(ty)
}
