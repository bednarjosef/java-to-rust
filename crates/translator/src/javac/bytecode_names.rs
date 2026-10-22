//! What javac's code generator gives the program's code, as the JVM's message of a
//! `NullPointerException` names it ("Cannot invoke \"Main$Box.size()\" because \"<local2>\" is
//! null"): the slots of the locals in a class file compiled without `-g`, and the methods and
//! static fields as the class file refers to them. `java/BytecodeNames.java` compiles the program
//! with javac, code generation included, and prints them; that costs a second compile, so the
//! translation asks for them only when the program reads such a message.

use super::{fail, java, parse};
use crate::ast::{Error, Pos};
use crate::value::Value;
use std::collections::HashMap;
use std::path::PathBuf;

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

/// The names of the code of a program's files. `modules` are the program's roots that are
/// modules, by name: the files of one that the JDK has too (javac's `jdk.compiler`) compile as
/// part of it, since the unnamed module may not hold its packages.
pub fn program_names(files: &[PathBuf], modules: &[(String, PathBuf)]) -> Result<BytecodeNames, Error> {
    let mut args = Vec::new();
    for (name, root) in modules {
        args.push("--patch-module".into());
        let mut patch = std::ffi::OsString::from(format!("{name}="));
        patch.push(root);
        args.push(patch);
    }
    args.extend(files.iter().map(|f| f.as_os_str().to_owned()));
    names(args, None)
}

fn names(files: Vec<std::ffi::OsString>, source: Option<&str>) -> Result<BytecodeNames, Error> {
    // The code generator's `binaryQualifier`, which names a member's class as a class file does.
    let mut command =
        java("BytecodeNames", &["--add-opens", "jdk.compiler/com.sun.tools.javac.jvm=ALL-UNNAMED"]);
    let output = match source {
        // A source on standard input is all that decides the run, which may have been kept.
        Some(source) if files.is_empty() => super::run_cache::run(command, source),
        _ => super::run_cache::run_uncached(command.args(files), source.unwrap_or_default()),
    }
    .map_err(|e| fail(format!("cannot run `java` (a JDK must be on PATH): {e}")).1)?;
    if !output.success {
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
        let pos = Pos::new(
            line.as_u64().unwrap_or_default() as usize,
            column.as_u64().unwrap_or_default() as usize,
        );
        let name = if kind == NameKind::Local { String::new() } else { name.to_string() };
        entries.insert((class.to_string(), pos, kind, name), text.as_str().map(str::to_string));
    }
    Ok(BytecodeNames { java: tree["java"].as_str().unwrap_or_default().to_string(), entries })
}

impl BytecodeNames {
    /// Adds the temps of the string `switch`es of the JDK's code (`jdk_switch_temps`), which
    /// `java/BytecodeNames.java` does not compile.
    pub fn add_jdk_switch_temps(&mut self, temps: SwitchTemps) {
        for (class, pos, text) in temps {
            self.entries.insert((class, pos, NameKind::Temp, "s$".to_string()), Some(text));
        }
    }
}

/// The temps of string `switch`es, by outermost class and position, with their text.
pub type SwitchTemps = Vec<(String, Pos, String)>;

/// The temps of the string `switch`es in `trees`, the JDK's units (see `unit_switch_temps`).
pub fn jdk_switch_temps(trees: &[&Value]) -> SwitchTemps {
    trees.iter().flat_map(|tree| unit_switch_temps(tree)).collect()
}

/// The JVM's names of the strings that the string `switch`es of a unit of the JDK's code switch
/// on: javac keeps the string in a variable of its own (`s$`, then the case's index in `tmp$`)
/// and calls its `hashCode()`, so a null string is `<localN>` in the message, a slot that the
/// JDK's class files (compiled with `-g`) leave unnamed. The slot is the code generator's: the
/// parameters' (`this` first), then the variables of the blocks around the `switch`, each in the
/// next slot when it is declared (before its initializer runs) and freed at the end of its block,
/// two for a `long` or `double`, and the variables that javac adds itself: a for-each loop's
/// array, length and index, or its iterator; a `synchronized` block's lock; an enclosing string
/// `switch`'s two. A `switch` in code whose slots this does not follow is left out: in a
/// constructor, initializer or lambda, in a `finally` block or a try-with-resources, in a method
/// with patterns, and a `switch` expression with values on the stack before it (an argument).
fn unit_switch_temps(unit: &Value) -> SwitchTemps {
    let mut out = Vec::new();
    for class in unit["typeDecls"].as_array().into_iter().flatten() {
        let Some(name) = class["symbol"]["type"].as_str() else { continue };
        let outermost = name.split('<').next().unwrap_or_default().to_string();
        classes(class, &outermost, &mut out);
    }
    out
}

/// The temps of the methods of `class` and of the classes in it.
fn classes(class: &Value, outermost: &str, out: &mut Vec<(String, Pos, String)>) {
    for member in class["members"].as_array().into_iter().flatten() {
        match member["kind"].as_str() {
            Some("METHOD") => {
                let symbol = &member["symbol"];
                if member["name"].as_str() == Some("<init>") || has_patterns(&member["body"]) {
                    continue;
                }
                let mut slots = SwitchSlots { outermost, out: &mut *out, next: 0 };
                if symbol["static"].as_bool() != Some(true) {
                    slots.next = 1;
                }
                for param in member["parameters"].as_array().into_iter().flatten() {
                    slots.next += size(param);
                }
                slots.statement(&member["body"]);
            }
            Some("CLASS" | "INTERFACE" | "ENUM" | "RECORD") => classes(member, outermost, out),
            _ => {}
        }
    }
    // The local and anonymous classes in the code, whose methods are their own.
    nested_classes(class, outermost, out, true);
}

/// The classes declared in the code under `v` (not its members, which `classes` walks).
fn nested_classes(v: &Value, outermost: &str, out: &mut Vec<(String, Pos, String)>, top: bool) {
    match v {
        Value::Object(fields) => {
            if !top && matches!(v["kind"].as_str(), Some("CLASS" | "INTERFACE" | "ENUM" | "RECORD")) {
                return classes(v, outermost, out);
            }
            for (key, value) in fields {
                if top && key == "members" {
                    for member in value.as_array().into_iter().flatten() {
                        if !matches!(member["kind"].as_str(), Some("CLASS" | "INTERFACE" | "ENUM" | "RECORD"))
                        {
                            nested_classes(member, outermost, out, false);
                        }
                    }
                } else if key != "symbol" {
                    nested_classes(value, outermost, out, false);
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|i| nested_classes(i, outermost, out, false)),
        _ => {}
    }
}

/// Whether the code under `v` has a pattern, whose variables javac declares where this does not
/// follow, or a `case null`.
fn has_patterns(v: &Value) -> bool {
    match v {
        Value::Object(fields) => {
            matches!(
                v["kind"].as_str(),
                Some("BINDING_PATTERN" | "RECORD_PATTERN" | "PATTERN_CASE_LABEL" | "ANY_PATTERN")
            ) || v["kind"].as_str() == Some("CONSTANT_CASE_LABEL")
                && v["constantExpression"]["kind"].as_str() == Some("NULL_LITERAL")
                || fields.iter().any(|(key, value)| key != "symbol" && has_patterns(value))
        }
        Value::Array(items) => items.iter().any(has_patterns),
        _ => false,
    }
}

/// The slots a variable declaration takes: two for a `long` or a `double`.
fn size(variable: &Value) -> usize {
    match variable["symbol"]["type"].as_str() {
        Some("long" | "double") => 2,
        _ => 1,
    }
}

/// The walk of a method's code, with the next free slot.
struct SwitchSlots<'a> {
    outermost: &'a str,
    out: &'a mut Vec<(String, Pos, String)>,
    next: usize,
}

impl SwitchSlots<'_> {
    fn statement(&mut self, s: &Value) {
        let start = self.next;
        let list = |v: &Value| v.as_array().cloned().unwrap_or_default();
        match s["kind"].as_str().unwrap_or_default() {
            "BLOCK" => {
                for statement in list(&s["statements"]) {
                    self.statement(&statement);
                }
            }
            "VARIABLE" => {
                // The variable's slot is taken before its initializer runs.
                self.next += size(s);
                self.expression(&s["initializer"], true);
                return;
            }
            "EXPRESSION_STATEMENT" => self.expression(&s["expression"], false),
            "RETURN" => self.expression(&s["expression"], true),
            "THROW" => self.expression(&s["expression"], false),
            "IF" => {
                self.expression(&s["condition"], false);
                self.statement(&s["thenStatement"]);
                self.statement(&s["elseStatement"]);
            }
            "WHILE_LOOP" | "DO_WHILE_LOOP" => {
                self.expression(&s["condition"], false);
                self.statement(&s["statement"]);
            }
            "FOR_LOOP" => {
                for init in list(&s["initializer"]) {
                    self.statement(&init);
                }
                self.expression(&s["condition"], false);
                for update in list(&s["update"]) {
                    self.statement(&update);
                }
                self.statement(&s["statement"]);
            }
            "ENHANCED_FOR_LOOP" => {
                self.expression(&s["expression"], false);
                let array = s["expression"]["javaType"].as_str().is_some_and(|t| t.ends_with("[]"));
                self.next += if array { 3 } else { 1 };
                self.next += size(&s["variable"]);
                self.statement(&s["statement"]);
            }
            "LABELED_STATEMENT" => self.statement(&s["statement"]),
            "SYNCHRONIZED" => {
                self.expression(&s["expression"], false);
                self.next += 1;
                self.statement(&s["block"]);
            }
            "SWITCH" => self.switch(s),
            "YIELD" => self.expression(&s["value"], false),
            "TRY" if list(&s["resources"]).is_empty() => {
                self.statement(&s["block"]);
                for catch in list(&s["catches"]) {
                    self.next = start + size(&catch["parameter"]);
                    self.statement(&catch["block"]);
                }
            }
            _ => {}
        }
        self.next = start;
    }

    /// The `switch`es of an expression; `alone` when the value of a `switch` expression that is
    /// all of it goes to an empty stack (an initializer, an assignment, a `return`).
    fn expression(&mut self, e: &Value, alone: bool) {
        match e["kind"].as_str().unwrap_or_default() {
            "PARENTHESIZED" => self.expression(&e["expression"], alone),
            "SWITCH_EXPRESSION" if alone => self.switch(e),
            "ASSIGNMENT" if e["variable"]["kind"].as_str() == Some("IDENTIFIER") => {
                self.expression(&e["expression"], true)
            }
            _ => {}
        }
    }

    /// A `switch` statement or expression: the temps of one on a string, then its cases.
    fn switch(&mut self, s: &Value) {
        let start = self.next;
        let mut selector = &s["expression"];
        while selector["kind"].as_str() == Some("PARENTHESIZED") {
            selector = &selector["expression"];
        }
        self.expression(selector, false);
        if selector["javaType"].as_str() == Some("java.lang.String") {
            let pos = Pos::new(
                selector["line"].as_u64().unwrap_or_default() as usize,
                selector["column"].as_u64().unwrap_or_default() as usize,
            );
            self.out.push((self.outermost.to_string(), pos, format!("<local{}>", self.next)));
            self.next += 2;
        }
        let cases = s["cases"].as_array().cloned().unwrap_or_default();
        // The statements of the old form's cases share a block; a rule's body is its own.
        let shared = self.next;
        for case in &cases {
            match &case["body"] {
                Value::Null => {
                    // A variable keeps its slot for the cases after it (`statement`).
                    for statement in case["statements"].as_array().into_iter().flatten() {
                        self.statement(statement);
                    }
                }
                body if matches!(body["kind"].as_str(), Some("BLOCK" | "THROW")) => {
                    self.next = shared;
                    self.statement(body);
                }
                body => {
                    self.next = shared;
                    self.expression(body, false);
                }
            }
        }
        self.next = start;
    }
}
