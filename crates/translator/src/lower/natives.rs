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
                Native::IdentityHash
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
            ("java.lang.Thread", "yield") => Native::ThreadYield,
            ("java.io.FileInputStream", "open0") => Native::FileOpenRead,
            ("java.io.FileInputStream", "read0") => {
                self.stream_native(node, receiver, args, Native::FileRead)?
            }
            ("java.io.FileInputStream", "readBytes") => {
                self.stream_native(node, receiver, args, Native::FileReadBytes)?
            }
            ("java.io.FileInputStream", "available0") => {
                self.stream_native(node, receiver, args, Native::FileAvailable)?
            }
            ("java.io.FileInputStream", "skip0") => {
                self.stream_native(node, receiver, args, Native::FileSkip)?
            }
            ("java.io.FileInputStream" | "java.io.FileOutputStream" | "java.io.FileDescriptor", "close0") => {
                self.stream_native(node, receiver, args, Native::FileClose)?
            }
            ("java.io.FileOutputStream", "open0") => Native::FileOpenWrite,
            // The `append` argument is not passed on: the runtime's descriptor remembers the mode.
            ("java.io.FileOutputStream", "write") => {
                args.truncate(1);
                self.stream_native(node, receiver, args, Native::FileWrite)?
            }
            ("java.io.FileOutputStream", "writeBytes") => {
                args.truncate(3);
                self.stream_native(node, receiver, args, Native::FileWriteBytes)?
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
        let literal = node
            .list("arguments")
            .next()
            .filter(|a| a.kind() == "MEMBER_SELECT" && a.str("identifier") == "class");
        let Some(literal) = literal else {
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
        let Some(element) = literal.child("expression")?.java_type().strip_suffix("[]") else {
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

    /// A native of a stream or descriptor: the receiver goes first, as the descriptor the object
    /// model supplies. The lowering of that descriptor is the object model's, not this one's.
    fn stream_native(
        &self,
        node: Node<'t>,
        receiver: Option<Node<'t>>,
        args: &mut Vec<Expr>,
        native: Native,
    ) -> Result<Native, Error> {
        let stream = match receiver {
            Some(r) => self.expr(r)?,
            None => self.this(node),
        };
        args.insert(0, stream);
        Ok(native)
    }
}
