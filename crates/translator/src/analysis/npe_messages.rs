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
        // The JDK's classes are compiled with the names of their locals (`-g`), so the JVM shows
        // those. The classes of `java.lang` that a program gets are the translator's own
        // (`java/Standard.java`, `java/Threads.java`), whose code is not the JDK's.
        let decl = &class.types[method.class];
        let jdk = super::exceptions::from_jdk(decl);
        if jdk && decl.package == "java.lang" {
            continue;
        }
        let at = Messages { class, names, outermost: &outermost, method, jdk };
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
        .filter_map(|id| facts.null_checked.get(id).map(|p| (*p, *id)))
        .min_by_key(|(p, _)| (p.line, p.column));
    if let Some((pos, id)) = missing {
        // A position in the JDK's code may be in any of its many files: the message names the
        // method.
        let at =
            class.methods.iter().filter(|m| super::exceptions::from_jdk(&class.types[m.class])).find(|m| {
                let mut here = false;
                for s in &m.body {
                    visit_stmt(s, &mut |e| visit_expr(e, &mut |x| here |= x.id == id), &mut |_| {});
                }
                here
            });
        let method = at.map_or(String::new(), |m| format!(" in `{}.{}`", class.types[m.class].name, m.name));
        let message = format!(
            "the message of the `NullPointerException` of this dereference{method} not supported yet"
        );
        return Err(Error { pos, message });
    }
    facts.npe_messages = messages;
    Ok(())
}

/// Whether `decl` is `java.lang.Object`.
fn is_object(decl: &TypeDecl) -> bool {
    decl.name == OBJECT || decl.name == "java.lang.Object"
}

/// The qualified name of the outermost class that declares type `t`, by which `BytecodeNames`
/// knows its code: `com.example.Main` for `com.example.Main.Inner`, and for the local class
/// `com.example.Main$1Local` or the lambda's `com.example.Main$$Lambda`.
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
    method: &'a Method,
    /// The method is the JDK's, whose class file has the names of its locals. Its source is
    /// taken to be that of the Java that runs the program (`CHECKED_JAVA`), which
    /// `BytecodeNames` does not compile: what it names is not known, and its messages that need
    /// it (a call, a static field, javac's own locals) are not certain.
    jdk: bool,
}

/// A field as the class file has it.
enum JavaField {
    /// A field of that name.
    Named(String),
    /// The enclosing instance of a lambda's class, which in the class file is the `this` of the
    /// method that holds the lambda's body.
    This,
    /// A local that a lambda captures, which in the class file is a parameter of the method that
    /// holds the lambda's body: `Some` with its name there, `None` where that is not certain.
    Captured(Option<String>),
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
                let call = self.call(x.pos, name).or_else(|| self.jdk_call(x));
                add(receiver, call.map(|m| format!("Cannot invoke \"{m}\"")))
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
                    // The methods of a charset that may be null.
                    Builtin::Charset(CharsetOp::Name) => "name",
                    Builtin::Charset(CharsetOp::DisplayName) => "displayName",
                    Builtin::Charset(CharsetOp::ToString) => "toString",
                    Builtin::Charset(CharsetOp::HashCode) => "hashCode",
                    Builtin::Charset(CharsetOp::Equals) => "equals",
                    // `i.intValue()`, which javac names by the boxed class.
                    Builtin::Boxed(BoxedOp::Unbox, kind) => {
                        let method = format!("{}Value", x.ty);
                        let call = match self.jdk {
                            true => Some(format!("java.lang.{}.{method}()", Type::boxed(*kind))),
                            false => self.call(x.pos, &method),
                        };
                        return add(&args[0], call.map(|m| format!("Cannot invoke \"{m}\"")));
                    }
                    // `Arrays.copyOf` of a null, which fails in the JDK's code (`-g`, so the
                    // parameter has its name): the copy of an array of objects first asks for its
                    // class, the others read its length.
                    Builtin::ArraysCopyOf => {
                        let failure = match args[0].ty {
                            Type::Array(element) if element.is_reference() => {
                                "Cannot invoke \"Object.getClass()\""
                            }
                            _ => "Cannot read the array length",
                        };
                        out.insert(args[0].id, Some(format!("{failure} because \"original\" is null")));
                        return;
                    }
                    // `System.arraycopy` of a null, which the JVM's native code throws without
                    // a message.
                    Builtin::Native(Native::ArrayCopy) => {
                        for array in [&args[0], &args[2]] {
                            out.insert(array.id, None);
                        }
                        return;
                    }
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
            ExprKind::Switch { selector, .. } => self.string_switch(selector, out),
            _ => {}
        }
    }

    /// The message of a `switch` on a string that is null. javac keeps the string in a local of
    /// its own (`s$`), and calls its `hashCode()` first.
    fn string_switch(&self, selector: &Expr, out: &mut HashMap<ExprId, Option<String>>) {
        if selector.ty == Type::String
            && let Some(local) = self.names.get(self.outermost, selector.pos, NameKind::Temp, "s$")
        {
            let message = format!("Cannot invoke \"String.hashCode()\" because \"{local}\" is null");
            out.insert(selector.id, Some(message));
        }
    }

    /// The messages of the dereferences that statement `s` makes itself, by object.
    fn stmt(&self, s: &Stmt, out: &mut HashMap<ExprId, Option<String>>) {
        if let Stmt::Switch { selector, .. } = s {
            self.string_switch(selector, out);
        }
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
        // A conversion between primitives is an instruction that the JVM does not follow back;
        // unboxing is a call of its own (`intValue()`).
        if let ExprKind::Cast(to, inner) = &e.kind
            && transparent(e).is_none()
        {
            let primitive = |t: Type| t.is_numeric() || t == Type::Boolean;
            return (primitive(*to) && primitive(inner.ty)).then_some(Source::Hidden);
        }
        let e = transparent(e)?;
        if let Some(call) = self.called(e) {
            return Some(Source::Shown(call));
        }
        let shown = match &e.kind {
            ExprKind::This => "this".to_string(),
            ExprKind::Null => "null".to_string(),
            // The constants that an instruction holds: `iconst`, `bipush`, `sipush`; a larger
            // one is loaded from the constant pool, which the JVM does not show.
            ExprKind::Int(v) if (-32768..=32767).contains(v) => v.to_string(),
            ExprKind::Int(_) => return Some(Source::Hidden),
            // `(s = a).next`: the class file keeps a copy of the value it stores (`dup`), which the
            // JVM follows back to where the value came from.
            ExprKind::Assign { op: None, target: Place::Var(_), value } => return self.source(value, detail),
            ExprKind::Var(name) if self.jdk => self.jdk_local(name)?,
            ExprKind::Var(_) => self.names.get(self.outermost, e.pos, NameKind::Local, "")?.to_string(),
            ExprKind::Static { name, .. } => {
                self.names.get(self.outermost, e.pos, NameKind::Static, name)?.to_string()
            }
            ExprKind::Field { object, name } => {
                let name = match self.java_field(object, name)? {
                    JavaField::Named(name) => name,
                    JavaField::This => return self.source(object, detail),
                    JavaField::Captured(local) => return Some(Source::Shown(local?)),
                };
                match self.source(object, detail - 1)? {
                    Source::Shown(object) => format!("{object}.{name}"),
                    Source::Hidden => name,
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
            // Values that the JVM does not follow back: computed ones, and new objects.
            ExprKind::Conditional(..) | ExprKind::Binary(..) | ExprKind::Unary(..) | ExprKind::New { .. } => {
                return Some(Source::Hidden);
            }
            _ => return None,
        };
        Some(Source::Shown(shown))
    }

    /// The method that `e` calls, as the class file names it, if it is a call.
    fn called(&self, e: &Expr) -> Option<String> {
        match &e.kind {
            ExprKind::MethodCall { name, .. } => self.call(e.pos, name).or_else(|| self.jdk_call(e)),
            ExprKind::Call { name, .. } => self.call(e.pos, name),
            _ => None,
        }
    }

    /// The method that the call `x` in the JDK's code calls, as the class file refers to it,
    /// which `BytecodeNames` does not compile: javac names an instance method by the erasure of
    /// the static type of the receiver and the erased types of the method's parameters, which
    /// the translation's types are, and the JVM shows `java.lang.Object` and `java.lang.String`
    /// by their simple names. `None` where the translation's types may not be the declared ones:
    /// a receiver of `Object`, whose type may have been an interface or a type variable with
    /// another bound, a string, which may have been a `CharSequence`, or another value type of
    /// the runtime.
    fn jdk_call(&self, x: &Expr) -> Option<String> {
        let ExprKind::MethodCall { receiver, name, resolved: Some(r), is_super: false, .. } = &x.kind else {
            return None;
        };
        if !self.jdk || self.class.methods[*r].is_static {
            return None;
        }
        // javac leaves a method that `Object` declares as `Object`'s, whatever the receiver
        // (`Gen.binaryQualifier`), and the JVM shows it as `Object.equals(Object)`.
        let callee = &self.class.methods[*r];
        if is_object(&self.class.types[callee.class]) {
            let params: Option<Vec<String>> = callee.params.iter().map(|p| self.java_param(p.ty)).collect();
            return Some(format!("Object.{name}({})", params?.join(", ")));
        }
        // `Comparable.compareTo(T)`, whose parameter erases to `Object`, of the translation's own
        // `Comparable`, which javac names `java.lang.Comparable` as the erasure of the receiver.
        if name == "compareTo"
            && let Some(t) = self.class.class_index(receiver.ty)
            && self.class.types[t].name == COMPARABLE
            && self.class.types[callee.class].name == COMPARABLE
        {
            return Some("java.lang.Comparable.compareTo(Object)".to_string());
        }
        let owner = self.java_class(receiver.ty)?;
        let params: Option<Vec<String>> =
            self.class.methods[*r].params.iter().map(|p| self.java_param(p.ty)).collect();
        Some(format!("{owner}.{name}({})", params?.join(", ")))
    }

    /// The binary name of a class of the JDK that `ty` is, when it is certain.
    fn java_class(&self, ty: Type) -> Option<String> {
        let t = self.class.class_index(ty)?;
        let decl = &self.class.types[t];
        if !matches!(decl.origin, Origin::Member) {
            return None;
        }
        // The translator's own classes of the JDK (`java/Threads.java`, `java/Standard.java`) have
        // their real names but no package: it is the names before the first class's, which the
        // JDK's packages all write in lower case and its classes not.
        let template_package = || {
            let segments: Vec<&str> = decl.name.split('.').collect();
            let n = segments.iter().take_while(|s| s.starts_with(|c: char| c.is_ascii_lowercase())).count();
            (segments.first() == Some(&"java") && n >= 2 && n < segments.len())
                .then(|| segments[..n].join("."))
        };
        let package = match decl.package.as_str() {
            "" => template_package()?,
            _ if super::exceptions::from_jdk(decl) => decl.package.clone(),
            _ => return None,
        };
        let rest = decl.name.strip_prefix(&package)?.strip_prefix('.')?;
        let name = format!("{package}.{}", rest.replace('.', "$"));
        (name != "java.lang.Object" && name != "java.lang.String").then_some(name)
    }

    /// A parameter's type as the JVM shows it in a method, when it is certain.
    fn java_param(&self, ty: Type) -> Option<String> {
        Some(match ty {
            Type::Int => "int".to_string(),
            Type::Long => "long".to_string(),
            Type::Double => "double".to_string(),
            Type::Byte => "byte".to_string(),
            Type::Short => "short".to_string(),
            Type::Char => "char".to_string(),
            Type::Float => "float".to_string(),
            Type::Boolean => "boolean".to_string(),
            Type::Array(element) => format!("{}[]", self.java_param(*element)?),
            // A parameter declared `Object`, or of a type variable whose erasure is `Object`, as
            // javac erases it in the descriptor too.
            Type::Class(..)
                if self.class.class_index(ty).is_some_and(|t| is_object(&self.class.types[t])) =>
            {
                "Object".to_string()
            }
            Type::Class(..) => self.java_class(ty)?,
            _ => return None,
        })
    }

    /// The method `name` of the call at `pos`, as the class file names it: `Main$Box.size()`.
    fn call(&self, pos: Pos, name: &str) -> Option<String> {
        self.names.get(self.outermost, pos, NameKind::Call, name).map(str::to_string)
    }

    /// The field `name` of `object` as the class file has it; `None` where that is not certain.
    /// The translation adds fields to a nested class for its enclosing instance and for the locals
    /// it captures. javac does too for a local or anonymous class (`val$x`, `this$0`), which reads
    /// them through `this` except in its constructors (where it reads its parameters), but not for
    /// a lambda, whose body is a method of the enclosing class that takes the captured locals as
    /// parameters.
    fn java_field(&self, object: &Expr, name: &str) -> Option<JavaField> {
        let synthetic = |t: usize| {
            let decl = &self.class.types[t];
            decl.captures.iter().any(|c| c.field == name)
                || (decl.outer.is_some() && name.trim_end_matches('_') == "outer")
        };
        let mut t = self.class.class_index(object.ty);
        let mut own = true;
        while let Some(c) = t {
            if synthetic(c) {
                // Only the current class's own, read through `this` outside a constructor.
                let current = self.method.class;
                if !own || c != current || !matches!(object.kind, ExprKind::This) || self.method.is_ctor {
                    return None;
                }
                let decl = &self.class.types[c];
                if decl.extends.is_some_and(|s| self.inherits_synthetic(s, name)) {
                    return None;
                }
                let capture = decl.captures.iter().find(|c| c.field == name);
                return match (&decl.origin, capture) {
                    (Origin::Lambda { .. }, Some(c)) if self.jdk => {
                        Some(JavaField::Captured(Some(c.name.clone())))
                    }
                    (Origin::Lambda { .. }, Some(_)) => Some(JavaField::Captured(
                        self.names.get(self.outermost, object.pos, NameKind::Local, "").map(str::to_string),
                    )),
                    // The enclosing instance of a lambda's class: `this`, unless that is itself a
                    // lambda's.
                    (Origin::Lambda { .. }, None) => {
                        let enclosing = decl.outer.map(|o| &self.class.types[o].origin);
                        (name == "outer" && !matches!(enclosing, Some(Origin::Lambda { .. }) | None))
                            .then_some(JavaField::This)
                    }
                    (Origin::Local { .. } | Origin::Anonymous { .. }, Some(c)) => {
                        Some(JavaField::Named(format!("val${}", c.name)))
                    }
                    _ => None,
                };
            }
            own = false;
            t = self.class.types[c].extends;
        }
        Some(JavaField::Named(name.to_string()))
    }

    /// The name of the local `name` of a method of the JDK in its source, which its class file
    /// has; `None` for one that the translation may have renamed (`s2` for a second `s`, see
    /// `lower::local_names`).
    fn jdk_local(&self, name: &str) -> Option<String> {
        let method = self.method;
        let base = name.trim_end_matches(|c: char| c.is_ascii_digit());
        if base != name && !method.params.iter().any(|p| p.name == name) {
            let mut declared = method.params.iter().any(|p| p.name == base);
            for s in &method.body {
                visit_stmt(s, &mut |_| {}, &mut |s| match s {
                    Stmt::Local { name, .. } | Stmt::ForEach { name, .. } => declared |= name == base,
                    Stmt::Try { catches, .. } => declared |= catches.iter().any(|c| c.name == base),
                    _ => {}
                });
            }
            if declared {
                return None;
            }
        }
        Some(name.to_string())
    }

    /// Whether class `t` or a superclass has the translation's field `name`.
    fn inherits_synthetic(&self, t: usize, name: &str) -> bool {
        let decl = &self.class.types[t];
        decl.captures.iter().any(|c| c.field == name)
            || (decl.outer.is_some() && name.trim_end_matches('_') == "outer")
            || decl.extends.is_some_and(|s| self.inherits_synthetic(s, name))
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
