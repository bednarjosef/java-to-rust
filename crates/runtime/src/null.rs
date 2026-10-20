/// Java's dereference of a null reference: a possibly-null value that is null throws
/// `NullPointerException`, and any other value is returned as it is.
pub fn non_null<T>(value: Option<T>) -> T {
    match value {
        Some(value) => value,
        None => crate::throw("NullPointerException", ""),
    }
}
