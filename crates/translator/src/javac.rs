//! Runs javac on the Java source, through the `JavaTree` program compiled by `build.rs`, and
//! returns the typed syntax tree it prints as JSON, with the members that javac adds to records
//! later (`records`).

mod records;

use crate::ast::{Error, Pos};
use serde_json::Value;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// The command that runs `JavaTree`.
fn java_tree() -> Command {
    let mut command = Command::new("java");
    // A short-lived JVM: skip the optimizing compiler, the parallel collector and perf data
    // (whose lock-file warnings go to stdout when many JVMs run at once).
    command.args([
        "-XX:TieredStopAtLevel=1",
        "-XX:+UseSerialGC",
        "-XX:-UsePerfData",
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
    let fail = |message: String| (None, Error { pos: Pos::default(), message });
    let mut command = java_tree();
    command.arg("--program");
    if !sourcepath.is_empty() {
        let joined = std::env::join_paths(sourcepath).map_err(|e| fail(format!("bad source path: {e}")))?;
        command.arg("--sourcepath").arg(joined);
    }
    let output = command
        .args(files)
        .output()
        .map_err(|e| fail(format!("cannot run `java` (a JDK must be on PATH): {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // The first compile error, printed as `FILE:LINE:COLUMN: message`.
        let first = stderr.lines().next().unwrap_or("javac failed");
        if let Some((place, message)) = first.split_once(": ") {
            let mut parts = place.rsplitn(3, ':');
            let (column, line, file) = (parts.next(), parts.next(), parts.next());
            if let (Some(column), Some(line), Some(file)) =
                (column.and_then(|c| c.parse().ok()), line.and_then(|l| l.parse().ok()), file)
            {
                let error = Error { pos: Pos { line, column }, message: message.trim().to_string() };
                return Err((Some(file.to_string()), error));
            }
        }
        return Err(fail(stderr.trim().to_string()));
    }
    let mut tree = parse(&output.stdout).map_err(|e| fail(format!("unreadable javac tree: {e}")))?;
    if let Some(units) = tree["units"].as_array_mut() {
        let mut trees: Vec<&mut Value> = units.iter_mut().map(|u| &mut u["tree"]).collect();
        records::complete(&mut trees);
    }
    Ok(tree)
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
