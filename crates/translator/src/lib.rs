//! Translates Java source code into Rust source code: parse with tree-sitter, lower to a typed
//! syntax tree, check types, generate Rust.

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
    Ok(codegen::generate(&class))
}
