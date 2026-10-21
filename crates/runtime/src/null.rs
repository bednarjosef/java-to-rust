/// Java's dereference of a null reference: a possibly-null value that is null throws
/// `NullPointerException`, and any other value is returned as it is.
pub fn non_null<T>(value: Option<T>) -> T {
    match value {
        Some(value) => value,
        None => crate::throw("java.lang.NullPointerException", ""),
    }
}

/// `non_null` where the program observes the `NullPointerException`. Java's message names what
/// was null in terms of the bytecode, which a translation cannot reproduce; the translator
/// rejects programs that read it.
pub fn try_non_null<T>(value: Option<T>) -> Result<T, crate::Exception> {
    value.ok_or_else(|| crate::Exception::bare("java.lang.NullPointerException"))
}

/// Java's string form of a reference that may be null: `null`, or the value's own.
pub struct OrNull<T>(Option<T>);

/// The value, or `null`, for formatting: `java::or_null(node.as_ref())`.
pub fn or_null<T: std::fmt::Display>(value: Option<T>) -> OrNull<T> {
    OrNull(value)
}

impl<T: std::fmt::Display> std::fmt::Display for OrNull<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match &self.0 {
            Some(value) => value.fmt(f),
            None => f.write_str("null"),
        }
    }
}

/// `a == b` on references that may be null: both null, or the same object.
pub fn same<T: ?Sized>(a: Option<&std::rc::Rc<T>>, b: Option<&std::rc::Rc<T>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => std::rc::Rc::ptr_eq(a, b),
        (a, b) => a.is_none() && b.is_none(),
    }
}
