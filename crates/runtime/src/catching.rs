//! The checked form of any operation of the runtime that may raise an exception: `catching(||
//! op)` runs the operation, and an exception that the runtime raises in it (`throw`) is returned
//! instead of ending the program, to go on as a thrown exception does. The translation writes it
//! where a `catch` or a `finally` may see a failure that has no checked form of its own, such as
//! the `NegativeArraySizeException` of `new int[n]` or what `System.arraycopy` throws.
//!
//! Inside `catching`, `throw` unwinds with the exception, which `catching` takes back; a panic of
//! anything else (a bug of the translation) goes on unwinding as it is.

use crate::Exception;
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

thread_local! {
    /// How many `catching` calls are running on this thread.
    static DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// The payload of the unwinding of an exception that the runtime raises inside `catching`.
struct Raised(Exception);

/// Runs `op`, returning the exception that the runtime raises in it.
pub fn catching<T>(op: impl FnOnce() -> T) -> Result<T, Exception> {
    DEPTH.with(|d| d.set(d.get() + 1));
    let outcome = catch_unwind(AssertUnwindSafe(op));
    DEPTH.with(|d| d.set(d.get() - 1));
    match outcome {
        Ok(value) => Ok(value),
        Err(payload) => match payload.downcast::<Raised>() {
            Ok(raised) => Err(raised.0),
            Err(payload) => resume_unwind(payload),
        },
    }
}

/// Inside `catching`, unwinds to it with the exception of class `class`; outside, returns.
pub(crate) fn raise(class: &str, message: &str) {
    if DEPTH.with(Cell::get) == 0 {
        return;
    }
    // The class is one of the standard classes, named by a literal in the runtime, but `throw`
    // takes any text: the name is kept for the rest of the program, which is rare and small.
    let class: &'static str = Box::leak(class.to_string().into_boxed_str());
    resume_unwind(Box::new(Raised(Exception::new(class, message))))
}

/// `raise` of an exception made already; outside `catching`, it is given back.
pub(crate) fn raise_exception(e: Exception) -> Exception {
    if DEPTH.with(Cell::get) == 0 {
        return e;
    }
    resume_unwind(Box::new(Raised(e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_exception_inside_catching_is_returned_and_a_value_passes() {
        assert_eq!(catching(|| 4), Ok(4));
        let e =
            catching(|| -> i32 { crate::throw("java.lang.NegativeArraySizeException", "-1") }).unwrap_err();
        assert_eq!(e, Exception::new("java.lang.NegativeArraySizeException", "-1"));
    }
}
