//! Translates Java source code into Rust source code: parse with tree-sitter, lower to a typed
//! syntax tree, check types, prove facts about the program, generate Rust.

pub mod analysis;
pub mod ast;
pub mod check;
pub mod codegen;
pub mod parse;

pub use ast::Error;

/// Translates one Java source file into the source of a Rust program that uses the
/// `java-runtime` crate.
pub fn translate(java: &str) -> Result<String, Error> {
    let mut class = parse::parse(java)?;
    check::check(&mut class)?;
    let facts = analysis::analyze(&class);
    Ok(codegen::generate(&class, &facts))
}
