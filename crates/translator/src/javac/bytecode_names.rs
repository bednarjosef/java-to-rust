//! What javac's code generator gives the program's code, as the JVM's message of a
//! `NullPointerException` names it ("Cannot invoke \"Main$Box.size()\" because \"<local2>\" is
//! null"): the slots of the locals in a class file compiled without `-g`, and the methods and
//! static fields as the class file refers to them. `java/BytecodeNames.java` compiles the program
//! with javac, code generation included, and prints them; that costs a second compile, so the
//! translation asks for them only when the program reads such a message.

use super::{fail, java};
use crate::ast::{Error, Pos};
use crate::value::Value;
use std::collections::{HashMap, HashSet};
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
    /// An object whose member of the name javac's accessor method (`access$000`) dereferences,
    /// named as the accessor's parameter: `x0`, `<parameter1>`.
    Accessor,
}

/// The names of a program's code, by the outermost class the code is in (`com.example.Main`),
/// the position where the name's expression starts, its kind and its name in the source.
pub struct BytecodeNames {
    /// The feature version of the Java that compiled the program, such as `26`.
    pub java: String,
    /// The texts of the names that are certain, by class, and by line, column, kind and name,
    /// each name and text an index into `strings`: those of the Minecraft server and its
    /// libraries are tens of millions, most of them the same few texts.
    entries: HashMap<String, ClassNames>,
    strings: Strings,
}

/// The texts of the names in one class, by line, column, kind and name.
type ClassNames = HashMap<(u32, u32, NameKind, u32), u32>;

/// Each name and text once.
#[derive(Default)]
struct Strings {
    list: Vec<Box<str>>,
    ids: HashMap<Box<str>, u32>,
}

impl Strings {
    fn id(&mut self, s: &str) -> u32 {
        if let Some(&id) = self.ids.get(s) {
            return id;
        }
        let id = self.list.len() as u32;
        self.list.push(s.into());
        self.ids.insert(s.into(), id);
        id
    }
}

impl BytecodeNames {
    /// The JVM's text for the name `name` of kind `kind` starting at `pos` in the code of the
    /// outermost class `class`, when it is certain. A local's name is not needed: one name
    /// starts at a position, and the translation may have renamed the local.
    pub fn get(&self, class: &str, pos: Pos, kind: NameKind, name: &str) -> Option<&str> {
        let name = if kind == NameKind::Local { "" } else { name };
        let name = *self.strings.ids.get(name)?;
        let text = *self.entries.get(class)?.get(&(pos.line as u32, pos.column as u32, kind, name))?;
        Some(&self.strings.list[text as usize])
    }

    /// How many names there are, in how many classes, with how many texts, for `--timings`.
    pub fn sizes(&self) -> String {
        let names: usize = self.entries.values().map(HashMap::len).sum();
        format!(
            "{names} names in {} classes, {} names and texts",
            self.entries.len(),
            self.strings.list.len()
        )
    }

    /// Sets the text of a name, `None` where it is uncertain, which `get` gives as it gives a
    /// name it does not know.
    fn set(&mut self, class: String, pos: Pos, kind: NameKind, name: &str, text: Option<&str>) {
        let name = if kind == NameKind::Local { "" } else { name };
        match text {
            Some(text) => {
                let key = (pos.line as u32, pos.column as u32, kind, self.strings.id(name));
                let text = self.strings.id(text);
                self.entries.entry(class).or_default().insert(key, text);
            }
            None => {
                if let (Some(&name), Some(entries)) =
                    (self.strings.ids.get(name), self.entries.get_mut(&class))
                {
                    entries.remove(&(pos.line as u32, pos.column as u32, kind, name));
                }
            }
        }
    }
}

/// The names of the code of one source file.
pub fn source_names(source: &str) -> Result<BytecodeNames, Error> {
    names(Vec::new(), Some(source), "")
}

/// What else decides the names of a program's code (`program_names`).
pub struct NameRoots<'a> {
    /// The library roots: the classes of the program's code from those that are not the JDK's
    /// compile with it, and get names too, as the library's code is translated with it.
    pub libraries: &'a [PathBuf],
    /// The class files name their locals (`Program::named_locals`).
    pub named_locals: bool,
    /// Compile errors do not stop it: the code of a file with one has no names
    /// (`Program::keep_going`).
    pub keep_going: bool,
    /// The files of the units that the translation reached, as paths or relative to their root:
    /// those of the library roots compile too, as the translation may reach a class that no
    /// code names (by `Class.forName`), which javac would not compile.
    pub reached: &'a [String],
}

/// The names of the code of a program's files. `modules` are the program's roots that are
/// modules, by name: the files of one that the JDK has too (javac's `jdk.compiler`) compile as
/// part of it, since the unnamed module may not hold its packages.
pub fn program_names(
    files: &[PathBuf],
    modules: &[(String, PathBuf)],
    roots: NameRoots,
) -> Result<BytecodeNames, Error> {
    let mut args = Vec::new();
    for root in roots.libraries {
        args.push("--sourcepath-root".into());
        args.push(root.as_os_str().to_owned());
    }
    if roots.named_locals {
        args.push("--named-locals".into());
    }
    if roots.keep_going {
        args.push("--keep-going".into());
    }
    if !roots.reached.is_empty() {
        args.push("--reached".into());
    }
    for (name, root) in modules {
        args.push("--patch-module".into());
        let mut patch = std::ffi::OsString::from(format!("{name}="));
        patch.push(root);
        args.push(patch);
    }
    args.extend(files.iter().map(|f| f.as_os_str().to_owned()));
    let reached = roots.reached.iter().fold(String::new(), |mut list, f| {
        list.push_str(f);
        list.push('\n');
        list
    });
    names(args, None, &reached)
}

/// The names of the code of `files` (with their options), or of `source` on standard input when
/// there are none; `input` is what standard input gives a program's run (`--reached`).
fn names(files: Vec<std::ffi::OsString>, source: Option<&str>, input: &str) -> Result<BytecodeNames, Error> {
    // The code generator's `binaryQualifier`, which names a member's class as a class file does.
    let mut command =
        java("BytecodeNames", &["--add-opens", "jdk.compiler/com.sun.tools.javac.jvm=ALL-UNNAMED"]);
    let cannot_run = |e: std::io::Error| fail(format!("cannot run `java` (a JDK must be on PATH): {e}")).1;
    let failed = |stderr: &[u8]| {
        // Without the JVM's notes of the options it picked up (`JAVA_TOOL_OPTIONS`).
        let stderr = String::from_utf8_lossy(stderr);
        let errors: Vec<&str> = stderr.lines().filter(|l| !l.starts_with("Picked up ")).collect();
        fail(format!("compiling the program for its class files failed: {}", errors.join("\n").trim())).1
    };
    let unreadable = |e: serde_json::Error| fail(format!("unreadable names of the class files: {e}")).1;
    if let Some(source) = source.filter(|_| files.is_empty()) {
        // A source on standard input is all that decides the run, which may have been kept.
        let output = super::run_cache::run(command, source).map_err(cannot_run)?;
        if !output.success {
            return Err(failed(&output.stderr));
        }
        return read_names(output.stdout.as_slice()).map_err(unreadable);
    }
    // The names of a program's files, read as they are printed: those of a program as large as
    // the Minecraft server and its libraries are gigabytes.
    use std::process::Stdio;
    let mut child = command
        .args(files)
        .stdin(if input.is_empty() { Stdio::null() } else { Stdio::piped() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(cannot_run)?;
    if let Some(mut stdin) = child.stdin.take() {
        let input = input.to_string();
        std::thread::spawn(move || std::io::Write::write_all(&mut stdin, input.as_bytes()));
    }
    let mut stderr = child.stderr.take().expect("stderr is piped");
    let errors = std::thread::spawn(move || {
        let mut errors = Vec::new();
        let _ = std::io::Read::read_to_end(&mut stderr, &mut errors);
        errors
    });
    let stdout = child.stdout.take().expect("stdout is piped");
    let names = read_names(std::io::BufReader::with_capacity(1 << 20, stdout));
    let status = child.wait().map_err(cannot_run)?;
    let errors = errors.join().unwrap_or_default();
    if !status.success() {
        return Err(failed(&errors));
    }
    names.map_err(unreadable)
}

/// The names that `java/BytecodeNames.java` prints, read entry by entry into the map.
fn read_names(json: impl std::io::Read) -> serde_json::Result<BytecodeNames> {
    use serde::de::{DeserializeSeed, Deserializer as _, IgnoredAny, MapAccess, SeqAccess, Visitor};
    use std::fmt;

    struct Names;
    impl<'de> Visitor<'de> for Names {
        type Value = BytecodeNames;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("the names of the class files")
        }
        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<BytecodeNames, A::Error> {
            let mut names =
                BytecodeNames { java: String::new(), entries: HashMap::new(), strings: Strings::default() };
            while let Some(key) = map.next_key::<String>()? {
                match key.as_str() {
                    "java" => names.java = map.next_value()?,
                    "entries" => map.next_value_seed(Each(&mut names))?,
                    _ => _ = map.next_value::<IgnoredAny>()?,
                }
            }
            Ok(names)
        }
    }
    /// Reads the entries into the map.
    struct Each<'m>(&'m mut BytecodeNames);
    impl<'de> DeserializeSeed<'de> for Each<'_> {
        type Value = ();
        fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
            d.deserialize_seq(self)
        }
    }
    impl<'de> Visitor<'de> for Each<'_> {
        type Value = ();
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("the entries of the names")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
            type Entry = (String, u64, u64, String, String, Option<String>);
            while let Some((class, line, column, kind, name, text)) = seq.next_element::<Entry>()? {
                let kind = match kind.as_str() {
                    "local" => NameKind::Local,
                    "temp" => NameKind::Temp,
                    "call" => NameKind::Call,
                    "static" => NameKind::Static,
                    "accessor" => NameKind::Accessor,
                    _ => continue,
                };
                self.0.set(class, Pos::new(line as usize, column as usize), kind, &name, text.as_deref());
            }
            Ok(())
        }
    }
    let mut reader = serde_json::Deserializer::from_reader(json);
    let names = (&mut reader).deserialize_map(Names)?;
    reader.end()?;
    Ok(names)
}

impl BytecodeNames {
    /// Adds the temps of the JDK's code (`jdk_temps`), which `java/BytecodeNames.java` does not
    /// compile.
    pub fn add_jdk_temps(&mut self, temps: JdkTemps) {
        for (class, pos, name, text) in temps {
            self.set(class, pos, NameKind::Temp, name, Some(&text));
        }
    }
}

/// The temps of javac's own in the JDK's code, by outermost class and position, with javac's name
/// for the variable (`s$` or `i$`) and the JVM's text for it.
pub type JdkTemps = Vec<(String, Pos, &'static str, String)>;

/// The temps of `trees`, the JDK's units (see `unit_temps`).
pub fn jdk_temps(trees: &[&Value]) -> JdkTemps {
    trees.iter().flat_map(|tree| unit_temps(tree)).collect()
}

/// The JVM's names of the variables of javac's own in a unit of the JDK's code whose value a
/// `NullPointerException`'s message can name, which the JDK's class files (compiled with `-g`)
/// leave unnamed, so that a null one is `<localN>` in the message: the string that a string
/// `switch` switches on, which javac keeps in a variable (`s$`, then the case's index in `tmp$`)
/// and calls `hashCode()` on, and the iterator of a for-each loop over an `Iterable` (`i$`), at
/// the position of the collection, which `iterator()` is called on and whose `hasNext()` and
/// `next()` are called. The slot is the code generator's: the
/// parameters' (`this` first), then the variables of the blocks around the `switch`, each in the
/// next slot when it is declared (before its initializer runs) and freed at the end of its block,
/// two for a `long` or `double`, and the variables that javac adds itself: a for-each loop's
/// array, length and index, or its iterator; a `synchronized` block's lock; an enclosing string
/// `switch`'s two. Each of javac's own is declared before the expression that it holds runs, as
/// a variable is. A `switch` expression's string is in the next slot whatever is on the stack
/// (javac declares it in a `LetExpr` around the `switch`). A `switch` in code whose slots this
/// does not follow is left out: in a constructor that javac gives parameters of its own (an
/// enum's, or an inner, local or anonymous class's), an initializer, a lambda that may use `this`
/// (see `lambda_captures`), in a `finally` block or a try-with-resources, in a method with patterns other than those of `instanceof` that this
/// follows (see `bindings`), in an expression whose own temps javac declares (a compound
/// assignment, an increment), and in the cases of a `switch` expression with a `try`, whose stack
/// javac keeps in variables.
///
/// The variables of `instanceof` patterns are javac's `TransPatterns`: a binding variable is
/// declared, without a value, by the outermost of the statements (`if`, `for`, `while`, `do`) and
/// expressions (`&&` and the other binary operators, `?:`, the `instanceof` itself) around it
/// below the nearest block or lambda, before that construct runs, and freed after it; the value
/// tested is kept in a variable of its own while it is tested, unless it is a local variable or a
/// parameter. A binding that stays in scope after its statement (`if (!(o instanceof T t))
/// return;`) is declared in the enclosing block instead, before the statement, and freed at the
/// end of the block (`kept_bindings`); a method where that is not certain, or where such a
/// statement is not one of a block's own, is left out (`untracked_patterns`).
fn unit_temps(unit: &Value) -> JdkTemps {
    let mut out = Vec::new();
    for class in unit["typeDecls"].as_array().into_iter().flatten() {
        let Some(name) = class["symbol"]["type"].as_str() else { continue };
        let outermost = name.split('<').next().unwrap_or_default().to_string();
        let plain = class["kind"].as_str() != Some("ENUM");
        classes(class, &outermost, plain, &mut out);
    }
    out
}

/// The temps of the methods of `class` and of the classes in it; of its constructors too where
/// they are `plain`, with no parameters of javac's own.
fn classes(class: &Value, outermost: &str, plain: bool, out: &mut JdkTemps) {
    for member in class["members"].as_array().into_iter().flatten() {
        match member["kind"].as_str() {
            Some("METHOD") => {
                let symbol = &member["symbol"];
                if member["name"].as_str() == Some("<init>") && !plain || untracked_patterns(&member["body"])
                {
                    continue;
                }
                let mark = out.len();
                let mut slots = SwitchSlots::new(outermost, &mut *out, 0);
                if symbol["static"].as_bool() != Some(true) {
                    slots.next = 1;
                }
                for param in member["parameters"].as_array().into_iter().flatten() {
                    slots.next += size(param);
                }
                slots.statement(&member["body"]);
                if slots.failed {
                    out.truncate(mark);
                }
            }
            Some(kind @ ("CLASS" | "INTERFACE" | "ENUM" | "RECORD")) => {
                let plain = kind != "ENUM" && member["symbol"]["static"].as_bool() == Some(true);
                classes(member, outermost, plain, out)
            }
            _ => {}
        }
    }
    // The local and anonymous classes in the code, whose methods are their own.
    nested_classes(class, outermost, out, true);
}

/// The classes declared in the code under `v` (not its members, which `classes` walks).
fn nested_classes(v: &Value, outermost: &str, out: &mut JdkTemps, top: bool) {
    match v {
        Value::Object(fields) => {
            if !top && matches!(v["kind"].as_str(), Some("CLASS" | "INTERFACE" | "ENUM" | "RECORD")) {
                return classes(v, outermost, false, out);
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

/// Whether the code under `v` has patterns whose variables the walk of slots does not follow:
/// a `switch` with patterns (or `case null`), a record pattern, `_` as a pattern, a pattern or
/// `instanceof` of a primitive type, or a binding that may stay in scope after its statement
/// (`preserved`) where that statement is not one of a block's own (the walk follows those, see
/// `kept_bindings`).
fn untracked_patterns(v: &Value) -> bool {
    fn untracked(v: &Value, in_block: bool) -> bool {
        match v {
            Value::Object(fields) => {
                let kind = v["kind"].as_str().unwrap_or_default();
                matches!(
                    kind,
                    "DECONSTRUCTION_PATTERN" | "RECORD_PATTERN" | "PATTERN_CASE_LABEL" | "ANY_PATTERN"
                ) || kind == "CONSTANT_CASE_LABEL"
                    && v["constantExpression"]["kind"].as_str() == Some("NULL_LITERAL")
                    || kind == "INSTANCE_OF" && primitive(&v["type"])
                    || kind == "BINDING_PATTERN" && primitive(&v["variable"])
                    || !in_block && preserved(v)
                    || fields.iter().any(|(key, value)| match value {
                        Value::Array(items) if kind == "BLOCK" && key == "statements" => {
                            items.iter().any(|s| untracked(s, true))
                        }
                        _ => key != "symbol" && untracked(value, false),
                    })
            }
            Value::Array(items) => items.iter().any(|i| untracked(i, false)),
            _ => false,
        }
    }
    untracked(v, false)
}

/// Whether the type of the tree `v` is a primitive type.
fn primitive(v: &Value) -> bool {
    let ty = v["symbol"]["type"].as_str().or(v["javaType"].as_str()).unwrap_or_default();
    matches!(ty, "boolean" | "byte" | "short" | "char" | "int" | "long" | "float" | "double")
}

/// Whether statement `s` may introduce bindings of its condition after itself (JLS 6.3.2), which
/// javac then declares in the enclosing block (`Attr.addBindings2Scope`): an `if` whose
/// condition binds when false and that has no `else`, or one whose branch may not complete
/// normally, and a loop whose condition binds when false.
fn preserved(s: &Value) -> bool {
    let (when_true, when_false) = condition_bindings(&s["condition"]);
    match s["kind"].as_str().unwrap_or_default() {
        "IF" if s["elseStatement"].is_null() => when_false,
        "IF" => {
            when_true && may_not_complete(&s["elseStatement"])
                || when_false && may_not_complete(&s["thenStatement"])
        }
        "WHILE_LOOP" | "DO_WHILE_LOOP" | "FOR_LOOP" => when_false,
        _ => false,
    }
}

/// The number of binding variables of statement `s`, one of a block's own, that stay in scope
/// after it (JLS 6.3.2), which javac declares in the block before the statement
/// (`TransPatterns.visitBlock`'s `tryPrepend`) rather than around it: an `if`'s bindings when
/// false where only its `else` (or the lack of one) completes normally, when true where only its
/// `then` does (`Attr.visitIf`), and a loop's bindings when false where nothing breaks out of
/// it. `None` where that is not certain: where whether a branch completes is not known
/// (`completes`), and for a labeled statement that may keep bindings.
fn kept_bindings(s: &Value) -> Option<usize> {
    match s["kind"].as_str().unwrap_or_default() {
        "IF" => {
            let (when_true, when_false) = condition_counts(&s["condition"]);
            if when_true + when_false == 0 {
                return Some(0);
            }
            let then = completes(&s["thenStatement"])?;
            let otherwise = if s["elseStatement"].is_null() { true } else { completes(&s["elseStatement"])? };
            Some(match (then, otherwise) {
                (true, false) => when_true,
                (false, true) => when_false,
                _ => 0,
            })
        }
        "WHILE_LOOP" | "DO_WHILE_LOOP" | "FOR_LOOP" => {
            let when_false = condition_counts(&s["condition"]).1;
            Some(if jumps_out(&s["statement"], "BREAK") { 0 } else { when_false })
        }
        "LABELED_STATEMENT" => {
            let mut inner = &s["statement"];
            while inner["kind"].as_str() == Some("LABELED_STATEMENT") {
                inner = &inner["statement"];
            }
            if preserved(inner) { None } else { Some(0) }
        }
        _ => Some(0),
    }
}

/// The numbers of the named bindings that the condition `e` introduces when true, and when
/// false (JLS 6.3.1).
fn condition_counts(e: &Value) -> (usize, usize) {
    match e["kind"].as_str().unwrap_or_default() {
        "PARENTHESIZED" => condition_counts(&e["expression"]),
        "LOGICAL_COMPLEMENT" => {
            let (t, f) = condition_counts(&e["expression"]);
            (f, t)
        }
        "CONDITIONAL_AND" => {
            (condition_counts(&e["leftOperand"]).0 + condition_counts(&e["rightOperand"]).0, 0)
        }
        "CONDITIONAL_OR" => {
            (0, condition_counts(&e["leftOperand"]).1 + condition_counts(&e["rightOperand"]).1)
        }
        "INSTANCE_OF" => {
            let pattern = &e["pattern"];
            let named = pattern["kind"].as_str() == Some("BINDING_PATTERN")
                && !matches!(pattern["variable"]["name"].as_str(), None | Some("" | "_"));
            (usize::from(named), 0)
        }
        _ => (0, 0),
    }
}

/// Whether statement `s` can complete normally (JLS 14.22), as javac's `Flow.aliveAfter`
/// decides; `None` where this does not follow the rules (a `switch`, a condition that may be a
/// constant expression other than a literal or a constant variable).
fn completes(s: &Value) -> Option<bool> {
    let either = |a: Option<bool>, b: Option<bool>| match (a, b) {
        (Some(true), _) | (_, Some(true)) => Some(true),
        (Some(false), Some(false)) => Some(false),
        _ => None,
    };
    match s["kind"].as_str().unwrap_or_default() {
        "BLOCK" => {
            // A statement after one that cannot complete is unreachable, which javac rejects.
            let mut known = true;
            for statement in s["statements"].as_array().into_iter().flatten() {
                match completes(statement) {
                    Some(false) => return Some(false),
                    Some(true) => {}
                    None => known = false,
                }
            }
            known.then_some(true)
        }
        "RETURN" | "THROW" | "BREAK" | "CONTINUE" | "YIELD" => Some(false),
        "IF" if s["elseStatement"].is_null() => Some(true),
        "IF" => either(completes(&s["thenStatement"]), completes(&s["elseStatement"])),
        "WHILE_LOOP" | "FOR_LOOP" => match constant_true(&s["condition"])? {
            true => Some(jumps_out(&s["statement"], "BREAK")),
            false => Some(true),
        },
        "DO_WHILE_LOOP" => {
            if jumps_out(&s["statement"], "BREAK") {
                return Some(true);
            }
            if constant_true(&s["condition"])? {
                return Some(false);
            }
            // The condition runs when the body completes or continues the loop.
            if jumps_out(&s["statement"], "CONTINUE") { Some(true) } else { completes(&s["statement"]) }
        }
        "LABELED_STATEMENT" => {
            let label = s["label"].as_str().unwrap_or_default();
            match completes(&s["statement"]) {
                Some(true) => Some(true),
                _ if breaks_to(&s["statement"], label) => Some(true),
                other => other,
            }
        }
        "SYNCHRONIZED" => completes(&s["block"]),
        "TRY" => {
            let finally = &s["finallyBlock"];
            if !finally.is_null() && !completes(finally)? {
                return Some(false);
            }
            let mut alive = completes(&s["block"]);
            for catch in s["catches"].as_array().into_iter().flatten() {
                alive = either(alive, completes(&catch["block"]));
            }
            alive
        }
        "EXPRESSION_STATEMENT"
        | "VARIABLE"
        | "EMPTY_STATEMENT"
        | "ENHANCED_FOR_LOOP"
        | "ASSERT"
        | "CLASS"
        | "INTERFACE"
        | "ENUM"
        | "RECORD" => Some(true),
        _ => None,
    }
}

/// Whether the loop condition `c` is the constant `true` (no condition is); `None` where it may
/// be a constant expression that this does not evaluate.
fn constant_true(c: &Value) -> Option<bool> {
    fn variable(v: &Value) -> bool {
        match v {
            Value::Object(fields) => {
                let kind = v["kind"].as_str().unwrap_or_default();
                matches!(kind, "IDENTIFIER" | "MEMBER_SELECT") && v["symbol"]["constant"].is_null()
                    || matches!(
                        kind,
                        "METHOD_INVOCATION" | "NEW_CLASS" | "INSTANCE_OF" | "ARRAY_ACCESS" | "ASSIGNMENT"
                    )
                    || fields.iter().any(|(key, value)| key != "symbol" && variable(value))
            }
            Value::Array(items) => items.iter().any(variable),
            _ => false,
        }
    }
    if c.is_null() {
        return Some(true);
    }
    match c["kind"].as_str().unwrap_or_default() {
        "PARENTHESIZED" => constant_true(&c["expression"]),
        "BOOLEAN_LITERAL" => c["value"].as_bool(),
        "IDENTIFIER" | "MEMBER_SELECT" if !c["symbol"]["constant"].is_null() => {
            Some(c["symbol"]["constant"].as_str() == Some("true"))
        }
        _ if variable(c) => Some(false),
        _ => None,
    }
}

/// Whether the body `v` of a loop has an unlabeled `jump` (`BREAK` or `CONTINUE`) of the loop:
/// one that is not in a loop of its own, or for a `break`, a `switch`; not in a class or a
/// lambda.
fn jumps_out(v: &Value, jump: &str) -> bool {
    match v {
        Value::Object(fields) => {
            let kind = v["kind"].as_str().unwrap_or_default();
            if kind == jump {
                return v["label"].is_null();
            }
            let own = matches!(kind, "WHILE_LOOP" | "DO_WHILE_LOOP" | "FOR_LOOP" | "ENHANCED_FOR_LOOP")
                || jump == "BREAK" && matches!(kind, "SWITCH" | "SWITCH_EXPRESSION")
                || matches!(kind, "CLASS" | "INTERFACE" | "ENUM" | "RECORD" | "LAMBDA_EXPRESSION");
            !own && fields.iter().any(|(key, value)| key != "symbol" && jumps_out(value, jump))
        }
        Value::Array(items) => items.iter().any(|i| jumps_out(i, jump)),
        _ => false,
    }
}

/// Whether the code `v` has a `break label;` (not in a class or a lambda).
fn breaks_to(v: &Value, label: &str) -> bool {
    match v {
        Value::Object(fields) => {
            let kind = v["kind"].as_str().unwrap_or_default();
            if kind == "BREAK" {
                return v["label"].as_str() == Some(label);
            }
            !matches!(kind, "CLASS" | "INTERFACE" | "ENUM" | "RECORD" | "LAMBDA_EXPRESSION")
                && fields.iter().any(|(key, value)| key != "symbol" && breaks_to(value, label))
        }
        Value::Array(items) => items.iter().any(|i| breaks_to(i, label)),
        _ => false,
    }
}

/// Whether the condition `e` introduces bindings when true, and when false (JLS 6.3.1).
fn condition_bindings(e: &Value) -> (bool, bool) {
    match e["kind"].as_str().unwrap_or_default() {
        "PARENTHESIZED" => condition_bindings(&e["expression"]),
        "LOGICAL_COMPLEMENT" => {
            let (t, f) = condition_bindings(&e["expression"]);
            (f, t)
        }
        "CONDITIONAL_AND" => {
            (condition_bindings(&e["leftOperand"]).0 || condition_bindings(&e["rightOperand"]).0, false)
        }
        "CONDITIONAL_OR" => {
            (false, condition_bindings(&e["leftOperand"]).1 || condition_bindings(&e["rightOperand"]).1)
        }
        "INSTANCE_OF" => (e["pattern"]["kind"].as_str() == Some("BINDING_PATTERN"), false),
        _ => (false, false),
    }
}

/// Whether statement `s` may not complete normally, over-approximated: it has a jump, a `return`
/// or a `throw` (outside the classes and lambdas in it), or a loop without a condition or with
/// `true`.
fn may_not_complete(s: &Value) -> bool {
    match s {
        Value::Object(fields) => {
            let kind = s["kind"].as_str().unwrap_or_default();
            if matches!(kind, "CLASS" | "INTERFACE" | "ENUM" | "RECORD" | "LAMBDA_EXPRESSION") {
                return false;
            }
            let mut condition = &s["condition"];
            while condition["kind"].as_str() == Some("PARENTHESIZED") {
                condition = &condition["expression"];
            }
            let endless = matches!(kind, "WHILE_LOOP" | "DO_WHILE_LOOP" | "FOR_LOOP")
                && (condition.is_null() || condition["value"].as_bool() == Some(true));
            endless
                || matches!(kind, "RETURN" | "THROW" | "BREAK" | "CONTINUE" | "YIELD")
                || fields.iter().any(|(key, value)| key != "symbol" && may_not_complete(value))
        }
        Value::Array(items) => items.iter().any(may_not_complete),
        _ => false,
    }
}

/// The number of binding variables that javac declares for the construct `v` (see
/// `unit_temps`): those of the `instanceof` patterns under it, not in a block, a lambda or a
/// class, which have contexts of their own; `_` has none.
fn bindings(v: &Value) -> usize {
    match v {
        Value::Object(fields) => {
            let kind = v["kind"].as_str().unwrap_or_default();
            if matches!(kind, "BLOCK" | "LAMBDA_EXPRESSION" | "CLASS" | "INTERFACE" | "ENUM" | "RECORD") {
                return 0;
            }
            let own =
                kind == "BINDING_PATTERN" && !matches!(v["variable"]["name"].as_str(), None | Some("" | "_"));
            usize::from(own)
                + fields
                    .iter()
                    .filter(|(key, _)| *key != "symbol")
                    .map(|(_, value)| bindings(value))
                    .sum::<usize>()
        }
        Value::Array(items) => items.iter().map(bindings).sum(),
        _ => 0,
    }
}

/// Whether javac keeps the value that `instanceof` with a pattern tests in a variable of its
/// own: unless it is a local variable or a parameter (`TransPatterns.visitTypeTest`).
fn tested_in_temp(e: &Value) -> bool {
    let mut value = &e["expression"];
    while value["kind"].as_str() == Some("PARENTHESIZED") {
        value = &value["expression"];
    }
    let local = matches!(
        value["symbol"]["kind"].as_str(),
        Some(
            "LOCAL_VARIABLE" | "PARAMETER" | "EXCEPTION_PARAMETER" | "RESOURCE_VARIABLE" | "BINDING_VARIABLE"
        )
    );
    !(value["kind"].as_str() == Some("IDENTIFIER") && local)
}

/// Whether the code of the `switch` expression `s` has a `try` or a `synchronized` block,
/// before which javac keeps the stack in variables of its own (`Gen.hasTry`).
fn has_try(v: &Value) -> bool {
    match v {
        Value::Object(fields) => {
            matches!(v["kind"].as_str(), Some("TRY" | "SYNCHRONIZED"))
                || !matches!(
                    v["kind"].as_str(),
                    Some("CLASS" | "INTERFACE" | "ENUM" | "RECORD" | "LAMBDA_EXPRESSION")
                ) && fields.iter().any(|(key, value)| key != "symbol" && has_try(value))
        }
        Value::Array(items) => items.iter().any(has_try),
        _ => false,
    }
}

/// The position of the tree `v`.
fn position(v: &Value) -> Pos {
    Pos::new(
        v["line"].as_u64().unwrap_or_default() as usize,
        v["column"].as_u64().unwrap_or_default() as usize,
    )
}

/// The slots that lambda `e` takes for the variables of the code around it that it uses, where
/// it uses nothing of `this`: javac makes it a static method whose parameters are those
/// variables, then its own, so only the slots they take together matter. A constant variable is
/// not one of them, javac writes its value. `None` where the lambda may use `this`: `this` or
/// `super`, `Outer.this`, an unqualified instance field or method, or a class it creates (which
/// may need the enclosing object).
fn lambda_captures(e: &Value) -> Option<usize> {
    fn declared<'v>(v: &'v Value, out: &mut HashSet<&'v str>) {
        match v {
            Value::Object(fields) => {
                if v["kind"].as_str() == Some("VARIABLE")
                    && let Some(name) = v["name"].as_str()
                {
                    out.insert(name);
                }
                fields.iter().filter(|(k, _)| *k != "symbol").for_each(|(_, f)| declared(f, out));
            }
            Value::Array(items) => items.iter().for_each(|i| declared(i, out)),
            _ => {}
        }
    }
    fn uses<'v>(v: &'v Value, own: &HashSet<&str>, captured: &mut HashMap<&'v str, usize>) -> bool {
        match v {
            Value::Object(fields) => {
                let kind = v["kind"].as_str().unwrap_or_default();
                let name = v["name"].as_str().unwrap_or_default();
                if kind == "NEW_CLASS" || kind == "MEMBER_SELECT" && matches!(name, "this" | "super") {
                    return false;
                }
                if kind == "IDENTIFIER" {
                    let symbol = &v["symbol"];
                    let local = matches!(
                        symbol["kind"].as_str(),
                        Some(
                            "LOCAL_VARIABLE"
                                | "PARAMETER"
                                | "EXCEPTION_PARAMETER"
                                | "BINDING_VARIABLE"
                                | "RESOURCE_VARIABLE"
                        )
                    );
                    let member = matches!(symbol["kind"].as_str(), Some("FIELD" | "METHOD"))
                        && symbol["static"].as_bool() != Some(true);
                    if name == "this" || name == "super" || member {
                        return false;
                    }
                    if local && !own.contains(name) && symbol["constant"].is_null() {
                        captured.insert(name, size(v));
                    }
                }
                fields.iter().filter(|(k, _)| *k != "symbol").all(|(_, f)| uses(f, own, captured))
            }
            Value::Array(items) => items.iter().all(|i| uses(i, own, captured)),
            _ => true,
        }
    }
    let mut own = HashSet::new();
    declared(e, &mut own);
    let mut captured = HashMap::new();
    uses(&e["body"], &own, &mut captured).then(|| captured.values().sum())
}

/// The slots a variable declaration takes: two for a `long` or a `double`.
fn size(variable: &Value) -> usize {
    match variable["symbol"]["type"].as_str() {
        Some("long" | "double") => 2,
        _ => 1,
    }
}

/// The kinds of the trees of binary operators, which are javac's `JCBinary`.
const BINARY: &[&str] = &[
    "PLUS",
    "MINUS",
    "MULTIPLY",
    "DIVIDE",
    "REMAINDER",
    "LEFT_SHIFT",
    "RIGHT_SHIFT",
    "UNSIGNED_RIGHT_SHIFT",
    "LESS_THAN",
    "GREATER_THAN",
    "LESS_THAN_EQUAL",
    "GREATER_THAN_EQUAL",
    "EQUAL_TO",
    "NOT_EQUAL_TO",
    "AND",
    "XOR",
    "OR",
    "CONDITIONAL_AND",
    "CONDITIONAL_OR",
];

/// The walk of a method's code, with the next free slot.
struct SwitchSlots<'a> {
    outermost: &'a str,
    out: &'a mut JdkTemps,
    next: usize,
    /// The walk is inside a construct that declares the binding variables under it (see
    /// `unit_temps`), and not in a block below it.
    claimed: bool,
    /// The number of the binding variables of the statement walked next that its block has
    /// declared before it (`kept_bindings`).
    kept: usize,
    /// The walk met code whose slots it does not know: its method's temps are left out.
    failed: bool,
}

impl<'a> SwitchSlots<'a> {
    fn new(outermost: &'a str, out: &'a mut JdkTemps, next: usize) -> Self {
        SwitchSlots { outermost, out, next, claimed: false, kept: 0, failed: false }
    }

    fn statement(&mut self, s: &Value) {
        let start = self.next;
        let claimed = self.claimed;
        let list = |v: &Value| v.as_array().cloned().unwrap_or_default();
        let kind = s["kind"].as_str().unwrap_or_default();
        // The binding variables of the patterns in the statement, declared before it.
        if matches!(kind, "IF" | "WHILE_LOOP" | "DO_WHILE_LOOP" | "FOR_LOOP") {
            self.claim(s);
        }
        match kind {
            "BLOCK" => {
                self.claimed = false;
                for statement in list(&s["statements"]) {
                    // The bindings that stay in scope after the statement, the block's from here.
                    match kept_bindings(&statement) {
                        Some(kept) => {
                            self.next += kept;
                            self.kept = kept;
                        }
                        None => self.failed = true,
                    }
                    self.statement(&statement);
                    self.kept = 0;
                }
            }
            "VARIABLE" => {
                // The variable's slot is taken before its initializer runs.
                self.next += size(s);
                self.expression(&s["initializer"]);
                return;
            }
            "EXPRESSION_STATEMENT" => self.expression(&s["expression"]),
            "RETURN" => self.expression(&s["expression"]),
            "THROW" => self.expression(&s["expression"]),
            "IF" => {
                self.expression(&s["condition"]);
                self.statement(&s["thenStatement"]);
                self.statement(&s["elseStatement"]);
            }
            "WHILE_LOOP" | "DO_WHILE_LOOP" => {
                self.expression(&s["condition"]);
                self.statement(&s["statement"]);
            }
            "FOR_LOOP" => {
                for init in list(&s["initializer"]) {
                    self.statement(&init);
                }
                self.expression(&s["condition"]);
                for update in list(&s["update"]) {
                    self.statement(&update);
                }
                self.statement(&s["statement"]);
            }
            "ENHANCED_FOR_LOOP" => {
                // The array, or the iterator, then its length and index.
                let array = s["expression"]["javaType"].as_str().is_some_and(|t| t.ends_with("[]"));
                if !array {
                    let pos = position(&s["expression"]);
                    self.out.push((self.outermost.to_string(), pos, "i$", format!("<local{}>", self.next)));
                }
                self.next += 1;
                self.expression(&s["expression"]);
                self.next += if array { 2 } else { 0 };
                self.next += size(&s["variable"]);
                self.statement(&s["statement"]);
            }
            "LABELED_STATEMENT" => self.statement(&s["statement"]),
            "SYNCHRONIZED" => {
                self.next += 1;
                self.expression(&s["expression"]);
                self.statement(&s["block"]);
            }
            "SWITCH" => self.switch(s),
            "YIELD" => self.expression(&s["value"]),
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
        self.claimed = claimed;
    }

    /// Declares the binding variables under the construct `v`, unless one around it does.
    fn claim(&mut self, v: &Value) {
        if !self.claimed {
            self.next += bindings(v).saturating_sub(std::mem::take(&mut self.kept));
            self.claimed = true;
        }
    }

    /// The `switch`es of an expression, through the expressions for which javac declares no
    /// variables of its own: not into a class's body, a method of its own, and into a lambda's,
    /// another method, with slots of its own.
    fn expression(&mut self, e: &Value) {
        let (start, claimed) = (self.next, self.claimed);
        let kind = e["kind"].as_str().unwrap_or_default();
        if kind == "INSTANCE_OF" && !e["pattern"].is_null() {
            self.claim(e);
            // The value tested, kept in a variable of its own while it is.
            self.next += usize::from(tested_in_temp(e));
            self.expression(&e["expression"]);
            (self.next, self.claimed) = (start, claimed);
            return;
        }
        if kind == "CONDITIONAL_EXPRESSION" || BINARY.contains(&kind) {
            self.claim(e);
        }
        let parts: &[&str] = match kind {
            "SWITCH_EXPRESSION" => return self.switch(e),
            // A lambda is a method of its own, whose slots are known where it uses nothing of
            // `this`: javac makes it static, with the variables it uses, then its parameters.
            "LAMBDA_EXPRESSION" => {
                if let Some(captured) = lambda_captures(e) {
                    let mark = self.out.len();
                    let mut slots = SwitchSlots::new(self.outermost, &mut *self.out, captured);
                    for param in e["parameters"].as_array().into_iter().flatten() {
                        slots.next += size(param);
                    }
                    match e["body"]["kind"].as_str() {
                        Some("BLOCK") => slots.statement(&e["body"]),
                        _ => slots.expression(&e["body"]),
                    }
                    if slots.failed {
                        self.out.truncate(mark);
                    }
                }
                return;
            }
            "PARENTHESIZED" | "TYPE_CAST" | "MEMBER_SELECT" | "INSTANCE_OF" | "UNARY_PLUS"
            | "UNARY_MINUS" | "BITWISE_COMPLEMENT" | "LOGICAL_COMPLEMENT" => &["expression"],
            "ASSIGNMENT" => &["variable", "expression"],
            "ARRAY_ACCESS" => &["expression", "index"],
            "METHOD_INVOCATION" => &["methodSelect", "arguments"],
            "NEW_CLASS" => &["enclosingExpression", "arguments"],
            "NEW_ARRAY" => &["dimensions", "initializers"],
            "CONDITIONAL_EXPRESSION" => &["condition", "trueExpression", "falseExpression"],
            _ if BINARY.contains(&kind) => &["leftOperand", "rightOperand"],
            _ => &[],
        };
        for part in parts {
            match &e[*part] {
                Value::Array(items) => items.iter().for_each(|i| self.expression(i)),
                one => self.expression(one),
            }
        }
        (self.next, self.claimed) = (start, claimed);
    }

    /// A `switch` statement or expression: the temps of one on a string, then its cases.
    fn switch(&mut self, s: &Value) {
        let start = self.next;
        let mut selector = &s["expression"];
        while selector["kind"].as_str() == Some("PARENTHESIZED") {
            selector = &selector["expression"];
        }
        if selector["javaType"].as_str() == Some("java.lang.String") {
            let pos = position(selector);
            self.out.push((self.outermost.to_string(), pos, "s$", format!("<local{}>", self.next)));
            // The string's, whose initializer the selector is, then the case's index.
            self.next += 1;
            self.expression(selector);
            self.next += 1;
        } else {
            self.expression(selector);
        }
        if s["kind"].as_str() == Some("SWITCH_EXPRESSION") && has_try(s) {
            self.next = start;
            return;
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
                    self.expression(body);
                }
            }
        }
        self.next = start;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methods with `instanceof` patterns and a for-each loop each; the slots of the iterators are
    /// javac's (`javap -c` of the class compiled by javac 26): 4, 4, 5, 5, 4 and 5.
    const PATTERNS: &str = "import java.util.List;

class Patterns {
    int statement(Object o, List<String> l) {
        int n = 0;
        if (o instanceof String s && s.length() > 0) {
            n++;
        }
        for (String x : l) {
            n += x.length();
        }
        return n;
    }

    int expression(Object o, List<String> l) {
        int n = (o instanceof Integer i ? i : 0) + (get() instanceof String t ? t.length() : 0);
        for (String x : l) {
            n++;
        }
        return n;
    }

    int inside(Object o, List<String> l) {
        int n = 0;
        while (o instanceof String s && !s.isEmpty()) {
            for (String x : l) {
                n += x.length() + s.length();
            }
            o = get();
        }
        return n;
    }

    int kept(Object o, List<String> l) {
        if (!(o instanceof String s)) {
            return 0;
        }
        int n = s.length();
        for (String x : l) {
            n++;
        }
        return n;
    }

    int notKept(Object o, List<String> l) {
        if (!(o instanceof String s)) {
            o = get();
        }
        int n = 0;
        for (String x : l) {
            n++;
        }
        return n;
    }

    int loop(Object o, List<String> l) {
        while (!(o instanceof String s)) {
            o = get();
        }
        int n = s.length();
        for (String x : l) {
            n++;
        }
        return n;
    }

    Object get() {
        return null;
    }
}
";

    #[test]
    fn iterators_after_instanceof_patterns() {
        let tree = crate::javac::tree(PATTERNS).expect("javac takes the source");
        let iterators: Vec<(usize, String)> = unit_temps(&tree)
            .into_iter()
            .filter(|t| t.2 == "i$")
            .map(|(_, pos, _, text)| (pos.line, text))
            .collect();
        // `kept`'s and `loop`'s bindings outlive their statements, in slots of the method's block
        // before `n`'s; `notKept`'s `if` completes either way, its binding is its own.
        let expected = [
            (9, "<local4>"),
            (17, "<local4>"),
            (26, "<local5>"),
            (39, "<local5>"),
            (50, "<local4>"),
            (61, "<local5>"),
        ];
        assert_eq!(iterators, expected.map(|(line, text)| (line, text.to_string())));
    }
}
