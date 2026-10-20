//! Calls into the native layer: the JDK methods that bottom out in the JVM (`System.arraycopy`,
//! `Math` and `StrictMath`, the bit conversions of `Double` and `Float`, identity hashes, clocks,
//! `System.err` and the rest of `System.out`), recognised by the method's owner and name.

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
                if !matches!(args[0].ty, Type::Class(_)) {
                    return unsupported(node, "`System.identityHashCode` of anything but an object");
                }
                Native::IdentityHash
            }
            ("java.lang.Object", "hashCode") => {
                let object = match receiver {
                    Some(r) => self.expr(r)?,
                    None => self.this(node),
                };
                if !matches!(object.ty, Type::Class(_)) {
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
                if let Some(arg) = args.first() {
                    if matches!(arg.ty, Type::Array(_)) {
                        return unsupported(node, "printing an array to `System.err`");
                    }
                    self.check_text(node, arg.ty)?;
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
            _ => return Ok(None),
        };
        Ok(Some(Builtin::Native(native)))
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
