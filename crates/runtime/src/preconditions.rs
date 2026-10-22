//! `Objects.checkFromToIndex` and `Objects.checkIndex`, as `jdk.internal.util.Preconditions`
//! words their `IndexOutOfBoundsException`s.

use crate::{Exception, uncaught};

const OUT: &str = "java.lang.IndexOutOfBoundsException";

/// `Objects.checkFromToIndex(from, to, length)`: `from..to` is a range of `0..length`.
pub fn check_from_to_index(from: i32, to: i32, length: i32) {
    try_check_from_to_index(from, to, length).unwrap_or_else(uncaught)
}

/// `check_from_to_index` where the program observes the exception.
pub fn try_check_from_to_index(from: i32, to: i32, length: i32) -> Result<(), Exception> {
    if from < 0 || from > to || to > length {
        return Err(Exception::new(OUT, format!("Range [{from}, {to}) out of bounds for length {length}")));
    }
    Ok(())
}

/// `Objects.checkIndex(index, length)`: `index` is in `0..length`.
pub fn check_index(index: i32, length: i32) {
    try_check_index(index, length).unwrap_or_else(uncaught)
}

/// `check_index` where the program observes the exception.
pub fn try_check_index(index: i32, length: i32) -> Result<(), Exception> {
    if index < 0 || index >= length {
        return Err(Exception::new(OUT, format!("Index {index} out of bounds for length {length}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_and_indexes() {
        assert!(try_check_from_to_index(0, 0, 0).is_ok());
        assert!(try_check_from_to_index(1, 3, 3).is_ok());
        let failure = |r: Result<(), Exception>| r.unwrap_err().to_string();
        assert_eq!(
            failure(try_check_from_to_index(3, 2, 5)),
            "java.lang.IndexOutOfBoundsException: Range [3, 2) out of bounds for length 5"
        );
        assert_eq!(
            failure(try_check_from_to_index(-1, 2, 5)),
            "java.lang.IndexOutOfBoundsException: Range [-1, 2) out of bounds for length 5"
        );
        assert!(try_check_index(5, 6).is_ok());
        assert_eq!(
            failure(try_check_index(6, 6)),
            "java.lang.IndexOutOfBoundsException: Index 6 out of bounds for length 6"
        );
        assert_eq!(
            failure(try_check_index(0, 0)),
            "java.lang.IndexOutOfBoundsException: Index 0 out of bounds for length 0"
        );
    }
}
