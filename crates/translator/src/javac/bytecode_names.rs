//! What javac's code generator gives the program's code, as the JVM's message of a
//! `NullPointerException` names it ("Cannot invoke \"Main$Box.size()\" because \"<local2>\" is
//! null"): the slots of the locals in a class file compiled without `-g`, and the methods and
//! static fields as the class file refers to them. `java/BytecodeNames.java` compiles the program
//! with javac, code generation included, and prints them; that costs a second compile, so the
//! translation asks for them only when the program reads such a message.

use super::{fail, java, parse};
use crate::ast::{Error, Pos};
use serde_json::Value;
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::Stdio;

/// The Java version whose messages the translation writes: the one they were checked against.
/// A program compiled by another reads messages that may differ, which is not translated.
pub const CHECKED_JAVA: &str = "26";

/// What a name stands for (see `java/BytecodeNames.java`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NameKind {
    /// A local variable or a parameter: `<local2>`, `<parameter1>`.
    Local,
    /// A variable that javac adds for a statement, holding the value of the expression that
    /// starts at the position: `arr$` (a for-each loop's array), `s$` (a switch's string).
    Temp,
    /// A method that a call calls: `Main$Box.size()`.
    Call,
    /// A static field: `Main$Box.shared`.
    Static,
}

/// The names of a program's code, by the outermost class the code is in (`com.example.Main`),
/// the position where the name's expression starts, its kind and its name in the source.
pub struct BytecodeNames {
    /// The feature version of the Java that compiled the program, such as `26`.
    pub java: String,
    /// The text of each name, `None` where javac's code generator leaves it uncertain.
    entries: HashMap<(String, Pos, NameKind, String), Option<String>>,
}

impl BytecodeNames {
    /// The JVM's text for the name `name` of kind `kind` starting at `pos` in the code of the
    /// outermost class `class`, when it is certain. A local's name is not needed: one name
    /// starts at a position, and the translation may have renamed the local.
    pub fn get(&self, class: &str, pos: Pos, kind: NameKind, name: &str) -> Option<&str> {
        let name = if kind == NameKind::Local { "" } else { name };
        self.entries.get(&(class.to_string(), pos, kind, name.to_string()))?.as_deref()
    }
}

/// The names of the code of one source file.
pub fn source_names(source: &str) -> Result<BytecodeNames, Error> {
    names(Vec::new(), Some(source))
}

/// The names of the code of a program's files.
pub fn program_names(files: &[PathBuf]) -> Result<BytecodeNames, Error> {
    names(files.iter().map(|f| f.as_os_str().to_owned()).collect(), None)
}

fn names(files: Vec<std::ffi::OsString>, source: Option<&str>) -> Result<BytecodeNames, Error> {
    // The code generator's `binaryQualifier`, which names a member's class as a class file does.
    let mut child =
        java("BytecodeNames", &["--add-opens", "jdk.compiler/com.sun.tools.javac.jvm=ALL-UNNAMED"])
            .args(files)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| fail(format!("cannot run `java` (a JDK must be on PATH): {e}")).1)?;
    let mut stdin = child.stdin.take().expect("stdin is piped");
    let source = source.unwrap_or_default().to_string();
    let writer = std::thread::spawn(move || stdin.write_all(source.as_bytes()));
    let output = child.wait_with_output().map_err(|e| fail(format!("running javac failed: {e}")).1)?;
    let _ = writer.join();
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(fail(format!("compiling the program for its class files failed: {}", stderr.trim())).1);
    }
    let tree =
        parse(&output.stdout).map_err(|e| fail(format!("unreadable names of the class files: {e}")).1)?;
    let mut entries = HashMap::new();
    for entry in tree["entries"].as_array().into_iter().flatten() {
        let Some(
            [
                Value::String(class),
                Value::Number(line),
                Value::Number(column),
                Value::String(kind),
                Value::String(name),
                text,
            ],
        ) = entry.as_array().map(Vec::as_slice)
        else {
            continue;
        };
        let kind = match kind.as_str() {
            "local" => NameKind::Local,
            "temp" => NameKind::Temp,
            "call" => NameKind::Call,
            "static" => NameKind::Static,
            _ => continue,
        };
        let pos = Pos {
            line: line.as_u64().unwrap_or_default() as usize,
            column: column.as_u64().unwrap_or_default() as usize,
        };
        let name = if kind == NameKind::Local { String::new() } else { name.clone() };
        entries.insert((class.clone(), pos, kind, name), text.as_str().map(str::to_string));
    }
    Ok(BytecodeNames { java: tree["java"].as_str().unwrap_or_default().to_string(), entries })
}
