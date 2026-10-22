//! The `ClassCastException` of a cast of an object whose class is known, with HotSpot's message
//! for two classes of `java.base`: the strings and boxes, which an `Object` or an interface holds.
//! The generator writes these where it knows the class of the object (see `codegen/class_cast.rs`);
//! other casts keep `cast` and its message, which names only the target.

use crate::{Exception, throw};

/// The message HotSpot gives a cast of a value of class `from` to class `to`, both by binary name
/// and both in `java.base`.
fn message(from: &str, to: &str) -> String {
    format!(
        "class {from} cannot be cast to class {to} ({from} and {to} are in module java.base of loader 'bootstrap')"
    )
}

/// `cast`, where a value of class `from` (named by `from()`) fails the cast to class `to`.
pub fn cast_from<T>(value: Option<T>, to: &str, from: impl FnOnce() -> &'static str) -> T {
    match value {
        Some(value) => value,
        None => throw("java.lang.ClassCastException", &message(from(), to)),
    }
}

/// `try_cast`, where a value of class `from` (named by `from()`) fails the cast to class `to`.
pub fn try_cast_from<T>(value: Option<T>, to: &str, from: impl FnOnce() -> &'static str) -> Result<T, Exception> {
    value.ok_or_else(|| Exception::new("java.lang.ClassCastException", message(from(), to)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_names_both_classes_and_their_module() {
        assert_eq!(
            message("java.lang.Long", "java.lang.Integer"),
            "class java.lang.Long cannot be cast to class java.lang.Integer (java.lang.Long and java.lang.Integer are in module java.base of loader 'bootstrap')"
        );
    }
}
