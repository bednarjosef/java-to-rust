//! Runs each program of `corpus/whole-program/` on Java and, translated whole with the JDK's own
//! sources, as Rust, and requires the two runs to print the same standard output and exit with
//! the same code. These programs use the JDK's code (default and static methods of its
//! interfaces, such as `Comparator.comparing`), which the translation takes from the sources of
//! `java.base` that `JAVA_BASE_SOURCES` names; those cannot be in the repository, so without the
//! variable the test says it skipped and passes. A program is one `.java` file, or a directory
//! with a source root `src/` and a file `main` that names the main class, as in
//! `corpus/projects/`. Translating with the JDK takes about a minute per program, so the programs
//! run one at a time; `CORPUS_FILTER=text` runs only those whose name contains `text`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Program {
    /// The binary name, such as `functionchains`.
    name: String,
    /// The source root.
    sources: PathBuf,
    /// The qualified name of the main class.
    main: String,
}

/// What a run printed and how it exited.
#[derive(Debug, PartialEq)]
struct Run {
    stdout: String,
    code: Option<i32>,
}

fn run(output: Output) -> Run {
    Run { stdout: String::from_utf8_lossy(&output.stdout).into_owned(), code: output.status.code() }
}

/// The programs, each with a source root of its own: a lone file is copied into one under
/// `work`, as javac rejects a listed file next to a source root of a module.
fn programs(root: &Path, work: &Path) -> Vec<Program> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(root).unwrap().map(|e| e.unwrap().path()).collect();
    entries.sort();
    let mut out = Vec::new();
    for path in entries {
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        if path.is_dir() {
            let main = std::fs::read_to_string(path.join("main")).unwrap().trim().to_string();
            out.push(Program { name: stem.to_lowercase(), sources: path.join("src"), main });
        } else if path.extension().is_some_and(|e| e == "java") {
            let sources = work.join(stem.to_lowercase()).join("src");
            let _ = std::fs::remove_dir_all(&sources);
            std::fs::create_dir_all(&sources).unwrap();
            std::fs::copy(&path, sources.join(path.file_name().unwrap())).unwrap();
            out.push(Program { name: stem.to_lowercase(), sources, main: stem });
        }
    }
    out
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

/// The program's run on Java, compiled with javac into `classes` first.
fn run_java(p: &Program, classes: &Path) -> Run {
    let _ = std::fs::remove_dir_all(classes);
    let mut sources = Vec::new();
    java_files(&p.sources, &mut sources);
    let javac = Command::new("javac")
        .arg("-d")
        .arg(classes)
        .args(&sources)
        .output()
        .expect("running `javac` failed; the test needs a JDK on PATH");
    assert!(javac.status.success(), "javac failed on {}: {}", p.name, String::from_utf8_lossy(&javac.stderr));
    run(Command::new("java").arg("-cp").arg(classes).arg(&p.main).output().expect("running `java` failed"))
}

/// The program's run translated with the JDK's sources, or why it does not get that far.
fn run_rust(p: &Program, library: &Path, work: &Path) -> Result<Run, String> {
    let dir = work.join(&p.name).join("crate");
    let _ = std::fs::remove_dir_all(&dir);
    let translated = Command::new(env!("CARGO_BIN_EXE_java-to-rust"))
        .args(["--main", &p.main])
        .arg("--source")
        .arg(&p.sources)
        .arg("--library")
        .arg(library)
        .arg("--crate")
        .arg(&dir)
        .output()
        .unwrap();
    if !translated.status.success() {
        return Err(format!("translation failed: {}", String::from_utf8_lossy(&translated.stderr)));
    }
    let target = work.join("cargo");
    let built = Command::new("cargo")
        .args(["build", "-q", "--manifest-path"])
        .arg(dir.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", &target)
        .output()
        .unwrap();
    if !built.status.success() {
        return Err(format!("does not compile: {}", String::from_utf8_lossy(&built.stderr)));
    }
    // The translator names the crate after the main class.
    let crate_name = p.main.rsplit('.').next().unwrap_or_default().to_lowercase();
    let binary = target.join("debug").join(crate_name);
    Ok(run(Command::new(binary).output().unwrap()))
}

#[test]
fn whole_programs_match_java() {
    let Some(library) = std::env::var_os("JAVA_BASE_SOURCES").map(PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to run these programs");
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/whole-program");
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("whole-program");
    let filter = std::env::var("CORPUS_FILTER").ok();
    let mut failures = Vec::new();
    let programs = programs(&root, &work);
    let selected = programs.iter().filter(|p| filter.as_ref().is_none_or(|f| p.name.contains(f.as_str())));
    for p in selected {
        let java = run_java(p, &work.join(&p.name).join("classes"));
        let verdict = match run_rust(p, &library, &work) {
            Ok(rust) if rust == java => "PASS".to_string(),
            Ok(rust) => {
                failures.push(format!("{}: Java {java:?}\nRust {rust:?}", p.name));
                "WRONG OUTPUT".to_string()
            }
            Err(why) => {
                let first = why.lines().next().unwrap_or_default().to_string();
                failures.push(format!("{}: {why}", p.name));
                first
            }
        };
        println!("{:<24} {verdict}", p.name);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
