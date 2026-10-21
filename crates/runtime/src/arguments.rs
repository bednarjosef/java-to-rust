//! The command-line arguments of the program, which `main(String[] args)` gets: every argument
//! after the program's own name. An argument that is not valid UTF-8 has each invalid byte made
//! into U+FFFD, as the JVM decodes it.

/// The arguments of the program, as strings, in order.
pub fn main_args() -> Vec<String> {
    std::env::args_os().skip(1).map(|arg| arg.to_string_lossy().into_owned()).collect()
}
