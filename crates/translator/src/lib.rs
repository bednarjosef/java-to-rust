//! Translates Java source code into Rust source code: compile with javac, which checks the
//! program and types it, lower javac's tree to the typed syntax tree of `ast`, prove facts about
//! the program, generate Rust.

pub mod analysis;
pub mod ast;
pub mod codegen;
pub mod javac;
pub mod lower;
pub mod program;
pub mod types;

pub use ast::Error;
pub use program::{Program, ProgramError, survey_program, translate_program};

/// Translates one Java source file into the source of a Rust program that uses the
/// `java-runtime` crate.
pub fn translate(java: &str) -> Result<String, Error> {
    let class = lower::lower(&javac::tree(java)?)?;
    let facts = analysis::analyze(&class)?;
    Ok(codegen::generate(&class, &facts))
}
