//! The checked form of any operation of the runtime that may raise an exception: `catching(||
//! op)` runs the operation, and an exception that the runtime raises in it (`throw`) is returned
//! instead of ending the program, to go on as a thrown exception does. The translation writes it
//! where a `catch` or a `finally` may see a failure that has no checked form of its own, such as
//! the `NegativeArraySizeException` of `new int[n]` or what `System.arraycopy` throws.
//!
//! Inside `catching`, `throw` unwinds with the exception, which `catching` takes back; a panic of
//! anything else (a bug of the translation) goes on unwinding as it is.
//!
//! `unwinding(|| body, raised)` is the body of a `try` that catches by unwinding: one with a
//! clause that catches every exception of a kind (`Throwable`, `Exception`, `RuntimeException`,
//! `Error`), whose calls then need no `Result` for it (see `analysis::exceptions`). Inside it, an
//! exception of the program that nothing on the way catches (`uncaught`, `throw_uncaught`) and a
//! failure of the runtime (`throw`) unwind to it, and it returns them as an `Err`, as the body's own
//! exceptions are.

use crate::Exception;
use std::any::Any;
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

thread_local! {
    /// How many `catching` calls are running on this thread.
    static DEPTH: Cell<usize> = const { Cell::new(0) };
    /// How many `unwinding` calls are running on this thread, outside the static initializations
    /// that run inside them (`exceptions_held`).
    static UNWINDING: Cell<usize> = const { Cell::new(0) };
}

/// The payload of the unwinding of an exception that the runtime raises inside `catching`.
struct Raised(Exception);

/// The payload of the unwinding of an exception of the program inside `unwinding`: the program's
/// `Throwable` (or a `java::Exception` that no class of the program stands for), which only the
/// thread that threw it takes back.
struct Thrown(Box<dyn ThrownValue>);

/// What `uncaught` may be given: anything it can show.
trait ThrownValue: Any + std::fmt::Display {}

impl<T: Any + std::fmt::Display> ThrownValue for T {}

// The payload never leaves its thread: `unwinding` on the same thread's stack takes it back.
unsafe impl Send for Thrown {}

/// Runs `op`, the body of a `try` that catches by unwinding, returning what it returns, or the
/// exception that unwound out of it: one of the program (`E`), or a failure of the runtime,
/// made an exception of the program by `raised`. A panic of anything else goes on unwinding.
#[inline]
pub fn unwinding<T, E: 'static>(
    op: impl FnOnce() -> Result<T, E>,
    raised: impl FnOnce(Exception) -> E,
) -> Result<T, E> {
    UNWINDING.with(|d| d.set(d.get() + 1));
    let outcome = catch_unwind(AssertUnwindSafe(op));
    UNWINDING.with(|d| d.set(d.get() - 1));
    match outcome {
        Ok(result) => result,
        Err(payload) => Err(unwound(payload, raised)),
    }
}

/// The exception of what unwound out of an `unwinding` body.
#[cold]
#[inline(never)]
fn unwound<E: 'static>(payload: Box<dyn Any + Send>, raised: impl FnOnce(Exception) -> E) -> E {
    let payload = match payload.downcast::<Raised>() {
        Ok(r) => return raised(r.0),
        Err(payload) => payload,
    };
    let thrown = match payload.downcast::<Thrown>() {
        Ok(thrown) => thrown.0,
        Err(payload) => resume_unwind(payload),
    };
    if (*thrown).type_id() == std::any::TypeId::of::<E>() {
        let any: Box<dyn Any> = thrown;
        return *any.downcast::<E>().expect("of the type tested");
    }
    if (*thrown).type_id() == std::any::TypeId::of::<Exception>() {
        let any: Box<dyn Any> = thrown;
        return raised(*any.downcast::<Exception>().expect("of the type tested"));
    }
    // Something else that ends the program where it is thrown (the text of an internal error).
    crate::exception::die(Shown(thrown))
}

/// A thrown value shown as it is.
struct Shown(Box<dyn ThrownValue>);

impl std::fmt::Display for Shown {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Inside `unwinding`, unwinds to it with the exception `e` of the program; outside, gives it back.
pub(crate) fn raise_thrown<D: std::fmt::Display + 'static>(e: D) -> D {
    if UNWINDING.with(Cell::get) == 0 {
        return e;
    }
    resume_unwind(Box::new(Thrown(Box::new(e))))
}

/// While the value lives, the exceptions on this thread end the program as they do outside
/// `unwinding`: a static initialization runs so, as its failure does not unwind out of it (the
/// class would stay started but not done; a class whose initialization can fail where it is
/// caught keeps its own state, `lower/clinit_errors.rs`).
pub struct ExceptionsHeld(usize);

/// See `ExceptionsHeld`.
pub fn exceptions_held() -> ExceptionsHeld {
    ExceptionsHeld(UNWINDING.with(|d| d.replace(0)))
}

impl Drop for ExceptionsHeld {
    fn drop(&mut self) {
        UNWINDING.with(|d| d.set(self.0));
    }
}

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

/// Inside `catching` or `unwinding`, unwinds to it with the exception of class `class`; outside,
/// returns.
pub(crate) fn raise(class: &str, message: &str) {
    if DEPTH.with(Cell::get) == 0 && UNWINDING.with(Cell::get) == 0 {
        return;
    }
    // The class is one of the standard classes, named by a literal in the runtime, but `throw`
    // takes any text: the name is kept for the rest of the program, which is rare and small.
    let class: &'static str = Box::leak(class.to_string().into_boxed_str());
    resume_unwind(Box::new(Raised(Exception::new(class, message))))
}

/// `raise` of an exception made already; outside `catching` and `unwinding`, it is given back.
pub(crate) fn raise_exception(e: Exception) -> Exception {
    if DEPTH.with(Cell::get) == 0 && UNWINDING.with(Cell::get) == 0 {
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

    #[derive(Debug, PartialEq)]
    struct Mine(&'static str);

    impl std::fmt::Display for Mine {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str(self.0)
        }
    }

    #[test]
    fn unwinding_returns_what_its_body_throws_raises_and_returns() {
        let raised = |e: Exception| Mine(e.class);
        assert_eq!(unwinding(|| Ok::<_, Mine>(3), raised), Ok(3));
        assert_eq!(unwinding(|| Err::<i32, _>(Mine("own")), raised), Err(Mine("own")));
        let deep = || -> Result<i32, Mine> { Ok(crate::uncaught::<i32>(Mine("deep"))) };
        assert_eq!(unwinding(deep, raised), Err(Mine("deep")));
        let failed = || -> Result<i32, Mine> { crate::throw("java.lang.ArithmeticException", "/ by zero") };
        assert_eq!(unwinding(failed, raised), Err(Mine("java.lang.ArithmeticException")));
        // The inner one takes what is thrown inside it; the outer one what the inner passes on.
        let nested = || -> Result<i32, Mine> {
            let inner = unwinding(|| -> Result<i32, Mine> { crate::throw_uncaught(Mine("inner")) }, raised);
            assert_eq!(inner, Err(Mine("inner")));
            crate::throw_uncaught(Mine("outer"))
        };
        assert_eq!(unwinding(nested, raised), Err(Mine("outer")));
        assert_eq!(UNWINDING.with(Cell::get), 0);
    }

    #[test]
    fn a_held_scope_counts_no_unwinding_and_restores_it() {
        let seen = unwinding(
            || -> Result<(usize, usize), Mine> {
                let held = {
                    let _held = exceptions_held();
                    UNWINDING.with(Cell::get)
                };
                Ok((held, UNWINDING.with(Cell::get)))
            },
            |e| Mine(e.class),
        );
        assert_eq!(seen, Ok((0, 1)));
    }
}
