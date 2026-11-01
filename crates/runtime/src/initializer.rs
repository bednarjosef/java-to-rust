//! The end of a program whose static initializer throws. Java wraps what the initializer throws in
//! `ExceptionInInitializerError`, and the thread that initialized the class dies with that.

use std::fmt;

/// `throw e` in a static initializer that nothing catches: the program ends as Java's does.
///
/// Inside a `try` that catches by unwinding (`java::unwinding`), `e` unwinds to it instead: only
/// in an initializer that keeps its class's state and catches what its body throws itself
/// (`lower/clinit_errors.rs`), as the others hold the exceptions (`java::exceptions_held`).
pub fn throw_initializer(e: impl fmt::Display + 'static) -> ! {
    let e = crate::catching::raise_thrown(e);
    crate::out::eprint_text(&format!(
        "Exception in thread \"main\" java.lang.ExceptionInInitializerError\nCaused by: {}\n",
        crate::uncaught_text(&e)
    ));
    crate::thread::run_shutdown_hooks();
    crate::drain_stdout();
    std::process::exit(1)
}

/// The `uncaught` of a static initializer, for a failure of the runtime in an expression.
pub fn initializer_uncaught<T>(e: impl fmt::Display + 'static) -> T {
    throw_initializer(e)
}
