//! Whole programs: a Java program spread over many files and packages, translated into one Rust
//! file in which every package is a module (`com.example.util` becomes `mod com { mod example {
//! mod util { .. } } }`). javac compiles all of it in one run, so that names across files resolve,
//! and only what is reachable from `main` is lowered and translated.

mod reach;
mod survey;

pub use reach::{Reach, class_name, never_true};
pub use survey::Survey;

use crate::ast::{Error, Pos};
use crate::{analysis, codegen, javac, lower};
use serde_json::Value;
use std::fmt;
use std::path::{Path, PathBuf};

/// The sources of a program and its entry point.
#[derive(Clone, Debug, Default)]
pub struct Program {
    /// The qualified name of the class whose `main` starts the program, such as
    /// `com.example.Main`.
    pub main: String,
    /// Source roots, laid out by package (`src/com/example/Main.java`): every `.java` file under
    /// them is compiled.
    pub sources: Vec<PathBuf>,
    /// Single source files, compiled with those under the source roots.
    pub files: Vec<PathBuf>,
    /// Library roots, such as the JDK's own sources: javac reads a file from them only when the
    /// program uses one of its classes, and only what is reachable from `main` is translated.
    pub libraries: Vec<PathBuf>,
}

/// A translation error with the file it is in, when it is in one.
#[derive(Debug)]
pub struct ProgramError {
    pub file: Option<String>,
    pub error: Error,
}

impl fmt::Display for ProgramError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.file {
            Some(file) => write!(f, "{file}:{}", self.error),
            None => write!(f, "{}", self.error.message),
        }
    }
}

impl std::error::Error for ProgramError {}

fn fail(message: String) -> ProgramError {
    ProgramError { file: None, error: Error { pos: Pos::default(), message } }
}

/// Translates a whole program into the source of one Rust program that uses the `java-runtime`
/// crate.
pub fn translate_program(program: &Program) -> Result<String, ProgramError> {
    let tree = compile(program)?;
    let units: Vec<&Value> = tree["units"].as_array().into_iter().flatten().collect();
    let trees: Vec<&Value> = units.iter().map(|u| &u["tree"]).collect();
    let reach = reach::reach(&trees, &program.main).map_err(fail)?;
    let class = lower::lower_program(&trees, &reach, &program.main).map_err(|(unit, error)| {
        ProgramError { file: unit.and_then(|u| units[u]["file"].as_str()).map(str::to_string), error }
    })?;
    let facts = analysis::analyze(&class);
    Ok(codegen::generate(&class, &facts))
}

/// The survey of what the code reachable from the program's `main` uses that the translator does
/// not support yet, as a text report and as JSON (see `survey`).
pub fn survey_program(program: &Program) -> Result<Survey, ProgramError> {
    let files = source_files(program)?;
    let tree = javac_tree(program, &files)?;
    let units: Vec<&Value> = tree["units"].as_array().into_iter().flatten().collect();
    let trees: Vec<&Value> = units.iter().map(|u| &u["tree"]).collect();
    let reach = reach::reach(&trees, &program.main).map_err(fail)?;
    Ok(survey::survey(&units, &reach, program, &files))
}

/// javac's trees of the program, with those of the library classes it uses.
fn compile(program: &Program) -> Result<Value, ProgramError> {
    javac_tree(program, &source_files(program)?)
}

fn javac_tree(program: &Program, files: &[PathBuf]) -> Result<Value, ProgramError> {
    let sourcepath: Vec<PathBuf> = program.sources.iter().chain(&program.libraries).cloned().collect();
    javac::program_tree(files, &sourcepath).map_err(|(file, error)| ProgramError { file, error })
}

/// The program's own files: the given files and every `.java` file under the source roots, sorted
/// and without duplicates.
fn source_files(program: &Program) -> Result<Vec<PathBuf>, ProgramError> {
    let mut files = program.files.clone();
    for root in &program.sources {
        java_files(root, &mut files).map_err(|e| fail(format!("{}: {e}", root.display())))?;
    }
    files.sort();
    files.dedup();
    if files.is_empty() {
        return Err(fail("no Java source files given".into()));
    }
    Ok(files)
}

/// Adds every `.java` file under `dir`, in a stable order.
fn java_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let mut entries: Vec<PathBuf> =
        std::fs::read_dir(dir)?.map(|e| e.map(|e| e.path())).collect::<Result<_, _>>()?;
    entries.sort();
    for path in entries {
        if path.is_dir() {
            java_files(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "java") {
            out.push(path);
        }
    }
    Ok(())
}
