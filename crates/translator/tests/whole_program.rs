//! Runs each program of `corpus/whole-program/` on Java and, translated whole with the JDK's own
//! sources, as Rust, and requires the two runs to print the same standard output and exit with
//! the same code. These programs use the JDK's code (default and static methods of its
//! interfaces, such as `Comparator.comparing`), which the translation takes from the sources of
//! `java.base` that `JAVA_BASE_SOURCES` names; those cannot be in the repository, so without the
//! variable the test says it skipped and passes. A program is one `.java` file, or a directory
//! with a source root `src/` and a file `main` that names the main class, as in
//! `corpus/projects/`. Translating with the JDK takes a few seconds per program, and the programs
//! run on `CORPUS_THREADS` threads (one per CPU by default); `CORPUS_FILTER=text` runs only those
//! whose name contains `text`. A program reads the file `Name.input` next to it (`input` in a
//! directory) on standard input, and nothing without one.
//!
//! As in the corpus test, Java's results and the runs of translations that are the same as in an
//! earlier run that matched Java are kept in the checks' cache (`check_cache::test_root`).
//! `CHECK_FRESH=1` does all the work again.

#[path = "corpus/parallel.rs"]
mod parallel;

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use java_to_rust::check_cache::{self, Fingerprint};
use parallel::parallel;

struct Program {
    /// The binary name, such as `functionchains`.
    name: String,
    /// The source root.
    sources: PathBuf,
    /// The qualified name of the main class.
    main: String,
    /// The file the runs read on standard input, if there is one.
    input: Option<PathBuf>,
}

/// Standard input of a run: the program's input file, or nothing.
fn stdin_of(p: &Program) -> Stdio {
    match &p.input {
        Some(path) => Stdio::from(std::fs::File::open(path).expect("the input file is readable")),
        None => Stdio::null(),
    }
}

/// What a run printed and how it exited.
#[derive(Clone, Debug, PartialEq)]
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
            let input = Some(path.join("input")).filter(|i| i.is_file());
            out.push(Program { name: stem.to_lowercase(), sources: path.join("src"), main, input });
        } else if path.extension().is_some_and(|e| e == "java") {
            let sources = work.join(stem.to_lowercase()).join("src");
            let _ = std::fs::remove_dir_all(&sources);
            std::fs::create_dir_all(&sources).unwrap();
            std::fs::copy(&path, sources.join(path.file_name().unwrap())).unwrap();
            let input = Some(path.with_extension("input")).filter(|i| i.is_file());
            out.push(Program { name: stem.to_lowercase(), sources, main: stem, input });
        }
    }
    out
}

/// The files under `dir`: the `.java` ones, or with `java` false the others, the resources
/// (`META-INF/services`), which the run on Java finds next to the classes.
fn files(dir: &Path, out: &mut Vec<PathBuf>, java: bool) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files(&path, out, java);
        } else if path.extension().is_some_and(|e| e == "java") == java {
            out.push(path);
        }
    }
}

/// The program's run on Java, compiled with javac into `classes` first, or its result stored in
/// `cache` by an earlier run with the same sources, input and Java version.
/// The packages of `java.base` that are not exported but that the programs may use.
const INTERNAL_EXPORTS: [&str; 2] = ["--add-exports", "java.base/sun.util.logging=ALL-UNNAMED"];

fn run_java(p: &Program, classes: &Path, cache: &Path) -> Run {
    let mut sources = Vec::new();
    files(&p.sources, &mut sources, true);
    sources.sort();
    let mut resources = Vec::new();
    files(&p.sources, &mut resources, false);
    resources.sort();
    let key = java_key(p, &sources.iter().chain(&resources).cloned().collect::<Vec<_>>());
    let stored = key.as_ref().and_then(|key| check_cache::load(cache, key)).and_then(|entry| {
        let text = String::from_utf8(entry).ok()?;
        let (code, stdout) = text.split_once('\n')?;
        Some(Run { stdout: stdout.to_string(), code: Some(code.parse().ok()?) })
    });
    if let Some(stored) = stored {
        return stored;
    }
    let _ = std::fs::remove_dir_all(classes);
    // A program may use the JDK's internal logger, as the JDK's own classes do; the translation,
    // which reads `java.base` from its sources, sees all of its packages.
    let javac = Command::new("javac")
        .args(INTERNAL_EXPORTS)
        .arg("-d")
        .arg(classes)
        .args(&sources)
        .output()
        .expect("running `javac` failed; the test needs a JDK on PATH");
    assert!(javac.status.success(), "javac failed on {}: {}", p.name, String::from_utf8_lossy(&javac.stderr));
    for resource in &resources {
        let to = classes.join(resource.strip_prefix(&p.sources).unwrap());
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(resource, to).unwrap();
    }
    let mut java = Command::new("java");
    java.args(INTERNAL_EXPORTS);
    // AWT is headless, as in every translation (`lower::system`).
    java.arg("-Djava.awt.headless=true");
    match module_name(&p.sources) {
        // A source root with a `module-info.java` is a named module, which runs from the module path.
        Some(module) => java.arg("--module-path").arg(classes).arg("-m").arg(format!("{module}/{}", p.main)),
        None => java.arg("-cp").arg(classes).arg(&p.main),
    };
    let java = java.stdin(stdin_of(p)).output();
    let result = run(java.expect("running `java` failed"));
    if let (Some(key), Some(code)) = (key, result.code) {
        check_cache::store(cache, &key, format!("{code}\n{}", result.stdout).as_bytes());
    }
    result
}

/// The name of the module that the source root `sources` declares in its `module-info.java`, if
/// it has one: the word after `module`, past the comments of its first lines.
fn module_name(sources: &Path) -> Option<String> {
    let source = std::fs::read_to_string(sources.join("module-info.java")).ok()?;
    let code: Vec<&str> = source.lines().filter(|l| !l.trim_start().starts_with("//")).collect();
    let words: Vec<&str> = code.iter().flat_map(|l| l.split_whitespace()).collect();
    let at = words.iter().position(|w| *w == "module")?;
    Some(words.get(at + 1)?.trim_end_matches('{').to_string())
}

/// What decides a program's run on Java: the Java version, the main class, the sources with
/// their paths in the source root, and the input. None if one cannot be read.
fn java_key(p: &Program, sources: &[PathBuf]) -> Option<String> {
    let mut fingerprint = Fingerprint::new();
    fingerprint.field(b"whole-program java run");
    fingerprint.field(check_cache::java_version().as_bytes());
    fingerprint.field(p.main.as_bytes());
    for file in sources {
        fingerprint.field(file.strip_prefix(&p.sources).ok()?.to_string_lossy().as_bytes());
        fingerprint.field(&std::fs::read(file).ok()?);
    }
    input_field(p, &mut fingerprint)?;
    Some(fingerprint.key())
}

fn input_field(p: &Program, fingerprint: &mut Fingerprint) -> Option<()> {
    fingerprint.field(b"input");
    if let Some(input) = &p.input {
        fingerprint.field(&std::fs::read(input).ok()?);
    }
    Some(())
}

/// The program's run translated with the JDK's sources, or why it does not get that far. A
/// translation that is the same as in an earlier run that matched Java, with the same runtime,
/// toolchain and input (`build`, the hash of all but the translation), is not built and run
/// again: its run is taken from `cache`. A fresh run comes with the key to store it under. With
/// `crates`, the translation is a workspace split into that many body crates (`--crates`).
fn run_rust(
    p: &Program,
    library: &Path,
    work: &Path,
    cache: &Path,
    build: Option<&str>,
    crates: Option<usize>,
) -> (Result<Run, String>, Option<String>) {
    let dir = work.join(&p.name).join("crate");
    let _ = std::fs::remove_dir_all(&dir);
    let translated = Command::new(env!("CARGO_BIN_EXE_java-to-rust"))
        .args(crates.into_iter().flat_map(|n| ["--crates".to_string(), n.to_string()]))
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
        return (Err(format!("translation failed: {}", String::from_utf8_lossy(&translated.stderr))), None);
    }
    let key = build.and_then(|build| rust_key(p, &dir, build));
    let stored = key.as_ref().and_then(|key| check_cache::load(cache, key)).and_then(|entry| {
        let text = String::from_utf8(entry).ok()?;
        let (code, stdout) = text.split_once('\n')?;
        Some(Run { stdout: stdout.to_string(), code: Some(code.parse().ok()?) })
    });
    if let Some(stored) = stored {
        return (Ok(stored), None);
    }
    let target = work.join("cargo");
    let built = Command::new("cargo")
        .args(["build", "-q", "--manifest-path"])
        .arg(dir.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", &target)
        .output()
        .unwrap();
    if !built.status.success() {
        return (Err(format!("does not compile: {}", String::from_utf8_lossy(&built.stderr))), None);
    }
    // The translator names the crate after the main class.
    let crate_name = p.main.rsplit('.').next().unwrap_or_default().to_lowercase();
    let binary = target.join("debug").join(crate_name);
    (Ok(run(Command::new(binary).stdin(stdin_of(p)).output().unwrap())), key)
}

/// The key of a translated program's run: `build`, the crate the translator wrote in `dir` (its
/// manifest without the runtime's path, which is another in each worktree), and the input.
fn rust_key(p: &Program, dir: &Path, build: &str) -> Option<String> {
    let runtime = runtime();
    let manifest = std::fs::read_to_string(dir.join("Cargo.toml")).ok()?;
    let mut fingerprint = Fingerprint::new();
    fingerprint.field(b"whole-program rust run");
    fingerprint.field(build.as_bytes());
    fingerprint.field(manifest.replace(runtime.to_str()?, "<runtime>").as_bytes());
    fingerprint.tree(&dir.join("src"), &[])?;
    input_field(p, &mut fingerprint)?;
    Some(fingerprint.key())
}

/// The runtime crate, which the translated crates depend on.
fn runtime() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../runtime").canonicalize().unwrap()
}

#[test]
fn whole_programs_match_java() {
    let Some(library) = std::env::var_os("JAVA_BASE_SOURCES").map(PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to run these programs");
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/whole-program");
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("whole-program");
    let cache = check_cache::test_root();
    let (java_cache, rust_cache) = (cache.join("whole-program-java"), cache.join("whole-program-rust"));
    std::fs::create_dir_all(&work).unwrap();
    let build = check_cache::rust_build_key(&runtime(), &work);
    let filter = std::env::var("CORPUS_FILTER").ok();
    let programs = programs(&root, &work);
    let selected: Vec<&Program> =
        programs.iter().filter(|p| filter.as_ref().is_none_or(|f| p.name.contains(f.as_str()))).collect();
    let results = parallel(selected.len(), |i| {
        let p = selected[i];
        let java = run_java(p, &work.join(&p.name).join("classes"), &java_cache);
        let (rust, key) = run_rust(p, &library, &work, &rust_cache, build.as_deref(), None);
        match rust {
            Ok(rust) if rust == java => {
                if let (Some(key), Some(code)) = (key, rust.code) {
                    check_cache::store(&rust_cache, &key, format!("{code}\n{}", rust.stdout).as_bytes());
                }
                ("PASS".to_string(), None)
            }
            Ok(rust) => {
                ("WRONG OUTPUT".to_string(), Some(format!("{}: Java {java:?}\nRust {rust:?}", p.name)))
            }
            Err(why) => {
                let first = why.lines().next().unwrap_or_default().to_string();
                (first, Some(format!("{}: {why}", p.name)))
            }
        }
    });
    let mut failures = Vec::new();
    for (p, (verdict, failure)) in selected.iter().zip(results) {
        println!("{:<24} {verdict}", p.name);
        failures.extend(failure);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// A program split into a header crate and three body crates (`--crates`, `split.rs`) builds and
/// prints what Java prints. Its comparators are trait objects whose methods, with the JDK's
/// default methods, move into the body crates.
#[test]
fn a_program_split_into_crates_matches_java() {
    let Some(library) = std::env::var_os("JAVA_BASE_SOURCES").map(PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to run this program");
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/whole-program");
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("whole-program-split");
    std::fs::create_dir_all(&work).unwrap();
    let programs = programs(&root, &work);
    let p = programs.iter().find(|p| p.name == "comparators").expect("corpus/whole-program/Comparators.java");
    let cache = check_cache::test_root().join("whole-program-java");
    let java = run_java(p, &work.join(&p.name).join("classes"), &cache);
    let (rust, _) = run_rust(p, &library, &work, &work.join("unused"), None, Some(3));
    assert_eq!(rust, Ok(java));
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

/// The manifest attributes of a package of the program's unnamed module are those of the jar it
/// is run from, or null without one, which the translation does not know: asking for them throws
/// (`java/patches/java/lang/Package.java`), where those of the JDK's packages are null as in Java
/// (`corpus/whole-program/PackagesOfClasses`). Where the class is known to be the program's, the
/// call is rejected instead (`uncertain_package_lookups_are_rejected`).
#[test]
fn a_program_package_manifest_is_not_in_the_translation() {
    let Some(library) = std::env::var_os("JAVA_BASE_SOURCES").map(PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to run this program");
        return;
    };
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("whole-program-package-manifest");
    let sources = work.join("src");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&sources).unwrap();
    let java = "public class Main {
    public static void main(String[] args) {
        Class<?> c = args.length == 0 ? Main.class : String.class;
        System.out.println(c.getPackage().getImplementationVersion());
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
    assert!(rust.contains("the manifest attributes of package"), "no manifest's failure in the translation");
}

/// What a package knows of itself where the translation cannot know it, and packages by name
/// that may or may not have been loaded yet, are rejected (`lower/packages.rs`;
/// `corpus/whole-program/PackageLookups` has those that are certain).
#[test]
fn uncertain_package_lookups_are_rejected() {
    let Some(library) = std::env::var_os("JAVA_BASE_SOURCES").map(PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to run this program");
        return;
    };
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("whole-program-package-lookups");
    let not_loaded = "`Package.getPackage` of a package that the caller has not loaded";
    let manifest = "the manifest attributes of a package of the class path";
    let cases = [
        ("System.out.println(Package.getPackage(\"javax.swing\"));", not_loaded),
        ("System.out.println(Package.getPackage(args[0]));", not_loaded),
        (
            "System.out.println(Package.getPackages().length);",
            "`Package.getPackages`, which depends on the classes",
        ),
        (
            "System.out.println(Main.class.getClassLoader().getDefinedPackage(\"p\"));",
            "`ClassLoader.getDefinedPackage`, which depends on the classes",
        ),
        ("System.out.println(p.Main.class.getPackage().getImplementationVersion());", manifest),
        ("System.out.println(new Main().getClass().getPackage().isSealed());", manifest),
        ("System.out.println(Package.getPackage(\"p\").toString());", manifest),
        (
            "System.out.println(String.class.getPackage().getAnnotations());",
            "the annotations of a package (`Package.getAnnotations`)",
        ),
        (
            "Package pkg = Main.class.getPackage();\n        System.out.println(pkg != null ? pkg.getSpecificationTitle() : \"none\");",
            manifest,
        ),
    ];
    for (i, (statements, message)) in cases.iter().enumerate() {
        let sources = work.join(i.to_string());
        let _ = std::fs::remove_dir_all(&sources);
        std::fs::create_dir_all(sources.join("p")).unwrap();
        let java = format!(
            "package p;\npublic class Main {{\n    @SuppressWarnings(\"deprecation\")\n    \
             public static void main(String[] args) {{\n        {statements}\n    }}\n}}\n"
        );
        std::fs::write(sources.join("p/Main.java"), java).unwrap();
        let translated = Command::new(env!("CARGO_BIN_EXE_java-to-rust"))
            .args(["--main", "p.Main", "--source"])
            .arg(&sources)
            .arg("--library")
            .arg(&library)
            .arg("-o")
            .arg(work.join(format!("{i}.rs")))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&translated.stderr);
        assert!(!translated.status.success(), "translated: {statements}");
        assert!(stderr.contains(message), "not rejected so: {statements}\n{stderr}");
    }
}

/// The runtime's classes implement `java.lang.reflect.Type` and `Serializable` (see
/// `corpus/whole-program/ClassesAsReflectTypes.java`), but not `Constable`, whose
/// `describeConstable()` would need the class's descriptor as an object: a class converted to a
/// `Constable`, and an `Object` that may hold a class cast to one and tested for one, are
/// rejected, not translated as values that are never a `Constable`.
#[test]
fn a_class_as_a_constable_is_rejected() {
    let Some(library) = std::env::var_os("JAVA_BASE_SOURCES").map(PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to run this program");
        return;
    };
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("whole-program-class-as-constable");
    let statements = [
        "Constable c = String.class;\n        System.out.println(c.describeConstable().isPresent());",
        "Object value = args.length == 0 ? Integer.class : \"x\";\n        System.out.println(value instanceof Constable);",
        "Object value = args.length == 0 ? Integer.class : \"x\";\n        System.out.println(((Constable) value).describeConstable().isPresent());",
    ];
    for (i, statements) in statements.iter().enumerate() {
        let sources = work.join(i.to_string());
        let _ = std::fs::remove_dir_all(&sources);
        std::fs::create_dir_all(&sources).unwrap();
        let java = format!(
            "import java.lang.constant.Constable;\npublic class Main {{\n    \
             public static void main(String[] args) {{\n        {statements}\n    }}\n}}\n"
        );
        std::fs::write(sources.join("Main.java"), java).unwrap();
        let translated = Command::new(env!("CARGO_BIN_EXE_java-to-rust"))
            .args(["--main", "Main", "--source"])
            .arg(&sources)
            .arg("--library")
            .arg(&library)
            .arg("-o")
            .arg(work.join(format!("{i}.rs")))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&translated.stderr);
        assert!(!translated.status.success(), "translated:\n{statements}");
        assert!(
            stderr.contains("a `java.lang.Class` as a `java.lang.constant.Constable` not supported yet"),
            "not rejected for the class:\n{statements}\n{stderr}"
        );
    }
}

/// Handles held as values that lowering does not follow are rejected: an access through a handle
/// that may be null whose arguments do something before Java's null check, a handle kept as an
/// `Object`, and the `MethodHandle` of a method other than the handle factories.
#[test]
fn run_time_handles_not_followed_are_rejected() {
    let Some(library) = std::env::var_os("JAVA_BASE_SOURCES").map(PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to run this program");
        return;
    };
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("whole-program-run-time-handle-rejections");
    let cases = [
        (
            "pick(args.length == 0).set(b, next(), 1);",
            "`VarHandle.set` of a handle that may be null, with arguments that do something",
        ),
        ("System.out.println(java.util.List.of(pick(true)).size());", "a `VarHandle` used as a value"),
        (
            "MethodHandle m = MethodHandles.lookup().findVirtual(Object.class, \"toString\", \
             MethodType.methodType(String.class));",
            "`Lookup.findVirtual` of a handle or arguments that the translation does not follow",
        ),
    ];
    for (i, (statement, message)) in cases.iter().enumerate() {
        let sources = work.join(i.to_string());
        let _ = std::fs::remove_dir_all(&sources);
        std::fs::create_dir_all(&sources).unwrap();
        let java = format!(
            "import java.lang.invoke.*;\nimport java.nio.*;\npublic class Main {{\n    \
             static int next() {{ return 0; }}\n    \
             static VarHandle pick(boolean on) {{\n        \
             return on ? MethodHandles.byteArrayViewVarHandle(int[].class, ByteOrder.BIG_ENDIAN) : null;\n    }}\n    \
             public static void main(String[] args) throws Throwable {{\n        byte[] b = new byte[8];\n        \
             {statement}\n    }}\n}}\n"
        );
        std::fs::write(sources.join("Main.java"), java).unwrap();
        let translated = Command::new(env!("CARGO_BIN_EXE_java-to-rust"))
            .args(["--main", "Main", "--source"])
            .arg(&sources)
            .arg("--library")
            .arg(&library)
            .arg("-o")
            .arg(work.join(format!("{i}.rs")))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&translated.stderr);
        assert!(!translated.status.success(), "translated:\n{statement}");
        assert!(stderr.contains(message), "not rejected:\n{statement}\n{stderr}");
    }
}

/// `sun.misc.Unsafe`'s methods of raw memory, at addresses and in arrays at their offsets, which
/// netty uses: they warn once on standard error, naming the class of their caller and where it was
/// loaded from, after their arguments are evaluated and the receiver is checked, and
/// `invokeCleaner` checks that its buffer is direct before. The class is in `jdk.unsupported`,
/// whose sources the translation reads next to those of `java.base`. Both runs are from the
/// directory of Java's classes, the class path, and print the same on both streams.
#[test]
fn sun_misc_raw_memory_matches_java() {
    let Some(java_base) = std::env::var_os("JAVA_BASE_SOURCES").map(PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to run this program");
        return;
    };
    let unsupported = java_base.parent().unwrap().join("jdk.unsupported");
    if !unsupported.is_dir() {
        println!("skipped: no sources of jdk.unsupported next to those of java.base");
        return;
    }
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("whole-program-raw-memory");
    let _ = std::fs::remove_dir_all(&work);
    let sources = work.join("src");
    let classes = work.join("classes");
    std::fs::create_dir_all(&sources).unwrap();
    std::fs::write(sources.join("RawMemory.java"), RAW_MEMORY).unwrap();
    let javac =
        Command::new("javac").arg("-d").arg(&classes).arg(sources.join("RawMemory.java")).output().unwrap();
    assert!(javac.status.success(), "{}", String::from_utf8_lossy(&javac.stderr));
    let java = Command::new("java").args(["-cp", ".", "RawMemory"]).current_dir(&classes).output().unwrap();
    let dir = work.join("crate");
    let translated = Command::new(env!("CARGO_BIN_EXE_java-to-rust"))
        .args(["--main", "RawMemory", "--source"])
        .arg(&sources)
        .arg("--library")
        .arg(&java_base)
        .arg("--library")
        .arg(&unsupported)
        .arg("--crate")
        .arg(&dir)
        .output()
        .unwrap();
    assert!(translated.status.success(), "{}", String::from_utf8_lossy(&translated.stderr));
    // The target directory of the corpus's programs, which has the runtime built.
    let target = Path::new(env!("CARGO_TARGET_TMPDIR")).join("whole-program").join("cargo");
    let built = Command::new("cargo")
        .args(["build", "-q", "--manifest-path"])
        .arg(dir.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", &target)
        .output()
        .unwrap();
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    let rust = Command::new(target.join("debug").join("rawmemory")).current_dir(&classes).output().unwrap();
    for (stream, java, rust) in
        [("output", &java.stdout, &rust.stdout), ("error", &java.stderr, &rust.stderr)]
    {
        assert_eq!(
            String::from_utf8_lossy(rust),
            String::from_utf8_lossy(java),
            "standard {stream} differs from Java's"
        );
    }
    assert_eq!(rust.status.code(), java.status.code());
}

const RAW_MEMORY: &str = r#"import java.lang.reflect.Field;
import java.nio.ByteBuffer;
import sun.misc.Unsafe;

public class RawMemory {
    static final Unsafe U;
    static final long BYTES;
    static final long INTS;
    static final long INT_SCALE;
    static final long SCRATCH;
    static Unsafe none;

    static long size(long bytes) {
        System.err.println("size " + bytes);
        return bytes;
    }

    static {
        try {
            Field f = Unsafe.class.getDeclaredField("theUnsafe");
            f.setAccessible(true);
            U = (Unsafe) f.get(null);
        } catch (ReflectiveOperationException e) {
            throw new ExceptionInInitializerError(e);
        }
        SCRATCH = U.allocateMemory(size(64));
        BYTES = U.arrayBaseOffset(byte[].class);
        INTS = U.arrayBaseOffset(int[].class);
        INT_SCALE = U.arrayIndexScale(int[].class);
    }

    public static void main(String[] args) {
        System.out.println(U.addressSize() + " " + U.pageSize() + " " + BYTES + " " + INTS + " " + INT_SCALE);
        long a = SCRATCH;
        U.setMemory(a, 16, (byte) 0);
        U.putInt(a, 0x01020304);
        U.putLong(a + 8, -2L);
        U.putByte(a + 4, (byte) 7);
        U.putShort(a + 5, (short) -300);
        System.out.println(U.getByte(a) + " " + U.getInt(a) + " " + U.getLong(a + 8) + " " + U.getShort(a + 5));
        U.putChar(a + 16, 'x');
        U.putFloat(a + 20, 1.5f);
        U.putDouble(a + 24, -0.25);
        System.out.println(U.getChar(a + 16) + " " + U.getFloat(a + 20) + " " + U.getDouble(a + 24));
        U.putAddress(a + 32, a);
        System.out.println(U.getAddress(a + 32) == a);
        byte[] bytes = new byte[16];
        U.copyMemory(null, a, bytes, BYTES, 16);
        System.out.println(java.util.Arrays.toString(bytes));
        System.out.println(U.getInt(bytes, BYTES) + " " + U.getLong(bytes, BYTES + 8));
        U.putByte(bytes, BYTES + 1, (byte) 9);
        long b = U.allocateMemory(4);
        U.copyMemory(bytes, BYTES, null, b, 4);
        System.out.println(U.getInt(b));
        long c = U.reallocateMemory(b, 32);
        U.copyMemory(a, c + 4, 16);
        System.out.println(U.getInt(c) + " " + U.getLong(c + 12));
        int[] ints = {5, 6, 7};
        System.out.println(U.getInt(ints, INTS + INT_SCALE * 2));
        U.freeMemory(c);
        U.setMemory(bytes, BYTES, 4, (byte) 1);
        System.out.println(java.util.Arrays.toString(bytes));
        try {
            U.invokeCleaner(ByteBuffer.allocate(8));
        } catch (IllegalArgumentException e) {
            System.out.println(e);
        }
        ByteBuffer direct = ByteBuffer.allocateDirect(8);
        try {
            U.invokeCleaner(direct.slice());
        } catch (IllegalArgumentException e) {
            System.out.println(e);
        }
        U.invokeCleaner(direct);
        System.out.println("cleaned");
        try {
            none.getInt(a);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            none.pageSize();
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        U.freeMemory(a);
    }
}
"#;

/// The accesses of a view handle of a `ByteBuffer` that the translation does not do: the atomic
/// updates, and a read or write at a type the handle's would have to narrow to, for which Java
/// throws `WrongMethodTypeException`.
#[test]
fn buffer_view_updates_and_narrowings_are_rejected() {
    let Some(library) = std::env::var_os("JAVA_BASE_SOURCES").map(PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to run this program");
        return;
    };
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("whole-program-buffer-view-rejections");
    let cases = [
        (
            "System.out.println(INTS.compareAndSet(b, 0, 0, 1));",
            "`VarHandle.compareAndSet` of a view of a byte buffer",
        ),
        ("System.out.println(INTS.getAndAdd(b, 0, 1));", "`VarHandle.getAndAdd` of a view of a byte buffer"),
        (
            "System.out.println((short) INTS.get(b, 0));",
            "`VarHandle.get` of a view at another type than its own",
        ),
        ("INTS.set(b, 0, 1L);", "`VarHandle.set` of a view at another type than its own"),
    ];
    for (i, (statement, message)) in cases.iter().enumerate() {
        let sources = work.join(i.to_string());
        let _ = std::fs::remove_dir_all(&sources);
        std::fs::create_dir_all(&sources).unwrap();
        let java = format!(
            "import java.lang.invoke.*;\nimport java.nio.*;\npublic class Main {{\n    \
             static final VarHandle INTS = MethodHandles.byteBufferViewVarHandle(int[].class, ByteOrder.BIG_ENDIAN);\n    \
             public static void main(String[] args) {{\n        ByteBuffer b = ByteBuffer.allocateDirect(8);\n        \
             {statement}\n    }}\n}}\n"
        );
        std::fs::write(sources.join("Main.java"), java).unwrap();
        let translated = Command::new(env!("CARGO_BIN_EXE_java-to-rust"))
            .args(["--main", "Main", "--source"])
            .arg(&sources)
            .arg("--library")
            .arg(&library)
            .arg("-o")
            .arg(work.join(format!("{i}.rs")))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&translated.stderr);
        assert!(!translated.status.success(), "translated:\n{statement}");
        assert!(
            stderr.contains(&format!("{message} not supported yet")),
            "not rejected:\n{statement}\n{stderr}"
        );
    }
}

/// A program whose source root and libraries are modules, with a `module-info.java` each, as the
/// JDK's own are: the libraries' sources are read and translated as those of a program without
/// modules are, and a module that the program uses but that is not given is named in the error.
#[test]
fn programs_and_libraries_that_are_modules() {
    let Some(java_base) = std::env::var_os("JAVA_BASE_SOURCES").map(PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to run this program");
        return;
    };
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("whole-program-modules");
    let _ = std::fs::remove_dir_all(&work);
    let files = [
        ("app/module-info.java", "module com.example.app {\n    requires org.lib;\n}\n"),
        (
            "app/com/example/app/Main.java",
            "package com.example.app;\nimport java.util.ArrayList;\nimport org.lib.text.Shout;\npublic class Main {\n    public static void main(String[] a) {\n        ArrayList<String> words = new ArrayList<>();\n        words.add(Shout.shout(\"hi\"));\n        System.out.println(words.get(0));\n    }\n}\n",
        ),
        ("lib/module-info.java", "module org.lib {\n    exports org.lib.text;\n}\n"),
        (
            "lib/org/lib/text/Shout.java",
            "package org.lib.text;\npublic class Shout {\n    public static String shout(String s) { return s + \"!\"; }\n}\n",
        ),
        (
            "logging/p/Main.java",
            "package p;\npublic class Main {\n    public static void main(String[] a) {\n        System.out.println(java.util.logging.Logger.getGlobal());\n    }\n}\n",
        ),
    ];
    for (path, java) in files {
        let path = work.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, java).unwrap();
    }
    let translate = |main: &str, source: &str, libraries: &[&Path]| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_java-to-rust"));
        command.args(["--main", main, "--source"]).arg(work.join(source));
        for library in libraries {
            command.arg("--library").arg(library);
        }
        command.arg("-o").arg(work.join("main.rs")).output().unwrap()
    };
    let translated = translate("com.example.app.Main", "app", &[&work.join("lib"), &java_base]);
    assert!(translated.status.success(), "{}", String::from_utf8_lossy(&translated.stderr));
    let rust = std::fs::read_to_string(work.join("main.rs")).unwrap();
    // The JDK's `ArrayList` and the library's class, translated from their sources.
    for wanted in ["pub(crate) struct ArrayList", "pub(crate) fn shout(s: &java::Wtf8) -> java::JString"] {
        assert!(rust.contains(wanted), "no {wanted} in the translation");
    }
    let translated = translate("p.Main", "logging", &[&java_base]);
    let stderr = String::from_utf8_lossy(&translated.stderr);
    assert!(
        stderr.contains(
            "package java.util.logging is in module java.logging, whose sources are not given: pass the root of its sources as --library"
        ),
        "{stderr}"
    );
}

/// `System.loadLibrary` and `System.load` (`java/lang/TranslatedLibraries.java`), which Java lets
/// code of a module without native access call only after a warning naming where the caller's
/// class was loaded from: translated in a module of the JDK that has native access (here a root
/// that declares itself `java.prefs`, as the JDK's sources do), and rejected in the unnamed
/// module, which Java would warn for.
#[test]
fn loading_native_libraries_needs_native_access() {
    let Some(java_base) = std::env::var_os("JAVA_BASE_SOURCES").map(PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to run this program");
        return;
    };
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("whole-program-native-libraries");
    let _ = std::fs::remove_dir_all(&work);
    let main = "package p;\npublic class Main {\n    public static void main(String[] a) {\n        \
                System.loadLibrary(\"zip\");\n        System.load(\"/nope/libx.so\");\n    }\n}\n";
    let files = [
        ("jdk/module-info.java", "module java.prefs {\n}\n"),
        ("jdk/p/Main.java", main),
        ("unnamed/p/Main.java", main),
    ];
    for (path, java) in files {
        let path = work.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, java).unwrap();
    }
    let translate = |source: &str| {
        Command::new(env!("CARGO_BIN_EXE_java-to-rust"))
            .args(["--main", "p.Main", "--source"])
            .arg(work.join(source))
            .arg("--library")
            .arg(&java_base)
            .arg("-o")
            .arg(work.join(format!("{source}.rs")))
            .output()
            .unwrap()
    };
    let translated = translate("jdk");
    assert!(translated.status.success(), "{}", String::from_utf8_lossy(&translated.stderr));
    let rust = std::fs::read_to_string(work.join("jdk.rs")).unwrap();
    for wanted in ["fn load_library(", "fn load("] {
        assert!(rust.contains(wanted), "no {wanted} in the translation");
    }
    let translated = translate("unnamed");
    let stderr = String::from_utf8_lossy(&translated.stderr);
    assert!(
        stderr.contains("`java.lang.System.loadLibrary` outside the JDK's modules with native access"),
        "{stderr}"
    );
}

/// What a translation's proxies cannot do throws (`java/overlay/java/lang/reflect/TranslatedProxies.java`,
/// `corpus/whole-program/Proxies.java` for what they do): a list of interfaces computed at run
/// time has no proxy class, and a proxy class in a dynamic module has no `Module`.
#[test]
fn what_proxies_cannot_do_throws() {
    let Some(library) = std::env::var_os("JAVA_BASE_SOURCES").map(PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to run this program");
        return;
    };
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("whole-program-proxies");
    let sources = work.join("src");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&sources).unwrap();
    let java = "import java.lang.reflect.Proxy;
public class Main {
    public interface Named { String name(); }
    public static void main(String[] args) throws Exception {
        Class<?>[] list = new Class<?>[args.length];
        for (int i = 0; i < list.length; i++) {
            list[i] = Class.forName(args[i]);
        }
        Object computed = Proxy.newProxyInstance(Main.class.getClassLoader(), list, (p, m, a) -> null);
        Object named = Proxy.newProxyInstance(Main.class.getClassLoader(), new Class<?>[] {Named.class}, (p, m, a) -> \"n\");
        System.out.println(computed + \" \" + named.getClass().getModule());
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
    for wanted in ["a proxy class of ", "the module of a proxy class, a dynamic module"] {
        assert!(rust.contains(wanted), "no {wanted} in the translation");
    }
}
