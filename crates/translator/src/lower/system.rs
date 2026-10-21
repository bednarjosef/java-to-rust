//! `System.getProperty`, and `VM.getSavedProperty`, which gives the same values. A key whose
//! value does not depend on the JVM is answered by the runtime at run time, as Java does. The
//! JVM's own keys are rejected, since a translated program would see different values from
//! Java's. Any other key is unknown: null, or the default.

use super::{Lowering, Node, Symbol, unsupported};
use crate::ast::*;
// Keys under `JVM_KEYS` that only the command line sets, never the JVM itself: unknown keys, as
// for a program run without them.
use crate::program::UNSET_PROPERTIES as UNSET_KEYS;

/// The keys the runtime answers (`java_runtime::system_property`).
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
];

/// Prefixes of keys that the JVM sets. A program reading one would see the JVM's value, which the
/// translation cannot give, so these are rejected rather than taken for unknown keys.
const JVM_KEYS: &[&str] = &[
    "java.",
    "jdk.",
    "sun.",
    "os.version",
    "file.encoding",
    "native.encoding",
    "stdout.encoding",
    "stderr.encoding",
    "user.language",
    "user.country",
    "user.script",
    "user.variant",
    "user.timezone",
];

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
        // The properties that the JVM saved as it started are the same (`VM.getSavedProperty`).
        if !matches!(
            (symbol.owner, symbol.name),
            ("java.lang.System", "getProperty") | ("jdk.internal.misc.VM", "getSavedProperty")
        ) {
            return Ok(None);
        }
        let Some(ExprKind::Str(key)) = args.first().map(|arg| &arg.kind) else {
            return unsupported(node, "`System.getProperty` with a key that is not a string literal");
        };
        let key = key.clone();
        if let Some(&name) = KEYS.iter().find(|k| **k == key) {
            // Java evaluates the default even though it is never the result, so it must have no
            // effect.
            if let Some(default) = args.get(1)
                && !matches!(default.kind, ExprKind::Str(_) | ExprKind::Null | ExprKind::Var(_))
            {
                return unsupported(
                    node,
                    "a default of `System.getProperty` that is not a literal or a variable",
                );
            }
            return Ok(Some(ExprKind::Builtin(Builtin::Native(Native::SystemProperty(name)), Vec::new())));
        }
        if JVM_KEYS.iter().any(|prefix| key.starts_with(prefix)) && !UNSET_KEYS.contains(&key.as_str()) {
            return unsupported(
                node,
                &format!(
                    "`System.getProperty(\"{key}\")`: the JVM sets this value, which the translation does not give"
                ),
            );
        }
        // An unknown key: the default if there is one, else null.
        Ok(Some(match args.get_mut(1) {
            Some(default) => std::mem::replace(&mut default.kind, ExprKind::Null),
            None => ExprKind::Null,
        }))
    }
}
