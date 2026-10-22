/// Java's dereference of a null reference: a possibly-null value that is null throws
/// `NullPointerException`, and any other value is returned as it is.
pub fn non_null<T>(value: Option<T>) -> T {
    match value {
        Some(value) => value,
        None => crate::throw("java.lang.NullPointerException", ""),
    }
}

/// `non_null` where the program observes the `NullPointerException`, without a message: where
/// nothing reads it, or where Java's has none (as `Objects.requireNonNull`'s).
pub fn try_non_null<T>(value: Option<T>) -> Result<T, crate::Exception> {
    value.ok_or_else(|| crate::Exception::bare("java.lang.NullPointerException"))
}

/// `try_non_null` with Java's message, which names what was null in terms of the class file:
/// `Cannot invoke "Main$Box.size()" because "<local2>" is null`.
pub fn try_non_null_message<T>(value: Option<T>, message: &'static str) -> Result<T, crate::Exception> {
    value.ok_or_else(|| crate::Exception::new("java.lang.NullPointerException", message))
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
pub fn same<H: Handle>(a: Option<&H>, b: Option<&H>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.address() == b.address(),
        (a, b) => a.is_none() && b.is_none(),
    }
}

/// A handle of an object: an `Rc`, or an `Arc` for an object that threads share. Its address is
/// the object's identity.
pub trait Handle {
    fn address(&self) -> *const u8;
}

impl<T: ?Sized> Handle for std::rc::Rc<T> {
    fn address(&self) -> *const u8 {
        std::rc::Rc::as_ptr(self) as *const u8
    }
}

impl<T: ?Sized> Handle for std::sync::Arc<T> {
    fn address(&self) -> *const u8 {
        std::sync::Arc::as_ptr(self) as *const u8
    }
}
