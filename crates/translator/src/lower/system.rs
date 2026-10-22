//! `System.getProperty`, `VM.getSavedProperty` (the same values, saved as the JVM started) and
//! `System.getProperties`. A translated program sees the properties of a plain `java Main` run:
//!
//! - The keys whose value depends on the machine or the run (`KEYS`: `user.dir`, `user.home`,
//!   `os.name`, `os.version`, ...) are answered by the runtime when the program runs, as the JVM
//!   answers them; `java.home` is the `JAVA_HOME` of the run, or else the home of the JDK that the
//!   translator ran with.
//! - `java.class.path` is `.`, as for `java Main` without a class path.
//! - Any other key that the JDK the translator runs with sets (`java -XshowSettings:properties`:
//!   `java.version`, `java.vendor`, `file.encoding`, `user.language`, ...) has that JDK's value,
//!   fixed in the translation.
//! - A key that it does not set (`java.locale.providers`, `jdk.calendar.japanese.supplemental.era`
//!   and any of the program's own) is unset: the default, or null.
//! - `REJECTED` keys, whose value the translation cannot match, are rejected: the command line of
//!   the run, and `user.timezone`, which the JDK sets once `TimeZone` first runs.
//!
//! The key must be a string literal, a constant, or a local of the method that is declared once
//! with a string literal and never assigned again (`FileSystems` names its key so).
//! `System.getProperties()` (in a whole program)
//! is the translator's `java.lang.TranslatedProperties.all()`
//! (`java/overlay/java/lang/TranslatedProperties.java`), a `Properties` of every key of a plain run,
//! each read with `System.getProperty`.

use super::{Lowering, Node, Symbol, unsupported};
use crate::ast::*;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

/// The keys the runtime answers when the program runs (`java_runtime::system_property`, and
/// `java_runtime::java_home` for `java.home`).
const KEYS: &[&str] = &[
    "line.separator",
    "file.separator",
    "path.separator",
    "user.dir",
    "user.home",
    "user.name",
    "java.io.tmpdir",
    "os.name",
    "os.arch",
    "os.version",
    "java.home",
];

/// Keys whose value a translated program cannot have: the command line of the run (the main class
/// and its arguments), and the time zone, which the JDK sets when `TimeZone` first runs.
const REJECTED: &[&str] = &["sun.java.command", "user.timezone"];

/// The properties of the JDK on the path (`java -XshowSettings:properties -version`), read once;
/// `None` when it cannot be run. A value listed on several lines (`java.library.path`) is a path
/// of the parts.
fn jdk_properties() -> Option<&'static HashMap<String, String>> {
    static PROPERTIES: OnceLock<Option<HashMap<String, String>>> = OnceLock::new();
    PROPERTIES
        .get_or_init(|| {
            let output = std::process::Command::new("java")
                .args(["-XshowSettings:properties", "-version"])
                .output()
                .ok()?;
            let text = String::from_utf8_lossy(&output.stderr);
            let mut properties = HashMap::new();
            let mut last: Option<String> = None;
            for line in text.lines().skip_while(|l| l.trim() != "Property settings:").skip(1) {
                if line.trim().is_empty() {
                    break;
                }
                if line.starts_with("        ") {
                    if let Some(value) = last.as_ref().and_then(|k| properties.get_mut(k)) {
                        let value: &mut String = value;
                        value.push(':');
                        value.push_str(line.trim());
                    }
                    continue;
                }
                let Some((key, value)) = line.trim().split_once(" = ") else { continue };
                properties.insert(key.to_string(), value.to_string());
                last = Some(key.to_string());
            }
            (!properties.is_empty()).then_some(properties)
        })
        .as_ref()
}

/// What a read of the property `key` gives.
enum Property {
    /// The runtime's answer, for one of `KEYS`.
    Run(&'static str),
    /// A value fixed in the translation.
    Fixed(String),
    /// No value: the default, or null.
    Unset,
}

/// The value of `key` for a translated program, or why it has none it can give.
fn property(key: &str) -> Result<Property, String> {
    if let Some(&name) = KEYS.iter().find(|k| **k == key) {
        return Ok(Property::Run(name));
    }
    if REJECTED.contains(&key) {
        return Err(format!(
            "`System.getProperty(\"{key}\")`: the JVM sets this value, which the translation does not give"
        ));
    }
    if key == "java.class.path" {
        return Ok(Property::Fixed(".".to_string()));
    }
    let Some(jdk) = jdk_properties() else {
        return Err(format!(
            "`System.getProperty(\"{key}\")` without a `java` on the path to read the JDK's value from"
        ));
    };
    Ok(jdk.get(key).map_or(Property::Unset, |value| Property::Fixed(value.clone())))
}

/// The home of the JDK on the path, which `java.home` falls back to.
fn jdk_home() -> String {
    jdk_properties().and_then(|p| p.get("java.home")).cloned().unwrap_or_default()
}

/// The security properties of the JDK on the path (`conf/security/java.security` under its home),
/// read once; `None` when there is no such file. A value continued with a `\` at the end of a
/// line takes in the next line, as `Properties.load` reads it.
fn security_properties() -> Option<&'static HashMap<String, String>> {
    static PROPERTIES: OnceLock<Option<HashMap<String, String>>> = OnceLock::new();
    PROPERTIES
        .get_or_init(|| {
            let path = std::path::Path::new(&jdk_home()).join("conf/security/java.security");
            let text = std::fs::read_to_string(path).ok()?;
            let mut properties = HashMap::new();
            let mut lines = text.lines();
            while let Some(line) = lines.next() {
                let line = line.trim_start();
                if line.is_empty() || line.starts_with(['#', '!']) {
                    continue;
                }
                let mut entry = line.to_string();
                while entry.ends_with('\\') {
                    entry.pop();
                    entry.push_str(lines.next().unwrap_or_default().trim_start());
                }
                if let Some((key, value)) = entry.split_once(['=', ':']) {
                    properties.insert(key.trim().to_string(), value.trim_start().to_string());
                }
            }
            Some(properties)
        })
        .as_ref()
}

/// `SecurityProperties.includedInExceptions(category)`: whether the system or security property
/// `jdk.includeInExceptions` lists the category. No system property is set in a plain run, so it
/// is the JDK's security property; `None` when the JDK's file cannot be read.
fn included_in_exceptions(category: &str) -> Option<bool> {
    let listed = security_properties()?.get("jdk.includeInExceptions");
    Some(listed.is_some_and(|list| list.split(',').any(|c| c.trim().eq_ignore_ascii_case(category))))
}

/// `Integer.decode` or `Long.decode` of `text` as `Integer.getInteger` and `Long.getLong` read a
/// property: a sign, then a hexadecimal (`0x`, `#`), octal (`0`) or decimal number, in range.
fn decode(text: &str, long: bool) -> Option<i64> {
    let (negative, digits) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };
    let (radix, digits) = if let Some(hex) = digits.strip_prefix("0x").or_else(|| digits.strip_prefix("0X")) {
        (16, hex)
    } else if let Some(hex) = digits.strip_prefix('#') {
        (16, hex)
    } else if digits.len() > 1
        && let Some(octal) = digits.strip_prefix('0')
    {
        (8, octal)
    } else {
        (10, digits)
    };
    if digits.is_empty() || digits.starts_with(['-', '+']) {
        return None;
    }
    let magnitude = i128::from_str_radix(digits, radix).ok()?;
    let value = if negative { -magnitude } else { magnitude };
    let (min, max) =
        if long { (i64::MIN as i128, i64::MAX as i128) } else { (i32::MIN as i128, i32::MAX as i128) };
    (min..=max).contains(&value).then_some(value as i64)
}

impl<'t> Lowering<'t> {
    /// `System.getProperty(key)` or `getProperty(key, default)`; `None` for any other method. The
    /// key must be a literal, so that the translation knows which value to give.
    pub(super) fn system_property_call(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
        args: &mut [Expr],
    ) -> Result<Option<ExprKind>, Error> {
        // `Boolean.getBoolean(key)`, true when the property is `"true"`. The JVM sets no property
        // to `"true"` (`java -XshowSettings:properties` lists none), only `-D` on the command line
        // does, which a translated program has no equivalent of, and the translation has no
        // `System.setProperty`: so it is false for every key, null included, as for a program run
        // without options. `jdk.compat.DecimalFormat`, which `DigitList` reads, is such a key. A
        // key whose evaluation has effects is still evaluated.
        if (symbol.owner, symbol.name) == ("java.lang.Boolean", "getBoolean") {
            let Some(key) = args.first_mut() else { return unsupported(node, "this `Boolean.getBoolean`") };
            if !has_effects(key) {
                return Ok(Some(ExprKind::Bool(false)));
            }
            let pos = node.pos();
            let key = std::mem::replace(key, self.synth(ExprKind::Null, pos, Type::String));
            let block =
                vec![Stmt::Expr(key), Stmt::Yield(self.synth(ExprKind::Bool(false), pos, Type::Boolean))];
            return Ok(Some(ExprKind::Switch {
                selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
                cases: vec![Case { labels: vec![CaseLabel::Default], body: block }],
            }));
        }
        // Values that a plain run has without a property for them: there is no security manager
        // (`java -Djava.security.manager` would set one), so the `jrt:` file system's check of
        // its caller's permission does nothing (`REPLACED_METHODS`); preview features are off
        // (`--enable-preview`); and the categories of detail in exceptions are those of the JDK's
        // security properties.
        match (symbol.owner, symbol.name, symbol.ty) {
            ("java.lang.System", "getSecurityManager", "()java.lang.SecurityManager") => {
                return Ok(Some(ExprKind::Null));
            }
            ("jdk.internal.jrtfs.JrtFileSystemProvider", "checkPermission", "()void") => {
                return Ok(Some(ExprKind::Builtin(Builtin::Native(Native::Nothing), Vec::new())));
            }
            ("jdk.internal.misc.PreviewFeatures", "isPreviewEnabled", "()boolean") => {
                return Ok(Some(ExprKind::Bool(false)));
            }
            ("sun.security.util.SecurityProperties", "includedInExceptions", "(java.lang.String)boolean") => {
                let key = node.list("arguments").next();
                let Some(category) = self.constant_text(key) else {
                    return unsupported(
                        node,
                        "`SecurityProperties.includedInExceptions` of a category that is not a constant",
                    );
                };
                let Some(included) = included_in_exceptions(&category) else {
                    return unsupported(
                        node,
                        "`SecurityProperties.includedInExceptions` without the JDK's security properties to read",
                    );
                };
                return Ok(Some(ExprKind::Bool(included)));
            }
            // `Runtime.version().feature()`, the release of the JDK the translator ran with
            // (`java.specification.version`, `26`), as the source versions of javac's code ask it,
            // without making the version (`Runtime.version()` parses it, see `REDIRECTED_CALLS`).
            ("java.lang.Runtime.Version", "feature", "()int") if crate::program::release_query(node.0) => {
                let feature = jdk_properties()
                    .and_then(|p| p.get("java.specification.version"))
                    .and_then(|v| v.parse::<i32>().ok());
                let Some(feature) = feature else {
                    return unsupported(
                        node,
                        "`Runtime.version()` without a `java` on the path to read the JDK's from",
                    );
                };
                return Ok(Some(ExprKind::Int(feature)));
            }
            ("java.lang.Integer", "getInteger", _) | ("java.lang.Long", "getLong", _) if symbol.is_static => {
                return self.number_property(node, symbol, args).map(Some);
            }
            _ => {}
        }
        if (symbol.owner, symbol.name) == ("java.lang.System", "getProperties") {
            let (owner, name, signature) = crate::program::TRANSLATED_PROPERTIES;
            let resolved = self
                .methods
                .iter()
                .position(|m| self.types[m.owner].javac == owner && m.name == name && m.sig == signature);
            if resolved.is_none() {
                return unsupported(node, "`System.getProperties` outside a whole program");
            }
            return Ok(Some(ExprKind::Call { name: name.to_string(), args: Vec::new(), resolved }));
        }
        if !matches!(
            (symbol.owner, symbol.name),
            ("java.lang.System", "getProperty") | ("jdk.internal.misc.VM", "getSavedProperty")
        ) {
            return Ok(None);
        }
        let key = match args.first().map(|arg| &arg.kind) {
            Some(ExprKind::Str(key)) => Some(key.clone()),
            _ => {
                let key = node.list("arguments").next();
                self.constant_text(key).or_else(|| key.and_then(|k| self.local_constant(k)))
            }
        };
        let Some(key) = key else {
            return unsupported(
                node,
                "`System.getProperty` with a key that is not a string literal or a constant",
            );
        };
        let property = match property(&key) {
            Ok(property) => property,
            Err(message) => return unsupported(node, &message),
        };
        // Java evaluates the default even where it is not the result, so it must have no effect
        // then.
        if !matches!(property, Property::Unset)
            && let Some(default) = args.get(1)
            && !matches!(default.kind, ExprKind::Str(_) | ExprKind::Null | ExprKind::Var(_))
        {
            return unsupported(
                node,
                "a default of `System.getProperty` that is not a literal or a variable",
            );
        }
        match property {
            Property::Run("java.home") => {
                let fallback = self.synth(ExprKind::Str(jdk_home()), node.pos(), Type::String);
                return Ok(Some(ExprKind::Builtin(
                    Builtin::Native(Native::SystemProperty("java.home")),
                    vec![fallback],
                )));
            }
            Property::Run(name) => {
                return Ok(Some(ExprKind::Builtin(
                    Builtin::Native(Native::SystemProperty(name)),
                    Vec::new(),
                )));
            }
            Property::Fixed(value) => return Ok(Some(ExprKind::Str(value))),
            Property::Unset => {}
        }
        // An unknown key: the default if there is one, else null.
        Ok(Some(match args.get_mut(1) {
            Some(default) => std::mem::replace(&mut default.kind, ExprKind::Null),
            None => ExprKind::Null,
        }))
    }
}

impl<'t> Lowering<'t> {
    /// `Integer.getInteger(key)`, `getInteger(key, default)` and `Long.getLong` likewise: the
    /// property's value decoded as a number, or the default (null without one) when the property
    /// is unset or is no number. The key must be a constant, as for `System.getProperty`, and one
    /// that the runtime answers (`user.dir`, ...) is no number.
    fn number_property(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
        args: &mut [Expr],
    ) -> Result<ExprKind, Error> {
        let long = symbol.owner == "java.lang.Long";
        let key = match args.first().map(|arg| &arg.kind) {
            Some(ExprKind::Str(key)) => Some(key.clone()),
            _ => self.constant_text(node.list("arguments").next()),
        };
        let Some(key) = key else {
            return unsupported(
                node,
                &format!("`{}.{}` with a key that is not a constant", symbol.owner, symbol.name),
            );
        };
        let value = match property(&key) {
            Ok(Property::Fixed(text)) => decode(&text, long),
            Ok(Property::Run(_) | Property::Unset) => None,
            Err(message) => return unsupported(node, &message),
        };
        let boxed = Type::boxed(if long { Type::Long } else { Type::Int });
        let pos = node.pos();
        let Some(value) = value else {
            return Ok(match args.get_mut(1) {
                Some(default) => {
                    let default = std::mem::replace(default, self.synth(ExprKind::Null, pos, boxed));
                    self.convert(default, boxed).kind
                }
                None => ExprKind::Null,
            });
        };
        // Java evaluates the default even where the property gives the value.
        if let Some(default) = args.get(1)
            && has_effects(default)
        {
            return unsupported(
                node,
                &format!("a default of `{}.{}` that has effects", symbol.owner, symbol.name),
            );
        }
        let number = if long {
            self.synth(ExprKind::Long(value), pos, Type::Long)
        } else {
            self.synth(ExprKind::Int(value as i32), pos, Type::Int)
        };
        Ok(self.convert(number, boxed).kind)
    }

    /// The text of `node` when it reads a local `String` that the method being lowered declares
    /// once, with a string literal, and never assigns again.
    fn local_constant(&self, node: Node<'t>) -> Option<String> {
        let symbol = node.symbol().filter(|s| s.kind == "LOCAL_VARIABLE" && node.kind() == "IDENTIFIER")?;
        let method = self.methods[self.method_ctx.get()?].node;
        let name = symbol.name;
        let (mut declarations, mut written) = (Vec::new(), false);
        walk(method.0, &mut |n| {
            let kind = n.kind();
            if kind == "VARIABLE" && n.str("name") == name {
                declarations.push(n);
            }
            let variable = if kind == "ASSIGNMENT" || kind.ends_with("_ASSIGNMENT") {
                n.get("variable")
            } else if kind.ends_with("_INCREMENT") || kind.ends_with("_DECREMENT") {
                n.get("expression")
            } else {
                None
            };
            written |= variable.is_some_and(|v| v.kind() == "IDENTIFIER" && v.str("name") == name);
        });
        match declarations[..] {
            [declaration] if !written => {
                let init = declaration.get("initializer").filter(|i| i.kind() == "STRING_LITERAL")?;
                init.0["value"].as_str().map(str::to_string)
            }
            _ => None,
        }
    }
}

/// Calls `f` on every node of the tree under `v`.
fn walk<'t>(v: &'t Value, f: &mut impl FnMut(Node<'t>)) {
    match v {
        Value::Object(map) => {
            f(Node(v));
            map.values().for_each(|c| walk(c, f));
        }
        Value::Array(items) => items.iter().for_each(|i| walk(i, f)),
        _ => {}
    }
}
