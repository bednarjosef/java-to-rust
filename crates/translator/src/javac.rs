//! Runs javac on the Java source, through the `JavaTree` program compiled by `build.rs`, and
//! returns the typed syntax tree it prints as JSON, with the members that javac adds to records
//! later (`records`).

mod records;

use crate::ast::{Error, Pos};
use serde_json::Value;
use std::borrow::Cow;
use std::collections::HashSet;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::thread::JoinHandle;

/// The command that runs `JavaTree`.
fn java_tree() -> Command {
    let mut command = Command::new("java");
    // A short-lived JVM: skip the optimizing compiler, the parallel collector and perf data
    // (whose lock-file warnings go to stdout when many JVMs run at once).
    command.args([
        "-XX:TieredStopAtLevel=1",
        "-XX:+UseSerialGC",
        "-XX:-UsePerfData",
        // `JavaTree` attributes the classes of a program one by one, with javac's own `analyze`.
        "--add-exports",
        "jdk.compiler/com.sun.tools.javac.api=ALL-UNNAMED",
        "-cp",
        env!("OUT_DIR"),
        "JavaTree",
    ]);
    command
}

pub fn tree(source: &str) -> Result<Value, Error> {
    let fail = |message: String| Error { pos: Pos::default(), message };
    let mut child = java_tree()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| fail(format!("cannot run `java` (a JDK must be on PATH): {e}")))?;
    let mut stdin = child.stdin.take().expect("stdin is piped");
    let source = source.to_string();
    let writer = std::thread::spawn(move || stdin.write_all(source.as_bytes()));
    let output = child.wait_with_output().map_err(|e| fail(format!("running javac failed: {e}")))?;
    let _ = writer.join();
    if !output.status.success() {
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
/// loaded from by package) as its source path, and returns
/// `{"units": [{"file": ..., "tree": ...}]}`: the files given, then the units javac loaded from
/// the source path. A compile error is returned with the file it is in.
pub fn program_tree(files: &[PathBuf], sourcepath: &[PathBuf]) -> Result<Value, (Option<String>, Error)> {
    let output = program_command(files, sourcepath)?
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
fn program_command(files: &[PathBuf], sourcepath: &[PathBuf]) -> Result<Command, (Option<String>, Error)> {
    let mut command = java_tree();
    command.arg("--program");
    if !sourcepath.is_empty() {
        let joined = std::env::join_paths(sourcepath).map_err(|e| fail(format!("bad source path: {e}")))?;
        command.arg("--sourcepath").arg(joined);
    }
    command.args(files);
    Ok(command)
}

/// The first compile error that `JavaTree` printed, as `FILE:LINE:COLUMN: message`, with its file.
fn compile_error(stderr: &[u8]) -> (Option<String>, Error) {
    let stderr = String::from_utf8_lossy(stderr);
    let first = stderr.lines().next().unwrap_or("javac failed");
    if let Some((place, message)) = first.split_once(": ") {
        let mut parts = place.rsplitn(3, ':');
        let (column, line, file) = (parts.next(), parts.next(), parts.next());
        if let (Some(column), Some(line), Some(file)) =
            (column.and_then(|c| c.parse().ok()), line.and_then(|l| l.parse().ok()), file)
        {
            let error = Error { pos: Pos { line, column }, message: message.trim().to_string() };
            return (Some(file.to_string()), error);
        }
    }
    fail(stderr.trim().to_string())
}

/// A javac run over a program that attributes the units of the source path only as they are
/// asked for (`JavaTree --program --serve`), where `program_tree` has javac attribute every unit
/// it loads: for a program that uses the JDK's collections, those are thousands of units, of
/// which the code reachable from `main` uses a few dozen.
pub struct Session {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
    errors: Option<JoinHandle<Vec<u8>>>,
    /// The units so far as javac printed them, `{"file": ..., "tree": ...}`: the program's files
    /// in the order given, then the others in the order they came.
    units: Vec<Value>,
    /// Whether completing the records changes each of `units` (`records::changes`).
    changes: Vec<bool>,
    /// The classes asked for so far.
    asked: HashSet<String>,
}

impl Session {
    /// Starts javac on the program's files, with `sourcepath` as its source path, and reads their
    /// units. A compile error is returned with the file it is in.
    pub fn start(files: &[PathBuf], sourcepath: &[PathBuf]) -> Result<Session, (Option<String>, Error)> {
        let mut child = program_command(files, sourcepath)?
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
        let mut session = Session {
            child,
            input,
            output,
            errors: Some(errors),
            units: Vec::new(),
            changes: Vec::new(),
            asked: HashSet::new(),
        };
        let units = session.read()?;
        session.add(units);
        Ok(session)
    }

    /// Asks for the units that declare these classes, those not asked for before, and returns
    /// how many new units javac gave.
    pub fn load(&mut self, classes: &[String]) -> Result<usize, (Option<String>, Error)> {
        let names: Vec<&str> =
            classes.iter().filter(|c| self.asked.insert(c.to_string())).map(String::as_str).collect();
        if names.is_empty() {
            return Ok(0);
        }
        let input = self.input.as_mut().expect("the input is open while the session lives");
        if writeln!(input, "{}", names.join(",")).and_then(|()| input.flush()).is_err() {
            return Err(self.failure());
        }
        let units = self.read()?;
        let count = units.len();
        self.add(units);
        Ok(count)
    }

    /// The units so far, with the members that javac adds to records written (see `records`):
    /// those that the units' code may use. Only the units that this changes are copies.
    pub fn units(&self) -> Vec<Cow<'_, Value>> {
        let survey = records::survey(&self.units.iter().map(|u| &u["tree"]).collect::<Vec<_>>());
        let complete = |unit: &Value| {
            let mut unit = unit.clone();
            records::complete_unit(&mut unit["tree"], &survey);
            unit
        };
        let changes = self.units.iter().zip(&self.changes);
        changes
            .map(|(u, &changes)| if changes { Cow::Owned(complete(u)) } else { Cow::Borrowed(u) })
            .collect()
    }

    /// The units, as `units` gives them, when no more are to come.
    pub fn finish(mut self) -> Vec<Value> {
        let mut units = std::mem::take(&mut self.units);
        let survey = records::survey(&units.iter().map(|u| &u["tree"]).collect::<Vec<_>>());
        for (unit, &changes) in units.iter_mut().zip(&self.changes) {
            if changes {
                records::complete_unit(&mut unit["tree"], &survey);
            }
        }
        units
    }

    fn add(&mut self, units: Vec<Value>) {
        self.changes.extend(units.iter().map(|u| records::changes(&u["tree"])));
        self.units.extend(units);
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
