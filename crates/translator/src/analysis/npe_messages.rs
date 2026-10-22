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

/// The messages of `Arrays.binarySearch` comparing a null key with a string, and a null element.
pub(crate) const SEARCH_NULL_KEY: &str = "Cannot read field \"value\" because \"anotherString\" is null";
pub(crate) const SEARCH_NULL_ELEMENT: &str =
    "Cannot invoke \"java.lang.Comparable.compareTo(Object)\" because \"midVal\" is null";
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
        // those. The standard classes of `java.lang` are the translator's own
        // (`java/Standard.java`, `java/Threads.java`), whose code is not the JDK's; the others are
        // the JDK's (`CharacterName`), and its overlays get the messages of their own code, as
        // other overlays do.
        let decl = &class.types[method.class];
        let jdk = super::exceptions::from_jdk(decl);
        let outer = decl.name.split('.').take(3).collect::<Vec<_>>().join(".");
        if jdk && decl.package == "java.lang" && crate::lower::is_standard(&outer) {
            continue;
        }
        // `CharSequence`'s default `getChars` in a class of the program is the JDK's code too.
        let jdk = jdk || method.jdk_code;
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
            // A message that the expression above gave (a native's).
            if out.contains_key(&object.id) {
                return;
            }
            // The JVM gives no message for the null receiver of an unbound method reference
            // (`String::length`), which the class it makes for the reference dereferences.
            if self.reference_receiver(object) {
                out.insert(object.id, None);
                return;
            }
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
            // Unboxing in the JDK's code, which javac writes as a call of the boxed class's
            // method (`Integer.intValue()`, also for a wider result), on the failing conversion.
            ExprKind::Cast(to, value)
                if self.jdk && !to.is_reference() && matches!(value.ty, Type::Boxed(_)) =>
            {
                let primitive = value.ty.unboxed();
                let action = format!("Cannot invoke \"java.lang.{}.{primitive}Value()\"", value.ty);
                if let Some(message) = self.message(&action, value) {
                    out.insert(x.id, Some(message));
                }
            }
            // The natives of `java.io`, whose C code throws an exception with no message for a null
            // string or array (a path, the name of a file to open, a buffer) and for a null `File`
            // whose path it reads (`lower::natives`); a null descriptor is a closed stream to it,
            // which the translation does not keep apart.
            ExprKind::Builtin(Builtin::Native(native), args) if native.is_java_io() => {
                for arg in args {
                    if arg.ty == Type::String || matches!(arg.ty, Type::Array(_)) {
                        out.insert(arg.id, None);
                    }
                    if let ExprKind::Field { object, name } = &arg.kind
                        && matches!(name.as_str(), "path" | "fd")
                    {
                        out.insert(object.id, None);
                    }
                }
            }
            ExprKind::Assign { op: None, target: Place::Field { object, .. }, value } if matches!(&value.kind, ExprKind::Builtin(Builtin::Native(n), _) if n.is_java_io()) =>
            {
                out.insert(object.id, None);
            }
            ExprKind::Field { object, name } => add(object, Some(format!("Cannot read field \"{name}\""))),
            ExprKind::Index(..) if let Some((object, name)) = enum_table(x) => {
                add(object, Some(format!("Cannot read field \"{name}\"")))
            }
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
            ExprKind::Assign { op, target: Place::Index(array, index), .. }
                if let Some((object, name)) = enum_table_row(array, index) =>
            {
                let verb = if op.is_some() { "read" } else { "assign" };
                add(object, Some(format!("Cannot {verb} field \"{name}\"")))
            }
            ExprKind::IncDec { target: Place::Index(array, index), .. }
                if let Some((object, name)) = enum_table_row(array, index) =>
            {
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
                // A method of `String`, which javac names by `String` with the parameters of the
                // overload it chose.
                if let Some((method, params)) = string_method(builtin, args) {
                    let call = match self.jdk {
                        true => (args[0].ty == Type::String).then(|| format!("String.{method}({params})")),
                        false => self.call(x.pos, method),
                    };
                    return add(&args[0], call.map(|m| format!("Cannot invoke \"{m}\"")));
                }
                // A method of a builder, named as the call names it: by the receiver's type,
                // `java.lang.StringBuilder.ensureCapacity(int)` (or `StringBuffer`), with the
                // parameters of the overload javac chose.
                if let Builtin::Builder(op) = builtin
                    && let Some(name) = builder_method(*op)
                {
                    let call = match self.jdk {
                        true => self.named_call(x.pos, name).flatten(),
                        false => self.call(x.pos, name),
                    };
                    return add(&args[0], call.map(|m| format!("Cannot invoke \"{m}\"")));
                }
                // The methods of a charset that may be null.
                let charset = match builtin {
                    Builtin::Charset(CharsetOp::Name) => Some("name"),
                    Builtin::Charset(CharsetOp::DisplayName) => Some("displayName"),
                    Builtin::Charset(CharsetOp::ToString) => Some("toString"),
                    Builtin::Charset(CharsetOp::HashCode) => Some("hashCode"),
                    Builtin::Charset(CharsetOp::Equals) => Some("equals"),
                    _ => None,
                };
                if let Some(method) = charset {
                    return add(&args[0], self.call(x.pos, method).map(|m| format!("Cannot invoke \"{m}\"")));
                }
                // An enum's own methods, of an enum that may be null. A null argument of
                // `compareTo` fails in `Enum.compareTo`, the JDK's code (`-g`, so its local has
                // its name).
                let enum_method = match builtin {
                    Builtin::EnumOrdinal => Some("ordinal"),
                    Builtin::EnumName => Some("name"),
                    Builtin::EnumCompareTo => Some("compareTo"),
                    _ => None,
                };
                if let Some(method) = enum_method {
                    add(&args[0], self.call(x.pos, method).map(|m| format!("Cannot invoke \"{m}\"")));
                    if let (Builtin::EnumCompareTo, Some(other)) = (builtin, args.get(1)) {
                        let failure = "Cannot invoke \"Object.getClass()\" because \"other\" is null";
                        out.insert(other.id, Some(failure.to_string()));
                    }
                    return;
                }
                match builtin {
                    // `i.intValue()`, which javac names by the boxed class.
                    Builtin::Boxed(BoxedOp::Unbox, kind) => {
                        let method = format!("{}Value", x.ty);
                        let call = match self.jdk {
                            true => Some(format!("java.lang.{}.{method}()", Type::boxed(*kind))),
                            false => self.call(x.pos, &method),
                        };
                        add(&args[0], call.map(|m| format!("Cannot invoke \"{m}\"")))
                    }
                    // `Arrays.copyOf` and `copyOfRange` of a null, which fail in the JDK's code
                    // (`-g`, so the parameter has its name): the copy of an array of objects first
                    // asks for its class, the others read its length (`copyOfRange` after checking
                    // the range unless `from` is 0, which `Gen::arrays_op` follows).
                    Builtin::ArraysCopyOf | Builtin::Arrays(ArraysOp::CopyOfRange) => {
                        let failure = match args[0].ty {
                            Type::Array(element) if element.is_reference() => {
                                "Cannot invoke \"Object.getClass()\""
                            }
                            _ => "Cannot read the array length",
                        };
                        out.insert(args[0].id, Some(format!("{failure} because \"original\" is null")));
                    }
                    // `new StringBuilder(s)` of a null string, which fails in the JDK's
                    // constructor.
                    Builtin::Builder(BuilderOp::New) if args.len() == 1 && args[0].ty == Type::String => {
                        let message = "Cannot invoke \"String.length()\" because \"str\" is null";
                        out.insert(args[0].id, Some(message.to_string()));
                    }
                    // `new String(s)` of a null, which fails in the JDK's constructor.
                    Builtin::String(StringOp::Identity) => {
                        let message = "Cannot read field \"value\" because \"original\" is null";
                        out.insert(args[0].id, Some(message.to_string()));
                    }
                    // `new String(bytes, ..)` of a null array, which fails in the JDK's
                    // constructor, unless a charset name that may be null fails first (a
                    // `Charset` never is null).
                    Builtin::String(StringOp::FromBytes)
                        if args[1..]
                            .iter()
                            .all(|a| a.ty != Type::String || matches!(a.kind, ExprKind::Str(_))) =>
                    {
                        let message = "Cannot read the array length because \"bytes\" is null";
                        out.insert(args[0].id, Some(message.to_string()));
                    }
                    // `Arrays.binarySearch` of a null array, which `Arrays` asks for its length, of
                    // a null key, which fails in `String.compareTo` of an element, and of a null
                    // element, whose `compareTo` `Arrays` calls (`-g`, so the parameters and locals
                    // have their names).
                    Builtin::Arrays(ArraysOp::BinarySearch | ArraysOp::BinarySearchRange) => {
                        let message = "Cannot read the array length because \"a\" is null";
                        // An `Object[]` that holds strings is the array, converted
                        // (`StringArrayOf`), and dereferenced as it is.
                        let mut array = &args[0];
                        while let ExprKind::Builtin(Builtin::StringArrayOf, inner) = &array.kind {
                            out.insert(array.id, Some(message.to_string()));
                            array = &inner[0];
                        }
                        out.insert(array.id, Some(message.to_string()));
                        out.insert(args[args.len() - 1].id, Some(SEARCH_NULL_KEY.to_string()));
                        out.insert(x.id, Some(SEARCH_NULL_ELEMENT.to_string()));
                    }
                    // A boxed argument of `Math`'s methods in the JDK's code, which the lowering
                    // passes as it is: javac unboxes it with the boxed class's method.
                    Builtin::Native(Native::Math { .. }) if self.jdk => {
                        for arg in args.iter().filter(|a| matches!(a.ty, Type::Boxed(_))) {
                            let action =
                                format!("Cannot invoke \"java.lang.{}.{}Value()\"", arg.ty, arg.ty.unboxed());
                            if let Some(message) = self.message(&action, arg) {
                                out.insert(arg.id, Some(message));
                            }
                        }
                    }
                    // `a.clone()` of a null array, a call that the class file names by the array's
                    // descriptor: `[I.clone()`.
                    Builtin::Arrays(ArraysOp::Clone) => {
                        let action =
                            self.descriptor(args[0].ty).map(|d| format!("Cannot invoke \"{d}.clone()\""));
                        if let Some(message) = action.and_then(|a| self.message(&a, &args[0])) {
                            out.insert(args[0].id, Some(message));
                        }
                    }
                    // `Arrays.fill` and `Arrays.sort` of a null array, which the JDK's code asks for
                    // its length (`-g`, so the parameter has its name).
                    Builtin::ArraysFill
                    | Builtin::ArraysSort
                    | Builtin::Arrays(ArraysOp::FillRange | ArraysOp::SortAll | ArraysOp::SortRange) => {
                        let message = "Cannot read the array length because \"a\" is null";
                        out.insert(args[0].id, Some(message.to_string()));
                    }
                    // `new String(ascii, hibyte, offset, count)` and `new String(chars, ..)` of a
                    // null array, which the JDK's constructor asks for its length.
                    Builtin::String(StringOp::FromHighByte) => {
                        let message = "Cannot read the array length because \"ascii\" is null";
                        out.insert(args[0].id, Some(message.to_string()));
                    }
                    Builtin::String(StringOp::FromChars) => {
                        let message = "Cannot read the array length because \"value\" is null";
                        out.insert(args[0].id, Some(message.to_string()));
                    }
                    // `System.arraycopy` of a null, which the JVM's native code throws without
                    // a message.
                    Builtin::Native(Native::ArrayCopy) => {
                        for array in [&args[0], &args[2]] {
                            out.insert(array.id, None);
                        }
                    }
                    // `new StringBuilder(seq)` of a null `seq`, which the JDK's constructor
                    // asks for its length (`-g`, so the parameter has its name).
                    Builtin::Builder(BuilderOp::New) if matches!(args[0].ty, Type::Class(c, _) if c == CHAR_SEQUENCE) =>
                    {
                        let failure = "Cannot invoke \"java.lang.CharSequence.length()\"";
                        out.insert(args[0].id, Some(format!("{failure} because \"seq\" is null")));
                    }
                    // `new StringBuilder(s)` of a null string, which the JDK's constructor asks for
                    // its length in `AbstractStringBuilder(String str)`.
                    Builtin::Builder(BuilderOp::New) if args[0].ty == Type::String => {
                        let failure = "Cannot invoke \"String.length()\"";
                        out.insert(args[0].id, Some(format!("{failure} because \"str\" is null")));
                    }
                    // `synchronized` on a null.
                    Builtin::MonitorEnter => {
                        add(&args[0], Some("Cannot enter synchronized block".to_string()))
                    }
                    // The JDK's `Objects.requireNonNull` of the text that
                    // `Long.parseLong(s, begin, end, radix)` and its kin read, whose exception has
                    // no message: the parse's own (`exceptions::failures`), and the text's for
                    // the unsigned forms.
                    Builtin::Bits(BitsOp::ParseRange, _) => {
                        out.insert(x.id, None);
                    }
                    // javac's `Objects.requireNonNull` of a pattern switch's selector, whose
                    // exception has no message.
                    Builtin::NullChecked
                    | Builtin::Unmatched
                    | Builtin::Unsigned(UnsignedOp::ParseRange, _) => {
                        out.insert(args[0].id, None);
                    }
                    _ => {}
                }
            }
            ExprKind::Switch { selector, .. } => self.switch(selector, out),
            _ => {}
        }
    }

    /// The message of a `switch` on a string or an enum that is null.
    fn switch(&self, selector: &Expr, out: &mut HashMap<ExprId, Option<String>>) {
        match selector.ty {
            Type::Enum(name) => self.enum_switch(name, selector, out),
            _ => self.string_switch(selector, out),
        }
    }

    /// The message of a `switch` on an enum that is null, of enum `name`: javac's switch asks for
    /// its `ordinal()`, named by the enum's binary name.
    fn enum_switch(&self, name: &str, selector: &Expr, out: &mut HashMap<ExprId, Option<String>>) {
        let Some(binary) = self.class.type_index(name).and_then(|t| self.binary_name(t)) else { return };
        if let Some(message) = self.message(&format!("Cannot invoke \"{binary}.ordinal()\""), selector) {
            out.insert(selector.id, Some(message));
        }
    }

    /// The message of a `switch` on a string that is null. javac keeps the string in a local of
    /// its own (`s$`), and calls its `hashCode()` first.
    fn string_switch(&self, selector: &Expr, out: &mut HashMap<ExprId, Option<String>>) {
        if selector.ty != Type::String {
            return;
        }
        // The JDK's temps are the walk of its slots (`jdk_switch_temps`), or else the slot that
        // is certain (`jdk_switch_slot`).
        let local = self.names.get(self.outermost, selector.pos, NameKind::Temp, "s$").map(str::to_string);
        // In the code of a lambda, javac's method for the body has that local in its table of
        // locals, as `s0$`, for each `switch`.
        let lambda = matches!(self.class.types[self.method.class].origin, Origin::Lambda { .. });
        let local = match self.jdk {
            true if lambda => local.or_else(|| Some("s0$".to_string())),
            true => local.or_else(|| self.jdk_switch_slot(selector).map(|slot| format!("<local{slot}>"))),
            false => local,
        };
        if let Some(local) = local {
            let message = format!("Cannot invoke \"String.hashCode()\" because \"{local}\" is null");
            out.insert(selector.id, Some(message));
        }
    }

    /// The slot of javac's local that holds the string of a `switch` on `selector` in the JDK's
    /// code, which the class file's table of locals leaves out (it is synthetic), where it is
    /// certain: the first slot after the parameters, when the method declares no local before
    /// the `switch` and has nothing else of javac's own there (another `switch`, a `try`, a
    /// for-each). Not for the code of a lambda (see `string_switch`), nor a constructor that javac
    /// gives parameters of its own (an enum's, or one of a class with an enclosing instance or
    /// captured locals).
    fn jdk_switch_slot(&self, selector: &Expr) -> Option<usize> {
        let method = self.method;
        let decl = &self.class.types[method.class];
        let synthetic_params =
            decl.kind == TypeKind::Enum || decl.outer.is_some() || !decl.captures.is_empty();
        if matches!(decl.origin, Origin::Lambda { .. }) || (method.is_ctor && synthetic_params) {
            return None;
        }
        let size = |ty: Type| if matches!(ty, Type::Long | Type::Double) { 2 } else { 1 };
        let slot = usize::from(!method.is_static) + method.params.iter().map(|p| size(p.ty)).sum::<usize>();
        let (clean, found) = (std::cell::Cell::new(true), std::cell::Cell::new(None));
        for s in &method.body {
            visit_stmt(
                s,
                &mut |e| {
                    visit_expr(e, &mut |x| match &x.kind {
                        _ if found.get().is_some() => {}
                        _ if x.id == selector.id => found.set(Some(clean.get())),
                        ExprKind::Switch { selector: other, .. } if other.id != selector.id => {
                            clean.set(false)
                        }
                        _ => {}
                    })
                },
                &mut |s| match s {
                    _ if found.get().is_some() => {}
                    Stmt::Switch { selector: other, .. } if other.id == selector.id => {}
                    Stmt::Local { .. } | Stmt::ForEach { .. } | Stmt::Try { .. } | Stmt::Switch { .. } => {
                        clean.set(false)
                    }
                    _ => {}
                },
            );
        }
        found.get()?.then_some(slot)
    }

    /// The messages of the dereferences that statement `s` makes itself, by object.
    fn stmt(&self, s: &Stmt, out: &mut HashMap<ExprId, Option<String>>) {
        if let Stmt::Switch { selector, .. } = s {
            self.switch(selector, out);
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
            // `i++` or `--i` of an `int` local: `iinc` and a load of the local, which the JVM
            // shows by its name.
            ExprKind::IncDec { target: Place::Var(name), .. } if self.jdk && e.ty == Type::Int => {
                self.jdk_local(name)?
            }
            ExprKind::Var(_) => self.names.get(self.outermost, e.pos, NameKind::Local, "")?.to_string(),
            // A static field of the JDK's, by the class that javac names it by where lowering noted
            // it (`Class::class_file`), else by its class's binary name: `java.util.Locale.ROOT`.
            ExprKind::Static { class, name } if self.jdk => match self.jdk_static(e.pos, name) {
                Some(named) => named?,
                None => {
                    let declared = self.class.types[*class].statics.iter().any(|f| &f.name == name);
                    format!("{}.{name}", self.java_class_at(*class).filter(|_| declared)?)
                }
            },
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
            // A field of an enum in the table form (`lower/enum_tables.rs`): `e.f` is
            // `fOfConstants[e.ordinal()]`, and `this.f` in a constructor
            // `fOfConstants[constantOrdinal]`.
            ExprKind::Index(array, index)
                if let ExprKind::Static { class, name } = &array.kind
                    && let Some(field) = crate::lower::tabled_field(name)
                    && self.class.types[*class].kind == TypeKind::Enum =>
            {
                let object = match &index.kind {
                    ExprKind::Builtin(Builtin::EnumOrdinal, args) => self.source(&args[0], detail - 1)?,
                    ExprKind::Var(v) if v == crate::lower::TABLE_ORDINAL => Source::Shown("this".to_string()),
                    _ => return None,
                };
                match object {
                    Source::Shown(object) => format!("{object}.{field}"),
                    Source::Hidden => field.to_string(),
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
            ExprKind::Call { name, .. } => self.call(e.pos, name).or_else(|| {
                // A static method of the JDK, as lowering found javac names it.
                let named = self.jdk.then(|| self.named_call(e.pos, name).flatten()).flatten();
                named.or_else(|| self.jdk_static_call(e))
            }),
            _ => None,
        }
    }

    /// The static method that the call `x` in the JDK's code calls, which javac names by the
    /// class that declares it, unless the call reaches it by inheritance: the class that the code
    /// names, or that inherits it, which the translation does not tell.
    fn jdk_static_call(&self, x: &Expr) -> Option<String> {
        let ExprKind::Call { name, resolved: Some(r), .. } = &x.kind else { return None };
        let callee = &self.class.methods[*r];
        if !self.jdk || !callee.is_static {
            return None;
        }
        // The classes the code is in, which an unqualified call may find the method by.
        let mut t = Some(self.method.class);
        while let Some(c) = t {
            if c != callee.class && self.extends(c, callee.class) {
                return None;
            }
            t = self.enclosing(c);
        }
        let owner = self.java_class_at(callee.class)?;
        let params: Option<Vec<String>> = callee.params.iter().map(|p| self.java_param(p.ty)).collect();
        Some(format!("{owner}.{name}({})", params?.join(", ")))
    }

    /// The class whose code class `t` is declared in: the one it is a member of, or that has
    /// the method it is local to.
    fn enclosing(&self, t: usize) -> Option<usize> {
        let decl = &self.class.types[t];
        match &decl.origin {
            Origin::Member => {
                let (outer, _) = decl.name.rsplit_once('.')?;
                self.class.types.iter().position(|o| o.name == outer && matches!(o.origin, Origin::Member))
            }
            Origin::Local { method, .. }
            | Origin::Anonymous { method, .. }
            | Origin::Lambda { method, .. } => method.map(|m| self.class.methods[m].class),
            Origin::Object => None,
        }
    }

    /// Whether class `t` extends or implements `ancestor`.
    fn extends(&self, t: usize, ancestor: usize) -> bool {
        let decl = &self.class.types[t];
        t == ancestor
            || decl
                .extends
                .into_iter()
                .chain(decl.implements.iter().copied())
                .any(|s| self.extends(s, ancestor))
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
        // As lowering found javac names it, where it noted the call.
        if let Some(Some(named)) = self.named_call(x.pos, name) {
            return Some(named);
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

    /// The method that the call at `pos` names, as the JVM shows it,
    /// `java.util.Properties.getProperty(String)`, where lowering noted the call
    /// (`Class::class_file`): `Some(None)` where it is not certain.
    fn named_call(&self, pos: Pos, name: &str) -> Option<Option<String>> {
        let key = (self.class.types[self.method.class].name.clone(), pos, name.to_string());
        let named = self.class.class_file.calls.get(&key)?;
        Some(named.as_ref().and_then(|call| {
            let params: Option<Vec<String>> = call.params.iter().map(|p| self.shown_class(p)).collect();
            Some(format!("{}.{}({})", self.shown_class(&call.class)?, call.name, params?.join(", ")))
        }))
    }

    /// The descriptor of type `ty` as the JVM shows an array's class in a message: `[I`,
    /// `[[Ljava.lang.String;`, when that is certain.
    fn descriptor(&self, ty: Type) -> Option<String> {
        let class = |name: &str| Some(format!("L{};", self.binary_name(self.class.type_index(name)?)?));
        Some(match ty {
            Type::Int => "I".to_string(),
            Type::Long => "J".to_string(),
            Type::Double => "D".to_string(),
            Type::Float => "F".to_string(),
            Type::Byte => "B".to_string(),
            Type::Short => "S".to_string(),
            Type::Char => "C".to_string(),
            Type::Boolean => "Z".to_string(),
            Type::Array(element) => format!("[{}", self.descriptor(*element)?),
            Type::String => "Ljava.lang.String;".to_string(),
            Type::Boxed(_) => format!("Ljava.lang.{ty};"),
            Type::JavaClass => "Ljava.lang.Class;".to_string(),
            Type::Class(name, _) | Type::Enum(name) => class(name)?,
            _ => return None,
        })
    }

    /// A type in javac's names as the JVM shows it in a message: a primitive as it is, an array
    /// with `[]`, `Object` and `String` by their simple names, and another class by its binary
    /// name, when that is certain.
    fn shown_class(&self, ty: &str) -> Option<String> {
        if let Some(element) = ty.strip_suffix("[]") {
            return Some(format!("{}[]", self.shown_class(element)?));
        }
        Some(match ty {
            "int" | "long" | "double" | "float" | "short" | "byte" | "char" | "boolean" => ty.to_string(),
            "java.lang.Object" => "Object".to_string(),
            "java.lang.String" => "String".to_string(),
            _ => match self.class.types.iter().position(|d| d.name == ty) {
                Some(t) => self.java_class_at(t)?,
                // A local or anonymous class of the JDK, by javac's name (`IsoFields.Field$1`).
                None if let Some(decl) = self.class.types.iter().find(|d| {
                    matches!(&d.origin, Origin::Local { binary, .. } | Origin::Anonymous { binary, .. } if binary == ty)
                }) =>
                {
                    let rest = ty.strip_prefix(decl.package.as_str())?.strip_prefix('.')?;
                    (super::exceptions::from_jdk(decl) && !decl.package.is_empty())
                        .then(|| format!("{}.{}", decl.package, rest.replace('.', "$")))?
                }
                // A class that the program does not have: its name, unless it may be nested.
                None => {
                    let segments: Vec<&str> = ty.split('.').collect();
                    let upper = |s: &str| s.starts_with(|c: char| c.is_ascii_uppercase());
                    let plain = segments.len() > 1 && segments.iter().filter(|s| upper(s)).count() == 1;
                    (plain && upper(segments.last()?)).then(|| ty.to_string())?
                }
            },
        })
    }

    /// The static field `name` that the JDK's code reads at `pos`, as the JVM shows it:
    /// `jdk.internal.foreign.AbstractMemorySegmentImpl.NIO_ACCESS`, by the class that lowering
    /// found names it (`Class::static_qualifiers`).
    /// `None` where lowering did not note the read, `Some(None)` where the name is not certain.
    fn jdk_static(&self, pos: Pos, name: &str) -> Option<Option<String>> {
        let key = (self.class.types[self.method.class].name.clone(), pos, name.to_string());
        let qualifier = self.class.class_file.static_qualifiers.get(&key)?;
        Some(qualifier.as_deref().and_then(|qualifier| {
            let t = self.class.types.iter().position(|d| d.name == qualifier)?;
            Some(format!("{}.{name}", self.java_class_at(t)?))
        }))
    }

    /// Whether `object` is the receiver of the call that an unbound method reference makes: the
    /// first parameter of the method of the reference's class, which the translation reads at the
    /// reference's position (`lower::lambdas`), where a lambda's body starts after its parameters.
    fn reference_receiver(&self, object: &Expr) -> bool {
        let method = self.method;
        matches!(self.class.types[method.class].origin, Origin::Lambda { .. })
            && !method.is_ctor
            && object.pos == method.pos
            && matches!(&object.kind, ExprKind::Var(name) if method.params.first().is_some_and(|p| &p.name == name))
    }

    /// The binary name of a class of the JDK that `ty` is, when it is certain.
    fn java_class(&self, ty: Type) -> Option<String> {
        self.java_class_at(self.class.class_index(ty)?)
    }

    /// The binary name of the class of the JDK that is type `t`, when it is certain.
    fn java_class_at(&self, t: usize) -> Option<String> {
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

    /// The binary name of class `t`, a member class of the program or the JDK:
    /// `com.example.Outer$Inner`.
    fn binary_name(&self, t: usize) -> Option<String> {
        let decl = &self.class.types[t];
        if is_object(decl) {
            return Some("java.lang.Object".to_string());
        }
        if super::exceptions::from_jdk(decl) || decl.package.is_empty() && decl.name.starts_with("java.") {
            return self.java_class_at(t);
        }
        if !matches!(decl.origin, Origin::Member) {
            return None;
        }
        Some(match decl.package.as_str() {
            "" => decl.name.replace('.', "$"),
            package => {
                format!("{package}.{}", decl.name.strip_prefix(package)?.strip_prefix('.')?.replace('.', "$"))
            }
        })
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
            // The JVM writes `java.lang.String` without its package, as `Object`. A string is one
            // where the code has `String`: a `CharSequence` is a class of its own (`lower::object`).
            Type::String => "String".to_string(),
            Type::JavaClass => "java.lang.Class".to_string(),
            // A parameter declared `Object`, or of a type variable whose erasure is `Object`, as
            // javac erases it in the descriptor too: also one of a Rust generic, whose type
            // variables have no bounds (`lower::real_generics`).
            Type::Class(..)
                if self.class.class_index(ty).is_some_and(|t| {
                    is_object(&self.class.types[t]) || self.class.types[t].kind == TypeKind::Param
                }) =>
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
        if self.jdk
            && name.trim_end_matches('_') == "outer"
            && let Some(field) = self.jdk_this_field(object)
        {
            return field;
        }
        let mut t = self.class.class_index(object.ty);
        let mut own = true;
        while let Some(c) = t {
            if synthetic(c) {
                // A class's own, read through `this` outside a constructor (where javac reads its
                // parameter instead); the enclosing instance of a member, local or anonymous
                // class also through another object of it, which javac reads as a field too.
                let this = matches!(object.kind, ExprKind::This);
                let decl = &self.class.types[c];
                let enclosing = decl.captures.iter().all(|c| c.field != name)
                    && matches!(
                        decl.origin,
                        Origin::Member | Origin::Local { .. } | Origin::Anonymous { .. }
                    );
                let through_this = c == self.method.class && this && !self.method.is_ctor;
                if !own || !(through_this || (enclosing && !this)) {
                    return None;
                }
                // A superclass's field of the same name is either one: certain only when it is
                // the enclosing instance that javac names alike in both.
                if decl.extends.is_some_and(|s| self.inherits_synthetic(s, name))
                    && !(enclosing && self.outer_named_alike(c))
                {
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
                    // The enclosing instance, which javac holds in `this$0` (`Class::class_file`).
                    (Origin::Member | Origin::Local { .. } | Origin::Anonymous { .. }, None) => {
                        self.class.class_file.outer_fields.get(&decl.name).cloned().map(JavaField::Named)
                    }
                    _ => None,
                };
            }
            own = false;
            t = self.class.types[c].extends;
        }
        Some(JavaField::Named(name.to_string()))
    }

    /// The enclosing instance of `object`, of an inner class of the JDK, as the class file reads
    /// it: javac gives each inner class a field of its own for it (`this$0`), which the class's
    /// code reads, while the translation may keep it in a superclass's of the same enclosing
    /// class. A member class's is read so through any object; a local or anonymous class's only
    /// through `this`, as the others are not certain. `None` for a value that is not an inner
    /// class's, `Some(None)` where the name is not certain, as in a constructor, which reads its
    /// parameter instead.
    fn jdk_this_field(&self, object: &Expr) -> Option<Option<JavaField>> {
        let o = self.class.class_index(object.ty)?;
        let decl = &self.class.types[o];
        let enclosing = decl.outer?;
        let through_this = matches!(object.kind, ExprKind::This) && o == self.method.class;
        let certain = match decl.origin {
            Origin::Member => true,
            Origin::Local { .. } | Origin::Anonymous { .. } => through_this,
            _ => return None,
        };
        if !certain || through_this && self.method.is_ctor {
            return Some(None);
        }
        let mut t = decl.extends;
        while let Some(c) = t {
            if self.class.types[c].outer.is_some_and(|e| e != enclosing) {
                return Some(None);
            }
            t = self.class.types[c].extends;
        }
        Some(self.this_field(o))
    }

    /// The field of the JDK's inner class `c` that javac keeps its enclosing instance in:
    /// `this$N`, where `N` is how many enclosing instances its enclosing class has itself.
    fn this_field(&self, c: usize) -> Option<JavaField> {
        let mut outer = self.class.types[c].outer?;
        let mut depth = 0;
        while let Some(o) = self.class.types[outer].outer {
            if !matches!(self.class.types[outer].origin, Origin::Member) {
                return None;
            }
            depth += 1;
            outer = o;
        }
        Some(JavaField::Named(format!("this${depth}")))
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

    /// Whether the enclosing instances of class `t` and of each of its superclasses that has
    /// one are fields of the same name in their class files.
    fn outer_named_alike(&self, t: usize) -> bool {
        let names = &self.class.class_file.outer_fields;
        let Some(own) = names.get(&self.class.types[t].name) else { return false };
        std::iter::successors(self.class.types[t].extends, |&s| self.class.types[s].extends)
            .filter(|&s| self.class.types[s].outer.is_some())
            .all(|s| names.get(&self.class.types[s].name) == Some(own))
    }

    /// Whether class `t` or a superclass has the translation's field `name`.
    fn inherits_synthetic(&self, t: usize, name: &str) -> bool {
        let decl = &self.class.types[t];
        decl.captures.iter().any(|c| c.field == name)
            || (decl.outer.is_some() && name.trim_end_matches('_') == "outer")
            || decl.extends.is_some_and(|s| self.inherits_synthetic(s, name))
    }
}

/// The method of `String` that a builtin on a string `args[0]` calls, with the parameters of the
/// overload as the JVM shows them (`String.indexOf(String, int)`); `None` for one that is no
/// method of the string, or whose overload the builtin does not tell.
fn string_method(builtin: &Builtin, args: &[Expr]) -> Option<(&'static str, String)> {
    let text = |t: Type| match t {
        Type::Char | Type::Int => Some("int"),
        Type::String => Some("String"),
        _ => None,
    };
    let rest: Vec<Type> = args[1..].iter().map(|a| a.ty).collect();
    let fixed = |name: &'static str, params: &str| Some((name, params.to_string()));
    let op = match builtin {
        Builtin::StringLength => return fixed("length", ""),
        Builtin::StringEquals => return fixed("equals", "Object"),
        Builtin::StringCharAt => return fixed("charAt", "int"),
        Builtin::String(op) => *op,
        _ => return None,
    };
    match (op, rest.as_slice()) {
        (StringOp::Substring, [_]) => fixed("substring", "int"),
        (StringOp::Substring, [_, _]) => fixed("substring", "int, int"),
        (StringOp::IndexOf | StringOp::LastIndexOf, [x, from @ ..]) if from.len() <= 1 => {
            let name = if op == StringOp::IndexOf { "indexOf" } else { "lastIndexOf" };
            Some((name, if from.is_empty() { text(*x)?.to_string() } else { format!("{}, int", text(*x)?) }))
        }
        (StringOp::Contains, [_]) => fixed("contains", "java.lang.CharSequence"),
        (StringOp::StartsWith, [_]) => fixed("startsWith", "String"),
        (StringOp::StartsWith, [_, _]) => fixed("startsWith", "String, int"),
        (StringOp::EndsWith, [_]) => fixed("endsWith", "String"),
        (StringOp::IsEmpty, []) => fixed("isEmpty", ""),
        (StringOp::IsBlank, []) => fixed("isBlank", ""),
        (StringOp::Trim, []) => fixed("trim", ""),
        (StringOp::Strip, []) => fixed("strip", ""),
        (StringOp::StripLeading, []) => fixed("stripLeading", ""),
        (StringOp::StripTrailing, []) => fixed("stripTrailing", ""),
        (StringOp::StripIndent, []) => fixed("stripIndent", ""),
        (StringOp::ToUpperCase, []) => fixed("toUpperCase", ""),
        (StringOp::ToLowerCase, []) => fixed("toLowerCase", ""),
        // With a locale, which lowering gives as its language (`case_in_locale`).
        (StringOp::ToUpperCase, [Type::String]) => fixed("toUpperCase", "java.util.Locale"),
        (StringOp::ToLowerCase, [Type::String]) => fixed("toLowerCase", "java.util.Locale"),
        (StringOp::Replace, [Type::Char, Type::Char]) => fixed("replace", "char, char"),
        (StringOp::Replace, [_, _]) => fixed("replace", "java.lang.CharSequence, java.lang.CharSequence"),
        (StringOp::ReplaceAll, [_, _]) => fixed("replaceAll", "String, String"),
        (StringOp::ReplaceFirst, [_, _]) => fixed("replaceFirst", "String, String"),
        (StringOp::Matches, [_]) => fixed("matches", "String"),
        (StringOp::Split, [_]) => fixed("split", "String"),
        (StringOp::Split, [_, _]) => fixed("split", "String, int"),
        (StringOp::Repeat, [_]) => fixed("repeat", "int"),
        (StringOp::CompareTo, [_]) => fixed("compareTo", "String"),
        (StringOp::CompareToIgnoreCase, [_]) => fixed("compareToIgnoreCase", "String"),
        (StringOp::EqualsIgnoreCase, [_]) => fixed("equalsIgnoreCase", "String"),
        (StringOp::EqualsOther, [_]) => fixed("equals", "Object"),
        (StringOp::Concat, [_]) => fixed("concat", "String"),
        (StringOp::ToCharArray, []) => fixed("toCharArray", ""),
        (StringOp::GetChars, [_, _, _, _]) => fixed("getChars", "int, int, char[], int"),
        (StringOp::GetLowBytes, [_, _, _, _]) => fixed("getBytes", "int, int, byte[], int"),
        (StringOp::GetBytes, []) => fixed("getBytes", ""),
        (StringOp::GetBytes, [Type::Charset]) => fixed("getBytes", "java.nio.charset.Charset"),
        (StringOp::GetBytes, [Type::String]) => fixed("getBytes", "String"),
        (StringOp::CharCodes, []) => fixed("chars", ""),
        (StringOp::Lines, []) => fixed("lines", ""),
        (StringOp::HashCode, []) => fixed("hashCode", ""),
        (StringOp::ToString, []) => fixed("toString", ""),
        (StringOp::Intern, []) => fixed("intern", ""),
        (StringOp::CodePointAt, [_]) => fixed("codePointAt", "int"),
        (StringOp::CodePointBefore, [_]) => fixed("codePointBefore", "int"),
        (StringOp::CodePointCount, [_, _]) => fixed("codePointCount", "int, int"),
        (StringOp::OffsetByCodePoints, [_, _]) => fixed("offsetByCodePoints", "int, int"),
        (StringOp::ContentEquals, [Type::String]) => fixed("contentEquals", "java.lang.CharSequence"),
        (StringOp::RegionMatches, [_, _, _, _]) => fixed("regionMatches", "int, String, int, int"),
        (StringOp::RegionMatches, [_, _, _, _, _]) => {
            fixed("regionMatches", "boolean, int, String, int, int")
        }
        _ => None,
    }
}

/// The name of the method of `StringBuilder` (or `StringBuffer`) that a builder's operation is;
/// `None` for a constructor.
fn builder_method(op: BuilderOp) -> Option<&'static str> {
    Some(match op {
        BuilderOp::New => return None,
        BuilderOp::Append => "append",
        BuilderOp::Insert => "insert",
        BuilderOp::Reverse => "reverse",
        BuilderOp::SetCharAt => "setCharAt",
        BuilderOp::CharAt => "charAt",
        BuilderOp::DeleteCharAt => "deleteCharAt",
        BuilderOp::Delete => "delete",
        BuilderOp::Replace => "replace",
        BuilderOp::IndexOf => "indexOf",
        BuilderOp::LastIndexOf => "lastIndexOf",
        BuilderOp::Length => "length",
        BuilderOp::SetLength => "setLength",
        BuilderOp::ToString => "toString",
        BuilderOp::Substring => "substring",
        BuilderOp::IsEmpty => "isEmpty",
        BuilderOp::CompareTo => "compareTo",
        BuilderOp::AppendCodePoint => "appendCodePoint",
        BuilderOp::Repeat => "repeat",
        BuilderOp::CodePointAt => "codePointAt",
        BuilderOp::CodePointBefore => "codePointBefore",
        BuilderOp::CodePointCount => "codePointCount",
        BuilderOp::OffsetByCodePoints => "offsetByCodePoints",
        BuilderOp::GetChars => "getChars",
        BuilderOp::EnsureCapacity => "ensureCapacity",
    })
}

/// `e` without the casts that the class file has no instruction for, or that pass the value
/// on as it is (`checkcast`); `None` for a conversion that is an instruction of its own.
fn transparent(e: &Expr) -> Option<&Expr> {
    let object = |t: Type| t.is_reference() || matches!(t, Type::Enum(_));
    match &e.kind {
        ExprKind::Cast(to, inner) if object(*to) && object(inner.ty) => transparent(inner),
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

/// An instance field of an enum kept as a table (see `lower::enum_tables`), `fOfConstants[e.ordinal()]`:
/// the enum constant `e` and the field's name `f`.
fn enum_table(x: &Expr) -> Option<(&Expr, &str)> {
    match &x.kind {
        ExprKind::Index(array, index) => enum_table_row(array, index),
        _ => None,
    }
}

/// `enum_table` of the parts of an index.
fn enum_table_row<'a>(array: &'a Expr, index: &'a Expr) -> Option<(&'a Expr, &'a str)> {
    match (&array.kind, &index.kind) {
        (ExprKind::Static { name, .. }, ExprKind::Builtin(Builtin::EnumOrdinal, args)) => {
            Some((args.first()?, crate::lower::tabled_field(name)?))
        }
        _ => None,
    }
}
