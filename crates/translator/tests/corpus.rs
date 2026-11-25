//! Runs every Java program in `corpus/` on Java and, translated, as Rust, and requires the two
//! runs to print the same standard output and exit with the same code. Java's run is the
//! expected result, so no expectations are written by hand.
//!
//! Translation, the Java runs, the Cargo build and the Rust runs each use one thread per CPU.
//! The Rust programs are one Cargo project with a binary per program, kept in `target/tmp`
//! between runs: a source is rewritten only when its translation changes, so cargo rebuilds
//! just the programs whose translation changed. Java's results are kept in the checks' cache
//! (`check_cache`: `~/.cache/java-to-rust-checks` unless `CHECK_CACHE` names another), which
//! every worktree shares, so a program runs on Java again only when its sources or the Java
//! version change (`java_cache`); so is the run of each translation that matched Java, so a
//! program whose translation, runtime and toolchain are unchanged is neither built nor run again
//! (`rust_cache`). `CORPUS_JAVA_FRESH=1` runs every program on Java, and `CHECK_FRESH=1` does
//! all the work again. The test prints how long its parts took. `CORPUS_FILTER=text` runs only
//! the programs whose name contains `text`. A single file gets the lines of `Name.args` next to it
//! as its arguments and reads `Name.input` next to it on standard input, which is empty without
//! one.

#[path = "corpus/java_cache.rs"]
mod java_cache;
#[path = "corpus/parallel.rs"]
mod parallel;
#[path = "corpus/rust_cache.rs"]
mod rust_cache;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Instant;

use java_cache::JavaCache;
use java_to_rust::check_cache;
use java_to_rust::output::{Layout, SourceFile};
use parallel::parallel;
use rust_cache::{RustCache, Stored};

/// The JVM arguments of every Java run: short-lived JVMs skip the optimizing compiler, the parallel
/// collector and perf data (whose lock-file warnings go to stdout when many JVMs run at once).
/// The output does not change. AWT is headless, as in every translation (`lower::system`).
const JVM_ARGS: [&str; 4] =
    ["-XX:TieredStopAtLevel=1", "-XX:+UseSerialGC", "-XX:-UsePerfData", "-Djava.awt.headless=true"];

struct Program {
    /// The binary name, such as `features-hello`.
    name: String,
    /// The source file, or for a project the source root.
    java: PathBuf,
    /// For a project, the qualified name of its main class.
    main: Option<String>,
    /// The command-line arguments both runs get: the lines of `Name.args` next to a single file.
    args: Vec<String>,
    /// What both runs read on standard input: the file `Name.input` next to a single file. Without
    /// one, standard input is empty.
    input: Option<PathBuf>,
    /// For a project, its library root `lib/`, if it has one: both runs have its classes.
    library: Option<PathBuf>,
    /// For a project, its root `optional/` of classes the library refers to, if it has one: Java
    /// compiles against them but runs without them, and the translator never sees them, so the
    /// code that uses them throws `NoClassDefFoundError` in both runs.
    optional: Option<PathBuf>,
}

/// Standard input of a run: the program's input file, or nothing.
fn stdin_of(p: &Program) -> Stdio {
    match &p.input {
        Some(path) => Stdio::from(std::fs::File::open(path).expect("the input file is readable")),
        None => Stdio::null(),
    }
}

#[derive(Clone, PartialEq)]
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
            let input = Some(java.with_extension("input")).filter(|i| i.is_file());
            let name = format!("{group}-{stem}");
            out.push(Program { name, java, main: None, args, input, library: None, optional: None });
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
/// package and a file `main` that names the main class, such as `com.example.Main`. A project may
/// also have a library root `lib/`, and a root `optional/` of classes missing at run time.
fn projects(root: &Path) -> Vec<Program> {
    let Ok(entries) = std::fs::read_dir(root) else { return Vec::new() };
    let mut dirs: Vec<PathBuf> = entries.map(|e| e.unwrap().path()).filter(|d| d.is_dir()).collect();
    dirs.sort();
    dirs.into_iter()
        .map(|dir| {
            let main = std::fs::read_to_string(dir.join("main")).unwrap().trim().to_string();
            let name = format!("projects-{}", dir.file_name().unwrap().to_string_lossy());
            let root = |name: &str| Some(dir.join(name)).filter(|d| d.is_dir());
            Program {
                name,
                java: dir.join("src"),
                main: Some(main),
                args: Vec::new(),
                input: None,
                library: root("lib"),
                optional: root("optional"),
            }
        })
        .collect()
}

/// Runs a program on Java: a single file with the source launcher, a project compiled with javac
/// into `classes/<name>` first. A project's optional classes are compiled into
/// `classes/<name>-optional`, which the program is compiled against but not run with.
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
            .stdin(stdin_of(p))
            .output()
            .expect("running `java` failed; the corpus test needs a JDK on PATH");
    };
    let javac = |out: &Path, roots: &[&PathBuf], class_path: Option<&Path>| {
        let _ = std::fs::remove_dir_all(out);
        let mut sources = Vec::new();
        for root in roots {
            java_files(root, &mut sources);
        }
        let mut javac = Command::new("javac");
        javac.arg("-d").arg(out);
        if let Some(class_path) = class_path {
            javac.arg("-cp").arg(class_path);
        }
        let javac = javac.args(&sources).output().expect("running `javac` failed");
        assert!(
            javac.status.success(),
            "javac failed on {}: {}",
            p.name,
            String::from_utf8_lossy(&javac.stderr)
        );
    };
    let optional = p.optional.as_ref().map(|root| {
        let out = classes.join(format!("{}-optional", p.name));
        javac(&out, &[root], None);
        out
    });
    let out = classes.join(&p.name);
    let roots: Vec<&PathBuf> = [&p.java].into_iter().chain(&p.library).collect();
    javac(&out, &roots, optional.as_deref());
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

/// The Rust sources of a program, or why it does not translate: a single file is one `main.rs`, a
/// project has a file per package, as the translator writes its crate. A project with optional
/// classes is translated past the compile errors of the code that uses them (`keep_going`).
fn rust_source(p: &Program) -> Result<Vec<SourceFile>, String> {
    let translated: Result<Vec<SourceFile>, String> = match &p.main {
        None => java_to_rust::translate(&std::fs::read_to_string(&p.java).unwrap())
            .map(|text| vec![SourceFile { path: "main.rs".into(), text }])
            .map_err(|e| e.to_string()),
        Some(main) => {
            let program = java_to_rust::Program {
                main: main.clone(),
                sources: vec![p.java.clone()],
                libraries: p.library.iter().cloned().collect(),
                keep_going: p.optional.is_some(),
                ..Default::default()
            };
            java_to_rust::translate_program_modules(&program)
                .map(|tree| tree.files(Layout::FilePerModule))
                .map_err(|e| e.to_string())
        }
    };
    translated.map_err(|e| format!("translation failed: {e}"))
}

/// Writes a program's files as the sources of its binary: `src/bin/<name>.rs` for one file,
/// `src/bin/<name>/` with its `main.rs` for several.
fn write_sources(bin_dir: &Path, name: &str, files: &[SourceFile]) {
    let (single, dir) = (bin_dir.join(format!("{name}.rs")), bin_dir.join(name));
    if let [file] = files {
        let _ = std::fs::remove_dir_all(&dir);
        write_if_changed(&single, &file.text);
        return;
    }
    let _ = std::fs::remove_file(&single);
    for file in files {
        let path = dir.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        write_if_changed(&path, &file.text);
    }
}

/// The text of a program's files, which decides its run: a single file's text, or each file's
/// after its path.
fn sources_text(files: &[SourceFile]) -> String {
    match files {
        [file] => file.text.clone(),
        _ => files.iter().map(|f| format!("{}\n{}", f.path.display(), f.text)).collect(),
    }
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

/// Compares one program's translation, build and run with Java's. A program with a stored run
/// (`rust_cache`) is not run; otherwise the run is returned, to be stored if it matched.
fn check(
    p: &Program,
    translation: &Result<Vec<SourceFile>, String>,
    build: &Build,
    stored: Option<&Run>,
    expected: &Run,
) -> (Result<(), Failure>, Option<Run>) {
    if let Err(e) = translation {
        return (Err(Failure { summary: e.clone(), details: String::new() }), None);
    }
    let (actual, fresh) = match stored {
        Some(stored) => (stored.clone(), None),
        None => {
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
                    build
                        .stderr
                        .lines()
                        .filter(|l| l.contains("error"))
                        .map(|l| format!("    {l}\n"))
                        .collect()
                } else {
                    errors.iter().map(|e| format!("    {e}\n")).collect()
                };
                return (Err(Failure { summary: "does not compile".to_string(), details }), None);
            };
            let actual = run(Command::new(binary).args(&p.args).stdin(stdin_of(p)).output().unwrap());
            (actual.clone(), Some(actual))
        }
    };
    if actual == *expected {
        return (Ok(()), fresh);
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
    (Err(Failure { summary: format!("differs from Java: {what}"), details }), None)
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
    let cargo_toml = format!(
        "[package]\nname = \"corpus\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n\
         [dependencies]\njava-runtime = {{ package = \"java-to-rust-runtime\", path = {runtime:?} }}\n\n[profile.dev]\n\
         debug = 0\nincremental = false\n\n[workspace]\n"
    );
    write_if_changed(&work.join("Cargo.toml"), &cargo_toml);
    // A source of a program that left the corpus must not be built as one of the binaries.
    let sources: Vec<String> =
        programs.iter().flat_map(|p| [format!("{}.rs", p.name), p.name.clone()]).collect();
    for entry in std::fs::read_dir(&bin_dir).unwrap() {
        let path = entry.unwrap().path();
        if !sources.contains(&path.file_name().unwrap().to_string_lossy().into_owned()) {
            if path.is_dir() {
                std::fs::remove_dir_all(path).unwrap();
            } else {
                std::fs::remove_file(path).unwrap();
            }
        }
    }
    // Java's results and the runs of unchanged translations are kept in the checks' cache.
    let cache = check_cache::test_root();
    let java_cache = JavaCache::open(&cache.join("java"));
    let rust_cache = RustCache::open(&cache.join("rust"), &runtime, &work, &cargo_toml);
    if let Ok(filter) = std::env::var("CORPUS_FILTER") {
        programs.retain(|p| p.name.contains(&filter));
        assert!(!programs.is_empty(), "no program name contains CORPUS_FILTER={filter}");
    }

    // Java's runs do not depend on the translations, so they run while the programs translate
    // and build.
    let classes = work.join("classes");
    let start = Instant::now();
    let (translations, keys, stored, build, expected, times) = std::thread::scope(|s| {
        let translate = s.spawn(|| {
            let translations = parallel(programs.len(), |i| rust_source(&programs[i]));
            let translated = start.elapsed();
            for (p, files) in programs.iter().zip(&translations) {
                if let Ok(files) = files {
                    write_sources(&bin_dir, &p.name, files);
                }
            }
            let keys: Vec<Option<String>> = programs
                .iter()
                .zip(&translations)
                .map(|(p, t)| t.as_ref().ok().and_then(|files| rust_cache.key(p, &sources_text(files))))
                .collect();
            let stored: Vec<Option<Stored>> =
                keys.iter().map(|k| k.as_ref().and_then(|k| rust_cache.load(k))).collect();
            let built: Vec<&str> = programs
                .iter()
                .zip(&translations)
                .zip(&stored)
                .filter(|((_, t), stored)| t.is_ok() && stored.is_none())
                .map(|((p, _), _)| p.name.as_str())
                .collect();
            let build = build_binaries(&work, &built);
            (translations, translated, keys, stored, build, start.elapsed())
        });
        let expected = parallel(programs.len(), |i| java_cache.run(&programs[i], &classes));
        let java = start.elapsed();
        let (translations, translated, keys, stored, build, built) = translate.join().unwrap();
        (translations, keys, stored, build, expected, [translated, built, java])
    });

    let outcomes = parallel(programs.len(), |i| {
        let stored = stored[i].as_ref().map(|s| &s.run);
        check(&programs[i], &translations[i], &build, stored, &expected[i].0)
    });
    let checked = start.elapsed();

    // The rustc warnings of each program: from this build, or stored with its run.
    let warnings: Vec<Vec<String>> = programs
        .iter()
        .zip(&stored)
        .map(|(p, stored)| match stored {
            Some(stored) => stored.warnings.clone(),
            None => build
                .diagnostics
                .get(&p.name)
                .into_iter()
                .flatten()
                .filter(|(error, _)| !error)
                .map(|(_, text)| text.clone())
                .collect(),
        })
        .collect();
    let mut failures = Vec::new();
    let mut report = String::from("\nprogram                          result\n");
    let mut details = String::new();
    for (i, p) in programs.iter().enumerate() {
        let (outcome, fresh) = &outcomes[i];
        if let (Ok(()), Some(run), Some(key)) = (outcome, fresh, &keys[i]) {
            rust_cache.store(key, run, &warnings[i]);
        }
        let status = match outcome {
            Ok(()) if !warnings[i].is_empty() => format!("ok ({} rustc warnings)", warnings[i].len()),
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
    let warning_lines: Vec<&str> = warnings.iter().flatten().map(String::as_str).collect();
    if !warning_lines.is_empty() {
        println!("rustc warnings:\n{}", warning_lines.join("\n"));
    }
    let [translated, built, java] = times;
    let java_runs = expected.iter().filter(|(_, stored)| !stored).count();
    let rust_runs = translations.iter().zip(&stored).filter(|(t, s)| t.is_ok() && s.is_none()).count();
    println!(
        "{} programs: translated in {:.1} s; {java_runs} run on Java ({} stored), done at {:.1} s; \
         {rust_runs} built and run ({} stored), built at {:.1} s; checked at {:.1} s",
        programs.len(),
        translated.as_secs_f64(),
        programs.len() - java_runs,
        java.as_secs_f64(),
        stored.iter().flatten().count(),
        built.as_secs_f64(),
        checked.as_secs_f64(),
    );
    assert!(
        failures.is_empty(),
        "{} of {} programs failed: {}\n{details}",
        failures.len(),
        programs.len(),
        failures.join(", ")
    );
}
