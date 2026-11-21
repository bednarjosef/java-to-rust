//! Runs javac on the Java source, through the `JavaTree` program compiled by `build.rs`, and
//! returns the typed syntax tree it prints as JSON, with the members that javac adds to records
//! later (`records`).

mod bytecode_names;
mod enum_bodies;
mod records;
pub use records::Completion;
mod run_cache;

pub use bytecode_names::{
    BytecodeNames, CHECKED_JAVA, JdkTemps, NameKind, NameRoots, jdk_temps, program_names, source_names,
};

use crate::ast::{Error, Pos};
use crate::value::Value;
use std::collections::HashSet;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::thread::JoinHandle;

/// The command that runs `JavaTree` on one source.
fn java_tree() -> Command {
    java("JavaTree", &[], Run::Short)
}

/// How long a JVM runs: on one source, for a second or two, or on a whole program, whose javac
/// runs for seconds to minutes (javac's own sources, 20 s; the Minecraft server, 103 s for its
/// 15,525 units).
#[derive(Clone, Copy, PartialEq)]
enum Run {
    Short,
    Long,
}

/// The command that runs `main`, one of the programs of `java/` that `build.rs` compiles, with
/// the JVM's `options`.
fn java(main: &str, options: &[&str], run: Run) -> Command {
    let mut command = Command::new("java");
    // Neither the parallel collector nor perf data (whose lock-file warnings go to stdout when
    // many JVMs run at once). A short-lived JVM skips the optimizing compiler too; a long one
    // gains from it (javac's census: its javac step takes 19-21 s on the first compiler alone,
    // 14-15 s with both).
    if run == Run::Short {
        command.arg("-XX:TieredStopAtLevel=1");
    }
    command.args(["-XX:+UseSerialGC", "-XX:-UsePerfData"]);
    // A small young generation: left to itself, the collector grows it to gigabytes of garbage
    // between collections (2.4 GB of memory for javac's 0.35 GB of live trees and symbols, for a
    // program that reaches 600 of the JDK's units), at no gain in speed.
    command.arg("-XX:MaxNewSize=128m");
    // `JavaTree` attributes the classes of a program one by one, with javac's own `analyze`;
    // `BytecodeNames` reads what javac's code generator decided.
    for package in JAVAC_PACKAGES {
        command.args(["--add-exports", &format!("jdk.compiler/com.sun.tools.javac.{package}=ALL-UNNAMED")]);
    }
    command.args(options).args(["-cp", env!("OUT_DIR"), main]);
    command
}

/// The packages of javac's implementation that the programs of `java/` use.
const JAVAC_PACKAGES: [&str; 8] = ["api", "code", "comp", "tree", "util", "jvm", "main", "resources"];

pub fn tree(source: &str) -> Result<Value, Error> {
    let fail = |message: String| Error { pos: Pos::default(), message };
    let output = run_cache::run(java_tree(), source)
        .map_err(|e| fail(format!("cannot run `java` (a JDK must be on PATH): {e}")))?;
    if !output.success {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // The first compile error, printed as `LINE:COLUMN: message`.
        let first = stderr.lines().next().unwrap_or("javac failed");
        let mut parts = first.splitn(3, ':');
        let (line, column, message) = (parts.next(), parts.next(), parts.next());
        return Err(
            match (line.and_then(|l| l.parse().ok()), column.and_then(|c| c.parse().ok()), message) {
                (Some(line), Some(column), Some(message)) => {
                    Error { pos: Pos::new(line, column), message: message.trim().to_string() }
                }
                _ => fail(stderr.trim().to_string()),
            },
        );
    }
    let mut tree = parse(&output.stdout).map_err(|e| fail(format!("unreadable javac tree: {e}")))?;
    records::complete(&mut [&mut tree]);
    Ok(tree)
}

/// Runs javac once on all the files of a program, with `sourcepath` (the roots that classes are
/// loaded from by package) as its source path, `module_info` the only `module-info.java` on it
/// that javac sees (see `JavaTree`'s `oneModule`), and the files of the source path that a patch
/// in `patches` names read as patched (see `java/JdkPatch.java`), and returns
/// `{"units": [{"file": ..., "tree": ...}]}`: the files given, then the units javac loaded from
/// the source path. A compile error is returned with the file it is in.
pub fn program_tree(
    files: &[PathBuf],
    sourcepath: &[PathBuf],
    module_info: Option<&Path>,
    patches: Option<&Path>,
) -> Result<Value, (Option<String>, Error)> {
    let output = program_command(files, sourcepath, module_info, patches)?
        .output()
        .map_err(|e| fail(format!("cannot run `java` (a JDK must be on PATH): {e}")))?;
    if !output.status.success() {
        return Err(compile_error(&output.stderr));
    }
    let mut tree = parse(&output.stdout).map_err(|e| fail(format!("unreadable javac tree: {e}")))?;
    if let Some(units) = tree["units"].as_array_mut() {
        let mut trees: Vec<&mut Value> = units.iter_mut().map(|u| &mut u["tree"]).collect();
        records::complete(&mut trees);
    }
    Ok(tree)
}

/// Applies every patch under `patches` (see `java/JdkPatch.java`) to the file of the same path in
/// the first of `roots` that has it, writing the patched files under `out` when it is given, and
/// returns why any patch was refused.
pub fn check_patches(patches: &Path, roots: &[PathBuf], out: Option<&Path>) -> Result<(), String> {
    let mut command = java("JdkPatch", &[], Run::Short);
    command.arg(patches).args(roots);
    if let Some(out) = out {
        command.arg("--out").arg(out);
    }
    let output = command.output().map_err(|e| format!("cannot run `java` (a JDK must be on PATH): {e}"))?;
    match output.status.success() {
        true => Ok(()),
        false => Err(String::from_utf8_lossy(&output.stderr).trim().to_string()),
    }
}

fn fail(message: String) -> (Option<String>, Error) {
    (None, Error { pos: Pos::default(), message })
}

/// The command that runs `JavaTree` on the files of a program.
fn program_command(
    files: &[PathBuf],
    sourcepath: &[PathBuf],
    module_info: Option<&Path>,
    patches: Option<&Path>,
) -> Result<Command, (Option<String>, Error)> {
    let mut command = java("JavaTree", &[], Run::Long);
    command.arg("--program");
    if !sourcepath.is_empty() {
        let joined = std::env::join_paths(sourcepath).map_err(|e| fail(format!("bad source path: {e}")))?;
        command.arg("--sourcepath").arg(joined);
    }
    if let Some(module_info) = module_info {
        command.arg("--module-info").arg(module_info);
    }
    if let Some(patches) = patches {
        command.arg("--patches").arg(patches);
    }
    command.args(files);
    Ok(command)
}

/// The first compile error that `JavaTree` printed, as `FILE:LINE:COLUMN: message`, with its file.
fn compile_error(stderr: &[u8]) -> (Option<String>, Error) {
    let stderr = String::from_utf8_lossy(stderr);
    let mut lines = stderr.lines();
    let first = lines.next().unwrap_or("javac failed");
    if let Some((place, message)) = first.split_once(": ") {
        let mut parts = place.rsplitn(3, ':');
        let (column, line, file) = (parts.next(), parts.next(), parts.next());
        if let (Some(column), Some(line), Some(file)) =
            (column.and_then(|c| c.parse().ok()), line.and_then(|l| l.parse().ok()), file)
        {
            // The lines of the same message, indented, below its first.
            let details = lines.take_while(|l| l.starts_with(char::is_whitespace));
            let message = missing_module(message, details).unwrap_or_else(|| message.trim().to_string());
            return (Some(file.to_string()), Error { pos: Pos::new(line, column), message });
        }
    }
    fail(stderr.trim().to_string())
}

/// The error of a package that javac cannot see because it is in a module whose sources are not
/// given: javac compiles all the sources as one module (see `program_tree`), which reads no other,
/// so the package's module is one of the JDK's that javac has only as class files. javac says
/// `package P is not visible` and, below, `(package P is declared in module M, but ...)`.
fn missing_module<'a>(message: &str, details: impl Iterator<Item = &'a str>) -> Option<String> {
    let package = message.trim().strip_prefix("package ")?.strip_suffix(" is not visible")?;
    let module = details.into_iter().find_map(|l| {
        let (_, rest) = l.split_once(" is declared in module ")?;
        rest.split([',', ')', ' ']).next()
    })?;
    Some(format!(
        "package {package} is in module {module}, whose sources are not given: pass the root of its sources as --library"
    ))
}

/// A javac run over a program that attributes the units of the source path only as they are
/// asked for (`JavaTree --program --serve`), where `program_tree` has javac attribute every unit
/// it loads: for a program that uses the JDK's collections, those are thousands of units, of
/// which the code reachable from `main` uses a few dozen. The units come as javac printed them,
/// `{"file": ..., "tree": ...}`, without the members that `Records` writes into records.
pub struct Session {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
    errors: Option<JoinHandle<Vec<u8>>>,
    /// The classes asked for so far.
    asked: HashSet<String>,
    /// The size of the trees received so far, in bytes of JSON.
    pub received: usize,
    /// The time taken so far to turn that JSON into values.
    pub parsing: std::time::Duration,
}

impl Session {
    /// Starts javac on the program's files, with `sourcepath`, `module_info` and `patches` as in
    /// `program_tree`, and returns their units, in the order given. A compile error is returned
    /// with the file it is in; with `keep_going`, javac goes on past compile errors instead, which
    /// it prints to standard error: it rewrites the code that names a missing class to throw
    /// `NoClassDefFoundError`, marking the piece with `"missingClass"`, and marks the piece of code
    /// that each other error is in with `"javacError"` (see `JavaTree.markErrors`).
    pub fn start(
        files: &[PathBuf],
        sourcepath: &[PathBuf],
        module_info: Option<&Path>,
        patches: Option<&Path>,
        keep_going: bool,
    ) -> Result<(Session, Vec<Value>), (Option<String>, Error)> {
        let mut command = program_command(files, sourcepath, module_info, patches)?;
        if keep_going {
            command.arg("--keep-going");
        }
        let mut child = command
            .arg("--serve")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| fail(format!("cannot run `java` (a JDK must be on PATH): {e}")))?;
        crate::timings::javac_process(Some(child.id()));
        let input = child.stdin.take();
        let output = BufReader::new(child.stdout.take().expect("stdout is piped"));
        let mut stderr = BufReader::new(child.stderr.take().expect("stderr is piped"));
        let errors = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let mut line = Vec::new();
            while stderr.read_until(b'\n', &mut line).is_ok_and(|n| n > 0) {
                // The errors that javac goes on past are passed on as they come.
                if keep_going {
                    let _ = std::io::stderr().write_all(&line);
                }
                bytes.append(&mut line);
            }
            bytes
        });
        let mut session = Session {
            child,
            input,
            output,
            errors: Some(errors),
            asked: HashSet::new(),
            received: 0,
            parsing: std::time::Duration::ZERO,
        };
        let units = session.read()?;
        Ok((session, units))
    }

    /// Asks for the units that declare these classes, those not asked for before, and returns
    /// those of them that javac had not given yet.
    pub fn load(&mut self, classes: &[String]) -> Result<Vec<Value>, (Option<String>, Error)> {
        let names: Vec<&str> =
            classes.iter().filter(|c| self.asked.insert(c.to_string())).map(String::as_str).collect();
        if names.is_empty() {
            return Ok(Vec::new());
        }
        let input = self.input.as_mut().expect("the input is open while the session lives");
        if writeln!(input, "{}", names.join(",")).and_then(|()| input.flush()).is_err() {
            return Err(self.failure());
        }
        self.read()
    }

    /// The units of one line of `JavaTree`'s output.
    fn read(&mut self) -> Result<Vec<Value>, (Option<String>, Error)> {
        let mut line = Vec::new();
        match self.output.read_until(b'\n', &mut line) {
            Ok(n) if n > 0 && line.ends_with(b"\n") => {}
            _ => return Err(self.failure()),
        }
        crate::timings::note_javac();
        self.received += line.len();
        let start = std::time::Instant::now();
        let units =
            parse_units(&line, parse_threads()).map_err(|e| fail(format!("unreadable javac tree: {e}")))?;
        self.parsing += start.elapsed();
        Ok(units)
    }

    /// Why javac stopped: the compile error that it printed.
    fn failure(&mut self) -> (Option<String>, Error) {
        crate::timings::javac_process(None);
        self.input = None;
        let _ = self.child.wait();
        let stderr = self.errors.take().and_then(|e| e.join().ok()).unwrap_or_default();
        compile_error(&stderr)
    }
}

/// The members that javac adds to records written (see `records`), into units that come in
/// rounds (`Session`): which members a record gets depends on every unit so far, and only grows
/// with them (`Completion::only_adds_to`).
#[derive(Default)]
pub struct Records {
    survey: records::Survey,
}

impl Records {
    /// Adds units to those that decide the members, and returns, for each, whether `complete`
    /// changes it: those with a record or without a package.
    pub fn add(&mut self, units: &[Value]) -> Vec<bool> {
        self.survey.add(&units.iter().map(|u| &u["tree"]).collect::<Vec<_>>());
        units.iter().map(|u| records::changes(&u["tree"])).collect()
    }

    /// Writes the members into the records of a unit, as the units so far decide them.
    pub fn complete(&self, unit: &mut Value) {
        records::complete_unit(&mut unit["tree"], &self.survey);
    }

    /// The record declarations of a unit, for `completion`.
    pub fn records_of(unit: &Value) -> Vec<&Value> {
        records::records(&unit["tree"])
    }

    /// What `complete` writes into these records of a unit (`records_of`) that the units so far
    /// decide: when it is the same as before a round's units were added, so is what `complete`
    /// makes of the unit.
    pub fn completion(&self, records: &[&Value]) -> Completion {
        records::completion(records, &self.survey)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        crate::timings::javac_process(None);
        // The end of its input ends javac.
        self.input = None;
        let _ = self.child.wait();
    }
}

/// How many threads read the units of a line of javac's trees (`parse_units`):
/// `JAVA_TO_RUST_PARSE_THREADS`, or one for each core, at most eight.
fn parse_threads() -> usize {
    std::env::var("JAVA_TO_RUST_PARSE_THREADS")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()).min(8))
        .max(1)
}

/// The smallest line of javac's trees that `parse_units` reads on several threads.
const PARALLEL_BYTES: usize = 1 << 20;

/// The stack of each thread that reads javac's trees: as large as that of the translator's own
/// (`main.rs`), as `parse` recurses as deep as the trees nest.
const STACK: usize = 1 << 30;

/// The units of a line of `JavaTree`'s output, `{"units":[...]}` (`JavaTree.print`), read on
/// `threads` threads when the line is large (`parse_cut`), else as one value; in their order
/// either way, the same values, or the same error.
fn parse_units(line: &[u8], threads: usize) -> serde_json::Result<Vec<Value>> {
    if threads > 1
        && line.len() >= PARALLEL_BYTES
        && let Some(units) = parse_cut(line, threads)
    {
        return Ok(units);
    }
    let mut tree = parse(line)?;
    Ok(match tree["units"].take() {
        Value::Array(units) => units.into(),
        _ => Vec::new(),
    })
}

/// The units of a line of javac's trees, each read from its own text on one of `threads`
/// threads; `None` when the line cannot be cut so (`cut_units`). Every unit starts with
/// `{"file":"`, which can be nowhere else in the line but at the start of an object of that first
/// key: inside a string its quotes would be escaped. So the line is cut where one unit ends and
/// the next starts, after each `}` followed by `,{"file":"`. A cut inside a unit, at an object of
/// its own with that first key, would leave the part of the unit before it unclosed, which reads
/// as no value: then this gives `None`, and the line is read whole.
fn parse_cut(line: &[u8], threads: usize) -> Option<Vec<Value>> {
    let body = line.strip_prefix(b"{\"units\":[")?.strip_suffix(b"]}\n")?;
    let pieces = cut_units(body, threads);
    if pieces.len() < 2 {
        return None;
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let mut units: Vec<Option<Value>> = std::iter::repeat_with(|| None).take(pieces.len()).collect();
    let read = std::thread::scope(|scope| {
        let readers: Vec<_> = (0..threads.min(pieces.len()))
            .map(|_| {
                let (next, pieces) = (&next, &pieces);
                std::thread::Builder::new()
                    .name("javac trees".to_string())
                    .stack_size(STACK)
                    .spawn_scoped(scope, move || {
                        let mut read = Vec::new();
                        loop {
                            let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let Some(piece) = pieces.get(i) else { return Some(read) };
                            match parse(piece) {
                                Ok(unit) => read.push((i, unit)),
                                Err(_) => {
                                    // The others stop at their next unit.
                                    next.store(pieces.len(), std::sync::atomic::Ordering::Relaxed);
                                    return None;
                                }
                            }
                        }
                    })
                    .expect("a thread to read javac's trees")
            })
            .collect();
        let mut all = true;
        for reader in readers {
            match reader.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic)) {
                Some(read) => read.into_iter().for_each(|(i, unit)| units[i] = Some(unit)),
                None => all = false,
            }
        }
        all
    });
    read.then(|| units.into_iter().map(|unit| unit.expect("every unit is read")).collect())
}

/// The texts of the units in `body`, the units of a line separated by commas (`parse_cut`):
/// cut after each `}` followed by `,{"file":"`, which `threads` threads look for at once, each in
/// a part of `body`.
fn cut_units(body: &[u8], threads: usize) -> Vec<&[u8]> {
    const NEXT: &[u8] = b"},{\"file\":\"";
    let part = body.len().div_ceil(threads).max(1);
    let cuts: Vec<usize> = std::thread::scope(|scope| {
        let finders: Vec<_> = body
            .chunks(part)
            .enumerate()
            .map(|(n, chunk)| {
                scope.spawn(move || {
                    let start = n * part;
                    let ends = chunk.iter().enumerate().filter(|&(_, &b)| b == b'}').map(|(i, _)| start + i);
                    ends.filter(|&i| body[i..].starts_with(NEXT)).collect::<Vec<usize>>()
                })
            })
            .collect();
        finders
            .into_iter()
            .flat_map(|f| f.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic)))
            .collect()
    });
    let mut pieces = Vec::with_capacity(cuts.len() + 1);
    let mut start = 0;
    for cut in cuts {
        pieces.push(&body[start..=cut]);
        start = cut + 2;
    }
    pieces.push(&body[start..]);
    pieces
}

/// Reads javac's JSON without serde_json's nesting limit: real code nests deeper than it allows,
/// in long chains of `+` or `else if`, so the caller runs on a large stack instead.
fn parse(json: &[u8]) -> serde_json::Result<Value> {
    let mut reader = serde_json::Deserializer::from_slice(json);
    reader.disable_recursion_limit();
    let value = serde::Deserialize::deserialize(&mut reader)?;
    reader.end()?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use crate::value::Value;

    /// A line of `JavaTree`'s output with `count` units, the `inner`th of which has an object of
    /// its own whose first key is `file`, after another in a list.
    fn units_line(count: usize, inner: Option<usize>) -> Vec<u8> {
        let units: Vec<String> = (0..count)
            .map(|i| {
                let list = if inner == Some(i) { r#"[{"x":1},{"file":"inner"}]"# } else { r#"[{"x":1},{"y":[]}]"# };
                // A string that holds the text the line is cut at, escaped.
                format!(
                    r#"{{"file":"/p/U{i}.java","tree":{{"kind":"CompilationUnit","n":{i},"f":{}.5,"s":"}},{{\"file\":\"x","list":{list}}}}}"#,
                    i % 7
                )
            })
            .collect();
        format!("{{\"units\":[{}]}}\n", units.join(",")).into_bytes()
    }

    /// The units of a line read as one value.
    fn whole(line: &[u8]) -> Vec<Value> {
        let mut tree = super::parse(line).expect("the line reads");
        match tree["units"].take() {
            Value::Array(units) => units.into(),
            _ => Vec::new(),
        }
    }

    #[test]
    fn a_line_of_units_reads_the_same_cut_into_its_units() {
        let line = units_line(500, None);
        let cut = super::parse_cut(&line, 4).expect("the line is cut into its units");
        assert_eq!(cut.len(), 500);
        assert_eq!(cut, whole(&line));
        // One unit is the line itself, which is read whole.
        assert!(super::parse_cut(&units_line(1, None), 4).is_none());
    }

    #[test]
    fn a_line_cut_inside_a_unit_is_read_whole() {
        let line = units_line(500, Some(321));
        assert!(super::parse_cut(&line, 4).is_none());
        // Large enough to be cut, and read whole when the cut fails.
        let large = units_line(20_000, Some(12_345));
        assert!(large.len() >= super::PARALLEL_BYTES);
        let read = super::parse_units(&large, 4).expect("the line reads");
        assert_eq!(read.len(), 20_000);
        assert_eq!(read, whole(&large));
        let plain = units_line(20_000, None);
        assert_eq!(super::parse_units(&plain, 4).expect("the line reads"), whole(&plain));
    }

    /// The types of the symbols named `name` anywhere in `value`.
    fn symbol_types(value: &Value, name: &str, out: &mut Vec<String>) {
        match value {
            Value::Object(fields) => {
                let symbol = &value["symbol"];
                if symbol["name"].as_str() == Some(name) {
                    out.extend(symbol["type"].as_str().map(str::to_string));
                }
                fields.values().for_each(|child| symbol_types(child, name, out));
            }
            Value::Array(items) => items.iter().for_each(|child| symbol_types(child, name, out)),
            _ => {}
        }
    }

    #[test]
    fn types_drop_type_annotations_and_keep_type_parameters() {
        let source = "
import java.lang.annotation.ElementType;
import java.lang.annotation.Target;
import java.util.List;

public class Main {
    @Target(ElementType.TYPE_USE)
    @interface Nullable {}

    static <T> @Nullable T first(List<@Nullable T> values) {
        return values.get(0);
    }

    public static void main(String[] args) {
        System.out.println(first(List.of(args.length)));
    }
}
";
        let tree = super::tree(source).expect("javac takes the source");
        let mut types = Vec::new();
        symbol_types(&tree, "first", &mut types);
        // The declaration's symbol and the call's.
        assert!(types.len() >= 2, "{types:?}");
        assert!(types.iter().all(|ty| ty == "<T>(java.util.List<T>)T"), "{types:?}");
    }
}
