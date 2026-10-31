/// Java's dereference of a null reference: a possibly-null value that is null throws
/// `NullPointerException`, and any other value is returned as it is.
#[inline]
pub fn non_null<T>(value: Option<T>) -> T {
    match value {
        Some(value) => value,
        None => crate::throw("java.lang.NullPointerException", ""),
    }
}

/// `non_null` with Java's message (`try_non_null_message`), where a `try` that catches by
/// unwinding may read it (`java::unwinding`).
#[inline]
pub fn non_null_message<T>(value: Option<T>, message: &'static str) -> T {
    match value {
        Some(value) => value,
        None => null_pointer_thrown(message),
    }
}

/// The `NullPointerException` of `non_null_message`, out of line.
#[cold]
#[inline(never)]
fn null_pointer_thrown(message: &str) -> ! {
    crate::throw("java.lang.NullPointerException", message)
}

/// The value of a field that every constructor of its object sets, to a value that is not null,
/// before anything can read it (`analysis/assigned_fields.rs`): its cell starts empty, as there is
/// no object to start it with, and holds a value by the time any code reads it, so the read has no
/// test. A build with debug assertions checks it all the same.
#[inline(always)]
pub fn assigned<T>(value: Option<T>) -> T {
    match value {
        Some(value) => value,
        None => unset_field(),
    }
}

/// `assigned` of a field read where it is.
#[inline(always)]
pub fn assigned_ref<T>(value: &Option<T>) -> &T {
    match value {
        Some(value) => value,
        None => unset_field(),
    }
}

/// A read of a field of `assigned` that found it empty: the analysis was wrong.
#[inline(always)]
fn unset_field() -> ! {
    if cfg!(debug_assertions) {
        unset_field_found()
    }
    // The analysis proves that every constructor sets the field before anything can read it.
    unsafe { std::hint::unreachable_unchecked() }
}

#[cold]
#[inline(never)]
fn unset_field_found() -> ! {
    panic!("a field read before its constructor set it, which the analysis ruled out")
}

/// `non_null` where the program observes the `NullPointerException`, without a message: where
/// nothing reads it, or where Java's has none (as `Objects.requireNonNull`'s).
#[inline]
pub fn try_non_null<T>(value: Option<T>) -> Result<T, crate::Exception> {
    match value {
        Some(value) => Ok(value),
        None => Err(bare_null_pointer()),
    }
}

/// `try_non_null` with Java's message, which names what was null in terms of the class file:
/// `Cannot invoke "Main$Box.size()" because "<local2>" is null`.
#[inline]
pub fn try_non_null_message<T>(value: Option<T>, message: &'static str) -> Result<T, crate::Exception> {
    match value {
        Some(value) => Ok(value),
        None => Err(null_pointer(message)),
    }
}

/// The `NullPointerException` with `message`: out of the line of the check above, which is
/// inlined where a reference is used.
#[cold]
#[inline(never)]
fn null_pointer(message: &'static str) -> crate::Exception {
    crate::Exception::new("java.lang.NullPointerException", message)
}

/// The `NullPointerException` without a message, out of the line of `try_non_null`.
#[cold]
#[inline(never)]
fn bare_null_pointer() -> crate::Exception {
    crate::Exception::bare("java.lang.NullPointerException")
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

/// A string concatenation's piece that may be null, as Java shows it.
impl<T: std::fmt::Display + crate::Piece> crate::Piece for OrNull<T> {
    fn push_to(&self, out: &mut crate::JString) {
        match &self.0 {
            Some(value) => value.push_to(out),
            None => out.push_str("null"),
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

/// `a == b` on two `Object`s that may both be strings or boxes (see `SameObject`), either of
/// which may be null.
pub fn same_object<T: SameObject>(a: Option<&T>, b: Option<&T>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.same_object(b),
        (a, b) => a.is_none() && b.is_none(),
    }
}

/// Java's `==` on the values of a type that holds strings or boxes as plain values, which the
/// translation implements for the enum of `Object`: objects are the same when they are one
/// object, and a string or a box is not the same as a different value. Two equal strings or boxes
/// may or may not be one object in Java, which the translation does not know, and end the program
/// (`unkept_identity`).
pub trait SameObject {
    fn same_object(&self, other: &Self) -> bool;
}

/// Ends the program at `==` on two equal strings or boxes held as `Object`s, whose identity the
/// translation does not keep.
pub fn unkept_identity() -> ! {
    crate::exception::die(
        "comparing two equal Strings or boxed values held as Objects by identity (==) is not supported by the translation",
    )
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

impl<T> Handle for crate::Arc<T> {
    fn address(&self) -> *const u8 {
        crate::Arc::as_ptr(self) as *const u8
    }
}
