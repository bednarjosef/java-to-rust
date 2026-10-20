//! Runs javac on the Java source, through the `JavaTree` program compiled by `build.rs`, and
//! returns the typed syntax tree it prints as JSON.

use crate::ast::{Error, Pos};
use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

pub fn tree(source: &str) -> Result<Value, Error> {
    let fail = |message: String| Error { pos: Pos::default(), message };
    let mut child = Command::new("java")
        // A short-lived JVM: skip the optimizing compiler, the parallel collector and perf data
        // (whose lock-file warnings go to stdout when many JVMs run at once).
        .args([
            "-XX:TieredStopAtLevel=1",
            "-XX:+UseSerialGC",
            "-XX:-UsePerfData",
            "-cp",
            env!("OUT_DIR"),
            "JavaTree",
        ])
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
    serde_json::from_slice(&output.stdout).map_err(|e| fail(format!("unreadable javac tree: {e}")))
}
