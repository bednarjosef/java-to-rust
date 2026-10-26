//! `java-to-rust Input.java` prints the Rust translation; `-o out.rs` writes it to a file and
//! `--crate DIR` writes a Cargo project that can be built and run with `cargo run`. A whole
//! program's project has a file per package (`output::Layout::FilePerModule`); printed or
//! written with `-o`, it is one file.

use java_to_rust::output::{Layout, SourceFile};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "usage: java-to-rust [--timings] <Input.java> [-o <output.rs> | --crate <directory>]
       java-to-rust [--timings] --main <package.Class> [--source <root>]... [--library <root>]... [--keep-going]
                    [--named-locals] [--approximate-messages]
                    [<File.java>]... [-o <output.rs> | --crate <directory> [--crates <n>]
                     | --survey [--survey-json <file>]
                     | --census [--census-json <file>] [--save-tree <file>]
                       [-o <output.rs> | --crate <directory>]
                     | --from-tree <file> (-o <output.rs> | --crate <directory> [--crates <n>])]
                    [--modules-json <file>]";

/// The stack that translation runs on: the trees of real code, and the recursion over them, can
/// be deeper than the main thread's stack allows.
const STACK: usize = 1 << 30;

/// glibc's `mallopt` setting of the most arenas that threads allocate from.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
const M_ARENA_MAX: i32 = -8;

#[cfg(all(target_os = "linux", target_env = "gnu"))]
unsafe extern "C" {
    fn mallopt(param: i32, value: i32) -> i32;
}

fn main() -> ExitCode {
    // The analyses run some steps on threads of their own (`analysis::analyze_from`). glibc gives
    // each thread an arena of its own, whose freed memory the others do not reuse: a third more
    // memory on javac's census. One arena keeps it as with one thread.
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    // SAFETY: `mallopt` only sets a parameter of the allocator, before any other thread starts.
    unsafe {
        mallopt(M_ARENA_MAX, 1);
    }
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
/// (`--census-json` also writes it as JSON). With `--keep-going`, javac goes on past compile
/// errors: code whose only errors name classes missing from the roots throws
/// `NoClassDefFoundError` where it runs, as Java's would, and other code with an error is rejected
/// (`Program::keep_going`). With `--approximate-messages`, the message of a `NullPointerException`
/// whose wording is not certain is approximated rather than the code rejected
/// (`Program::approximate_messages`). With `-o` or `--crate`, a census writes its translation too, a trial
/// one in which each rejected piece of code ends the program with the rejection
/// (`Program::ends_rejected`). `--modules-json` writes the modules of the translation (of a
/// census: with every rejected piece stubbed) and what their items name
/// (`output::ModuleTree::dependencies`).
fn run_program(args: &[String]) -> Result<(), String> {
    let mut program = java_to_rust::Program::default();
    let mut output = None;
    let mut survey = false;
    let mut survey_json = None;
    let mut census = false;
    let mut census_json = None;
    let mut modules_json = None;
    let mut crates = None;
    let mut save_tree = None;
    let mut from_tree = None;
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
            "--named-locals" => program.named_locals = true,
            "--approximate-messages" => program.approximate_messages = true,
            "--modules-json" => modules_json = Some(value()?),
            "--save-tree" => save_tree = Some(value()?),
            "--from-tree" => from_tree = Some(value()?),
            "--crates" => crates = Some(value()?.parse::<usize>().map_err(|e| format!("--crates: {e}"))?),
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
    // A census's translation saved with `--save-tree`, written again without taking the census:
    // for a change to how a translation is written or split.
    if let Some(path) = from_tree {
        let Some((flag, out)) = &output else {
            return Err(format!("--from-tree needs -o or --crate\n{USAGE}"));
        };
        let tree = java_to_rust::timings::phase("tree read", || read_tree(&path))?;
        return write_translation(tree, crate_name(&program.main), crates, flag, out);
    }
    if census {
        program.ends_rejected = output.is_some();
        let census = java_to_rust::census_program(&program).map_err(|e| e.to_string())?;
        print!("{}", census.report);
        if let Some(path) = census_json {
            let json = serde_json::to_string_pretty(&census.json).map_err(|e| e.to_string())?;
            std::fs::write(&path, json + "\n").map_err(|e| format!("{path}: {e}"))?;
        }
        use java_to_rust::timings::phase;
        if let (Some(path), Some(tree)) = (modules_json, &census.modules) {
            phase("census: modules json", || write_dependencies(&path, tree))?;
        }
        if let Some((flag, path)) = &output {
            let Some(tree) = census.modules else {
                return Err("the census ended early, so it has no translation to write".to_string());
            };
            // The program is gone by now, but the allocator keeps the memory it had, which the
            // large texts of the write do not reuse: given back, they fit where it was.
            drop((census.report, census.json));
            release_free_memory();
            if let Some(file) = &save_tree {
                java_to_rust::timings::phase("census: tree saved", || save_tree_to(&tree, file))?;
            }
            let name = crate_name(&program.main);
            return phase("census: write", || write_translation(tree, name, crates, flag, path));
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
    match &output {
        Some((flag, path)) => write_translation(tree, crate_name(&program.main), crates, flag, path),
        None => emit(None, &crate_name(&program.main), &tree.files(Layout::OneFile)),
    }
}

/// Writes a whole program's translation with `flag` (`-o` or `--crate`) to `path`: with
/// `--crates`, a project is a workspace split into that many body crates (`split.rs`).
fn write_translation(
    tree: java_to_rust::output::ModuleTree,
    name: String,
    crates: Option<usize>,
    flag: &str,
    path: &str,
) -> Result<(), String> {
    if let (Some(crates), "--crate") = (crates, flag) {
        let workspace = java_to_rust::split::split(tree, crates)?;
        return workspace.write(Path::new(path), &name).map_err(|e| format!("{path}: {e}"));
    }
    emit(Some((flag, path)), &name, &tree.files(layout(flag)))
}

/// Saves a census's translation, before it is written, to `path` (`--save-tree`), for
/// `--from-tree` to write it again.
fn save_tree_to(tree: &java_to_rust::output::ModuleTree, path: &str) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{path}: {e}"))?;
    let mut out = std::io::BufWriter::with_capacity(1 << 20, file);
    bincode::serialize_into(&mut out, tree).map_err(|e| format!("{path}: {e}"))?;
    std::io::Write::flush(&mut out).map_err(|e| format!("{path}: {e}"))
}

/// A translation that `--save-tree` saved to `path`.
fn read_tree(path: &str) -> Result<java_to_rust::output::ModuleTree, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{path}: {e}"))?;
    bincode::deserialize_from(std::io::BufReader::with_capacity(1 << 20, file))
        .map_err(|e| format!("{path}: {e}"))
}

/// Gives the memory that the allocator holds free back to the system (glibc's `malloc_trim`),
/// as `--timings` then shows as what the translator holds now.
fn release_free_memory() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        unsafe extern "C" {
            fn malloc_trim(pad: usize) -> i32;
        }
        // SAFETY: `malloc_trim` only gives back pages that no allocation holds.
        unsafe { malloc_trim(0) };
        java_to_rust::timings::phase("census: memory given back", || ());
    }
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
/// checkout. Its debug builds keep line tables only, which backtraces need: full debug
/// information makes a large program's binary several times larger and adds to its build's
/// memory. Its release builds use fat LTO, which makes the translated javac about 10% faster
/// (docs/performance.md, "The translated javac"). On x86-64, its `.cargo/config.toml` builds for
/// processors with AVX2 (`x86-64-v3`, Haswell and later), whose wider vector instructions a loop
/// over an array uses, as the JVM's compiler uses those of the processor it runs on; Cargo reads
/// it when it runs in the project's directory, and adds flags given as `CARGO_BUILD_RUSTFLAGS` or
/// `--config build.rustflags=..` to it (`RUSTFLAGS` replaces it).
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
             [dependencies]\njava-runtime = {{ path = {:?} }}\n\n\
             [profile.dev]\ndebug = \"line-tables-only\"\nincremental = false\n\n\
             [profile.release]\nlto = \"fat\"\n\n[workspace]\n",
            runtime.display().to_string()
        ),
    )?;
    if cfg!(target_arch = "x86_64") {
        std::fs::create_dir_all(dir.join(".cargo"))?;
        std::fs::write(
            dir.join(".cargo/config.toml"),
            "[build]\nrustflags = [\"-C\", \"target-cpu=x86-64-v3\"]\n",
        )?;
    }
    for file in files {
        let path = dir.join("src").join(&file.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, &file.text)?;
    }
    Ok(())
}
