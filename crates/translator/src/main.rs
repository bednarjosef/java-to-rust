//! `java-to-rust Input.java` prints the Rust translation; `-o out.rs` writes it to a file and
//! `--crate DIR` writes a Cargo project that can be built and run with `cargo run`. A whole
//! program's project has a file per package (`output::Layout::FilePerModule`); printed or
//! written with `-o`, it is one file.

use java_to_rust::output::{Layout, SourceFile};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "usage: java-to-rust [--timings] <Input.java> [-o <output.rs> | --crate <directory>]
       java-to-rust [--timings] --main <package.Class> [--source <root>]... [--library <root>]... [<File.java>]...
                    [-o <output.rs> | --crate <directory> | --survey [--survey-json <file>]
                     | --census [--census-json <file>] [--keep-going] [-o <output.rs> | --crate <directory>]]
                    [--modules-json <file>]";

/// The stack that translation runs on: the trees of real code, and the recursion over them, can
/// be deeper than the main thread's stack allows.
const STACK: usize = 1 << 30;

fn main() -> ExitCode {
    let args = std::env::args().skip(1).collect();
    let translation = std::thread::Builder::new().stack_size(STACK).spawn(move || run(args));
    let result = translation
        .map_err(|e| e.to_string())
        .and_then(|t| t.join().map_err(|_| "translation panicked".to_string()));
    match result.and_then(|r| r) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run(mut args: Vec<String>) -> Result<(), String> {
    // `--timings` prints the time of each phase to standard error.
    if let Some(at) = args.iter().position(|a| a == "--timings") {
        args.remove(at);
        java_to_rust::timings::enable();
    }
    if args.iter().any(|a| {
        matches!(
            a.as_str(),
            "--main"
                | "--source"
                | "--library"
                | "--survey"
                | "--survey-json"
                | "--census"
                | "--census-json"
                | "--keep-going"
                | "--modules-json"
        )
    }) {
        return run_program(&args);
    }
    let (input, output) = match &args[..] {
        [input] => (input, None),
        [input, flag, out] if flag == "-o" || flag == "--crate" => {
            (input, Some((flag.as_str(), out.as_str())))
        }
        _ => return Err(USAGE.into()),
    };
    let java = std::fs::read_to_string(input).map_err(|e| format!("{input}: {e}"))?;
    let rust = java_to_rust::translate(&java).map_err(|e| format!("{input}:{e}"))?;
    emit(output, input, &[SourceFile { path: "main.rs".into(), text: rust }])
}

/// Prints the translation, or writes it to a file (`-o`), or its files to a Cargo project
/// (`--crate`) named after `input`. Printed or written to one file, `files` is one file.
fn emit(output: Option<(&str, &str)>, input: &str, files: &[SourceFile]) -> Result<(), String> {
    match output {
        None => print!("{}", files[0].text),
        Some(("-o", path)) => std::fs::write(path, &files[0].text).map_err(|e| format!("{path}: {e}"))?,
        Some((_, dir)) => write_crate(Path::new(dir), input, files).map_err(|e| format!("{dir}: {e}"))?,
    }
    Ok(())
}

/// Whole-program mode: `--main` names the class whose `main` starts the program, each
/// `--source` is a source root laid out by package, each `--library` a root that classes are
/// only read from when the program uses them, and other arguments are single source files.
/// `--survey` prints what the reachable code uses instead (`--survey-json` also writes it as JSON
/// to a file), and `--census` what the translator rejects in it, past every rejection
/// (`--census-json` also writes it as JSON); with `--keep-going`, a census goes on past javac's
/// compile errors, and rejects the code that they are in; with `-o` or `--crate`, a census writes its
/// translation too, a trial one in which each rejected piece of code ends the program with the
/// rejection (`Program::ends_rejected`). `--modules-json` writes the modules of the translation (of
/// a census: with every rejected piece stubbed) and what their items name
/// (`output::ModuleTree::dependencies`).
fn run_program(args: &[String]) -> Result<(), String> {
    let mut program = java_to_rust::Program::default();
    let mut output = None;
    let mut survey = false;
    let mut survey_json = None;
    let mut census = false;
    let mut census_json = None;
    let mut modules_json = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let mut value = || args.next().cloned().ok_or_else(|| format!("{arg} needs a value\n{USAGE}"));
        match arg.as_str() {
            "--main" => program.main = value()?,
            "--source" => program.sources.push(value()?.into()),
            "--library" => program.libraries.push(value()?.into()),
            "-o" | "--crate" => output = Some((arg.as_str(), value()?)),
            "--survey" => survey = true,
            "--survey-json" => {
                survey = true;
                survey_json = Some(value()?);
            }
            "--census" => census = true,
            "--keep-going" => program.keep_going = true,
            "--modules-json" => modules_json = Some(value()?),
            "--census-json" => {
                census = true;
                census_json = Some(value()?);
            }
            flag if flag.starts_with('-') => return Err(format!("unknown option {flag}\n{USAGE}")),
            file => program.files.push(file.into()),
        }
    }
    if program.main.is_empty() {
        return Err(format!("--main is required for a program\n{USAGE}"));
    }
    if program.keep_going && !census {
        return Err(format!("--keep-going is only for a census\n{USAGE}"));
    }
    if census {
        program.ends_rejected = output.is_some();
        let census = java_to_rust::census_program(&program).map_err(|e| e.to_string())?;
        print!("{}", census.report);
        if let Some(path) = census_json {
            let json = serde_json::to_string_pretty(&census.json).map_err(|e| e.to_string())?;
            std::fs::write(&path, json + "\n").map_err(|e| format!("{path}: {e}"))?;
        }
        if let (Some(path), Some(tree)) = (modules_json, &census.modules) {
            write_dependencies(&path, tree)?;
        }
        if let Some((flag, path)) = &output {
            let Some(tree) = &census.modules else {
                return Err("the census ended early, so it has no translation to write".to_string());
            };
            emit(Some((flag, path.as_str())), &crate_name(&program.main), &tree.files(layout(flag)))?;
        }
        return Ok(());
    }
    if survey {
        let census = java_to_rust::survey_program(&program).map_err(|e| e.to_string())?;
        print!("{}", census.report);
        if let Some(path) = survey_json {
            let json = serde_json::to_string_pretty(&census.json).map_err(|e| e.to_string())?;
            std::fs::write(&path, json + "\n").map_err(|e| format!("{path}: {e}"))?;
        }
        return Ok(());
    }
    let tree = java_to_rust::translate_program_modules(&program).map_err(|e| e.to_string())?;
    if let Some(path) = modules_json {
        write_dependencies(&path, &tree)?;
    }
    let name = crate_name(&program.main);
    let layout = output.as_ref().map_or(Layout::OneFile, |(flag, _)| layout(flag));
    emit(output.as_ref().map(|(flag, path)| (*flag, path.as_str())), &name, &tree.files(layout))
}

/// The name of a whole program's project: its main class's simple name.
fn crate_name(main: &str) -> String {
    main.rsplit('.').next().unwrap_or("translated").to_string()
}

/// A whole program's project (`--crate`) has a file per package; one file (`-o`) is one file.
fn layout(flag: &str) -> Layout {
    if flag == "--crate" { Layout::FilePerModule } else { Layout::OneFile }
}

/// Writes the modules of a translation and what their items name, as JSON, to `path`.
fn write_dependencies(path: &str, tree: &java_to_rust::output::ModuleTree) -> Result<(), String> {
    let json = serde_json::to_string(&tree.dependencies()).map_err(|e| e.to_string())?;
    std::fs::write(path, json + "\n").map_err(|e| format!("{path}: {e}"))
}

/// A Cargo project around the translation's files, depending on the runtime crate of this
/// checkout.
fn write_crate(dir: &Path, input: &str, files: &[SourceFile]) -> std::io::Result<()> {
    let name = Path::new(input)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("translated")
        .to_lowercase()
        .replace(|c: char| !c.is_ascii_alphanumeric(), "-");
    let runtime: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../runtime").canonicalize()?;
    std::fs::create_dir_all(dir.join("src"))?;
    std::fs::write(
        dir.join("Cargo.toml"),
        format!(
            "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n\
             [dependencies]\njava-runtime = {{ path = {:?} }}\n\n[workspace]\n",
            runtime.display().to_string()
        ),
    )?;
    for file in files {
        let path = dir.join("src").join(&file.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, &file.text)?;
    }
    Ok(())
}
