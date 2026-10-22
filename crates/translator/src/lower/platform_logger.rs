//! `sun.util.logging.PlatformLogger`, through which the JDK's code reports what is wrong with its
//! configuration (`Currency` a bad currency property, `AbstractChronology` a duplicate
//! chronology). Without the `java.logging` module, its logger is the JDK's `SimpleConsoleLogger`,
//! as `System.Logger`'s is (see `natives::system_log`): it prints the messages of level `INFO` and
//! above to `System.err`, each after a line with the time and the caller, and with the stack
//! trace of a `Throwable` given with it. The translation prints `LEVEL: message` without that
//! line, which a translated program has no stack to find the caller in, and the trace as
//! `printStackTrace()` does.
//!
//! A logger has no value of its own: a local variable that `PlatformLogger.getLogger(name)`
//! initializes is left out, and the methods are lowered here on that variable or on
//! `getLogger(name)` itself. The name is evaluated for nothing but its effects, so it must have
//! none. Anything else done with a logger is rejected.

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
        self.logger_receiver(call)?;
        let args: Vec<Node<'t>> = call.list("arguments").collect();
        let throwable = symbol.ty == "(java.lang.String,java.lang.Throwable)void";
        if symbol.ty != "(java.lang.String)void" && !throwable {
            return unsupported(call, "this `PlatformLogger` method");
        }
        let level = symbol.name.to_uppercase();
        let shown = LEVELS.iter().any(|&(l, shown)| l == level && shown);
        let pos = call.pos();
        let message = self.expr(args[0])?;
        if !shown {
            let mut out = vec![Stmt::Expr(message)];
            if throwable {
                out.push(Stmt::Expr(self.expr(args[1])?));
            }
            return Ok(Some(out));
        }
        let prefix = self.synth(ExprKind::Str(format!("{level}: ")), pos, Type::String);
        let text = ExprKind::Binary(BinOp::Add, Box::new(prefix), Box::new(self.as_text(message)));
        let text = self.synth(text, pos, Type::String);
        let print = ExprKind::Builtin(Builtin::Native(Native::ErrPrint { newline: true }), vec![text]);
        let mut out = vec![Stmt::Expr(self.synth(print, pos, Type::Void))];
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
        self.logger_receiver(node)?;
        Ok(Some(ExprKind::Bool(answer)))
    }

    /// Checks that the receiver of the call `node` of a `PlatformLogger` method is a variable that
    /// `logger_variable` left out, or `getLogger(name)`.
    fn logger_receiver(&self, node: Node<'t>) -> Result<(), Error> {
        let receiver = node.child("methodSelect")?.get("expression");
        let local = receiver.is_some_and(|r| {
            r.kind() == "IDENTIFIER" && r.symbol().is_some_and(|s| s.kind == "LOCAL_VARIABLE")
        });
        if local || receiver.is_some_and(|r| self.get_logger(r).unwrap_or(false)) {
            return Ok(());
        }
        unsupported(node, "a `PlatformLogger` other than `PlatformLogger.getLogger(name)`")
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
        let symbol =
            serde_json::json!({"kind": "METHOD", "name": name, "owner": owner, "static": false, "type": sig});
        let print = serde_json::json!({"kind": "METHOD_INVOCATION", "line": line, "column": column,
            "javaType": "void", "arguments": [],
            "methodSelect": {"kind": "MEMBER_SELECT", "line": line, "column": column, "identifier": name,
                "expression": throwable.0.clone(), "symbol": symbol},
            "symbol": symbol});
        let print: &'t serde_json::Value = Box::leak(Box::new(print));
        self.expr(Node(print))
    }
}
