//! Translates Java source code into Rust source code: compile with javac, which checks the
//! program and types it, lower javac's tree to the typed syntax tree of `ast`, prove facts about
//! the program, generate Rust.

pub mod analysis;
pub mod ast;
pub mod check_cache;
pub mod codegen;
pub mod javac;
pub mod lower;
pub mod output;
pub mod program;
pub mod timings;
pub mod types;
pub mod value;

pub use ast::Error;
pub use program::{
    Program, ProgramError, census_program, survey_program, translate_program, translate_program_modules,
};

/// Translates one Java source file into the source of a Rust program that uses the
/// `java-runtime` crate.
pub fn translate(java: &str) -> Result<String, Error> {
    let (rust, notes) = translate_with_notes(java)?;
    for note in notes {
        eprintln!("note: {}:{}: {}", note.pos.line, note.pos.column, note.message);
    }
    Ok(rust)
}

/// `translate`, with what the translation assumed (`ast::Note`) rather than printed.
pub fn translate_with_notes(java: &str) -> Result<(String, Vec<ast::Note>), Error> {
    let mut class = lower::lower(&javac::tree(java)?)?;
    let notes = std::mem::take(&mut class.notes);
    analysis::string_identity(&mut class).map_err(|mut errors| errors.remove(0))?;
    analysis::whole_objects(&mut class)?;
    let mut facts = analysis::analyze(&class)?;
    // A `catch` reads the message of a `NullPointerException`: it names the class file's code.
    if facts.npe_message_read.is_some() {
        analysis::npe_messages(&class, &mut facts, &javac::source_names(java)?)?;
    }
    Ok((codegen::generate(&class, &facts)?, notes))
}
