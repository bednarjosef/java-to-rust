//! `java-to-rust Input.java` prints the Rust translation; `-o out.rs` writes it to a file and
//! `--crate DIR` writes a Cargo project that can be built and run with `cargo run`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "usage: java-to-rust [--timings] <Input.java> [-o <output.rs> | --crate <directory>]
       java-to-rust [--timings] --main <package.Class> [--source <root>]... [--library <root>]... [<File.java>]...
                    [-o <output.rs> | --crate <directory> | --survey [--survey-json <file>]]";

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
    if args
        .iter()
        .any(|a| matches!(a.as_str(), "--main" | "--source" | "--library" | "--survey" | "--survey-json"))
    {
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
    emit(output, input, &rust)
}

/// Prints the translation, or writes it to a file (`-o`) or a Cargo project (`--crate`) named
/// after `input`.
fn emit(output: Option<(&str, &str)>, input: &str, rust: &str) -> Result<(), String> {
    match output {
        None => print!("{rust}"),
        Some(("-o", path)) => std::fs::write(path, rust).map_err(|e| format!("{path}: {e}"))?,
        Some((_, dir)) => write_crate(Path::new(dir), input, rust).map_err(|e| format!("{dir}: {e}"))?,
    }
    Ok(())
}

/// Whole-program mode: `--main` names the class whose `main` starts the program, each
/// `--source` is a source root laid out by package, each `--library` a root that classes are
/// only read from when the program uses them, and other arguments are single source files.
/// `--survey` prints what the reachable code uses that is not supported yet instead, and
/// `--survey-json` also writes that census as JSON to a file.
fn run_program(args: &[String]) -> Result<(), String> {
    let mut program = java_to_rust::Program::default();
    let mut output = None;
    let mut survey = false;
    let mut survey_json = None;
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
            flag if flag.starts_with('-') => return Err(format!("unknown option {flag}\n{USAGE}")),
            file => program.files.push(file.into()),
        }
    }
    if program.main.is_empty() {
        return Err(format!("--main is required for a program\n{USAGE}"));
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
    let rust = java_to_rust::translate_program(&program).map_err(|e| e.to_string())?;
    let name = program.main.rsplit('.').next().unwrap_or("translated").to_string();
    emit(output.as_ref().map(|(flag, path)| (*flag, path.as_str())), &name, &rust)
}

/// A Cargo project around the translation, depending on the runtime crate of this checkout.
fn write_crate(dir: &Path, input: &str, rust: &str) -> std::io::Result<()> {
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
    std::fs::write(dir.join("src/main.rs"), rust)
}
