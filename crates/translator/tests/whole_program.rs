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
//! directory) on standard input, and nothing without one. A program that uses another module of
//! the JDK (`sun.misc`, `jdk.net`) is translated with that module's
//! sources too, from the folder next to `java.base`'s, and skipped without them
//! (`module_libraries`).
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

/// The packages of `java.base` that are not exported but that the programs may use: Java and the
/// translator are given them.
const INTERNAL_EXPORTS: [&str; 2] = ["--add-exports", "java.base/sun.util.logging=ALL-UNNAMED"];

/// The packages of `java.base` that the programs are compiled against but run without, as a
/// library built against the JDK's internals is: using them throws `IllegalAccessError`.
const COMPILE_ONLY_EXPORTS: [&str; 4] = [
    "--add-exports",
    "java.base/jdk.internal.misc=ALL-UNNAMED",
    "--add-exports",
    "java.base/jdk.internal.util=ALL-UNNAMED",
];

/// The program's run on Java, compiled with javac into `classes` first, or its result stored in
/// `cache` by an earlier run with the same sources, input and Java version.
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
        .args(COMPILE_ONLY_EXPORTS)
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
    // Both runs leave out the library path that cargo sets for the test, which Java and the
    // translation read into `java.library.path`.
    java.env_remove("LD_LIBRARY_PATH");
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

/// The sources of the JDK's other modules, next to `java.base`'s, that a program uses: those of
/// `jdk.unsupported` for `sun.misc.Unsafe` and the other classes of `sun.misc`, of `jdk.net` for
/// its socket options, and of `java.logging` for `java.util.logging`.
/// A program whose modules' sources are missing is skipped (`whole_programs_match_java`).
fn module_libraries(p: &Program, library: &Path) -> Vec<PathBuf> {
    const MODULES: [(&str, &str); 3] =
        [("jdk.unsupported", "sun.misc."), ("jdk.net", "jdk.net."), ("java.logging", "java.util.logging.")];
    fn uses(dir: &Path, marker: &str) -> bool {
        std::fs::read_dir(dir).into_iter().flatten().flatten().any(|e| {
            let path = e.path();
            if path.is_dir() {
                return uses(&path, marker);
            }
            path.extension().is_some_and(|x| x == "java")
                && std::fs::read_to_string(&path).is_ok_and(|text| text.contains(marker))
        })
    }
    let Some(parent) = library.parent() else { return Vec::new() };
    MODULES
        .iter()
        .filter(|(_, marker)| uses(&p.sources, marker))
        .map(|(module, _)| parent.join(module))
        .collect()
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
        .args(INTERNAL_EXPORTS)
        .arg("--source")
        .arg(&p.sources)
        .arg("--library")
        .arg(library)
        .args(module_libraries(p, library).into_iter().flat_map(|l| ["--library".into(), l.into_os_string()]))
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
    (Ok(run(Command::new(binary).env_remove("LD_LIBRARY_PATH").stdin(stdin_of(p)).output().unwrap())), key)
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
    let mut selected: Vec<&Program> =
        programs.iter().filter(|p| filter.as_ref().is_none_or(|f| p.name.contains(f.as_str()))).collect();
    selected.retain(|p| match module_libraries(p, &library).into_iter().find(|root| !root.is_dir()) {
        Some(missing) => {
            println!("{:<24} skipped: no sources of {} next to java.base's", p.name, missing.display());
            false
        }
        None => true,
    });
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

/// Array types that become `Object[]`s one after another: `Block[]` meets the `Object[]` it is
/// stored in; then `(Block[]) other` casts an `Object` to an `Object[]`, which the `Item[]` held
/// as an `Object` may be; and `String[][]` with `String[]` noted is an `Object[][]`, which meets
/// the `Object[]` it is stored in. The first lowering notes them all, the later steps by meeting
/// again the types that met (`lower::covariant`), so the program is lowered twice, not four times;
/// once, with the arrays predicted before it (`lower::predict`).
const COVARIANT_STEPS: &str = r#"
public class Main {
    static class Item { String name; Item(String n) { name = n; } }
    static class Block { int id; Block(int i) { id = i; } }

    static Object hold(Object o) { return o; }

    public static void main(String[] args) {
        Item[] items = { new Item("a"), new Item("b") };
        Block[] blocks = { new Block(1) };
        Object[] objects = blocks;
        Object held = hold(items);
        Object other = hold(blocks);
        Block[] back = (Block[]) other;
        System.out.println(objects.length + " " + back[0].id + " " + (held instanceof Block[]));
        String[][] grid = { { "x" }, { "y", "z" } };
        Object[] rows = grid;
        System.out.println(rows.length + " " + ((String[]) rows[1])[1]);
    }
}
"#;

#[test]
fn covariant_arrays_found_step_by_step_take_one_more_lowering() {
    let Some(library) = std::env::var_os("JAVA_BASE_SOURCES").map(PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to run this program");
        return;
    };
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("whole-program-covariant-steps");
    let sources = work.join("src");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&sources).unwrap();
    std::fs::write(sources.join("Main.java"), COVARIANT_STEPS).unwrap();
    let (stderr, unpredicted) = translate_counted(&sources, &library, &work, Some("1"));
    assert_eq!(attempts(&stderr), 2, "{stderr}");
    assert_predicted_all(&stderr);
    let (stderr, predicted) = translate_counted(&sources, &library, &work, None);
    assert_eq!(attempts(&stderr), 1, "{stderr}");
    assert_eq!(predicted, unpredicted);
}

/// The translation of the program in `sources` with `--timings`, with
/// `JAVA_TO_RUST_PREDICTION_CHECK` set to `check`: its timings and its Rust.
fn translate_counted(sources: &Path, library: &Path, work: &Path, check: Option<&str>) -> (String, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_java-to-rust"));
    if let Some(check) = check {
        command.env("JAVA_TO_RUST_PREDICTION_CHECK", check);
    }
    let translated = command
        .args(["--timings", "--main", "Main", "--source"])
        .arg(sources)
        .arg("--library")
        .arg(library)
        .arg("-o")
        .arg(work.join("main.rs"))
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&translated.stderr).into_owned();
    assert!(translated.status.success(), "the translation failed:\n{stderr}");
    (stderr, std::fs::read_to_string(work.join("main.rs")).unwrap())
}

/// How many times the timings `stderr` say the program was lowered: `timing: lowering attempt 2:
/// 0.001 s, ...`.
fn attempts(stderr: &str) -> usize {
    stderr
        .lines()
        .filter(|l| l.strip_prefix("timing: lowering attempt ").is_some_and(|l| l.contains(": ")))
        .count()
}

/// The checked prediction's line of the timings `stderr`, whose array types and Rust generics it
/// predicted all (`N of N`) and no more: `timing: prediction: 3 of 3 covariant array types, 0
/// more; 2 of 2 failed Rust generics, 0 more`.
fn assert_predicted_all(stderr: &str) {
    let line = stderr
        .lines()
        .find(|l| l.starts_with("timing: prediction: ") && l.contains(" of "))
        .unwrap_or_else(|| panic!("no prediction checked:\n{stderr}"));
    for half in line.trim_start_matches("timing: prediction: ").split("; ") {
        let numbers: Vec<&str> = half.split(' ').collect();
        assert_eq!(numbers[0], numbers[2], "{line}\n{stderr}");
        assert!(half.ends_with(", 0 more"), "{line}\n{stderr}");
    }
    assert!(!stderr.contains("prediction missed"), "{stderr}");
}

/// Arrays that meet where lowering makes code of its own, not where the program converts them:
/// the old `Tag[]` that `getAndSet` of a `VarHandle` gives as an `Object`, read from the field
/// (the call itself passes no array), meets the `(Object[])` cast; and the `Label[]` that `Method.invoke` gets in its `Object[]`
/// meets the `Named[]` that the table of the program's methods casts it to for the call
/// (`lower::reflection`); and the `Step[]` that `Arrays.copyOf(.., Object[].class)` copies, which
/// lowering copies as it is, meets the copy's `Object[]`, as in the queue of a
/// `ScheduledThreadPoolExecutor`. The prediction finds them all, and the program is lowered once.
const ARRAYS_IN_LOWERED_CODE: &str = r#"
import java.lang.invoke.MethodHandles;
import java.lang.invoke.VarHandle;
import java.lang.reflect.Method;
import java.util.Arrays;

public class Main {
    static class Step {}

    static class Tag {
        final String name;
        Tag(String name) { this.name = name; }
    }

    interface Named {}

    static class Label implements Named {}

    static class Shelf {
        Tag[] tags = { new Tag("a") };
    }

    static final VarHandle TAGS;
    static {
        try {
            TAGS = MethodHandles.lookup().findVarHandle(Shelf.class, "tags", Tag[].class);
        } catch (ReflectiveOperationException e) {
            throw new ExceptionInInitializerError(e);
        }
    }

    public static int count(Named[] names) { return names.length; }

    public static void main(String[] args) throws Exception {
        Shelf shelf = new Shelf();
        Object old = TAGS.getAndSet(shelf, null);
        Object[] back = (Object[]) old;
        System.out.println(back.length + " " + (shelf.tags == null));
        Method count = Main.class.getMethod("count", Named[].class);
        System.out.println(count.invoke(null, new Object[] { new Label[2] }));
        Step[] steps = { new Step(), new Step() };
        System.out.println(Arrays.copyOf(steps, 1, Object[].class).length);
    }
}
"#;

#[test]
fn arrays_held_as_objects_in_lowered_code_are_predicted() {
    let Some(library) = std::env::var_os("JAVA_BASE_SOURCES").map(PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to run this program");
        return;
    };
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("whole-program-arrays-in-lowered-code");
    let sources = work.join("src");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&sources).unwrap();
    std::fs::write(sources.join("Main.java"), ARRAYS_IN_LOWERED_CODE).unwrap();
    let (stderr, unpredicted) = translate_counted(&sources, &library, &work, Some("1"));
    let again = stderr.lines().find(|l| l.contains("is lowered again for")).unwrap_or_default();
    for array in ["Main.Tag[]", "Main.Label[]", "Main.Named[]", "Main.Step[]"] {
        assert!(again.contains(array), "{array} not noted:\n{stderr}");
    }
    assert_predicted_all(&stderr);
    let (stderr, predicted) = translate_counted(&sources, &library, &work, None);
    assert_eq!(attempts(&stderr), 1, "{stderr}");
    assert_eq!(predicted, unpredicted);
}

/// Arrays that a lambda's or a method reference's method returns as the descriptor's return
/// type: the `Mode[]` of `Mode::values` as the `E[]` of a `Supplier<E[]>`, as Minecraft passes its
/// enums' constants to `StringRepresentable.fromEnum`, and the `Shade[]` and `Size[]` that a
/// lambda's expression and `return` give as `Object[]`s. The prediction finds them all, and the
/// program is lowered once.
const ARRAYS_FROM_LAMBDAS: &str = r#"
import java.util.function.Supplier;

public class Main {
    interface Named { String serialName(); }

    enum Mode implements Named {
        FAST, SLOW;
        public String serialName() { return name().toLowerCase(); }
    }

    enum Shade { LIGHT, DARK }

    enum Size { SMALL, LARGE }

    static <E extends Enum<E> & Named> String names(Supplier<E[]> values) {
        StringBuilder out = new StringBuilder();
        for (E e : values.get()) out.append(e.serialName()).append(' ');
        return out.toString().trim();
    }

    static int count(Supplier<Object[]> values) { return values.get().length; }

    public static void main(String[] args) {
        System.out.println(names(Mode::values));
        System.out.println(count(() -> Shade.values()));
        System.out.println(count(() -> {
            Size[] sizes = Size.values();
            return sizes;
        }));
    }
}
"#;

#[test]
fn arrays_that_lambdas_return_are_predicted() {
    let Some(library) = std::env::var_os("JAVA_BASE_SOURCES").map(PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to run this program");
        return;
    };
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("whole-program-arrays-from-lambdas");
    let sources = work.join("src");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&sources).unwrap();
    std::fs::write(sources.join("Main.java"), ARRAYS_FROM_LAMBDAS).unwrap();
    let (stderr, unpredicted) = translate_counted(&sources, &library, &work, Some("1"));
    let again = stderr.lines().find(|l| l.contains("is lowered again for")).unwrap_or_default();
    for array in ["Main.Mode[]", "Main.Shade[]", "Main.Size[]"] {
        assert!(again.contains(array), "{array} not noted:\n{stderr}");
    }
    assert_predicted_all(&stderr);
    let (stderr, predicted) = translate_counted(&sources, &library, &work, None);
    assert_eq!(attempts(&stderr), 1, "{stderr}");
    assert_eq!(predicted, unpredicted);
}

/// `Box<String>[]` becomes an `Object[]` (`asList`'s `T[]`), whose elements are `Object`s, which
/// the objects of the Rust generic class `Box` cannot be: `Box` is erased in the lowering that
/// notes the array, and its array, `Box[]` once erased, is noted with it. The program is lowered
/// twice, not once more for `Box` and again for `Box[]`.
const ARRAY_OF_A_GENERIC_CLASS: &str = r#"
import java.util.Arrays;
import java.util.List;

public class Main {
    static class Box<U> {
        U value;
        Box(U value) { this.value = value; }
    }

    @SafeVarargs
    static int count(Box<String>... boxes) {
        List<Box<String>> list = Arrays.asList(boxes);
        return list.size();
    }

    public static void main(String[] args) {
        System.out.println(count(new Box<>("a"), new Box<>("b")));
    }
}
"#;

#[test]
fn a_covariant_array_of_a_rust_generic_class_erases_it_in_the_same_lowering() {
    let Some(library) = std::env::var_os("JAVA_BASE_SOURCES").map(PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to run this program");
        return;
    };
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("whole-program-generic-class-array");
    let sources = work.join("src");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&sources).unwrap();
    std::fs::write(sources.join("Main.java"), ARRAY_OF_A_GENERIC_CLASS).unwrap();
    // Without the prediction, which fails `Box` and notes `Box[]` before the first lowering.
    let (stderr, unpredicted) = translate_counted(&sources, &library, &work, Some("unseeded"));
    assert_eq!(attempts(&stderr), 2, "{stderr}");
    assert!(stderr.contains("(Main.Box<String>[], Main.Box[])"), "{stderr}");
    // The arrays are predicted with `Box` failed, as the lowerings find them with it.
    let (stderr, _) = translate_counted(&sources, &library, &work, Some("1"));
    assert_predicted_all(&stderr);
    let (stderr, predicted) = translate_counted(&sources, &library, &work, None);
    assert_eq!(attempts(&stderr), 1, "{stderr}");
    assert_eq!(predicted, unpredicted);
}

/// A Rust generic whose values go to `System.arraycopy` (`copy`) is failed where lowering meets
/// it, so the lowering that notes `Block[]` drops it too: the program is lowered twice. Were the
/// call rejected instead, the next lowering would erase every Rust generic, `requireNonNull`'s
/// among them, whose `Query[]` would then become an `Object` that meets the `(Object[])` cast,
/// and be lowered twice more. With the prediction, which fails `copy` and notes `Block[]` before,
/// it is lowered once.
const ARRAYCOPY_OF_A_TYPE_VARIABLE: &str = r#"
import java.util.Objects;

public class Main {
    static class Block { int id; Block(int i) { id = i; } }
    static class Query { String name; Query(String n) { name = n; } }

    static <T> T copy(T source, T dest, int length) {
        System.arraycopy(source, 0, dest, 0, length);
        return dest;
    }

    static <T> T pick(T a, T b, boolean first) { return first ? a : b; }

    static int count(Query... queries) {
        Objects.requireNonNull(queries, "queries");
        return queries.length;
    }

    static Object hold(Object o) { return o; }

    public static void main(String[] args) {
        Block[] blocks = { new Block(1) };
        Object[] objects = blocks;
        int[] copied = copy(new int[] { 1, 2, 3 }, new int[3], 3);
        Object held = hold(new Object[] { "x" });
        Object[] back = (Object[]) held;
        System.out.println(objects.length + " " + copied[2] + " " + back[0] + " " + count(new Query("q"))
            + " " + pick("a", "b", false));
    }
}
"#;

/// A rejection that can run, in code that uses the Rust generic `id`, erases `id` alone: `pick`
/// stays a Rust generic.
const REJECTED_WITH_A_RUST_GENERIC: &str = r#"
public class Main {
    static <T> T id(T x) { return x; }

    static <T> T pick(T a, T b, boolean first) { return first ? a : b; }

    static void odd() {
        String s = id("a");
        System.out.println(s + System.identityHashCode(5));
    }

    public static void main(String[] args) {
        System.out.println(pick("a", "b", false));
        odd();
    }
}
"#;

/// A rejection that can run in code that uses the Rust generic `id`, but that no type changes:
/// asking for the caller in a method that is not static. The program is lowered once.
const CALLER_WITH_A_RUST_GENERIC: &str = r#"
public class Main {
    static <T> T id(T x) { return x; }

    String odd() {
        String s = id("a");
        return s + StackWalker.getInstance(StackWalker.Option.RETAIN_CLASS_REFERENCE).getCallerClass().getName();
    }

    public static void main(String[] args) {
        System.out.println(new Main().odd());
    }
}
"#;

#[test]
fn a_rust_generic_that_fails_erases_only_itself() {
    let Some(library) = std::env::var_os("JAVA_BASE_SOURCES").map(PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to run this program");
        return;
    };
    // Without the prediction, unless `predicted`.
    let translate_as = |name: &str, java: &str, predicted: bool| {
        let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
        let sources = work.join("src");
        let _ = std::fs::remove_dir_all(&work);
        std::fs::create_dir_all(&sources).unwrap();
        std::fs::write(sources.join("Main.java"), java).unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_java-to-rust"));
        if !predicted {
            command.env("JAVA_TO_RUST_PREDICTION_CHECK", "unseeded");
        }
        let translated = command
            .args(["--timings", "--main", "Main", "--source"])
            .arg(&sources)
            .arg("--library")
            .arg(&library)
            .arg("-o")
            .arg(work.join("main.rs"))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&translated.stderr).into_owned();
        (
            translated.status.success(),
            stderr,
            std::fs::read_to_string(work.join("main.rs")).unwrap_or_default(),
        )
    };
    let translate = |name: &str, java: &str| translate_as(name, java, false);
    let (ok, stderr, rust) = translate("whole-program-arraycopy-generic", ARRAYCOPY_OF_A_TYPE_VARIABLE);
    assert!(ok, "the translation failed:\n{stderr}");
    assert_eq!(attempts(&stderr), 2, "{stderr}");
    for generic in ["fn pick<T", "fn require_non_null<T"] {
        assert!(rust.contains(generic), "no {generic} in the translation:\n{stderr}");
    }
    let (ok, stderr, predicted) =
        translate_as("whole-program-arraycopy-generic-predicted", ARRAYCOPY_OF_A_TYPE_VARIABLE, true);
    assert!(ok, "the translation failed:\n{stderr}");
    assert_eq!(attempts(&stderr), 1, "{stderr}");
    assert_eq!(predicted, rust);
    let (_, stderr, _) = translate("whole-program-rejected-generic", REJECTED_WITH_A_RUST_GENERIC);
    assert!(
        stderr.contains("is lowered again without 0 of 0 Rust generic classes and 1 of 2 methods (Main.id)"),
        "{stderr}"
    );
    let (_, stderr, _) = translate("whole-program-caller-generic", CALLER_WITH_A_RUST_GENERIC);
    assert!(stderr.contains("getCallerClass"), "{stderr}");
    assert!(!stderr.contains("is lowered again"), "{stderr}");
}

/// A resource of a class of the JDK is not in the translation: `Character.getName` reads
/// `uniName.dat`, a data file of the JDK that is generated when the JDK is built and is not in
/// its sources, and the program ends there, naming it (`TranslatedModules.classResource`). The
/// program's own resources are read from its roots (`corpus/whole-program/ClassPathResources`).
#[test]
fn a_resource_of_the_jdk_ends_the_program() {
    let Some(library) = std::env::var_os("JAVA_BASE_SOURCES").map(PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to run this program");
        return;
    };
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("whole-program-resource");
    let sources = work.join("src");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&sources).unwrap();
    let java = "public class Main {
    public static void main(String[] args) {
        System.out.println(Character.getName('A'));
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
    for wanted in ["is not part of the translated program", "\"uniName.dat\""] {
        assert!(rust.contains(wanted), "no {wanted} in the translation");
    }
}

/// The manifest attributes of a package of the program's unnamed module are those of the
/// manifest of its root (`META-INF/MANIFEST.MF`, that of the jar its classes come from): from
/// the package's own section, else from the main attributes. A root without one is a directory
/// of the class path, whose packages have none, as those of the JDK's packages are null in Java
/// (`corpus/whole-program/PackagesOfClasses`).
#[test]
fn a_program_package_has_the_manifest_of_its_root() {
    let Some(java_base) = std::env::var_os("JAVA_BASE_SOURCES").map(PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to run this program");
        return;
    };
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("whole-program-package-manifest");
    let _ = std::fs::remove_dir_all(&work);
    let files = [
        (
            "app/META-INF/MANIFEST.MF",
            "Manifest-Version: 1.0\r\nSpecification-Title: Shapes Ap\r\n p\r\nSpecification-Version: 2.1\r\n\
             Implementation-Version: 2.1.7\r\n\r\nName: p/inner/\r\nSpecification-Title: Inner\r\n\r\n",
        ),
        ("app/p/inner/Shape.java", "package p.inner;\npublic class Shape {\n}\n"),
        ("lib/q/Helper.java", "package q;\npublic class Helper {\n}\n"),
        (
            "app/p/Main.java",
            "package p;\npublic class Main {\n    static String show(Package p) {\n        \
             return p.getSpecificationTitle() + \" \" + p.getSpecificationVersion() + \" \" + p.getImplementationVersion() \
             + \" \" + p.getImplementationVendor() + \" \" + p.isCompatibleWith(\"2.0\");\n    }\n    \
             public static void main(String[] a) {\n        Package pkg = Main.class.getPackage();\n        \
             System.out.println(show(pkg) + \" | \" + pkg);\n        \
             System.out.println(show(p.inner.Shape.class.getPackage()));\n        \
             System.out.println(q.Helper.class.getPackage().getImplementationVersion());\n    }\n}\n",
        ),
    ];
    for (path, text) in files {
        let path = work.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    let translated = Command::new(env!("CARGO_BIN_EXE_java-to-rust"))
        .args(["--main", "p.Main", "--source"])
        .arg(work.join("app"))
        .arg("--library")
        .arg(work.join("lib"))
        .arg("--library")
        .arg(&java_base)
        .arg("--crate")
        .arg(work.join("crate"))
        .output()
        .unwrap();
    assert!(translated.status.success(), "{}", String::from_utf8_lossy(&translated.stderr));
    let built = Command::new("cargo")
        .args(["run", "-q", "--manifest-path"])
        .arg(work.join("crate/Cargo.toml"))
        .env("CARGO_TARGET_DIR", work.join("target"))
        .output()
        .unwrap();
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    assert_eq!(
        String::from_utf8_lossy(&built.stdout),
        "Shapes App 2.1 2.1.7 null true | package p, Shapes App, version 2.1\n\
         Inner 2.1 2.1.7 null true\n\
         null\n"
    );
}

/// A package's annotations, which the translation does not have, and packages by name
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
        (
            "System.out.println(String.class.getPackage().getAnnotations());",
            "the annotations of a package (`Package.getAnnotations`)",
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

/// What a translation's secure random numbers cannot do throws (see
/// `corpus/whole-program/SecureRandoms.java` for what they do): `DRBG`, whose class the SUN
/// provider registers but the translation leaves out, so its lookup fails as one of a class
/// missing from the class path does, and a seed source that is not a file, which would open a
/// connection (the translation's `java.security.egd` is unset, but the seed generator's code
/// reads it).
#[test]
fn what_secure_randoms_cannot_do_throws() {
    let Some(library) = std::env::var_os("JAVA_BASE_SOURCES").map(PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to run this program");
        return;
    };
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("whole-program-secure-randoms");
    let sources = work.join("src");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&sources).unwrap();
    let java = "import java.security.SecureRandom;
public class Main {
    public static void main(String[] args) throws Exception {
        System.out.println(SecureRandom.getInstance(\"SHA1PRNG\").generateSeed(4).length);
        System.out.println(SecureRandom.getInstance(\"DRBG\").nextInt());
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
    for wanted in [
        "\"sun.security.provider.DRBG\"",
        ", which is not in the translation",
        "not a file, which the translation does not read",
    ] {
        assert!(rust.contains(wanted), "no {wanted} in the translation");
    }
}

/// A library's annotation type that reflection reads on a member is lowered from its
/// declaration, though only an annotation names it: jspecify's `@Nullable`, on the type only
/// (`@Target(TYPE_USE)`), is never among a field's annotations, as log4j's `PluginBuilder` reads
/// them, while the library's `@Attribute` is.
#[test]
fn a_library_annotation_on_the_type_only_is_not_a_members() {
    let Some(library) = std::env::var_os("JAVA_BASE_SOURCES").map(PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to run this program");
        return;
    };
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("whole-program-library-annotations");
    let _ = std::fs::remove_dir_all(&work);
    let (sources, annotations) = (work.join("src"), work.join("lib"));
    std::fs::create_dir_all(&sources).unwrap();
    std::fs::create_dir_all(annotations.join("org/example")).unwrap();
    for (name, target) in [("Nullable", "TYPE_USE"), ("Attribute", "FIELD")] {
        let value = if name == "Attribute" { "String value();" } else { "" };
        let java = format!(
            "package org.example;\nimport java.lang.annotation.*;\n\
             @Retention(RetentionPolicy.RUNTIME)\n@Target(ElementType.{target})\n\
             public @interface {name} {{ {value} }}\n"
        );
        std::fs::write(annotations.join(format!("org/example/{name}.java")), java).unwrap();
    }
    let java = "import java.lang.annotation.Annotation;
import org.example.Attribute;
import org.example.Nullable;
public class Main {
    static class Builder {
        @Attribute(\"name\") @Nullable String name;
        @Nullable Integer size;
    }
    public static void main(String[] args) throws Exception {
        for (String n : new String[] {\"name\", \"size\"}) {
            for (Annotation a : Builder.class.getDeclaredField(n).getDeclaredAnnotations()) {
                System.out.println(n + \" \" + ((Attribute) a).value());
            }
        }
    }
}
";
    std::fs::write(sources.join("Main.java"), java).unwrap();
    let translated = Command::new(env!("CARGO_BIN_EXE_java-to-rust"))
        .args(["--main", "Main", "--source"])
        .arg(&sources)
        .arg("--library")
        .arg(&annotations)
        .arg("--library")
        .arg(library)
        .arg("-o")
        .arg(work.join("main.rs"))
        .output()
        .unwrap();
    assert!(translated.status.success(), "{}", String::from_utf8_lossy(&translated.stderr));
    let rust = std::fs::read_to_string(work.join("main.rs")).unwrap();
    assert!(rust.contains("Main.Builder#name@org.example.Attribute"), "no `@Attribute` of the field `name`");
    assert!(!rust.contains("@org.example.Nullable"), "`@Nullable` is made as an annotation of a member");
}

/// An annotation whose class is missing from the class path is not among a member's annotations,
/// as the JVM skips one it cannot load: a census (`--keep-going`) of code compiled with an
/// optional library's annotations, as log4j's plugins are, reads the others.
#[test]
fn an_annotation_of_a_missing_class_is_not_a_members() {
    let Some(library) = std::env::var_os("JAVA_BASE_SOURCES").map(PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to run this program");
        return;
    };
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("whole-program-missing-annotations");
    let _ = std::fs::remove_dir_all(&work);
    let (sources, annotations) = (work.join("src"), work.join("lib"));
    std::fs::create_dir_all(&sources).unwrap();
    std::fs::create_dir_all(annotations.join("org/example")).unwrap();
    let attribute = "package org.example;\nimport java.lang.annotation.*;\n\
                     @Retention(RetentionPolicy.RUNTIME)\n@Target(ElementType.FIELD)\n\
                     public @interface Attribute { String value(); }\n";
    std::fs::write(annotations.join("org/example/Attribute.java"), attribute).unwrap();
    let java = "import java.lang.annotation.Annotation;
import org.example.Attribute;
import org.optional.Checked;
public class Main {
    static class Builder {
        @Checked @Attribute(\"name\") String name;
        @Checked Integer size;
    }
    public static void main(String[] args) throws Exception {
        for (String n : new String[] {\"name\", \"size\"}) {
            for (Annotation a : Builder.class.getDeclaredField(n).getDeclaredAnnotations()) {
                System.out.println(n + \" \" + ((Attribute) a).value());
            }
        }
    }
}
";
    std::fs::write(sources.join("Main.java"), java).unwrap();
    let translated = Command::new(env!("CARGO_BIN_EXE_java-to-rust"))
        .args(["--main", "Main", "--keep-going", "--source"])
        .arg(&sources)
        .arg("--library")
        .arg(&annotations)
        .arg("--library")
        .arg(library)
        .arg("-o")
        .arg(work.join("main.rs"))
        .output()
        .unwrap();
    assert!(translated.status.success(), "{}", String::from_utf8_lossy(&translated.stderr));
    let rust = std::fs::read_to_string(work.join("main.rs")).unwrap();
    assert!(rust.contains("Main.Builder#name@org.example.Attribute"), "no `@Attribute` of the field `name`");
}

/// A library class that extends one missing from the class path, so that it implements an
/// interface's methods only through what it cannot inherit (log4j's `MpscBlockingQueue` without
/// JCTools), is translated by a census (`--keep-going`) into a crate that compiles: Java could
/// never load it, and the interface's dispatch runs the others.
#[test]
fn a_class_of_a_missing_superclass_still_compiles() {
    let Some(library) = std::env::var_os("JAVA_BASE_SOURCES").map(PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to run this program");
        return;
    };
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("whole-program-missing-superclass");
    let _ = std::fs::remove_dir_all(&work);
    let (sources, pools) = (work.join("src"), work.join("lib"));
    std::fs::create_dir_all(&sources).unwrap();
    std::fs::create_dir_all(pools.join("org/example")).unwrap();
    let sized = "package org.example;\npublic interface Sized { int size(); boolean isEmpty(); }\n";
    std::fs::write(pools.join("org/example/Sized.java"), sized).unwrap();
    let pool = "package org.example;\n\
                public class Pool extends org.optional.Queue implements Sized {\n\
                    public Pool(int capacity) { super(capacity); }\n\
                }\n";
    std::fs::write(pools.join("org/example/Pool.java"), pool).unwrap();
    let java = "import org.example.*;
public class Main {
    static class Fixed implements Sized {
        public int size() { return 3; }
        public boolean isEmpty() { return false; }
    }
    public static void main(String[] args) {
        Sized sized = args.length > 5 ? new Pool(4) : new Fixed();
        System.out.println(sized.size() + \" \" + sized.isEmpty());
    }
}
";
    std::fs::write(sources.join("Main.java"), java).unwrap();
    let dir = work.join("crate");
    let translated = Command::new(env!("CARGO_BIN_EXE_java-to-rust"))
        .args(["--main", "Main", "--census", "--keep-going", "--source"])
        .arg(&sources)
        .arg("--library")
        .arg(&pools)
        .arg("--library")
        .arg(library)
        .arg("--crate")
        .arg(&dir)
        .output()
        .unwrap();
    assert!(dir.join("Cargo.toml").exists(), "{}", String::from_utf8_lossy(&translated.stderr));
    let target = work.join("cargo");
    let built = Command::new("cargo")
        .args(["build", "-q", "--manifest-path"])
        .arg(dir.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", &target)
        .output()
        .unwrap();
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    let ran = Command::new(target.join("debug/main")).env_remove("LD_LIBRARY_PATH").output().unwrap();
    assert_eq!(String::from_utf8_lossy(&ran.stdout), "3 false\n");
}

/// A `match` on the class number over more classes than are written whole (`CUT_ABOVE` in
/// `codegen/dispatch.rs`, 2048) is cut into methods by number (`PART_CLASSES`, 256 each): a
/// program with 2100 classes that shows each object and its class's name runs as Java does,
/// through methods of nine parts.
#[test]
fn a_match_over_thousands_of_classes_is_cut_into_parts() {
    let Some(library) = std::env::var_os("JAVA_BASE_SOURCES").map(PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to run this program");
        return;
    };
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("whole-program-thousands-of-classes");
    let _ = std::fs::remove_dir_all(&work);
    let sources = work.join("src");
    std::fs::create_dir_all(&sources).unwrap();
    let count = 2100;
    let mut java = String::from("public class Main {\n");
    for i in 0..count {
        java.push_str(&format!(
            "    static class K{i} {{ public String toString() {{ return \"K\" + {i}; }} }}\n"
        ));
    }
    let made: Vec<String> = (0..count).map(|i| format!("new K{i}()")).collect();
    java.push_str(&format!(
        "    public static void main(String[] args) {{
        Object[] all = {{ {} }};
        int total = 0;
        for (Object o : all) total += o.toString().length() + o.getClass().getName().length();
        System.out.println(all.length + \" \" + total);
        System.out.println(all[0] + \" \" + all[{last}] + \" \" + all[{last}].getClass().getName());
        System.out.println(all[{last}].getClass() == all[args.length].getClass());
        synchronized (all[{last} - args.length]) {{
            System.out.println(\"locked\");
        }}
        try {{
            K0 first = (K0) all[{last} - args.length];
            System.out.println(first);
        }} catch (ClassCastException e) {{
            System.out.println(e.getMessage());
        }}
    }}
}}
",
        made.join(", "),
        last = count - 1
    ));
    std::fs::write(sources.join("Main.java"), java).unwrap();
    let dir = work.join("crate");
    let translated = Command::new(env!("CARGO_BIN_EXE_java-to-rust"))
        .args(["--main", "Main", "--source"])
        .arg(&sources)
        .arg("--library")
        .arg(library)
        .arg("--crate")
        .arg(&dir)
        .output()
        .unwrap();
    assert!(translated.status.success(), "{}", String::from_utf8_lossy(&translated.stderr));
    let code = std::fs::read_to_string(dir.join("src/main.rs")).unwrap();
    assert!(code.contains("_of_part_1("), "the class matches were not cut");
    let target = work.join("cargo");
    let built = Command::new("cargo")
        .args(["build", "-q", "--manifest-path"])
        .arg(dir.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", &target)
        .output()
        .unwrap();
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    let ran = Command::new(target.join("debug/main")).env_remove("LD_LIBRARY_PATH").output().unwrap();
    let total: usize = (0..count).map(|i| format!("K{i}").len() + format!("Main$K{i}").len()).sum();
    let last = count - 1;
    let cast = format!(
        "class Main$K{last} cannot be cast to class Main$K0 (Main$K{last} and Main$K0 are in unnamed module of \
         loader 'app')"
    );
    assert_eq!(
        String::from_utf8_lossy(&ran.stdout),
        format!("{count} {total}\nK0 K{last} Main$K{last}\nfalse\nlocked\n{cast}\n")
    );
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

/// A class that extends one the translation leaves out (`ObjectInputStream`, which the program
/// also names as a type, so that it is declared without constructors) is rejected at its
/// `super(..)`, which has no constructor to call.
#[test]
fn a_subclass_of_an_untranslated_class_is_rejected() {
    let Some(library) = std::env::var_os("JAVA_BASE_SOURCES").map(PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to run this program");
        return;
    };
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("whole-program-untranslated-superclass");
    let sources = work.join("src");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&sources).unwrap();
    let java = "import java.io.*;
public class Main {
    static final class Loading extends ObjectInputStream {
        Loading(InputStream in) throws IOException {
            super(in);
        }
    }

    static ObjectInputStream last;

    public static void main(String[] args) throws IOException {
        if (args.length > 3) {
            last = new Loading(new ByteArrayInputStream(new byte[0]));
        }
        System.out.println(last == null);
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
    let stderr = String::from_utf8_lossy(&translated.stderr);
    assert!(!translated.status.success(), "translated");
    assert!(
        stderr.contains(
            "Main.java:5:13: `super(..)` of `java.io.ObjectInputStream` (serialization) not supported yet"
        ),
        "{stderr}"
    );
}

/// Handles held as values that lowering does not follow are rejected: an access through a handle
/// that may be null whose arguments do something before Java's null check, and a handle kept as
/// an `Object`. (A lookup of a method other than the handle factories throws when it runs, which
/// `UnfollowedHandleLookups` shows.)
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

/// `getAnnotation` of an annotation type of a library that is not the JDK's, as gson's `Excluder`
/// reads its `@Since` of every class it adapts: the library's classes are translated, so which
/// of them have it is known, as for the program's own annotation types.
#[test]
fn annotations_of_a_library_are_read() {
    let Some(java_base) = std::env::var_os("JAVA_BASE_SOURCES").map(PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to run this program");
        return;
    };
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("whole-program-library-annotations-read");
    let _ = std::fs::remove_dir_all(&work);
    let files = [
        (
            "lib/org/lib/Since.java",
            "package org.lib;\nimport java.lang.annotation.*;\n@Retention(RetentionPolicy.RUNTIME)\n@Target({ElementType.TYPE, ElementType.FIELD})\npublic @interface Since {\n    double value();\n}\n",
        ),
        (
            "lib/org/lib/Excluder.java",
            "package org.lib;\npublic class Excluder {\n    public static String since(Class<?> c) {\n        Since s = c.getAnnotation(Since.class);\n        return s == null ? \"always\" : \"since \" + s.value();\n    }\n}\n",
        ),
        ("lib/org/lib/Versioned.java", "package org.lib;\n@Since(1.5)\npublic class Versioned {\n}\n"),
        (
            "app/p/Main.java",
            "package p;\nimport org.lib.*;\npublic class Main {\n    public static void main(String[] a) {\n        System.out.println(Excluder.since(Versioned.class) + \" \" + Excluder.since(Main.class));\n    }\n}\n",
        ),
    ];
    for (path, java) in files {
        let path = work.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, java).unwrap();
    }
    let translated = Command::new(env!("CARGO_BIN_EXE_java-to-rust"))
        .args(["--main", "p.Main", "--source"])
        .arg(work.join("app"))
        .arg("--library")
        .arg(work.join("lib"))
        .arg("--library")
        .arg(&java_base)
        .arg("--crate")
        .arg(work.join("crate"))
        .output()
        .unwrap();
    assert!(translated.status.success(), "{}", String::from_utf8_lossy(&translated.stderr));
    let built = Command::new("cargo")
        .args(["run", "-q", "--manifest-path"])
        .arg(work.join("crate/Cargo.toml"))
        .env("CARGO_TARGET_DIR", work.join("target"))
        .output()
        .unwrap();
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    assert_eq!(String::from_utf8_lossy(&built.stdout), "since 1.5 always\n");
}

/// `System.loadLibrary` and `System.load` (`java/lang/TranslatedLibraries.java`), which Java lets
/// code of a module without native access call only after a warning naming where the caller's
/// class was loaded from: in a module of the JDK that has native access (here a root that
/// declares itself `java.prefs`, as the JDK's sources do) they find the library's file, and in
/// the unnamed module, which Java would warn for, they throw as Java does when it cannot load the
/// library (`corpus/whole-program/MissingNativeLibraries.java` runs them).
#[test]
fn loading_native_libraries_without_native_access_loads_nothing() {
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
    assert!(translated.status.success(), "{}", String::from_utf8_lossy(&translated.stderr));
    let rust = std::fs::read_to_string(work.join("unnamed.rs")).unwrap();
    for wanted in ["fn load_library_unlinked(", "fn load_unlinked("] {
        assert!(rust.contains(wanted), "no {wanted} in the translation");
    }
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

/// Rust generics that fail for each of the reasons lowering finds, and two that stay: the
/// prediction made before the first lowering names exactly those that fail
/// (`lower::predict`, checked with `JAVA_TO_RUST_PREDICTION_CHECK`).
const GENERICS_THAT_FAIL: &str = r#"
import java.util.*;
import java.util.function.Supplier;

public class Main {
    static class Box<T> { T value; Box(T v) { value = v; } T get() { return value; } }
    static class Pair<A, B> {
        A a; B b;
        Pair(A a, B b) { this.a = a; this.b = b; }
        public boolean equals(Object o) { return o instanceof Pair<?, ?> p && Objects.equals(a, p.a); }
    }
    interface Node { <A> void accept(A arg); }
    static class Leaf implements Node { public <A> void accept(A arg) { System.out.println("leaf"); } }
    static class Visitor<A> { void visit(Node n, A arg) { n.accept(arg); } }

    static <T> T keep(T v) { return v; }
    static <T> T first(List<T> list) { return list.get(0); }
    static <T> String text(T v) { return "v" + v; }
    static <T> T cast(Object o) { return keep((T) o); }
    static <T> Supplier<T> later(T v) { return () -> v; }
    static <T> List<T> one(T v) { return List.of(v); }

    public static void main(String[] args) {
        Box<String> box = new Box<>(keep("b"));
        new Visitor<String>().visit(new Leaf(), "x");
        System.out.println(box.get() + first(one("f")) + text(3) + later("l").get()
            + new Pair<>("a", 1).equals(new Pair<>("a", 2)) + Main.<String>cast("c"));
    }
}
"#;

#[test]
fn the_rust_generics_that_fail_are_predicted() {
    let Some(library) = std::env::var_os("JAVA_BASE_SOURCES").map(PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to run this program");
        return;
    };
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("whole-program-predicted-generics");
    let sources = work.join("src");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&sources).unwrap();
    std::fs::write(sources.join("Main.java"), GENERICS_THAT_FAIL).unwrap();
    let translate = |check: bool| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_java-to-rust"));
        if check {
            command.env("JAVA_TO_RUST_PREDICTION_CHECK", "unseeded");
        }
        let translated = command
            .args(["--timings", "--main", "Main", "--source"])
            .arg(&sources)
            .arg("--library")
            .arg(&library)
            .arg("-o")
            .arg(work.join("main.rs"))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&translated.stderr).into_owned();
        assert!(translated.status.success(), "the translation failed:\n{stderr}");
        (stderr, std::fs::read_to_string(work.join("main.rs")).unwrap())
    };
    let (checked, unseeded) = translate(true);
    let line = checked
        .lines()
        .find(|l| l.starts_with("timing: prediction: ") && l.contains(" of "))
        .unwrap_or_else(|| panic!("no prediction checked:\n{checked}"));
    let generics = line.split("; ").nth(1).unwrap();
    let numbers: Vec<&str> = generics.split(' ').collect();
    assert_eq!(numbers[0], numbers[2], "{line}\n{checked}");
    assert!(generics.ends_with(", 0 more"), "{line}\n{checked}");
    assert!(!checked.contains("prediction missed generic"), "{checked}");
    // The Rust generics that stay are the same with the prediction used.
    let (_, seeded) = translate(false);
    for generic in ["struct MainBox<T", "fn keep<T"] {
        assert!(seeded.contains(generic) && unseeded.contains(generic), "no {generic} in the translation");
    }
    assert_eq!(seeded, unseeded);
}
