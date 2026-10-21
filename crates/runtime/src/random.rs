//! `SecureRandom.getSeed(int)`: the seed bytes come from the operating system's random source,
//! as the JDK's seed generator on Linux reads them (`/dev/urandom`). The bytes differ from run to
//! run, so a program can observe only their number.

use crate::{Array, Failure, from_vec};
use std::fs::File;
use std::io::Read;

/// `SecureRandom.getSeed(numBytes)`, where a negative count fails with Java's
/// `IllegalArgumentException`.
pub fn try_secure_seed(num_bytes: i32) -> Result<Array<i8>, Failure> {
    if num_bytes < 0 {
        return Err(Failure {
            class: "java.lang.IllegalArgumentException",
            message: "numBytes cannot be negative".to_string(),
        });
    }
    let mut bytes = vec![0u8; num_bytes as usize];
    if !bytes.is_empty() {
        let read = File::open("/dev/urandom").and_then(|mut source| source.read_exact(&mut bytes));
        if let Err(e) = read {
            return Err(Failure {
                class: "java.security.ProviderException",
                message: format!("seed generator: {e}"),
            });
        }
    }
    Ok(from_vec(bytes.into_iter().map(|b| b as i8).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds_have_the_requested_length() {
        assert_eq!(try_secure_seed(0).unwrap().borrow().len(), 0);
        assert_eq!(try_secure_seed(16).unwrap().borrow().len(), 16);
    }

    #[test]
    fn a_negative_count_fails() {
        let e = try_secure_seed(-1).unwrap_err();
        assert_eq!(e.class, "java.lang.IllegalArgumentException");
        assert_eq!(e.message, "numBytes cannot be negative");
    }
}
