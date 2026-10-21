//! Java's message of a `NullPointerException` that the runtime raises, for the programs whose
//! `catch` reads it (`Facts::npe_message_read`). The JVM writes it from the class file: what the
//! failing instruction does and, when it can tell, where the null came from:
//!
//! ```text
//! Cannot invoke "Main$Box.size()" because "<local2>.next" is null
//! Cannot read field "name" because the return value of "Main.find(String)" is null
//! Cannot load from int array because "Main$Box.shared" is null
//! ```
//!
//! A class file compiled without `-g` has no names of locals: a local is `<localN>`, its slot,
//! or `<parameterN>`. What javac's code generator decided for the program's code (the slots, and
//! the methods and fields as the class file names them) comes from javac itself
//! (`javac::BytecodeNames`); the shape of each message from the JVM's own (checked against
//! Java 26, `javac::CHECKED_JAVA`). A dereference whose message is not certain gets none, and
//! the program is not translated where a `catch` may read it.

use super::Facts;
use crate::ast::*;
use crate::javac::{BytecodeNames, CHECKED_JAVA, NameKind};
use std::cell::RefCell;
use std::collections::HashMap;

/// How many steps back the JVM follows where a null came from: `a.b.c.d.e` is shown from `b`.
const MAX_DETAIL: usize = 5;

/// Fills in `Facts::npe_messages`, and rejects the program where a message that a `catch` may
/// read (`Facts::npe_message_needed`) is not certain.
pub fn npe_messages(class: &Class, facts: &mut Facts, names: &BytecodeNames) -> Result<(), Error> {
    let first_needed = || {
        facts
            .npe_message_needed
            .iter()
            .filter_map(|id| facts.null_checked.get(id))
            .min_by_key(|p| (p.line, p.column))
    };
    if names.java != CHECKED_JAVA {
        let pos = first_needed().copied().or(facts.npe_message_read).unwrap_or_default();
        let message = format!(
            "the message of a `NullPointerException` with Java {} not supported yet (it is checked against Java {CHECKED_JAVA})",
            names.java
        );
        return Err(Error { pos, message });
    }
    let messages = RefCell::new(HashMap::new());
    for method in &class.methods {
        let Some(outermost) = outermost(class, method.class) else { continue };
        let at = Messages { class, names, outermost: &outermost };
        for s in &method.body {
            visit_stmt(
                s,
                &mut |e| visit_expr(e, &mut |x| at.expr(x, &mut messages.borrow_mut())),
                &mut |s| at.stmt(s, &mut messages.borrow_mut()),
            );
        }
    }
    let mut messages = messages.into_inner();
    messages.retain(|id, _| facts.null_checked.contains_key(id));
    let missing = facts
        .npe_message_needed
        .iter()
        .filter(|id| !messages.contains_key(id))
        .filter_map(|id| facts.null_checked.get(id))
        .min_by_key(|p| (p.line, p.column));
    if let Some(&pos) = missing {
        let message = "the message of the `NullPointerException` of this dereference not supported yet";
        return Err(Error { pos, message: message.to_string() });
    }
    facts.npe_messages = messages;
    Ok(())
}

/// The qualified name of the outermost class that declares type `t`, by which `BytecodeNames`
/// knows its code: `com.example.Main` for `com.example.Main.Inner`, `Main$1` or `Main$$Lambda`.
fn outermost(class: &Class, t: usize) -> Option<String> {
    let decl = &class.types[t];
    match &decl.origin {
        Origin::Member => {
            let rest = decl.name.strip_prefix(&decl.package).and_then(|r| r.strip_prefix('.'));
            match rest {
                Some(rest) if !decl.package.is_empty() => {
                    Some(format!("{}.{}", decl.package, rest.split('.').next().unwrap_or_default()))
                }
                _ => decl.name.split('.').next().map(str::to_string),
            }
        }
        Origin::Local { binary, .. } | Origin::Anonymous { binary, .. } | Origin::Lambda { binary, .. } => {
            binary.split('$').next().map(str::to_string)
        }
        Origin::Object => None,
    }
}

/// The messages of the code of one method.
struct Messages<'a> {
    class: &'a Class,
    names: &'a BytecodeNames,
    /// The outermost class of the method's class (see `outermost`).
    outermost: &'a str,
}

/// Where a null came from, as the JVM shows it.
enum Source {
    /// Shown, as in `because "<local1>.next" is null`.
    Shown(String),
    /// Not followed by the JVM, such as the value of `c ? a : b`: the message has no `because`.
    Hidden,
}

impl Messages<'_> {
    /// The messages of the dereferences that expression `x` makes itself, by object.
    fn expr(&self, x: &Expr, out: &mut HashMap<ExprId, Option<String>>) {
        let mut add = |object: &Expr, action: Option<String>| {
            if let Some(message) = action.and_then(|a| self.message(&a, object)) {
                out.insert(object.id, Some(message));
            }
        };
        match &x.kind {
            ExprKind::MethodCall { receiver, name, resolved: Some(r), is_super: false, .. }
                if !self.class.methods[*r].is_static =>
            {
                add(receiver, self.call(x.pos, name).map(|m| format!("Cannot invoke \"{m}\"")))
            }
            ExprKind::Field { object, name } => add(object, Some(format!("Cannot read field \"{name}\""))),
            ExprKind::Index(array, _) => {
                add(array, element(array).map(|t| format!("Cannot load from {t} array")))
            }
            ExprKind::Length(array) => add(array, Some("Cannot read the array length".to_string())),
            ExprKind::Assign { op, target: Place::Field { object, name }, .. } => {
                let verb = if op.is_some() { "read" } else { "assign" };
                add(object, Some(format!("Cannot {verb} field \"{name}\"")))
            }
            ExprKind::IncDec { target: Place::Field { object, name }, .. } => {
                add(object, Some(format!("Cannot read field \"{name}\"")))
            }
            ExprKind::Assign { op, target: Place::Index(array, _), .. } => {
                let verb = if op.is_some() { "load from" } else { "store to" };
                add(array, element(array).map(|t| format!("Cannot {verb} {t} array")))
            }
            ExprKind::IncDec { target: Place::Index(array, _), .. } => {
                add(array, element(array).map(|t| format!("Cannot load from {t} array")))
            }
            ExprKind::Builtin(builtin, args) if !args.is_empty() => {
                let method = match builtin {
                    Builtin::StringLength => "length",
                    Builtin::StringEquals => "equals",
                    Builtin::StringCharAt => "charAt",
                    // `synchronized` on a null.
                    Builtin::MonitorEnter => {
                        return add(&args[0], Some("Cannot enter synchronized block".to_string()));
                    }
                    // javac's `Objects.requireNonNull` of a pattern switch's selector, whose
                    // exception has no message.
                    Builtin::NullChecked | Builtin::Unmatched => {
                        out.insert(args[0].id, None);
                        return;
                    }
                    _ => return,
                };
                add(&args[0], self.call(x.pos, method).map(|m| format!("Cannot invoke \"{m}\"")))
            }
            _ => {}
        }
    }

    /// The messages of the dereferences that statement `s` makes itself, by object.
    fn stmt(&self, s: &Stmt, out: &mut HashMap<ExprId, Option<String>>) {
        if let Stmt::Throw(e) = s {
            // The value is converted to `Throwable`, a cast that the class file does not have.
            let thrown = match &e.kind {
                ExprKind::Cast(_, inner) => inner,
                _ => e,
            };
            if let Some(message) = self.message("Cannot throw exception", thrown) {
                out.insert(thrown.id, Some(message));
            }
        }
    }

    /// The message of `action` on `object`, which is null: with where it came from, if the JVM
    /// shows that.
    fn message(&self, action: &str, object: &Expr) -> Option<String> {
        let object = transparent(object)?;
        if let Some(call) = self.called(object) {
            return Some(format!("{action} because the return value of \"{call}\" is null"));
        }
        Some(match self.source(object, MAX_DETAIL)? {
            Source::Shown(text) => format!("{action} because \"{text}\" is null"),
            Source::Hidden => action.to_string(),
        })
    }

    /// Where the value of `e` came from, following at most `detail` steps; `None` where that is
    /// not certain.
    fn source(&self, e: &Expr, detail: usize) -> Option<Source> {
        if detail == 0 {
            return Some(Source::Hidden);
        }
        let e = transparent(e)?;
        if let Some(call) = self.called(e) {
            return Some(Source::Shown(call));
        }
        let shown = match &e.kind {
            ExprKind::This => "this".to_string(),
            ExprKind::Null => "null".to_string(),
            // The constants that an instruction holds: `iconst`, `bipush`, `sipush`.
            ExprKind::Int(v) if (-32768..=32767).contains(v) => v.to_string(),
            ExprKind::Var(_) => {
                let local = self.names.get(self.outermost, e.pos, NameKind::Local, "")?;
                // Locals other than parameters come with their slots (not yet).
                if !local.starts_with("<parameter") {
                    return None;
                }
                local.to_string()
            }
            ExprKind::Static { name, .. } => {
                self.names.get(self.outermost, e.pos, NameKind::Static, name)?.to_string()
            }
            ExprKind::Field { object, name } => {
                if self.synthetic_field(object.ty, name) {
                    return None;
                }
                match self.source(object, detail - 1)? {
                    Source::Shown(object) => format!("{object}.{name}"),
                    Source::Hidden => name.clone(),
                }
            }
            ExprKind::Index(array, index) => {
                let array = match self.source(array, detail - 1)? {
                    Source::Shown(array) => array,
                    Source::Hidden => "<array>".to_string(),
                };
                let index = match self.source(index, detail - 1)? {
                    Source::Shown(index) => index,
                    Source::Hidden => "...".to_string(),
                };
                format!("{array}[{index}]")
            }
            // Values that the JVM does not follow back: computed ones.
            ExprKind::Conditional(..) | ExprKind::Binary(..) | ExprKind::Unary(..) => {
                return Some(Source::Hidden);
            }
            _ => return None,
        };
        Some(Source::Shown(shown))
    }

    /// The method that `e` calls, as the class file names it, if it is a call.
    fn called(&self, e: &Expr) -> Option<String> {
        match &e.kind {
            ExprKind::MethodCall { name, .. } | ExprKind::Call { name, .. } => self.call(e.pos, name),
            _ => None,
        }
    }

    /// The method `name` of the call at `pos`, as the class file names it: `Main$Box.size()`.
    fn call(&self, pos: Pos, name: &str) -> Option<String> {
        self.names.get(self.outermost, pos, NameKind::Call, name).map(str::to_string)
    }

    /// Whether field `name` of an object of type `ty` is one that the translation adds to a
    /// nested class, for its enclosing instance or a captured local, whose name the class file
    /// does not have.
    fn synthetic_field(&self, ty: Type, name: &str) -> bool {
        let mut t = self.class.class_index(ty);
        while let Some(c) = t {
            let decl = &self.class.types[c];
            if decl.captures.iter().any(|capture| capture.field == name)
                || (decl.outer.is_some() && name.trim_end_matches('_') == "outer")
            {
                return true;
            }
            t = decl.extends;
        }
        false
    }
}

/// `e` without the casts that the class file has no instruction for, or that pass the value
/// on as it is (`checkcast`); `None` for a conversion that is an instruction of its own.
fn transparent(e: &Expr) -> Option<&Expr> {
    match &e.kind {
        ExprKind::Cast(to, inner) if to.is_reference() && inner.ty.is_reference() => transparent(inner),
        ExprKind::Cast(Type::Int, inner) if matches!(inner.ty, Type::Char | Type::Byte | Type::Short) => {
            transparent(inner)
        }
        ExprKind::Cast(..) => None,
        _ => Some(e),
    }
}

/// The kind of array of the array `array`, as the JVM's messages name it: `int`, `object`.
fn element(array: &Expr) -> Option<&'static str> {
    let Type::Array(element) = array.ty else { return None };
    Some(match element {
        Type::Int => "int",
        Type::Byte | Type::Boolean => "byte/boolean",
        Type::Char => "char",
        Type::Short => "short",
        Type::Long => "long",
        Type::Float => "float",
        Type::Double => "double",
        _ => "object",
    })
}
