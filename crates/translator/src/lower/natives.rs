//! Calls into the native layer: the JDK methods that bottom out in the JVM (`System.arraycopy`,
//! `Math` and `StrictMath`, the bit conversions of `Double` and `Float`, identity hashes, clocks,
//! `System.err` and the rest of `System.out`), recognised by the method's owner and name, and the
//! natives of translated JDK classes (`declared_native`). A native method's declaration is never
//! lowered; a call of one the runtime does not implement is rejected with its name.

use super::{Lowering, Node, Symbol, error, unsupported};
use crate::ast::*;

impl<'t> Lowering<'t> {
    /// The builtin for a call of a native-layer method, `None` for any other method. `args` are
    /// the lowered arguments; an instance method's receiver is put first.
    pub(super) fn native_call(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
        receiver: Option<Node<'t>>,
        args: &mut Vec<Expr>,
    ) -> Result<Option<Builtin>, Error> {
        // `System.out` or `System.err` as the receiver of a `PrintStream` method.
        let stream = receiver
            .and_then(|r| r.symbol())
            .filter(|s| s.kind == "FIELD" && s.owner == "java.lang.System")
            .map(|s| s.name);
        // A `Runtime` with no state of its own: the receiver is `Runtime.getRuntime()` or a name
        // that holds one, and neither needs evaluating.
        let on_runtime = receiver
            .is_some_and(|r| r.kind() == "IDENTIFIER" || r.symbol().is_some_and(|s| s.name == "getRuntime"));
        let native = match (symbol.owner, symbol.name) {
            // `StrictMath` gives the same results as `Math` for these.
            ("java.lang.StrictMath", "abs") => return Ok(Some(Builtin::MathAbs)),
            ("java.lang.StrictMath", "max") => return Ok(Some(Builtin::MathMax)),
            ("java.lang.StrictMath", "min") => return Ok(Some(Builtin::MathMin)),
            ("java.lang.StrictMath", "sqrt") => return Ok(Some(Builtin::MathSqrt)),
            ("java.lang.Math", "abs" | "max" | "min" | "sqrt" | "pow") => return Ok(None),
            (owner @ ("java.lang.Math" | "java.lang.StrictMath"), name) => {
                let Some(name) = MATH_FUNCTIONS.iter().find(|f| **f == name) else {
                    return unsupported(node, &format!("method `{owner}.{name}`"));
                };
                Native::Math { name, strict: owner == "java.lang.StrictMath" }
            }
            ("Array", "clone") => return self.array_clone(node, receiver, args),
            ("java.lang.System", "arraycopy") => {
                let (src, dest) = (args[0].ty, args[2].ty);
                // An `Object` that holds an array, as the JDK's code generic over the kind of array
                // passes one (`SpinedBuffer.OfPrimitive`'s `T_ARR`): both are then `Object`s, and
                // the generator matches their arrays' types.
                if (Self::is_object(src) || Self::is_object(dest))
                    && [src, dest].iter().all(|&ty| Self::is_object(ty) || matches!(ty, Type::Array(_)))
                {
                    let all = std::mem::take(args).into_iter().enumerate();
                    *args = all
                        .map(|(i, a)| if i == 0 || i == 2 { self.convert(a, Type::class(OBJECT)) } else { a })
                        .collect();
                    return Ok(Some(Builtin::Native(Native::ArrayCopy)));
                }
                if !matches!(src, Type::Array(_)) || src != dest {
                    return unsupported(node, "`System.arraycopy` between arrays of different types");
                }
                Native::ArrayCopy
            }
            ("java.lang.Double", "doubleToLongBits") => Native::DoubleToLongBits,
            ("java.lang.Double", "doubleToRawLongBits") => Native::DoubleToRawLongBits,
            ("java.lang.Double", "longBitsToDouble") => Native::LongBitsToDouble,
            ("java.lang.Float", "floatToIntBits") => Native::FloatToIntBits,
            ("java.lang.Float", "floatToRawIntBits") => Native::FloatToRawIntBits,
            ("java.lang.Float", "intBitsToFloat") => Native::IntBitsToFloat,
            ("java.lang.System", "identityHashCode") => {
                if !matches!(args[0].ty, Type::Class(..)) {
                    return unsupported(node, "`System.identityHashCode` of anything but an object");
                }
                Native::SystemIdentityHash
            }
            ("java.lang.Object", "hashCode") => {
                let object = match receiver {
                    Some(r) => self.expr(r)?,
                    None => self.this(node),
                };
                if !matches!(object.ty, Type::Class(..)) {
                    return unsupported(node, "`hashCode` of anything but an object");
                }
                args.insert(0, object);
                Native::IdentityHash
            }
            ("java.lang.System", "currentTimeMillis") => Native::CurrentTimeMillis,
            ("java.lang.System", "nanoTime") => Native::NanoTime,
            // The seed generator of `SecureRandom` is the operating system's; the JDK's provider
            // stack behind it is not translated (`UNTRANSLATED`).
            ("java.security.SecureRandom", "getSeed") if symbol.ty == "(int)byte[]" => Native::SecureSeed,
            ("java.lang.System", "lineSeparator") => Native::LineSeparator,
            ("java.lang.System", "getenv") => {
                return error(
                    node.pos(),
                    "`System.getenv` is not supported: the environment differs from run to run, so \
                     the translation could not be checked against Java"
                        .to_string(),
                );
            }
            ("java.lang.Runtime", "availableProcessors") if on_runtime => Native::AvailableProcessors,
            ("java.lang.Runtime", "totalMemory") if on_runtime => Native::TotalMemory,
            ("java.lang.Runtime", "freeMemory") if on_runtime => Native::FreeMemory,
            ("java.lang.Runtime", "maxMemory") if on_runtime => Native::MaxMemory,
            ("java.lang.Runtime", "gc") if on_runtime => Native::Gc,
            ("java.lang.Runtime", "halt") if on_runtime => Native::Halt,
            ("java.lang.Thread", "sleep") if matches!(args.first(), Some(a) if matches!(a.ty, Type::Long | Type::Int)) => {
                Native::Sleep { nanos: args.len() == 2 }
            }
            ("java.lang.Thread", "onSpinWait") => Native::OnSpinWait,
            ("java.lang.Thread", "holdsLock") => {
                self.check_lock(node, &args[0])?;
                Native::HoldsLock
            }
            ("java.lang.Thread", "yield") => Native::ThreadYield,
            // `LockSupport`'s, on the runtime's permit of a thread (`java_runtime::park`).
            ("jdk.internal.misc.Unsafe", "park") if symbol.ty == "(boolean,long)void" => Native::Park,
            ("jdk.internal.misc.Unsafe", "unpark") if symbol.ty == "(java.lang.Object)void" => {
                args[0] = self.thread_id(node)?;
                Native::Unpark
            }
            ("java.io.PrintStream", name @ ("print" | "println"))
                if stream == Some("err") && symbol.ty == "(char[])void" =>
            {
                Native::ErrPrintChars { newline: name == "println" }
            }
            ("java.io.PrintStream", name @ ("print" | "println")) if stream == Some("err") => {
                if let Some(arg) = args.first()
                    && matches!(arg.ty, Type::Array(_))
                {
                    return unsupported(node, "printing an array to `System.err`");
                }
                Native::ErrPrint { newline: name == "println" }
            }
            ("java.io.PrintStream", name @ ("print" | "println"))
                if stream == Some("out") && symbol.ty == "(char[])void" =>
            {
                Native::PrintChars { newline: name == "println" }
            }
            ("java.io.PrintStream", "write") if stream == Some("out") && symbol.ty == "(int)void" => {
                Native::OutWrite
            }
            ("java.io.PrintStream", "flush") if stream == Some("err") => Native::ErrFlush,
            ("java.io.PrintStream", "flush") if stream.is_some() => Native::OutFlush,
            // A native of a translated class: the runtime's, or a rejection that names it. (A
            // native of the JDK that the translator's own class has in Java, as `Thread` has
            // `currentThread`, is a method like another.)
            (owner, name)
                if symbol.native
                    && self.type_index(owner).is_some()
                    && self.method_index(symbol).is_none() =>
            {
                match declared_native(owner, name, symbol.ty) {
                    Some(Native::ThreadStart) => {
                        self.thread_body(node, args);
                        Native::ThreadStart
                    }
                    Some(Native::InstallUncaught) => {
                        self.uncaught_dispatch(node, args);
                        Native::InstallUncaught
                    }
                    Some(native) => native,
                    None => {
                        return unsupported(node, &format!("native method `{owner}.{name}{}`", symbol.ty));
                    }
                }
            }
            _ => return Ok(None),
        };
        Ok(Some(Builtin::Native(native)))
    }

    /// `System.getLogger(name).log(System.Logger.Level.L, message)`, as `LocaleServiceProviderPool`
    /// reports a provider that returned null. The JDK's default logger, without `java.logging`,
    /// prints the messages of level `INFO` and above to `System.err`, after a line with the time
    /// and the caller; the translation prints `L: message`, without that line, which a translated
    /// program has no stack to find the caller in. `None` for any other call; it runs before the
    /// arguments are lowered, as the level is a constant of `System`'s nested enum, which is the
    /// runtime's.
    pub(super) fn system_log(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
        select: Node<'t>,
    ) -> Result<Option<ExprKind>, Error> {
        if (symbol.owner, symbol.name, symbol.ty)
            != ("java.lang.System.Logger", "log", "(java.lang.System.Logger.Level,java.lang.String)void")
        {
            return Ok(None);
        }
        let logger = select.get("expression").filter(|_| select.kind() == "MEMBER_SELECT");
        let get_logger = logger.filter(|l| {
            l.kind() == "METHOD_INVOCATION"
                && l.symbol().is_some_and(|s| (s.owner, s.name) == ("java.lang.System", "getLogger"))
        });
        let Some(get_logger) = get_logger else {
            return unsupported(node, "a `System.Logger` other than `System.getLogger(name)`");
        };
        let mut arguments = node.list("arguments");
        let (Some(level), Some(message)) = (arguments.next(), arguments.next()) else {
            return unsupported(node, "this `System.Logger.log`");
        };
        let level = (level.kind() == "MEMBER_SELECT").then(|| level.str("identifier"));
        let shown = match level {
            Some("INFO" | "WARNING" | "ERROR") => true,
            Some("ALL" | "TRACE" | "DEBUG" | "OFF") => false,
            _ => return unsupported(node, "`System.Logger.log` of a level that is not a constant"),
        };
        let pos = node.pos();
        // The logger's name is evaluated for nothing but its effects, which the name of a class
        // literal has none of.
        let name = get_logger.list("arguments").next().expect("getLogger has a name");
        let of_literal = name.kind() == "METHOD_INVOCATION"
            && name
                .child("methodSelect")
                .ok()
                .and_then(|s| s.get("expression"))
                .is_some_and(super::classes::is_literal);
        if !of_literal && has_effects(&self.expr(name)?) {
            return unsupported(node, "a `System.getLogger` name that does more than name the logger");
        }
        let message = self.expr(message)?;
        let level = level.expect("a constant level");
        if !shown {
            return Ok(Some(message.kind));
        }
        let prefix = self.synth(ExprKind::Str(format!("{level}: ")), pos, Type::String);
        let text = ExprKind::Binary(BinOp::Add, Box::new(prefix), Box::new(self.as_text(message)));
        let text = self.synth(text, pos, Type::String);
        Ok(Some(ExprKind::Builtin(Builtin::Native(Native::ErrPrint { newline: true }), vec![text])))
    }

    /// The id of the thread that `Unsafe.unpark(thread)` (`node`) passes, of the translator's
    /// `Thread`: its field `tid`. A variable is one that `LockSupport.unpark` has tested; a field
    /// read (`v.owner`) may be null, which `unpark` ignores, as it does an id of no thread: there
    /// the id is `t != null ? t.tid : -1`, the read made twice, which has no effects.
    fn thread_id(&self, node: Node<'t>) -> Result<Expr, Error> {
        let arg = node.list("arguments").next();
        let path = |mut n: Node<'t>| loop {
            match n.kind() {
                "IDENTIFIER" => return true,
                "MEMBER_SELECT" if n.symbol().is_some_and(|s| s.kind == "FIELD") => match n.get("expression")
                {
                    Some(e) => n = e,
                    None => return false,
                },
                _ => return false,
            }
        };
        let Some(arg) = arg.filter(|a| path(*a)) else {
            return unsupported(node, "`Unsafe.unpark` of anything but a variable or a field");
        };
        let thread = self.expr(arg)?;
        let class = self.type_index("java.lang.Thread");
        let of_thread = match thread.ty {
            Type::Class(name, _) => self
                .type_index(name)
                .is_some_and(|t| Some(t) == class || class.is_some_and(|c| self.is_subclass(t, c))),
            _ => false,
        };
        if !of_thread {
            return unsupported(node, "`Unsafe.unpark` of a value that is not a `Thread`");
        }
        let pos = node.pos();
        let tid = super::objects::read(Place::Field { object: Box::new(thread), name: "tid".to_string() });
        let tid = self.synth(tid, pos, Type::Long);
        if arg.kind() == "IDENTIFIER" {
            return Ok(tid);
        }
        let again = self.expr(arg)?;
        let null = self.synth(ExprKind::Null, pos, Type::Null);
        let test =
            self.synth(ExprKind::Binary(BinOp::Ne, Box::new(again), Box::new(null)), pos, Type::Boolean);
        let none = self.synth(ExprKind::Long(-1), pos, Type::Long);
        Ok(self.synth(ExprKind::Conditional(Box::new(test), Box::new(tid), Box::new(none)), pos, Type::Long))
    }

    /// A call whose argument is a class literal, which the translation has no value for, of a
    /// method that needs only the class it names: `Unsafe.arrayBaseOffset(int[].class)` and
    /// `arrayIndexScale`, whose null check cannot fail for a literal and which reachability
    /// does not enter (`REPLACED_METHODS`), and the native `CDS.initializeFromArchive(C.class)`,
    /// which does nothing. `None` for any other call; it runs before the arguments are lowered.
    pub(super) fn class_literal_call(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
        select: Node<'t>,
    ) -> Result<Option<ExprKind>, Error> {
        let method = match (symbol.owner, symbol.name, symbol.ty) {
            ("jdk.internal.misc.CDS", "initializeFromArchive", _) => "CDS.initializeFromArchive",
            ("jdk.internal.misc.Unsafe", "arrayBaseOffset", "(java.lang.Class<?>)long") => {
                "Unsafe.arrayBaseOffset"
            }
            ("jdk.internal.misc.Unsafe", "arrayIndexScale", "(java.lang.Class<?>)int") => {
                "Unsafe.arrayIndexScale"
            }
            _ => return Ok(None),
        };
        // A class literal, or a variable of an array class's type, which can hold nothing else
        // (`ForkJoinPool` keeps `ForkJoinTask[].class` in a local).
        let class = node.list("arguments").next().and_then(|a| {
            if a.kind() == "MEMBER_SELECT" && a.str("identifier") == "class" {
                return Some(a.get("expression")?.java_type());
            }
            let ty = a.java_type().strip_prefix("java.lang.Class<")?.strip_suffix('>')?;
            (a.kind() == "IDENTIFIER" && ty.ends_with("[]")).then_some(ty)
        });
        let Some(class) = class else {
            return unsupported(node, &format!("`{method}` of anything but a class literal"));
        };
        if symbol.name == "initializeFromArchive" {
            return Ok(Some(ExprKind::Builtin(Builtin::Native(Native::Nothing), Vec::new())));
        }
        // `Unsafe` has no state: its receiver, a name or `Unsafe.getUnsafe()`, needs no evaluating.
        let receiver = select.get("expression").filter(|_| select.kind() == "MEMBER_SELECT");
        let stateless = receiver.is_none_or(|r| match r.symbol() {
            Some(s) => s.kind == "FIELD" || s.name == "getUnsafe",
            None => false,
        });
        if !stateless {
            return unsupported(node, &format!("`{method}` on this receiver"));
        }
        let Some(element) = class.strip_suffix("[]") else {
            return unsupported(node, &format!("`{method}` of a class that is not an array"));
        };
        let native = if symbol.name == "arrayBaseOffset" {
            Native::ArrayBaseOffset
        } else {
            let primitive = ["boolean", "byte", "short", "char", "int", "long", "float", "double"];
            Native::ArrayIndexScale(
                primitive.into_iter().find(|p| *p == element).unwrap_or("java.lang.Object"),
            )
        };
        Ok(Some(ExprKind::Builtin(Builtin::Native(native), Vec::new())))
    }

    /// What the thread that `Thread.start0(t, ..)` starts runs: `t.run()`, added to the arguments
    /// as the last, for the code that starts the thread to run on it (`ThreadStart`). Calling it
    /// here, in the program's terms, lets every analysis see the call of `run` and the overrides
    /// it may run.
    fn thread_body(&self, node: Node<'t>, args: &mut Vec<Expr>) {
        let ExprKind::Var(name) = &args[0].kind else { unreachable!("`startThread` passes its parameter") };
        let run = self.methods.iter().position(|m| {
            self.types[m.owner].javac == "java.lang.Thread" && m.name == "run" && m.sig == "()void"
        });
        let receiver = Box::new(self.synth(ExprKind::Var(name.clone()), args[0].pos, args[0].ty));
        let call = ExprKind::MethodCall {
            receiver,
            name: "run".to_string(),
            args: Vec::new(),
            resolved: run,
            is_super: false,
        };
        args.push(self.synth(call, node.pos(), Type::Void));
    }

    /// What the runtime runs, once `Thread.installUncaught0()` has run, on a thread that an
    /// exception nothing catches ends: `Thread.dispatchUncaught()`, added as the argument of the
    /// native (`InstallUncaught`) so that every analysis sees the call and what it reaches.
    fn uncaught_dispatch(&self, node: Node<'t>, args: &mut Vec<Expr>) {
        let dispatch = self.methods.iter().position(|m| {
            self.types[m.owner].javac == "java.lang.Thread"
                && m.name == "dispatchUncaught"
                && m.sig == "()void"
        });
        let call =
            ExprKind::Call { name: "dispatchUncaught".to_string(), args: Vec::new(), resolved: dispatch };
        args.push(self.synth(call, node.pos(), Type::Void));
    }

    /// `Blocker.begin()`, `begin(blocking)` and `end(attempted)`, which the JDK's file streams and
    /// channels call around each operation that may block, so that a virtual thread may give
    /// its carrier thread to another: a translated program has no virtual threads, so `begin`
    /// is `false` and `end(false)` does nothing, and `Blocker`'s initialization, which needs the
    /// JVM's `JavaLangAccess`, never runs (`REPLACED_METHODS`). An argument is evaluated, then
    /// the value is given.
    pub(super) fn blocker_call(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
        args: &mut Vec<Expr>,
    ) -> Result<Option<ExprKind>, Error> {
        if symbol.owner != "jdk.internal.misc.Blocker" {
            return Ok(None);
        }
        let pos = node.pos();
        let value = match (symbol.name, symbol.ty) {
            ("begin", "()boolean" | "(boolean)boolean") => {
                self.synth(ExprKind::Bool(false), pos, Type::Boolean)
            }
            ("end", "(boolean)void") => {
                self.synth(ExprKind::Builtin(Builtin::Native(Native::Nothing), Vec::new()), pos, Type::Void)
            }
            _ => return unsupported(node, &format!("`Blocker.{}`", symbol.name)),
        };
        let mut block: Vec<Stmt> = std::mem::take(args).into_iter().map(Stmt::Expr).collect();
        if block.is_empty() {
            return Ok(Some(value.kind));
        }
        block.push(if value.ty == Type::Void { Stmt::Expr(value) } else { Stmt::Yield(value) });
        Ok(Some(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body: block }],
        }))
    }

    /// A native of `java.io.UnixFileSystem` (`ast::FILE_SYSTEM`), whose `File` arguments become
    /// the paths they hold, the `path` field that `File.getPath()` returns. `None` for any other
    /// method; a native of the class that the runtime does not implement is rejected.
    ///
    /// Also `FileCleanable.register(fd)`, which each file stream calls once it is open so that
    /// the JDK's cleaner thread closes the descriptor after the stream is garbage collected: it
    /// does nothing (`REPLACED_METHODS`), as nothing is collected that way in a translated
    /// program. A stream that the program never closes stays open until the program ends. Its
    /// argument is the stream's `fd` field or a parameter, which reads nothing else.
    ///
    /// And a native of the translator's `UnixNativeDispatcher` that the runtime implements
    /// (`ast::UNIX_CALLS`), with its arguments as they are.
    pub(super) fn file_system_call(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
        args: &mut Vec<Expr>,
    ) -> Result<Option<ExprKind>, Error> {
        if (symbol.owner, symbol.name) == ("java.io.FileCleanable", "register") {
            if !matches!(&args[..], [arg] if matches!(arg.kind, ExprKind::Var(_) | ExprKind::Field { .. })) {
                return unsupported(node, "`FileCleanable.register` of anything but a variable or a field");
            }
            return Ok(Some(ExprKind::Builtin(Builtin::Native(Native::Nothing), Vec::new())));
        }
        if symbol.native
            && symbol.owner == crate::ast::UNIX_DISPATCHER
            && let Some(name) = crate::ast::unix_call(symbol.name)
        {
            return Ok(Some(ExprKind::Builtin(Builtin::Native(Native::Unix(name)), std::mem::take(args))));
        }
        if !symbol.native || symbol.owner != "java.io.UnixFileSystem" || symbol.name == "initIDs" {
            return Ok(None);
        }
        let Some(&name) = crate::ast::FILE_SYSTEM.iter().find(|&&n| n == symbol.name) else {
            return unsupported(
                node,
                &format!("native method `java.io.UnixFileSystem.{}{}`", symbol.name, symbol.ty),
            );
        };
        for arg in args.iter_mut() {
            if matches!(arg.ty, Type::Class(..)) {
                let pos = arg.pos;
                let file = Box::new(std::mem::replace(arg, self.synth(ExprKind::Null, pos, Type::String)));
                let path = Place::Field { object: file, name: "path".into() };
                *arg = self.synth(super::objects::read(path), pos, Type::String);
            }
        }
        Ok(Some(ExprKind::Builtin(Builtin::Native(Native::FileSystem(name)), std::mem::take(args))))
    }

    /// A native of `FileInputStream`, `FileOutputStream`, `RandomAccessFile` or `FileDescriptor`,
    /// which acts on the descriptor of its object: the number in the `fd` field of the stream's
    /// `FileDescriptor` (or of the `FileDescriptor` itself), which goes first among the arguments.
    /// `open0` stores the number it opens there, and `close0` sets it to -1 once it is closed, as
    /// the JVM's natives do. `None` for any other method.
    pub(super) fn descriptor_call(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
        receiver: Option<Node<'t>>,
        args: &mut Vec<Expr>,
    ) -> Result<Option<ExprKind>, Error> {
        if !symbol.native {
            return Ok(None);
        }
        let native = match (symbol.owner, symbol.name) {
            ("java.io.FileInputStream", "open0") => Native::FileOpenRead,
            ("java.io.FileInputStream", "read0") => Native::FileRead,
            ("java.io.FileInputStream", "readBytes") => Native::FileReadBytes,
            ("java.io.FileInputStream", "available0") => Native::FileAvailable,
            ("java.io.FileInputStream", "skip0") => Native::FileSkip,
            ("java.io.FileInputStream", "length0") => Native::FileLength,
            ("java.io.FileInputStream", "position0") => Native::FilePosition,
            // Its argument is the stream's own descriptor, which the stream's `fd` gives.
            ("java.io.FileInputStream", "isRegularFile0") => {
                args.clear();
                Native::FileIsRegular
            }
            ("java.io.FileDescriptor", "close0") => Native::FileClose,
            ("java.io.FileOutputStream", "open0") => Native::FileOpenWrite,
            // The `append` argument is not passed on: the runtime's descriptor remembers the mode.
            ("java.io.FileOutputStream", "write") => {
                args.truncate(1);
                Native::FileWrite
            }
            ("java.io.FileOutputStream", "writeBytes") => {
                args.truncate(3);
                Native::FileWriteBytes
            }
            ("java.io.RandomAccessFile", "open0") => Native::FileOpenRandom,
            ("java.io.RandomAccessFile", "read0") => Native::FileRead,
            ("java.io.RandomAccessFile", "readBytes0") => Native::FileReadBytes,
            ("java.io.RandomAccessFile", "write0") => Native::FileWrite,
            ("java.io.RandomAccessFile", "writeBytes0") => Native::FileWriteBytes,
            ("java.io.RandomAccessFile", "getFilePointer") => Native::FilePosition,
            ("java.io.RandomAccessFile", "seek0") => Native::FileSeek,
            ("java.io.RandomAccessFile", "length0") => Native::FileLength,
            ("java.io.RandomAccessFile", "setLength0") => Native::FileSetLength,
            _ => return Ok(None),
        };
        let pos = node.pos();
        // `getFilePointer` is public, so a program calls it on its own `RandomAccessFile`; the
        // others are private, and called on `this`.
        // Taken by the one read of the descriptor that a native on another object makes.
        let object = std::cell::Cell::new(match receiver {
            Some(r) if !(r.kind() == "IDENTIFIER" && r.str("name") == "this") => {
                if native != Native::FilePosition {
                    return unsupported(r, "a native of a stream on another object than `this`");
                }
                Some(self.expr(r)?)
            }
            _ => None,
        });
        let descriptor_ty = self.ty(node, "java.io.FileDescriptor")?;
        let descriptor = || {
            let object = object.take().unwrap_or_else(|| self.this(node));
            if symbol.owner == "java.io.FileDescriptor" {
                return object;
            }
            let field = Place::Field { object: Box::new(object), name: "fd".into() };
            self.synth(super::objects::read(field), pos, descriptor_ty)
        };
        let number = || Place::Field { object: Box::new(descriptor()), name: "fd".into() };
        let call =
            |args: Vec<Expr>, ty: Type| self.synth(ExprKind::Builtin(Builtin::Native(native), args), pos, ty);
        Ok(Some(match native {
            Native::FileOpenRead | Native::FileOpenWrite | Native::FileOpenRandom => {
                let value = Box::new(call(std::mem::take(args), Type::Int));
                ExprKind::Assign { op: None, target: number(), value }
            }
            Native::FileClose => {
                let fd = self.synth(super::objects::read(number()), pos, Type::Int);
                let closed = self.synth(ExprKind::Int(-1), pos, Type::Int);
                let reset = ExprKind::Assign { op: None, target: number(), value: Box::new(closed) };
                let block = vec![
                    Stmt::Expr(call(vec![fd], Type::Void)),
                    Stmt::Expr(self.synth(reset, pos, Type::Int)),
                ];
                ExprKind::Switch {
                    selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
                    cases: vec![Case { labels: vec![CaseLabel::Default], body: block }],
                }
            }
            _ => {
                args.insert(0, self.synth(super::objects::read(number()), pos, Type::Int));
                ExprKind::Builtin(Builtin::Native(native), std::mem::take(args))
            }
        }))
    }
}
