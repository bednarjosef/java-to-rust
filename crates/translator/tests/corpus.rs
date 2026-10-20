//! Runs every Java program in `corpus/` on Java and, translated, as Rust, and requires the two
//! runs to print the same standard output and exit with the same code. Java's run is the
//! expected result, so no expectations are written by hand.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Program {
    /// The binary name, such as `features-hello`.
    name: String,
    /// The source file, or for a project the source root.
    java: PathBuf,
    /// For a project, the qualified name of its main class.
    main: Option<String>,
}

#[derive(PartialEq)]
struct Run {
    stdout: String,
    code: Option<i32>,
}

fn run(output: Output) -> Run {
    Run { stdout: String::from_utf8_lossy(&output.stdout).into_owned(), code: output.status.code() }
}

fn programs(root: &Path) -> Vec<Program> {
    let mut out = Vec::new();
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root).unwrap().map(|e| e.unwrap().path()).collect();
    dirs.sort();
    for dir in dirs.into_iter().filter(|d| d.is_dir()) {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "java"))
            .collect();
        files.sort();
        for java in files {
            let stem = java.file_stem().unwrap().to_string_lossy().to_lowercase();
            let group = dir.file_name().unwrap().to_string_lossy().to_string();
            out.push(Program { name: format!("{group}-{stem}"), java, main: None });
        }
    }
    out
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
            Program { name, java: dir.join("src"), main: Some(main) }
        })
        .collect()
}

/// Runs a program on Java: a single file with the source launcher, a project compiled with javac
/// into `classes/<name>` first.
fn run_java(p: &Program, classes: &Path) -> Output {
    // Without perf data, a JVM never warns on stdout about another JVM's lock file.
    let java = || {
        let mut java = Command::new("java");
        java.arg("-XX:-UsePerfData");
        java
    };
    let Some(main) = &p.main else {
        return java()
            .arg(&p.java)
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

#[test]
fn corpus_matches_java() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest.join("../../corpus");
    let mut programs = programs(&root);
    programs.extend(projects(&root.join("projects")));
    assert!(!programs.is_empty(), "no programs in {}", root.display());

    // One Cargo project with a binary per program, so the runtime is compiled once.
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("corpus");
    let bin_dir = work.join("src/bin");
    let _ = std::fs::remove_dir_all(&bin_dir);
    std::fs::create_dir_all(&bin_dir).unwrap();
    let runtime = manifest.join("../runtime").canonicalize().unwrap();
    std::fs::write(
        work.join("Cargo.toml"),
        format!(
            "[package]\nname = \"corpus\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n\
             [dependencies]\njava-runtime = {{ path = {:?} }}\n\n[workspace]\n",
            runtime.display().to_string()
        ),
    )
    .unwrap();

    let translations: Vec<Result<(), String>> = programs
        .iter()
        .map(|p| {
            let rust = match &p.main {
                None => java_to_rust::translate(&std::fs::read_to_string(&p.java).unwrap())
                    .map_err(|e| format!("translation failed: {e}"))?,
                Some(main) => {
                    let program = java_to_rust::Program {
                        main: main.clone(),
                        sources: vec![p.java.clone()],
                        ..Default::default()
                    };
                    java_to_rust::translate_program(&program)
                        .map_err(|e| format!("translation failed: {e}"))?
                }
            };
            std::fs::write(bin_dir.join(format!("{}.rs", p.name)), rust).unwrap();
            Ok(())
        })
        .collect();

    let expected: Vec<Run> = std::thread::scope(|s| {
        let handles: Vec<_> =
            programs.iter().map(|p| s.spawn(|| run(run_java(p, &work.join("classes"))))).collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    let build = Command::new("cargo")
        .args(["build", "--bins", "--keep-going", "--message-format=short", "--target-dir"])
        .arg(work.join("target"))
        .current_dir(&work)
        .env_remove("RUSTFLAGS")
        .output()
        .unwrap();
    let build_log = String::from_utf8_lossy(&build.stderr).into_owned();

    let mut failures = Vec::new();
    let mut report = String::from("\nprogram                          result\n");
    for ((program, translated), expected) in programs.iter().zip(&translations).zip(&expected) {
        let source = format!("src/bin/{}.rs:", program.name);
        let warnings =
            build_log.lines().filter(|l| l.starts_with(&source) && l.contains(": warning")).count();
        let result = match translated {
            Err(e) => Err(e.clone()),
            Ok(()) => {
                let binary = work.join("target/debug").join(&program.name);
                if !binary.exists() {
                    let errors: Vec<&str> = build_log.lines().filter(|l| l.starts_with(&source)).collect();
                    Err(format!("does not compile:\n      {}", errors.join("\n      ")))
                } else {
                    let actual = run(Command::new(&binary).output().unwrap());
                    if actual == *expected {
                        Ok(())
                    } else {
                        Err(format!(
                            "differs from Java\n--- java (exit {:?})\n{}--- rust (exit {:?})\n{}",
                            expected.code, expected.stdout, actual.code, actual.stdout
                        ))
                    }
                }
            }
        };
        let status = match &result {
            Ok(()) if warnings > 0 => format!("ok ({warnings} rustc warnings)"),
            Ok(()) => "ok".to_string(),
            Err(e) => format!("FAIL: {e}"),
        };
        report.push_str(&format!("{:<32} {status}\n", program.name));
        if result.is_err() {
            failures.push(program.name.clone());
        }
    }
    println!("{report}");
    if build_log.contains("warning") {
        println!(
            "rustc warnings:\n{}",
            build_log.lines().filter(|l| l.contains("warning")).collect::<Vec<_>>().join("\n")
        );
    }
    assert!(
        failures.is_empty(),
        "{} of {} programs failed: {}{report}",
        failures.len(),
        programs.len(),
        failures.join(", ")
    );
}
