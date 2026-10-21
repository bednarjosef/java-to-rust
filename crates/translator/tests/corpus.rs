//! Runs every Java program in `corpus/` on Java and, translated, as Rust, and requires the two
//! runs to print the same standard output and exit with the same code. Java's run is the
//! expected result, so no expectations are written by hand.
//!
//! Translation, the Java runs, the Cargo build and the Rust runs each use one thread per CPU.
//! The Rust programs are one Cargo project with a binary per program, kept in `target/tmp`
//! between runs: a source is rewritten only when its translation changes, so cargo rebuilds
//! just the programs whose translation changed. Java's results are kept the same way, in
//! `java_cache`, so a program runs on Java again only when its sources or the Java version change.
//! `CORPUS_JAVA_FRESH=1` runs every program on Java. `CORPUS_FILTER=text` runs only the programs
//! whose name contains `text`.

#[path = "corpus/java_cache.rs"]
mod java_cache;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use java_cache::JavaCache;

/// The JVM arguments of every Java run: short-lived JVMs skip the optimizing compiler, the parallel
/// collector and perf data (whose lock-file warnings go to stdout when many JVMs run at once).
/// The output does not change.
const JVM_ARGS: [&str; 3] = ["-XX:TieredStopAtLevel=1", "-XX:+UseSerialGC", "-XX:-UsePerfData"];

struct Program {
    /// The binary name, such as `features-hello`.
    name: String,
    /// The source file, or for a project the source root.
    java: PathBuf,
    /// For a project, the qualified name of its main class.
    main: Option<String>,
    /// The command-line arguments both runs get: the lines of `Name.args` next to a single file.
    args: Vec<String>,
}

#[derive(PartialEq)]
struct Run {
    stdout: String,
    code: Option<i32>,
}

/// Why a program fails: a summary for the table, and the details printed after it.
struct Failure {
    summary: String,
    details: String,
}

/// What cargo reported for the binaries of one build.
#[derive(Default)]
struct Build {
    /// The binary of each program that compiled, by program name.
    binaries: HashMap<String, PathBuf>,
    /// The rustc diagnostics with a source location, by program: `(is an error, text)`.
    diagnostics: HashMap<String, Vec<(bool, String)>>,
    /// Cargo's standard error, shown for a program that failed without a located diagnostic.
    stderr: String,
}

fn run(output: Output) -> Run {
    Run { stdout: String::from_utf8_lossy(&output.stdout).into_owned(), code: output.status.code() }
}

/// Calls `f` on each index below `n` on one thread per CPU (or `CORPUS_THREADS` threads, to keep
/// memory down when several test runs share a machine), and returns the results in order.
fn parallel<R: Send>(n: usize, f: impl Fn(usize) -> R + Sync) -> Vec<R> {
    let threads = std::env::var("CORPUS_THREADS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&n: &usize| n > 0)
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()));
    let next = AtomicUsize::new(0);
    let results: Vec<Mutex<Option<R>>> = (0..n).map(|_| Mutex::new(None)).collect();
    std::thread::scope(|s| {
        for _ in 0..threads.min(n) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= n {
                        break;
                    }
                    *results[i].lock().unwrap() = Some(f(i));
                }
            });
        }
    });
    results.into_iter().map(|r| r.into_inner().unwrap().expect("every index is run")).collect()
}

/// Writes `text` to `path` unless the file already holds it, so that cargo does not rebuild a
/// program whose translation did not change.
fn write_if_changed(path: &Path, text: &str) {
    if std::fs::read_to_string(path).is_ok_and(|old| old == text) {
        return;
    }
    std::fs::write(path, text).unwrap();
}

fn programs(root: &Path) -> Vec<Program> {
    let mut out = Vec::new();
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root).unwrap().map(|e| e.unwrap().path()).collect();
    dirs.sort();
    for dir in dirs.into_iter().filter(|d| d.is_dir() && !d.ends_with("whole-program")) {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "java"))
            .collect();
        files.sort();
        for java in files {
            let stem = java.file_stem().unwrap().to_string_lossy().to_lowercase();
            let group = dir.file_name().unwrap().to_string_lossy().to_string();
            let args = read_args(&java);
            out.push(Program { name: format!("{group}-{stem}"), java, main: None, args });
        }
    }
    out
}

/// The command-line arguments of a single file: one per line of the `.args` file next to it, if
/// there is one.
fn read_args(java: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(java.with_extension("args")) else { return Vec::new() };
    text.lines().map(str::to_string).collect()
}

/// The multi-file programs: every directory of `projects/` with a source root `src/` laid out by
/// package and a file `main` that names the main class, such as `com.example.Main`.
fn projects(root: &Path) -> Vec<Program> {
    let Ok(entries) = std::fs::read_dir(root) else { return Vec::new() };
    let mut dirs: Vec<PathBuf> = entries.map(|e| e.unwrap().path()).filter(|d| d.is_dir()).collect();
    dirs.sort();
    dirs.into_iter()
        .map(|dir| {
            let main = std::fs::read_to_string(dir.join("main")).unwrap().trim().to_string();
            let name = format!("projects-{}", dir.file_name().unwrap().to_string_lossy());
            Program { name, java: dir.join("src"), main: Some(main), args: Vec::new() }
        })
        .collect()
}

/// Runs a program on Java: a single file with the source launcher, a project compiled with javac
/// into `classes/<name>` first.
fn run_java(p: &Program, classes: &Path) -> Output {
    let java = || {
        let mut java = Command::new("java");
        java.args(JVM_ARGS);
        java
    };
    let Some(main) = &p.main else {
        return java()
            .arg(&p.java)
            .args(&p.args)
            .output()
            .expect("running `java` failed; the corpus test needs a JDK on PATH");
    };
    let out = classes.join(&p.name);
    let _ = std::fs::remove_dir_all(&out);
    let mut sources = Vec::new();
    java_files(&p.java, &mut sources);
    let javac =
        Command::new("javac").arg("-d").arg(&out).args(&sources).output().expect("running `javac` failed");
    assert!(javac.status.success(), "javac failed on {}: {}", p.name, String::from_utf8_lossy(&javac.stderr));
    java().arg("-cp").arg(&out).arg(main).output().expect("running `java` failed")
}

fn java_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            java_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "java") {
            out.push(path);
        }
    }
}

/// The Rust source of a program, or why it does not translate.
fn rust_source(p: &Program) -> Result<String, String> {
    let translated: Result<String, String> = match &p.main {
        None => {
            java_to_rust::translate(&std::fs::read_to_string(&p.java).unwrap()).map_err(|e| e.to_string())
        }
        Some(main) => {
            let program = java_to_rust::Program {
                main: main.clone(),
                sources: vec![p.java.clone()],
                ..Default::default()
            };
            java_to_rust::translate_program(&program).map_err(|e| e.to_string())
        }
    };
    translated.map_err(|e| format!("translation failed: {e}"))
}

/// Builds the binaries of `names` with one cargo call, which compiles the runtime once and the
/// programs in parallel. A program with no binary in the messages did not compile: cargo leaves
/// an earlier binary on disk, so the build's messages decide, not the files.
fn build_binaries(work: &Path, names: &[&str]) -> Build {
    let mut build = Build::default();
    if names.is_empty() {
        return build;
    }
    let mut command = Command::new("cargo");
    command
        .args(["build", "--keep-going", "--message-format=json", "--target-dir"])
        .arg(work.join("target"))
        .current_dir(work)
        .env_remove("RUSTFLAGS");
    for name in names {
        command.args(["--bin", name]);
    }
    let output = command.output().expect("running `cargo` failed");
    build.stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Ok(message) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        let target = message["target"]["name"].as_str().unwrap_or_default().to_string();
        match message["reason"].as_str() {
            Some("compiler-artifact") if message["target"]["kind"][0] == "bin" => {
                if let Some(path) = message["executable"].as_str() {
                    build.binaries.insert(target, PathBuf::from(path));
                }
            }
            Some("compiler-message") => {
                let diagnostic = &message["message"];
                let spans = diagnostic["spans"].as_array().map_or(&[][..], Vec::as_slice);
                let Some(span) = spans.iter().find(|s| s["is_primary"] == true) else { continue };
                let level = diagnostic["level"].as_str().unwrap_or_default();
                let place = format!(
                    "{}:{}:{}",
                    span["file_name"].as_str().unwrap_or_default(),
                    span["line_start"],
                    span["column_start"]
                );
                let text =
                    format!("{place}: {level}: {}", diagnostic["message"].as_str().unwrap_or_default());
                build.diagnostics.entry(target).or_default().push((level == "error", text));
            }
            _ => {}
        }
    }
    build
}

/// The lines that differ between two outputs, after the lines they start and end with in common:
/// `-` marks a line of Java's output and `+` a line of Rust's, under the line they start at.
fn diff(java: &str, rust: &str) -> String {
    let (a, b): (Vec<&str>, Vec<&str>) = (java.lines().collect(), rust.lines().collect());
    let head = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let tail = a[head..].iter().rev().zip(b[head..].iter().rev()).take_while(|(x, y)| x == y).count();
    let (a, b) = (&a[head..a.len() - tail], &b[head..b.len() - tail]);
    if a.is_empty() && b.is_empty() {
        return format!("    outputs differ only in line endings: {java:?} and {rust:?}\n");
    }
    const SHOWN: usize = 40;
    let lines: Vec<String> =
        a.iter().map(|l| format!("- {l}")).chain(b.iter().map(|l| format!("+ {l}"))).collect();
    let mut out = format!("    @@ line {} @@\n", head + 1);
    for line in lines.iter().take(SHOWN) {
        out.push_str(&format!("    {line}\n"));
    }
    if lines.len() > SHOWN {
        out.push_str(&format!("    ... {} more lines\n", lines.len() - SHOWN));
    }
    out
}

/// Compares one program's translation, build and run with Java's.
fn check(
    p: &Program,
    translation: &Result<String, String>,
    build: &Build,
    expected: &Run,
) -> Result<(), Failure> {
    if let Err(e) = translation {
        return Err(Failure { summary: e.clone(), details: String::new() });
    }
    let Some(binary) = build.binaries.get(&p.name) else {
        let errors: Vec<&str> = build
            .diagnostics
            .get(&p.name)
            .into_iter()
            .flatten()
            .filter(|(error, _)| *error)
            .map(|(_, text)| text.as_str())
            .collect();
        let details = if errors.is_empty() {
            build.stderr.lines().filter(|l| l.contains("error")).map(|l| format!("    {l}\n")).collect()
        } else {
            errors.iter().map(|e| format!("    {e}\n")).collect()
        };
        return Err(Failure { summary: "does not compile".to_string(), details });
    };
    let actual = run(Command::new(binary).args(&p.args).output().unwrap());
    if actual == *expected {
        return Ok(());
    }
    let code = expected.code != actual.code;
    let output = expected.stdout != actual.stdout;
    let what = match (code, output) {
        (true, true) => "exit code and output",
        (true, false) => "exit code",
        _ => "output",
    };
    let mut details = String::new();
    if code {
        details.push_str(&format!("    exit code: java {:?}, rust {:?}\n", expected.code, actual.code));
    }
    if output {
        details.push_str(&diff(&expected.stdout, &actual.stdout));
    }
    Err(Failure { summary: format!("differs from Java: {what}"), details })
}

#[test]
fn corpus_matches_java() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest.join("../../corpus");
    let mut programs = programs(&root);
    programs.extend(projects(&root.join("projects")));
    assert!(!programs.is_empty(), "no programs in {}", root.display());

    // One Cargo project with a binary per program, so the runtime is compiled once. It persists
    // between runs, so only the programs whose translation changed are compiled again.
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("corpus");
    let bin_dir = work.join("src/bin");
    std::fs::create_dir_all(&bin_dir).unwrap();
    let runtime = manifest.join("../runtime").canonicalize().unwrap();
    write_if_changed(
        &work.join("Cargo.toml"),
        &format!(
            "[package]\nname = \"corpus\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n\
             [dependencies]\njava-runtime = {{ path = {runtime:?} }}\n\n[profile.dev]\n\
             debug = 0\nincremental = false\n\n[workspace]\n"
        ),
    );
    // A source of a program that left the corpus must not be built as one of the binaries.
    let sources: Vec<String> = programs.iter().map(|p| format!("{}.rs", p.name)).collect();
    for entry in std::fs::read_dir(&bin_dir).unwrap() {
        let path = entry.unwrap().path();
        if !sources.contains(&path.file_name().unwrap().to_string_lossy().into_owned()) {
            std::fs::remove_file(path).unwrap();
        }
    }
    let java_cache = JavaCache::open(&work.join("java-cache"), &programs);
    if let Ok(filter) = std::env::var("CORPUS_FILTER") {
        programs.retain(|p| p.name.contains(&filter));
        assert!(!programs.is_empty(), "no program name contains CORPUS_FILTER={filter}");
    }

    // Java's runs do not depend on the translations, so they run while the programs translate
    // and build.
    let classes = work.join("classes");
    let (translations, build, expected) = std::thread::scope(|s| {
        let translate = s.spawn(|| {
            let translations = parallel(programs.len(), |i| rust_source(&programs[i]));
            for (p, rust) in programs.iter().zip(&translations) {
                if let Ok(rust) = rust {
                    write_if_changed(&bin_dir.join(format!("{}.rs", p.name)), rust);
                }
            }
            let built: Vec<&str> = programs
                .iter()
                .zip(&translations)
                .filter(|(_, t)| t.is_ok())
                .map(|(p, _)| p.name.as_str())
                .collect();
            let build = build_binaries(&work, &built);
            (translations, build)
        });
        let expected = parallel(programs.len(), |i| java_cache.run(&programs[i], &classes));
        let (translations, build) = translate.join().unwrap();
        (translations, build, expected)
    });

    let outcomes = parallel(programs.len(), |i| check(&programs[i], &translations[i], &build, &expected[i]));

    let mut failures = Vec::new();
    let mut report = String::from("\nprogram                          result\n");
    let mut details = String::new();
    for (p, outcome) in programs.iter().zip(&outcomes) {
        let warnings = build.diagnostics.get(&p.name).map_or(0, |d| d.iter().filter(|(e, _)| !e).count());
        let status = match outcome {
            Ok(()) if warnings > 0 => format!("ok ({warnings} rustc warnings)"),
            Ok(()) => "ok".to_string(),
            Err(failure) => {
                failures.push(p.name.clone());
                details.push_str(&format!("\n{}: {}\n{}", p.name, failure.summary, failure.details));
                format!("FAIL: {}", failure.summary)
            }
        };
        report.push_str(&format!("{:<32} {status}\n", p.name));
    }
    println!("{report}");
    let warning_lines: Vec<&str> = programs
        .iter()
        .filter_map(|p| build.diagnostics.get(&p.name))
        .flatten()
        .filter(|(error, _)| !error)
        .map(|(_, text)| text.as_str())
        .collect();
    if !warning_lines.is_empty() {
        println!("rustc warnings:\n{}", warning_lines.join("\n"));
    }
    assert!(
        failures.is_empty(),
        "{} of {} programs failed: {}\n{details}",
        failures.len(),
        programs.len(),
        failures.join(", ")
    );
}
