//! `sun.util.logging.PlatformLogger`, through which the JDK's code reports what is wrong with its
//! configuration (`Currency` a bad currency property, `AbstractChronology` a duplicate
//! chronology). Without the `java.logging` module, its logger is the JDK's `SimpleConsoleLogger`,
//! as `System.Logger`'s is (see `natives::system_log`): it prints the messages of level `INFO` and
//! above to `System.err`, each after a line with the time and the caller, and with the stack
//! trace of a `Throwable` given with it. The translation prints `LEVEL: message` without that
//! line, which a translated program has no stack to find the caller in, and the trace as
//! `printStackTrace()` does.
//!
//! A logger has no value of its own, as every logger that `getLogger(name)` gives behaves the same
//! where nothing configures logging. A local variable that `PlatformLogger.getLogger(name)`
//! initializes is left out, and so is a *logger field*: a static field whose declaration
//! initializes it so, with a name that is a constant, that nothing else assigns (it is final, or
//! private and its class assigns it nowhere), and that is set before any other code of its
//! class's static initializer runs, so that no read in that code finds it still null. The methods
//! are lowered here on such a variable, on such a field, or on `getLogger(name)` itself. A logger
//! field of a class other than the one whose code reads it and its superclasses, as a nested
//! class reads its outer class's, is read after initializing its class, as Java's read of the
//! field does. The name is evaluated for nothing but its effects, so it must have none. The
//! logging methods that take parameters for a `MessageFormat` pattern evaluate them, and are
//! rejected only at a level that the logger prints. Anything else done with a logger is rejected.

use super::*;

/// The class, which is the runtime's: reachability does not enter it.
const PLATFORM_LOGGER: &str = "sun.util.logging.PlatformLogger";

/// The levels of `PlatformLogger.Level`, with whether the default logger prints them.
const LEVELS: [(&str, bool); 9] = [
    ("ALL", false),
    ("FINEST", false),
    ("FINER", false),
    ("FINE", false),
    ("CONFIG", false),
    ("INFO", true),
    ("WARNING", true),
    ("SEVERE", true),
    ("OFF", false),
];

/// The methods that log a message at a level, by name.
const LOGGING: [&str; 7] = ["finest", "finer", "fine", "config", "info", "warning", "severe"];

/// Whether the logger name `node` is a constant: a string literal, a compile-time constant, or a
/// concatenation of them.
fn constant_name(node: Node) -> bool {
    match node.kind() {
        "STRING_LITERAL" => true,
        "PARENTHESIZED" => node.get("expression").is_some_and(constant_name),
        "PLUS" => [node.get("leftOperand"), node.get("rightOperand")]
            .into_iter()
            .all(|o| o.is_some_and(constant_name)),
        _ => node.symbol().is_some_and(|s| s.kind == "FIELD" && s.constant.is_some()),
    }
}

/// Whether some node of the tree under `v` is one that `f` tells apart.
fn any_node<'t>(v: &'t crate::value::Value, f: &impl Fn(Node<'t>) -> bool) -> bool {
    match v {
        crate::value::Value::Object(map) => f(Node(v)) || map.values().any(|c| any_node(c, f)),
        crate::value::Value::Array(items) => items.iter().any(|i| any_node(i, f)),
        _ => false,
    }
}

/// The member of the standard `Throwable` that prints a `Throwable` logged with a message, which
/// the standard classes keep when a program logs one (see `exceptions::Standard::select`).
pub(super) const PRINT_STACK_TRACE: (&str, &str, &str) = ("java.lang.Throwable", "printStackTrace", "()void");

/// Whether the operation `operation`, as `exceptions::RAISED` names one (`owner#name(sig)ret`), is
/// a `PlatformLogger` method that logs a message with a `Throwable`.
pub(super) fn logs_throwable(operation: &str) -> bool {
    LOGGING.iter().any(|name| {
        operation == format!("{PLATFORM_LOGGER}#{name}(java.lang.String,java.lang.Throwable)void")
    })
}

impl<'t> Lowering<'t> {
    /// Whether the local variable `node` is a `PlatformLogger`, which is left out: its value is
    /// `getLogger(name)`, or it is rejected.
    pub(super) fn logger_variable(&self, node: Node<'t>) -> Result<bool, Error> {
        if node.symbol().is_none_or(|s| s.ty != PLATFORM_LOGGER) {
            return Ok(false);
        }
        match node.get("initializer") {
            Some(init) if self.get_logger(init)? => Ok(true),
            _ => unsupported(
                node,
                "a `PlatformLogger` variable that `PlatformLogger.getLogger(name)` does not set",
            ),
        }
    }

    /// The statements of the expression statement `node` when it logs a message through a
    /// `PlatformLogger`: printing the message, and the trace of a `Throwable` given with it,
    /// when the level is one that the logger prints; else only the message's effects.
    pub(super) fn logger_statement(&self, node: Node<'t>) -> Result<Option<Vec<Stmt>>, Error> {
        let call = node.child("expression")?;
        let Some(symbol) = call.symbol().filter(|_| call.kind() == "METHOD_INVOCATION") else {
            return Ok(None);
        };
        if symbol.owner != PLATFORM_LOGGER || !LOGGING.contains(&symbol.name) {
            return Ok(None);
        }
        let mut out: Vec<Stmt> = self.logger_receiver(call)?.into_iter().map(Stmt::Expr).collect();
        let args: Vec<Node<'t>> = call.list("arguments").collect();
        let throwable = symbol.ty == "(java.lang.String,java.lang.Throwable)void";
        let parameters = symbol.ty == "(java.lang.String,java.lang.Object[])void";
        if symbol.ty != "(java.lang.String)void" && !throwable && !parameters {
            return unsupported(call, "this `PlatformLogger` method");
        }
        let level = symbol.name.to_uppercase();
        let shown = LEVELS.iter().any(|&(l, shown)| l == level && shown);
        let pos = call.pos();
        if !shown {
            for arg in args {
                out.push(Stmt::Expr(self.expr(arg)?));
            }
            return Ok(Some(out));
        }
        if parameters {
            return unsupported(call, "logging a message with parameters through a `PlatformLogger`");
        }
        let message = self.expr(args[0])?;
        let prefix = self.synth(ExprKind::Str(format!("{level}: ")), pos, Type::String);
        let text = ExprKind::Binary(BinOp::Add, Box::new(prefix), Box::new(self.as_text(message)));
        let text = self.synth(text, pos, Type::String);
        let print = ExprKind::Builtin(Builtin::Native(Native::ErrPrint { newline: true }), vec![text]);
        out.push(Stmt::Expr(self.synth(print, pos, Type::Void)));
        if throwable {
            out.push(Stmt::Expr(self.print_stack_trace(call, args[1])?));
        }
        Ok(Some(out))
    }

    /// `logger.isLoggable(PlatformLogger.Level.L)` and `logger.isEnabled()` (`node`): whether the
    /// default logger prints messages of that level, and that it prints some. `None` for any
    /// other call.
    pub(super) fn logger_query(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
    ) -> Result<Option<ExprKind>, Error> {
        if symbol.owner != PLATFORM_LOGGER {
            return Ok(None);
        }
        let answer = match (symbol.name, symbol.ty) {
            ("isEnabled", "()boolean") => true,
            ("isLoggable", "(sun.util.logging.PlatformLogger.Level)boolean") => {
                let level = node.list("arguments").next().filter(|l| l.kind() == "MEMBER_SELECT");
                let level = level.map(|l| l.str("identifier"));
                match LEVELS.iter().find(|&&(l, _)| Some(l) == level) {
                    Some(&(_, shown)) => shown,
                    None => {
                        return unsupported(
                            node,
                            "`PlatformLogger.isLoggable` of a level that is not a constant",
                        );
                    }
                }
            }
            (name, _) if LOGGING.contains(&name) => {
                return unsupported(node, "logging through a `PlatformLogger` where it is not a statement");
            }
            _ => return Ok(None),
        };
        let Some(init) = self.logger_receiver(node)? else { return Ok(Some(ExprKind::Bool(answer))) };
        // `{ init C; answer }`, a block as `by_name::ensure_initialized` makes one.
        let pos = node.pos();
        let answer = self.synth(ExprKind::Bool(answer), pos, Type::Boolean);
        Ok(Some(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case {
                labels: vec![CaseLabel::Default],
                body: vec![Stmt::Expr(init), Stmt::Yield(answer)],
            }],
        }))
    }

    /// Checks that the receiver of the call `node` of a `PlatformLogger` method is a variable that
    /// `logger_variable` left out, a logger field, or `getLogger(name)`. The result is the
    /// initialization of the class of a logger field that the current class's code may find not
    /// yet initialized, which is to run first.
    fn logger_receiver(&self, node: Node<'t>) -> Result<Option<Expr>, Error> {
        let receiver = node.child("methodSelect")?.get("expression");
        let local = receiver.is_some_and(|r| {
            r.kind() == "IDENTIFIER" && r.symbol().is_some_and(|s| s.kind == "LOCAL_VARIABLE")
        });
        if local || receiver.is_some_and(|r| self.get_logger(r).unwrap_or(false)) {
            return Ok(None);
        }
        let Some(owner) = receiver.and_then(|r| self.logger_field_read(r)) else {
            return unsupported(node, "a `PlatformLogger` other than `PlatformLogger.getLogger(name)`");
        };
        // The current class's code runs after its initialization, which initializes its
        // superclasses first.
        let mut class = Some(self.current.get());
        while let Some(c) = class {
            if c == owner {
                return Ok(None);
            }
            class = self.superclass(c).ok().flatten();
        }
        let init = ExprKind::Builtin(Builtin::InitializeClass { class: owner }, Vec::new());
        Ok(Some(self.synth(init, node.pos(), Type::Void)))
    }

    /// The class of the logger field that `node` reads, by its name or selected from its class.
    fn logger_field_read(&self, node: Node<'t>) -> Option<usize> {
        let symbol = node.symbol().filter(|s| s.kind == "FIELD" && s.is_static && s.ty == PLATFORM_LOGGER)?;
        let of_class = match node.kind() {
            "IDENTIFIER" => true,
            "MEMBER_SELECT" => node
                .get("expression")
                .and_then(|e| e.symbol())
                .is_some_and(|s| matches!(s.kind, "CLASS" | "INTERFACE" | "ENUM" | "RECORD")),
            _ => false,
        };
        let owner = self.type_index(symbol.owner).filter(|_| of_class)?;
        let member = (self.types[owner].node.list("members"))
            .find(|m| m.kind() == "VARIABLE" && m.str("name") == symbol.name)?;
        self.is_logger_field(owner, member).then_some(owner)
    }

    /// Whether `member` of class `owner` is a logger field (see the module's comment), which is no
    /// field of the translation.
    pub(super) fn is_logger_field(&self, owner: usize, member: Node<'t>) -> bool {
        let initialized = |m: Node<'t>| {
            m.kind() == "VARIABLE"
                && m.has_flag("STATIC")
                && m.symbol().is_some_and(|s| s.ty == PLATFORM_LOGGER)
                && m.get("initializer").is_some_and(|init| {
                    init.kind() == "METHOD_INVOCATION"
                        && init.symbol().is_some_and(|s| (s.owner, s.name) == (PLATFORM_LOGGER, "getLogger"))
                        && init.list("arguments").next().is_some_and(constant_name)
                })
        };
        if !initialized(member) {
            return false;
        }
        let class = self.types[owner].node;
        if !member.has_flag("FINAL") {
            let (name, javac) = (member.str("name"), self.types[owner].javac.as_str());
            let assigns = |n: Node<'t>| {
                (n.kind() == "ASSIGNMENT" || n.kind().ends_with("_ASSIGNMENT"))
                    && n.get("variable")
                        .and_then(|v| v.symbol())
                        .is_some_and(|s| s.kind == "FIELD" && (s.owner, s.name) == (javac, name))
            };
            if !member.has_flag("PRIVATE") || any_node(class.0, &assigns) {
                return false;
            }
        }
        // No code of the static initializer runs before it is set.
        let before = class.list("members").take_while(|m| !std::ptr::eq(m.0, member.0));
        before.into_iter().all(|m| {
            !Self::is_static_block(m)
                && (m.kind() != "VARIABLE"
                    || !m.has_flag("STATIC")
                    || m.get("initializer").is_none()
                    || Self::is_constant(m)
                    || initialized(m))
        })
    }

    /// Whether `node` is `PlatformLogger.getLogger(name)` with a name without effects.
    fn get_logger(&self, node: Node<'t>) -> Result<bool, Error> {
        let is_get = node.kind() == "METHOD_INVOCATION"
            && node.symbol().is_some_and(|s| (s.owner, s.name) == (PLATFORM_LOGGER, "getLogger"));
        let Some(name) = node.list("arguments").next().filter(|_| is_get) else { return Ok(false) };
        if has_effects(&self.expr(name)?) {
            return unsupported(
                node,
                "a `PlatformLogger.getLogger` name that does more than name the logger",
            );
        }
        Ok(true)
    }

    /// `throwable.printStackTrace()` for the `Throwable` that `call` logs.
    fn print_stack_trace(&self, call: Node<'t>, throwable: Node<'t>) -> Result<Expr, Error> {
        let (owner, name, sig) = PRINT_STACK_TRACE;
        let found =
            self.methods.iter().any(|m| self.types[m.owner].javac == owner && m.name == name && m.sig == sig);
        if !found {
            return unsupported(call, "logging a `Throwable` through a `PlatformLogger`");
        }
        let (line, column) = (&call.0["line"], &call.0["column"]);
        let symbol = crate::value::json!({"kind": "METHOD", "name": name, "owner": owner, "static": false, "type": sig});
        let print = crate::value::json!({"kind": "METHOD_INVOCATION", "line": line, "column": column,
            "javaType": "void", "arguments": [],
            "methodSelect": {"kind": "MEMBER_SELECT", "line": line, "column": column, "identifier": name,
                "expression": throwable.0.clone(), "symbol": symbol},
            "symbol": symbol});
        let print: &'t crate::value::Value = Box::leak(Box::new(print));
        self.expr(Node(print))
    }
}
