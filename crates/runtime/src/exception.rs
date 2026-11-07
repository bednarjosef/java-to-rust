//! Exceptions as values. A translated method that can throw returns `Result<T, Throwable>`, where
//! `Throwable` is the program's own enum of its exception classes; what the runtime raises is an
//! [`Exception`], which the program converts into that enum (`From<java::Exception>`) so that `?`
//! propagates it. Where no `catch` or `finally` can see a failure, the runtime's exiting forms
//! (`java::div`, `java::get`, ...) end the program at once, as an uncaught Java exception does.

use std::fmt;

/// Runs `make` out of line, as code that rarely runs: the making of an exception that a `throw`
/// throws (`throw new X(message)`), whose message and object would otherwise make the function
/// that checks a condition too large to inline where it is called.
#[cold]
#[inline(never)]
pub fn cold<T>(make: impl FnOnce() -> T) -> T {
    make()
}

/// Runs `make` out of line as `cold` does, where making the exception can itself throw (its
/// message calls a method that can): `make` gives the exception made (`Ok`) or the one thrown
/// while making it (`Err`), and either is what the `throw` throws, as in Java.
#[cold]
#[inline(never)]
pub fn cold_or_raised<T>(make: impl FnOnce() -> Result<T, T>) -> T {
    match make() {
        Ok(e) | Err(e) => e,
    }
}

/// An exception that the runtime raises: the qualified name of its standard class, such as
/// `java.lang.ArithmeticException`, and its message, behind one pointer (`Raised`).
///
/// One pointer, which is never null, so that a failing path costs its call and nothing else: a
/// `Result<T, Exception>` keeps its tag apart from the pointer, the function that makes the
/// exception returns it in a register, and the `match` after the call knows that it got an
/// `Err`. With the class and the message in place (40 bytes), each check that an inlined
/// `java::try_` function made wrote them to memory, read them back to tell `Ok` from `Err` (the
/// message's niche held the tag) and copied them on to the conversion into `Throwable`, and the
/// path back into the hot code kept its registers saved around the call.
#[derive(Clone, PartialEq, Eq)]
pub struct Exception(Box<Raised>);

/// What an `Exception` holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Raised {
    pub class: &'static str,
    pub message: Option<String>,
}

impl Exception {
    /// Out of line, as every exception is made where a check fails: the allocations and the
    /// message stay out of the function that checks.
    #[cold]
    #[inline(never)]
    pub fn new(class: &'static str, message: impl Into<String>) -> Exception {
        Exception(Box::new(Raised { class, message: Some(message.into()) }))
    }

    /// An exception without a message, whose `getMessage()` is null.
    #[cold]
    #[inline(never)]
    pub fn bare(class: &'static str) -> Exception {
        Exception(Box::new(Raised { class, message: None }))
    }

    /// The message, taken out of the exception.
    pub fn into_message(self) -> Option<String> {
        self.0.message
    }
}

impl std::ops::Deref for Exception {
    type Target = Raised;

    fn deref(&self) -> &Raised {
        &self.0
    }
}

impl std::ops::DerefMut for Exception {
    fn deref_mut(&mut self) -> &mut Raised {
        &mut self.0
    }
}

impl fmt::Debug for Exception {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// `Throwable.toString()`: the class name, then `: ` and the message if there is one.
impl fmt::Display for Exception {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.message {
            Some(message) => write!(f, "{}: {message}", self.class),
            None => f.write_str(self.class),
        }
    }
}

/// Ends the program the way an exception that nothing catches does: Java prints
/// `Exception in thread "main" ` and the exception's stack trace (`uncaught_text`: its
/// `toString()`, then its causes and suppressed exceptions, without frames) to standard error and
/// exits with code 1. Generic in what it
/// "returns", so that it fits anywhere a value is expected: `f().unwrap_or_else(java::uncaught)`.
/// Inside a `try` that catches by unwinding (`java::unwinding`), the exception unwinds to it
/// instead.
pub fn uncaught<T>(e: impl fmt::Display + 'static) -> T {
    die(crate::catching::raise_thrown(e))
}

/// `Object.clone()` of an object of the class with binary name `class`, which is not
/// `Cloneable`: Java throws a `CloneNotSupportedException` whose message is the class's name.
pub fn clone_refused<T>(class: &str) -> T {
    try_clone_refused(class).unwrap_or_else(uncaught)
}

/// `clone_refused` where the program observes the `CloneNotSupportedException`.
pub fn try_clone_refused<T>(class: &str) -> Result<T, Exception> {
    Err(Exception::new("java.lang.CloneNotSupportedException", class))
}

/// `throw e` where nothing catches what is thrown: the program ends as `uncaught` describes (or
/// the exception unwinds to a `try` that catches by unwinding).
pub fn throw_uncaught(e: impl fmt::Display + 'static) -> ! {
    die(crate::catching::raise_thrown(e))
}

///
/// On a thread that the program started, only that thread ends, with the thread's name in the
/// message; the others go on. On the main thread of a program that started threads, the JVM
/// waits for the other non-daemon threads before it exits with 1, and so does this. Once the
/// program has set an uncaught-exception handler, the exception goes to the handlers instead of
/// being printed here (see `thread::dispatch_uncaught`).
pub(crate) fn die(e: impl fmt::Display + 'static) -> ! {
    if crate::thread::dispatch_uncaught(&e) {
        crate::thread::end_dispatched()
    }
    if crate::thread::on_worker() {
        crate::thread::end_worker(e)
    }
    let name =
        if crate::thread::threads_started() { crate::thread::current_record().name() } else { "main".into() };
    crate::out::eprint_text(&format!("Exception in thread \"{name}\" {}\n", crate::uncaught_text(&e)));
    crate::thread::await_threads();
    crate::thread::run_shutdown_hooks();
    crate::drain_stdout();
    std::process::exit(1)
}

/// How a `try` block with a `finally` ended, which the code after the `finally` acts on: it
/// completed normally, or did so with the values of the locals that it set for the code after
/// the `finally` (`Carried`), returned a value, jumped out (a `break` or `continue`, numbered by
/// the translation), or threw. A block that cannot return, jump or carry locals has `Infallible`
/// for `T`, `J` or `N`, so that a `match` on it needs no arm for them.
pub enum Completion<T, E, J = usize, N = std::convert::Infallible> {
    Normal,
    Carried(N),
    Return(T),
    Jump(J),
    Throw(E),
}

/// What a `try` block evaluates to when an exception leaves it: `Err(e)` of a block of
/// `Result`, `Completion::Throw(e)` of a block of `Completion`.
pub trait Thrown<E> {
    fn thrown(e: E) -> Self;
}

impl<T, E> Thrown<E> for Result<T, E> {
    fn thrown(e: E) -> Self {
        Err(e)
    }
}

impl<T, E, J, N> Thrown<E> for Completion<T, E, J, N> {
    fn thrown(e: E) -> Self {
        Completion::Throw(e)
    }
}

/// `?` for the inside of a `try` block: the value of `Ok`, or else the error leaves the block
/// with the given label, converted to the block's error type.
///
/// ```
/// use java_runtime as java;
/// let attempt: Result<(), java::Exception> = 'attempt: {
///     let q = java::or_break!(java::try_div(1, 0), 'attempt);
///     println!("{q}");
///     Ok(())
/// };
/// assert!(attempt.is_err());
/// ```
#[macro_export]
macro_rules! or_break {
    ($e:expr, $label:lifetime) => {
        match $e {
            ::core::result::Result::Ok(value) => value,
            ::core::result::Result::Err(e) => {
                break $label $crate::Thrown::thrown(::core::convert::From::from(e));
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<i32, Exception> {
        crate::try_parse_radix::<i32>(Some(crate::Wtf8::new(s)), 10)
    }

    #[test]
    fn an_exception_is_one_pointer_and_a_result_keeps_its_tag_apart() {
        use std::mem::size_of;
        assert_eq!(size_of::<Exception>(), size_of::<usize>());
        // `Ok(())` is the null pointer; any other `Ok` has a tag of its own, which the code
        // that fails writes as a constant, never a word of what the exception's maker returned.
        assert_eq!(size_of::<Result<(), Exception>>(), size_of::<usize>());
        assert_eq!(size_of::<Result<i32, Exception>>(), 2 * size_of::<usize>());
        assert_eq!(size_of::<Result<Option<std::sync::Arc<u8>>, Exception>>(), 2 * size_of::<usize>());
        let e = Exception::new("java.lang.ArithmeticException", "/ by zero");
        assert_eq!((e.class, e.message.as_deref()), ("java.lang.ArithmeticException", Some("/ by zero")));
        assert_eq!(
            format!("{e:?}"),
            "Raised { class: \"java.lang.ArithmeticException\", message: Some(\"/ by zero\") }"
        );
    }

    #[test]
    fn a_block_of_result_ends_with_the_error() {
        let mut seen = Vec::new();
        for s in ["12", "x", "3"] {
            let attempt: Result<(), Exception> = 'attempt: {
                let n = or_break!(parse(s), 'attempt);
                seen.push(n);
                Ok(())
            };
            if let Err(e) = attempt {
                assert_eq!(e.to_string(), "java.lang.NumberFormatException: For input string: \"x\"");
            }
        }
        assert_eq!(seen, [12, 3]);
    }

    #[test]
    fn a_block_of_completion_ends_with_throw() {
        let completion: Completion<i32, Exception> = 'attempt: {
            let q = or_break!(crate::try_div(7, 0), 'attempt);
            Completion::Return(q)
        };
        assert!(matches!(completion, Completion::Throw(e) if e.class == "java.lang.ArithmeticException"));
    }

    fn message<T: fmt::Debug>(result: Result<T, Exception>) -> String {
        result.unwrap_err().to_string()
    }

    #[test]
    fn the_checked_forms_raise_what_java_throws() {
        let a = crate::from_vec(vec![1, 2, 3]);
        assert_eq!(crate::try_get(&a, 2), Ok(3));
        assert_eq!(
            message(crate::try_get(&a, 3)),
            "java.lang.ArrayIndexOutOfBoundsException: Index 3 out of bounds for length 3"
        );
        assert_eq!(
            message(crate::try_set(&a, -1, 0)),
            "java.lang.ArrayIndexOutOfBoundsException: Index -1 out of bounds for length 3"
        );
        assert_eq!(
            crate::try_update(&a, 0, |v| crate::try_div(v, 0)).unwrap_err().class,
            "java.lang.ArithmeticException"
        );
        assert_eq!(crate::try_post_update(&a, 0, |v| Ok(v + 10)), Ok(1));
        assert_eq!(crate::get(&a, 0), 11);
        assert_eq!(message(crate::try_filled(-2, 0)), "java.lang.NegativeArraySizeException: -2");
        assert_eq!(message(crate::try_check_sizes(&[2, -1])), "java.lang.NegativeArraySizeException: -1");
        let mut v = crate::try_new_vec(2, 0).unwrap();
        assert_eq!(crate::try_put(&mut v, 1, 5), Ok(5));
        assert_eq!(crate::try_at(&v, 1), Ok(5));
        assert!(crate::try_modify(&mut v, 2, Ok).is_err());
        assert_eq!(message(crate::try_rem(1i64, 0)), "java.lang.ArithmeticException: / by zero");
        assert_eq!(crate::try_div(i32::MIN, -1), Ok(i32::MIN));
        assert_eq!(message(crate::try_non_null::<i32>(None)), "java.lang.NullPointerException");
        assert_eq!(
            message(crate::try_substring(crate::Wtf8::new("abc"), 2, 1)),
            "java.lang.StringIndexOutOfBoundsException: Range [2, 1) out of bounds for length 3"
        );
        assert_eq!(
            message(crate::try_substring_from(crate::Wtf8::new("abc"), 5)),
            "java.lang.StringIndexOutOfBoundsException: Range [5, 3) out of bounds for length 3"
        );
        assert_eq!(crate::try_substring_from(crate::Wtf8::new("abc"), 1), Ok(crate::JString::from("bc")));
        assert_eq!(
            message(crate::try_char_at(crate::Wtf8::new("é"), 1)),
            "java.lang.StringIndexOutOfBoundsException: Index 1 out of bounds for length 1"
        );
        assert_eq!(crate::try_ascii_char_at(crate::Wtf8::new("ab"), 1), Ok(b'b' as u16));
        assert_eq!(
            message(crate::try_parse_radix::<i32>(None, 10)),
            "java.lang.NumberFormatException: Cannot parse null string"
        );
        assert_eq!(
            message(crate::try_parse_radix::<i32>(Some(crate::Wtf8::new("3000000000")), 10)),
            "java.lang.NumberFormatException: For input string: \"3000000000\""
        );
        assert_eq!(
            message(crate::try_parse_double(crate::Wtf8::new(" "))),
            "java.lang.NumberFormatException: empty String"
        );
        assert_eq!(
            message(crate::try_parse_float(crate::Wtf8::new(" 1x "))),
            "java.lang.NumberFormatException: For input string: \"1x\""
        );
        assert_eq!(crate::try_parse_double(crate::Wtf8::new("2.5")), Ok(2.5));
    }

    #[test]
    fn exceptions_print_like_java() {
        assert_eq!(Exception::bare("java.lang.RuntimeException").to_string(), "java.lang.RuntimeException");
        assert_eq!(Exception::new("java.lang.Error", "x").to_string(), "java.lang.Error: x");
    }
}
