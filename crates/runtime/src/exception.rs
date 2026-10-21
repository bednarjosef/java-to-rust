//! Exceptions as values. A translated method that can throw returns `Result<T, Throwable>`, where
//! `Throwable` is the program's own enum of its exception classes; what the runtime raises is an
//! [`Exception`], which the program converts into that enum (`From<java::Exception>`) so that `?`
//! propagates it. Where no `catch` or `finally` can see a failure, the runtime's exiting forms
//! (`java::div`, `java::get`, ...) end the program at once, as an uncaught Java exception does.

use std::fmt;

/// An exception that the runtime raises: the qualified name of its standard class, such as
/// `java.lang.ArithmeticException`, and its message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Exception {
    pub class: &'static str,
    pub message: Option<String>,
}

impl Exception {
    pub fn new(class: &'static str, message: impl Into<String>) -> Exception {
        Exception { class, message: Some(message.into()) }
    }

    /// An exception without a message, whose `getMessage()` is null.
    pub fn bare(class: &'static str) -> Exception {
        Exception { class, message: None }
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
/// `Exception in thread "main" ` and the exception's `toString()` (then its stack, which a
/// translation does not have) to standard error and exits with code 1. Generic in what it
/// "returns", so that it fits anywhere a value is expected: `f().unwrap_or_else(java::uncaught)`.
pub fn uncaught<T>(e: impl fmt::Display) -> T {
    die(e)
}

/// `throw e` where nothing catches what is thrown: the program ends as `uncaught` describes.
pub fn throw_uncaught(e: impl fmt::Display) -> ! {
    die(e)
}

pub(crate) fn die(e: impl fmt::Display) -> ! {
    crate::flush();
    eprintln!("Exception in thread \"main\" {e}");
    std::process::exit(1)
}

/// How a `try` block with a `finally` ended, which the code after the `finally` acts on: it
/// completed normally, returned a value, jumped out (a `break` or `continue`, numbered by the
/// translation), or threw. A block that cannot return or jump has `Infallible` for `T` or `J`,
/// so that a `match` on it needs no arm for them.
pub enum Completion<T, E, J = usize> {
    Normal,
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

impl<T, E, J> Thrown<E> for Completion<T, E, J> {
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
        crate::try_parse_number::<i32>(s)
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
            message(crate::try_substring("abc", 2, 1)),
            "java.lang.StringIndexOutOfBoundsException: Range [2, 1) out of bounds for length 3"
        );
        assert_eq!(
            message(crate::try_substring_from("abc", 5)),
            "java.lang.StringIndexOutOfBoundsException: Range [5, 3) out of bounds for length 3"
        );
        assert_eq!(crate::try_substring_from("abc", 1), Ok("bc".to_string()));
        assert_eq!(
            message(crate::try_char_at("é", 1)),
            "java.lang.StringIndexOutOfBoundsException: Index 1 out of bounds for length 1"
        );
        assert_eq!(crate::try_ascii_char_at("ab", 1), Ok(b'b' as u16));
        assert_eq!(
            message(crate::try_parse_radix::<i32>(None, 10)),
            "java.lang.NumberFormatException: Cannot parse null string: null"
        );
        assert_eq!(
            message(crate::try_parse_radix::<i32>(Some("3000000000"), 10)),
            "java.lang.NumberFormatException: For input string: \"3000000000\""
        );
        assert_eq!(message(crate::try_parse_double(" ")), "java.lang.NumberFormatException: empty String");
        assert_eq!(
            message(crate::try_parse_float(" 1x ")),
            "java.lang.NumberFormatException: For input string: \"1x\""
        );
        assert_eq!(crate::try_parse_double("2.5"), Ok(2.5));
    }

    #[test]
    fn exceptions_print_like_java() {
        assert_eq!(Exception::bare("java.lang.RuntimeException").to_string(), "java.lang.RuntimeException");
        assert_eq!(Exception::new("java.lang.Error", "x").to_string(), "java.lang.Error: x");
    }
}
