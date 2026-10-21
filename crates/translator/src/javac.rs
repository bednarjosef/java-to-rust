//! Runs javac on the Java source, through the `JavaTree` program compiled by `build.rs`, and
//! returns the typed syntax tree it prints as JSON, with the members that javac adds to records
//! later (`records`).

mod bytecode_names;
mod records;
mod run_cache;

pub use bytecode_names::{BytecodeNames, CHECKED_JAVA, NameKind, program_names, source_names};

use crate::ast::{Error, Pos};
use serde_json::Value;
use std::collections::HashSet;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::thread::JoinHandle;

/// The command that runs `JavaTree`.
fn java_tree() -> Command {
    java("JavaTree", &[])
}

/// The command that runs `main`, one of the programs of `java/` that `build.rs` compiles, with
/// the JVM's `options`.
fn java(main: &str, options: &[&str]) -> Command {
    let mut command = Command::new("java");
    // A short-lived JVM: skip the optimizing compiler, the parallel collector and perf data
    // (whose lock-file warnings go to stdout when many JVMs run at once).
    command.args(["-XX:TieredStopAtLevel=1", "-XX:+UseSerialGC", "-XX:-UsePerfData"]);
    // `JavaTree` attributes the classes of a program one by one, with javac's own `analyze`;
    // `BytecodeNames` reads what javac's code generator decided.
    for package in JAVAC_PACKAGES {
        command.args(["--add-exports", &format!("jdk.compiler/com.sun.tools.javac.{package}=ALL-UNNAMED")]);
    }
    command.args(options).args(["-cp", env!("OUT_DIR"), main]);
    command
}

/// The packages of javac's implementation that the programs of `java/` use.
const JAVAC_PACKAGES: [&str; 5] = ["api", "code", "tree", "util", "jvm"];

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
                    Error { pos: Pos { line, column }, message: message.trim().to_string() }
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
/// loaded from by package) as its source path, and `module_info` the only `module-info.java` on
/// it that javac sees (see `JavaTree`'s `oneModule`), and returns
/// `{"units": [{"file": ..., "tree": ...}]}`: the files given, then the units javac loaded from
/// the source path. A compile error is returned with the file it is in.
pub fn program_tree(
    files: &[PathBuf],
    sourcepath: &[PathBuf],
    module_info: Option<&Path>,
) -> Result<Value, (Option<String>, Error)> {
    let output = program_command(files, sourcepath, module_info)?
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

fn fail(message: String) -> (Option<String>, Error) {
    (None, Error { pos: Pos::default(), message })
}

/// The command that runs `JavaTree` on the files of a program.
fn program_command(
    files: &[PathBuf],
    sourcepath: &[PathBuf],
    module_info: Option<&Path>,
) -> Result<Command, (Option<String>, Error)> {
    let mut command = java_tree();
    command.arg("--program");
    if !sourcepath.is_empty() {
        let joined = std::env::join_paths(sourcepath).map_err(|e| fail(format!("bad source path: {e}")))?;
        command.arg("--sourcepath").arg(joined);
    }
    if let Some(module_info) = module_info {
        command.arg("--module-info").arg(module_info);
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
            return (Some(file.to_string()), Error { pos: Pos { line, column }, message });
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
}

impl Session {
    /// Starts javac on the program's files, with `sourcepath` as its source path and
    /// `module_info` as in `program_tree`, and returns their units, in the order given. A compile
    /// error is returned with the file it is in.
    pub fn start(
        files: &[PathBuf],
        sourcepath: &[PathBuf],
        module_info: Option<&Path>,
    ) -> Result<(Session, Vec<Value>), (Option<String>, Error)> {
        let mut child = program_command(files, sourcepath, module_info)?
            .arg("--serve")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| fail(format!("cannot run `java` (a JDK must be on PATH): {e}")))?;
        let input = child.stdin.take();
        let output = BufReader::new(child.stdout.take().expect("stdout is piped"));
        let mut stderr = child.stderr.take().expect("stderr is piped");
        let errors = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = std::io::Read::read_to_end(&mut stderr, &mut bytes);
            bytes
        });
        let mut session = Session { child, input, output, errors: Some(errors), asked: HashSet::new() };
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
        let mut tree = parse(&line).map_err(|e| fail(format!("unreadable javac tree: {e}")))?;
        Ok(match tree["units"].take() {
            Value::Array(units) => units,
            _ => Vec::new(),
        })
    }

    /// Why javac stopped: the compile error that it printed.
    fn failure(&mut self) -> (Option<String>, Error) {
        self.input = None;
        let _ = self.child.wait();
        let stderr = self.errors.take().and_then(|e| e.join().ok()).unwrap_or_default();
        compile_error(&stderr)
    }
}

/// The members that javac adds to records written (see `records`), into units that come in
/// rounds (`Session`): which members a record gets depends on every unit so far.
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

    /// The records of a unit, as `complete` writes them: when these are the same as before a
    /// round's units were added, so is what `complete` makes of the unit.
    pub fn completed_records(&self, unit: &Value) -> Vec<Value> {
        records::completed_records(&unit["tree"], &self.survey)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // The end of its input ends javac.
        self.input = None;
        let _ = self.child.wait();
    }
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
