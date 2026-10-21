//! Whole programs: a Java program spread over many files and packages, translated into one Rust
//! file in which every package is a module (`com.example.util` becomes `mod com { mod example {
//! mod util { .. } } }`). javac compiles all of it in one run, so that names across files resolve,
//! and only what is reachable from `main` is lowered and translated.

mod reach;
mod survey;

pub use reach::{
    CLASS_KINDS, Reach, STRING_CHARS, class_name, component_array, live_branch, unsafe_field_access,
};
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
    use crate::timings::phase;
    let (units, reach) = load(program)?;
    let units: Vec<&Value> = units.iter().collect();
    let trees: Vec<&Value> = units.iter().map(|u| &u["tree"]).collect();
    let mut class =
        phase("lower", || lower::lower_program(&trees, &reach, &program.main)).map_err(|(unit, error)| {
            ProgramError { file: unit.and_then(|u| units[u]["file"].as_str()).map(str::to_string), error }
        })?;
    phase("whole objects", || analysis::whole_objects(&mut class))
        .map_err(|error| ProgramError { file: locate(&units, &class, error.pos), error })?;
    let located = |error: Error| ProgramError { file: locate(&units, &class, error.pos), error };
    let facts = phase("analyze", || analysis::analyze(&class)).map_err(located)?;
    phase("codegen", || codegen::generate(&class, &facts)).map_err(located)
}

/// The units of the program and of the library classes that its reachable code uses, with what
/// that code reaches. javac attributes the program's files, and a library's unit only when
/// reachability names one of its classes: each round reaches from `main` over the units so far
/// and asks javac for those of the classes it named that none of them declares
/// (`Reach::missing`), until it gets no more. What a round reaches only grows with the units, so
/// the last round reaches what a walk over every unit that javac can load would; the units are a
/// few dozen where javac, left to itself, attributes thousands (see `javac::Session`). The
/// program's files come first, in the order given, and then the library's in the order they
/// were asked for, so that the first unsupported construct reported is near the program's code.
fn load(program: &Program) -> Result<(Vec<Value>, Reach), ProgramError> {
    let files = source_files(program)?;
    let javac_error = |(file, error)| ProgramError { file, error };
    let (mut javac, mut reaching) = (std::time::Duration::ZERO, std::time::Duration::ZERO);
    let start = std::time::Instant::now();
    let mut session = javac::Session::start(&files, &sourcepath(program)).map_err(javac_error)?;
    javac += start.elapsed();
    for round in 1.. {
        let start = std::time::Instant::now();
        let reach = {
            let units = session.units();
            let trees: Vec<&Value> = units.iter().map(|u| &u["tree"]).collect();
            reach::reach(&trees, &program.main).map_err(fail)?
        };
        reaching += start.elapsed();
        let start = std::time::Instant::now();
        let loaded = session.load(&reach.missing.iter().cloned().collect::<Vec<_>>()).map_err(javac_error)?;
        javac += start.elapsed();
        if loaded == 0 {
            let units = session.finish();
            if crate::timings::enabled() {
                let (javac, reaching) = (javac.as_secs_f64(), reaching.as_secs_f64());
                let (classes, methods) = (reach.classes.len(), reach.methods.len());
                eprintln!("timing: javac: {javac:.3} s, {} units", units.len());
                eprintln!(
                    "timing: reach: {reaching:.3} s, {round} rounds, {classes} classes and {methods} methods"
                );
            }
            return Ok((units, reach));
        }
    }
    unreachable!("the rounds end when javac has no more units to give")
}

/// The file of the code at `pos` that an analysis or the generator rejected, which the error
/// does not carry: the unit of the classes whose methods have code there, when they agree on one
/// (a line and column of one file may be that of another file's code too).
fn locate(units: &[&Value], class: &crate::ast::Class, pos: Pos) -> Option<String> {
    use crate::ast::{visit_expr, visit_stmt};
    let mut files = std::collections::BTreeSet::new();
    for method in &class.methods {
        let mut here = method.pos == pos;
        let decl = &class.types[method.class];
        let fields = decl.fields.iter().chain(&decl.statics).filter_map(|f| f.init.as_ref());
        fields.for_each(|e| visit_expr(e, &mut |x| here |= x.pos == pos));
        for s in &method.body {
            visit_stmt(s, &mut |e| visit_expr(e, &mut |x| here |= x.pos == pos), &mut |_| {});
        }
        if !here {
            continue;
        }
        // The unit that declares the class, or the class that it is nested in.
        let name = &decl.name;
        let unit = units.iter().find(|u| {
            u["tree"]["typeDecls"].as_array().into_iter().flatten().any(|d| {
                let top = class_name(d);
                name == &top || name.strip_prefix(top.as_str()).is_some_and(|r| r.starts_with(['.', '$']))
            })
        });
        files.insert(unit.and_then(|u| u["file"].as_str())?);
    }
    let mut files = files.into_iter();
    match (files.next(), files.next()) {
        (Some(file), None) => Some(file.to_string()),
        _ => None,
    }
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

/// javac's trees of the program, with those of every library unit that javac loads for it.
fn javac_tree(program: &Program, files: &[PathBuf]) -> Result<Value, ProgramError> {
    javac::program_tree(files, &sourcepath(program)).map_err(|(file, error)| ProgramError { file, error })
}

/// The roots that javac loads classes from: the program's, then the libraries'.
fn sourcepath(program: &Program) -> Vec<PathBuf> {
    program.sources.iter().chain(&program.libraries).cloned().collect()
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
