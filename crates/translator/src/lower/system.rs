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
//! The key must be a string literal or a constant. `System.getProperties()` (in a whole program)
//! is the translator's `java.lang.TranslatedProperties.all()`
//! (`java/overlay/java/lang/TranslatedProperties.java`), a `Properties` of every key of a plain run,
//! each read with `System.getProperty`.

use super::{Lowering, Node, Symbol, unsupported};
use crate::ast::*;
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
            _ => self.constant_text(node.list("arguments").next()),
        };
        let Some(key) = key else {
            return unsupported(node, "`System.getProperty` with a key that is not a string literal");
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
