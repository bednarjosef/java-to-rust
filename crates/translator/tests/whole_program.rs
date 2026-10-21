//! Runs each program of `corpus/whole-program/` on Java and, translated whole with the JDK's own
//! sources, as Rust, and requires the two runs to print the same standard output and exit with
//! the same code. These programs use the JDK's code (default and static methods of its
//! interfaces, such as `Comparator.comparing`), which the translation takes from the sources of
//! `java.base` that `JAVA_BASE_SOURCES` names; those cannot be in the repository, so without the
//! variable the test says it skipped and passes. A program is one `.java` file, or a directory
//! with a source root `src/` and a file `main` that names the main class, as in
//! `corpus/projects/`. Translating with the JDK takes a few seconds per program, and the programs
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

/// A program that uses the JDK's collections, boxing and a lambda, as
/// `targets/IntegerKeyedGroups.java` does. Its translation is rejected or not depending on what
/// the translator supports; this test is about what javac does for it.
const COLLECTIONS: &str = "import java.util.*;

public class Main {
    public static void main(String[] args) {
        Map<Integer, List<String>> groups = new HashMap<>();
        groups.computeIfAbsent(3, k -> new ArrayList<>()).add(\"ann\");
        Set<Integer> primes = new HashSet<>(List.of(2, 3, 5, 7));
        primes.retainAll(Set.of(1, 3, 5));
        System.out.println(groups + \" \" + primes.size());
    }
}
";

/// javac attributes only the units of the classes that the program's reachable code uses, as
/// reachability asks for them: a few dozen of the JDK's. Left to itself, javac attributes every
/// unit that the code of the units it attributes names, some 2500 for programs like this one,
/// which took half a minute and 7 GB, and ten minutes on a machine short of memory.
#[test]
fn a_program_using_collections_has_only_reachable_units_attributed() {
    let Some(library) = std::env::var_os("JAVA_BASE_SOURCES").map(PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to run this program");
        return;
    };
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("whole-program-units");
    let sources = work.join("src");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&sources).unwrap();
    std::fs::write(sources.join("Main.java"), COLLECTIONS).unwrap();
    let start = std::time::Instant::now();
    let translated = Command::new(env!("CARGO_BIN_EXE_java-to-rust"))
        .args(["--timings", "--main", "Main", "--source"])
        .arg(&sources)
        .arg("--library")
        .arg(library)
        .arg("-o")
        .arg(work.join("main.rs"))
        .output()
        .unwrap();
    let elapsed = start.elapsed();
    let stderr = String::from_utf8_lossy(&translated.stderr);
    // `timing: javac: 1.234 s, 36 units`
    let units: usize = stderr
        .lines()
        .find_map(|l| l.strip_prefix("timing: javac: "))
        .and_then(|l| l.split(", ").nth(1))
        .and_then(|u| u.strip_suffix(" units"))
        .and_then(|u| u.parse().ok())
        .unwrap_or_else(|| panic!("no unit count among the timings:\n{stderr}"));
    assert!(units < 200, "javac attributed {units} units:\n{stderr}");
    assert!(elapsed.as_secs() < 120, "the translation took {elapsed:?}:\n{stderr}");
}

/// A translated program has no resources: `Class.getResourceAsStream` ends it, naming the
/// resource, as `Character.getName` does when it reads `uniName.dat`, a data file of the JDK that
/// is generated when the JDK is built and is not in its sources.
#[test]
fn reading_a_resource_ends_the_program() {
    let Some(library) = std::env::var_os("JAVA_BASE_SOURCES").map(PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to run this program");
        return;
    };
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("whole-program-resource");
    let sources = work.join("src");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&sources).unwrap();
    let java = "public class Main {
    public static void main(String[] args) throws Exception {
        java.io.InputStream in = Main.class.getResourceAsStream(\"table.dat\");
        if (in != null) {
            in.close();
        }
    }
}
";
    std::fs::write(sources.join("Main.java"), java).unwrap();
    let translated = Command::new(env!("CARGO_BIN_EXE_java-to-rust"))
        .args(["--main", "Main", "--source"])
        .arg(&sources)
        .arg("--library")
        .arg(library)
        .arg("-o")
        .arg(work.join("main.rs"))
        .output()
        .unwrap();
    assert!(translated.status.success(), "{}", String::from_utf8_lossy(&translated.stderr));
    let rust = std::fs::read_to_string(work.join("main.rs")).unwrap();
    for wanted in ["is not part of the translated program", "\"table.dat\"", "String::from(\"Main\")"] {
        assert!(rust.contains(wanted), "no {wanted} in the translation");
    }
}
