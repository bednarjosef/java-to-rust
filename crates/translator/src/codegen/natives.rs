//! Calls into the native layer (`ast::Native`): Rust's own float methods where they give Java's
//! results, and the runtime's `natives`, `math`, `fdlibm` and `out` otherwise.

use super::*;

impl<'a> Gen<'a> {
    pub(super) fn native(&mut self, native: Native, args: &'a [Expr], ty: Type, id: ExprId) -> Code {
        let call = |g: &mut Self, f: &str, args: Vec<String>| {
            Code::atom(format!("{}({})", g.runtime(f), args.join(", ")))
        };
        match native {
            Native::Math { name, strict } => self.math(name, strict, args, ty, id),
            Native::ArrayCopy => self.array_copy(args, id),
            Native::DoubleToLongBits => {
                let x = self.value(&args[0], Type::Double).text;
                call(self, "double_to_long_bits", vec![x])
            }
            Native::DoubleToRawLongBits | Native::FloatToRawIntBits => {
                let (from, to) = if native == Native::DoubleToRawLongBits {
                    (Type::Double, "i64")
                } else {
                    (Type::Float, "i32")
                };
                let x = self.value(&args[0], from);
                Code::new(format!("{}.to_bits() as {to}", self.recv(&x, from)), AS)
            }
            Native::LongBitsToDouble | Native::IntBitsToFloat => {
                let (from, unsigned, to) = if native == Native::LongBitsToDouble {
                    (Type::Long, "u64", "f64")
                } else {
                    (Type::Int, "u32", "f32")
                };
                let bits = self.value(&args[0], from);
                Code::atom(format!("{to}::from_bits({} as {unsigned})", bits.cast_operand()))
            }
            Native::FloatToIntBits => {
                let x = self.value(&args[0], Type::Float).text;
                call(self, "float_to_int_bits", vec![x])
            }
            // A class's identity is its own, as its hash is (`Class::hash_code`); a null one's is 0.
            Native::SystemIdentityHash if args[0].ty == Type::JavaClass => {
                let class = self.expr(&args[0]).at(ATOM);
                if self.nullable(&args[0]) {
                    Code::atom(format!("{class}.map_or(0, |c| c.hash_code())"))
                } else {
                    Code::atom(format!("{class}.hash_code()"))
                }
            }
            // A null object's is 0 (an enum's `enum_identity_hash` answers that itself).
            Native::SystemIdentityHash if self.enum_of(&args[0]).is_none() && self.nullable(&args[0]) => {
                let option = format!("{}.as_ref()", self.object_place(&args[0]));
                let hash = match self.struct_class(&args[0]) {
                    Some(c) => format!("{}({})", self.runtime("address_hash"), self.address(c, "h")),
                    None => format!("{}(h)", self.runtime("identity_hash")),
                };
                Code::atom(format!("{option}.map_or(0, |h| {hash})"))
            }
            Native::IdentityHash | Native::SystemIdentityHash => {
                // The address of the class's struct, which `Object.hashCode` hashes too.
                if self.enum_of(&args[0]).is_none()
                    && let Some(c) = self.struct_class(&args[0])
                {
                    let address = self.receiver_address(&args[0], c);
                    return call(self, "address_hash", vec![address]);
                }
                if let Some(t) = self.enum_of(&args[0]) {
                    return self.enum_identity_hash(t, &args[0]);
                }
                let object = match &args[0].kind {
                    ExprKind::Var(name) => format!("&{}", self.local(name)),
                    // `this` is already a reference to the `Rc`.
                    ExprKind::This => self.this_name().to_string(),
                    _ => format!("&{}", self.expr(&args[0]).at(UNARY)),
                };
                call(self, "identity_hash", vec![object])
            }
            Native::CurrentTimeMillis => call(self, "current_time_millis", vec![]),
            Native::NanoTime => call(self, "nano_time", vec![]),
            Native::AvailableProcessors => call(self, "available_processors", vec![]),
            Native::LineSeparator => call(self, "line_separator", vec![]),
            Native::OutWrite => {
                let b = self.value(&args[0], Type::Int).text;
                call(self, "write_byte", vec![b])
            }
            Native::OutFlush => call(self, "flush", vec![]),
            Native::ErrFlush => call(self, "err_flush", vec![]),
            Native::StandardWrite
            | Native::StandardWriteBytes
            | Native::StandardFlush
            | Native::StandardClose => self.standard_native(native, args),
            Native::ErrPrint { newline } => {
                let mac = self.runtime(if newline { "eprintln!" } else { "eprint!" });
                if args.is_empty() {
                    return Code::atom(format!("{mac}()"));
                }
                let mut parts = Vec::new();
                self.parts(&args[0], &mut parts);
                self.format(&mac, parts)
            }
            Native::PrintChars { newline } => {
                let chars = self.slice_of(&args[0]);
                call(self, if newline { "println_chars" } else { "print_chars" }, vec![chars])
            }
            Native::ErrPrintChars { newline } => {
                let chars = self.slice_of(&args[0]);
                call(self, if newline { "eprintln_chars" } else { "eprint_chars" }, vec![chars])
            }
            // `java.home` falls back to the translator's JDK, its argument.
            Native::SystemProperty("java.home") => {
                let fallback = self.text_ref(&args[0]);
                call(self, "java_home", vec![fallback])
            }
            Native::SystemProperty(key) => call(self, "system_property", vec![rust_string(key)]),
            Native::Getenv => {
                let name = self.text_ref(&args[0]);
                call(self, "getenv", vec![name])
            }
            Native::CopyMemory { .. } => self.copy_memory(args),
            Native::AllocateNative => {
                let bytes = self.value(&args[0], Type::Long).text;
                call(self, "allocate_native", vec![bytes])
            }
            Native::ReallocateNative => {
                let address = self.value(&args[0], Type::Long).text;
                let bytes = self.value(&args[1], Type::Long).text;
                call(self, "reallocate_native", vec![address, bytes])
            }
            Native::PageSize => call(self, "page_size", vec![]),
            Native::FreeNative => {
                let address = self.value(&args[0], Type::Long).text;
                call(self, "free_native", vec![address])
            }
            Native::SetMemory => self.set_memory(args),
            Native::FullFence => {
                Code::atom("std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst)".to_string())
            }
            Native::TotalMemory => call(self, "total_memory", vec![]),
            Native::FreeMemory => call(self, "free_memory", vec![]),
            Native::MaxMemory => call(self, "max_memory", vec![]),
            Native::Gc => call(self, "gc", vec![]),
            Native::Halt => {
                let status = self.value(&args[0], Type::Int).text;
                call(self, "halt", vec![status])
            }
            Native::Sleep { nanos: false } => {
                let millis = self.value(&args[0], Type::Long).text;
                self.may_fail(id, "sleep", "", &millis)
            }
            Native::Sleep { nanos: true } => {
                let millis = self.value(&args[0], Type::Long).text;
                let nanos = self.value(&args[1], Type::Int).text;
                self.may_fail(id, "sleep_nanos", "", &format!("{millis}, {nanos}"))
            }
            Native::OnSpinWait => call(self, "on_spin_wait", vec![]),
            Native::ThreadYield => call(self, "thread_yield", vec![]),
            Native::ThreadNumber => call(self, "next_thread_number", vec![]),
            Native::ThreadId => call(self, "next_thread_id", vec![]),
            Native::MainThreadId => call(self, "main_thread_id", vec![]),
            Native::HasCurrentThread => call(self, "has_current_thread", vec![]),
            Native::SetCurrentThread => {
                let thread = self.value(&args[0], args[0].ty).text;
                call(self, "set_current_thread", vec![thread])
            }
            Native::CurrentThread => {
                let ty = self.ty_name(ty);
                call(self, &format!("current_thread::<{ty}>"), vec![])
            }
            Native::ThreadStart => self.thread_start(args),
            Native::MonitorWait | Native::MonitorNotify { .. } | Native::HoldsLock => {
                self.monitor_native(native, args, id)
            }
            Native::ThreadAlive => {
                let tid = self.value(&args[0], Type::Long).text;
                call(self, "thread_alive", vec![tid])
            }
            Native::ThreadState => {
                let tid = self.value(&args[0], Type::Long).text;
                call(self, "thread_state", vec![tid])
            }
            Native::ThreadJoin => {
                let (tid, millis) =
                    (self.value(&args[0], Type::Long).text, self.value(&args[1], Type::Long).text);
                self.may_fail(id, "join_thread", "", &format!("{tid}, {millis}"))
            }
            Native::ThreadInterrupt => {
                let tid = self.value(&args[0], Type::Long).text;
                call(self, "interrupt_thread", vec![tid])
            }
            Native::ThreadIsInterrupted => {
                let tid = self.value(&args[0], Type::Long).text;
                call(self, "is_interrupted", vec![tid])
            }
            Native::ThreadClearInterrupt => call(self, "thread_interrupted", vec![]),
            Native::Park => {
                let (absolute, time) =
                    (self.value(&args[0], Type::Boolean).text, self.value(&args[1], Type::Long).text);
                call(self, "unsafe_park", vec![absolute, time])
            }
            Native::Unpark => {
                let tid = self.value(&args[0], Type::Long).text;
                call(self, "unpark", vec![tid])
            }
            Native::InstallUncaught => self.install_uncaught(args),
            Native::InstallTrace => self.install_trace(args),
            Native::UncaughtException => {
                let ty = self.ty_name(ty);
                call(self, &format!("pending_uncaught::<{ty}>"), vec![])
            }
            Native::PrintUncaught => {
                let name = self.thread_name(&args[0]);
                let e = self.value(&args[1], args[1].ty).text;
                let e =
                    if self.nullable(&args[1]) { format!("{}({e})", self.runtime("non_null")) } else { e };
                call(self, "print_uncaught", vec![format!("&{name}"), format!("&{e}")])
            }
            Native::ThreadRename => {
                let tid = self.value(&args[0], Type::Long).text;
                let name = format!("&{}", self.thread_name(&args[1]));
                call(self, "rename_thread", vec![tid, name])
            }
            Native::FileOpenRead
            | Native::FileOpenWrite
            | Native::FileRead
            | Native::FileReadBytes
            | Native::FileAvailable
            | Native::FileSkip
            | Native::FileClose
            | Native::FileWrite
            | Native::FileWriteBytes
            | Native::FileIsRegular
            | Native::FileLength
            | Native::FilePosition
            | Native::FileOpenRandom
            | Native::FileSeek
            | Native::FileSetLength => self.file_native(native, args, id),
            Native::FileSystem(name) => self.file_system_native(name, args, ty, id),
            Native::Unix(name) => self.unix_native(crate::ast::UNIX_CALLS, name, args, ty),
            Native::Net(name) => self.unix_native(crate::ast::NET_CALLS, name, args, ty),
            // The lowering drops its statement; anywhere else it is the unit it returns.
            Native::Nothing => Code::atom("()"),
            Native::Missing { what } => {
                Code::atom(format!("unreachable!(\"the native method {what} never runs\")"))
            }
            Native::CdsConfigStatus => call(self, "cds_config_status", vec![]),
            Native::CdsRandomSeed => call(self, "cds_random_seed_for_dumping", vec![]),
            Native::ContinuationsSupported => call(self, "continuations_supported", vec![]),
            Native::NanoTimeAdjustment => {
                let offset = self.value(&args[0], Type::Long).text;
                call(self, "nano_time_adjustment", vec![offset])
            }
            // The javaHome argument is dropped: it is a property read, and `time_zone.rs` does not use it.
            Native::SystemTimeZoneId => call(self, "system_time_zone_id", vec![]),
            Native::SystemGmtOffsetId => call(self, "system_gmt_offset_id", vec![]),
            Native::ArrayBaseOffset => call(self, "array_base_offset", vec![]),
            Native::ArrayIndexScale(element) => call(self, "array_index_scale", vec![rust_string(element)]),
            Native::DescriptorHandle | Native::DescriptorAppend => {
                let fd = self.value(&args[0], Type::Int).text;
                call(
                    self,
                    if native == Native::DescriptorHandle { "fd_handle" } else { "fd_append" },
                    vec![fd],
                )
            }
            Native::Crc32Update => {
                let ints = args.iter().map(|a| self.value(a, Type::Int).text).collect();
                call(self, "crc32_update", ints)
            }
            Native::Crc32UpdateBytes => {
                let crc = self.value(&args[0], Type::Int).text;
                let bytes = self.slice_of(&args[1]);
                let (off, len) = (self.value(&args[2], Type::Int).text, self.value(&args[3], Type::Int).text);
                call(self, "crc32_update_bytes", vec![crc, bytes, off, len])
            }
            Native::InflateInit
            | Native::InflateBytes
            | Native::InflateAdler
            | Native::InflateReset
            | Native::InflateEnd
            | Native::InflateSetDictionary => self.inflate_native(native, args, id),
            Native::NoPendingReferences => Code::atom("false"),
            Native::SecureSeed => {
                let num_bytes = self.value(&args[0], Type::Int).text;
                let call = Code::atom(format!("{}({num_bytes})", self.runtime("try_secure_seed")));
                if self.facts.raising.contains(&id) {
                    let checked = format!("{}.map_err({})", call.text, self.runtime("Failure::exception"));
                    Code::atom(self.passed_on(Code::atom(checked)))
                } else {
                    Code::atom(format!("{}({})", self.runtime("or_throw"), call.text))
                }
            }
        }
    }

    /// `Thread.start0(t, tid, name, daemon)` with the call `t.run()` that lowering added: the new
    /// thread gets its own handle of `t`, makes it its current thread and runs the call, outside
    /// any `try` of the code that starts it. An exception that escapes `run` ends that thread.
    fn thread_start(&mut self, args: &'a [Expr]) -> Code {
        let [thread, tid, name, daemon, run] = args else { unreachable!("start0 has the call of run") };
        let ExprKind::Var(var) = &thread.kind else { unreachable!("start0's thread is a parameter") };
        let handle = self.value(thread, thread.ty).text;
        let (tid, daemon) = (self.value(tid, Type::Long).text, self.value(daemon, Type::Boolean).text);
        let name = self.thread_name(name);
        let local = self.local(var);
        let set = self.runtime("set_current_thread");
        // The call is outside the `try`s of the starting code, and outside the `java::catching`
        // of one around the start.
        let attempts = std::mem::take(&mut self.attempts);
        let catching = std::mem::take(&mut self.catching_now);
        let throwing = std::mem::replace(&mut self.throwing, false);
        let body = self.expr(run).text;
        self.attempts = attempts;
        self.catching_now = catching;
        self.throwing = throwing;
        let start = self.runtime("start_thread");
        Code::atom(format!(
            "{start}({tid}, {name}, {daemon}, {{ let {local} = {handle}; move || {{ let {local} = &{local}; \
             {set}({local}.clone()); {body}; }} }})"
        ))
    }

    /// `Thread.installUncaught0()` with the call `Thread.dispatchUncaught()` that lowering added:
    /// the runtime gets two functions. The first makes the exception that ends a thread (a value
    /// of `Throwable`, or a failure of the runtime of a class the program has objects of) the
    /// pending one that `uncaught0()` returns, and runs the call, on that thread; the second names
    /// the class of an exception that the handler itself lets escape.
    fn install_uncaught(&mut self, args: &'a [Expr]) -> Code {
        let throwable = self.throwable_type();
        let attempts = std::mem::take(&mut self.attempts);
        let catching = std::mem::take(&mut self.catching_now);
        let throwing = std::mem::replace(&mut self.throwing, false);
        let body = self.expr(&args[0]).text;
        self.attempts = attempts;
        self.catching_now = catching;
        self.throwing = throwing;
        let raised: Vec<String> =
            self.facts.raised_classes.iter().map(|&c| rust_string(&self.class.types[c].name)).collect();
        let runtime = if raised.is_empty() {
            String::new()
        } else {
            format!(
                " else if let Some(e) = e.downcast_ref::<{}>().filter(|e| matches!(e.class, {})) \
                 {{ {throwable}::from(e.clone()) }}",
                self.runtime("Exception"),
                raised.join(" | ")
            )
        };
        let t = self.class.type_index("java.lang.Throwable").expect("the program has `Throwable`");
        let arms: Vec<String> = self
            .enum_variants(t)
            .into_iter()
            .map(|c| format!("{}(_) => {}", self.variant(t, c), rust_string(&self.java_class_name(c, false))))
            .collect();
        let (set, pending) = (self.runtime("set_uncaught_hook"), self.runtime("set_pending_uncaught"));
        Code::atom(format!(
            "{set}(|e: &dyn std::any::Any| {{ let uncaught: {throwable} = if let Some(e) = \
             e.downcast_ref::<{throwable}>() {{ e.clone() }}{runtime} else {{ return false; }}; \
             {pending}(uncaught); {body}; true }}, |e: &dyn std::any::Any| \
             e.downcast_ref::<{throwable}>().map(|e| String::from(match e {{ {} }})))",
            arms.join(", ")
        ))
    }

    /// `Native::InstallTrace` with its call of `Throwable.uncaughtTrace()`: the runtime gets a
    /// function that makes an uncaught exception of the program's `Throwable` the pending one,
    /// which `uncaught0()` returns, and gives the text of its stack trace.
    fn install_trace(&mut self, args: &'a [Expr]) -> Code {
        let throwable = self.throwable_type();
        let attempts = std::mem::take(&mut self.attempts);
        let throwing = std::mem::replace(&mut self.throwing, false);
        let text = self.value(&args[0], Type::String).text;
        self.attempts = attempts;
        self.throwing = throwing;
        let (set, pending) = (self.runtime("set_trace_hook"), self.runtime("set_pending_uncaught"));
        Code::atom(format!(
            "{set}(|e: &dyn std::any::Any| e.downcast_ref::<{throwable}>().map(|e| {{ \
             {pending}(e.clone()); {text} }}))"
        ))
    }

    /// A thread's name as a `String`, which `Thread` checks is not null.
    fn thread_name(&mut self, name: &'a Expr) -> String {
        let text = self.value(name, Type::String).text;
        if self.nullable(name) { format!("{}({text})", self.runtime("non_null")) } else { text }
    }

    /// `System.identityHashCode(e)` of a value of the enum of `t` (`Object`, an interface, a
    /// class with subclasses): the identity hash of the object in its variant, which is what
    /// `Object.hashCode` gives where no class overrides it; the hash of the value for a string or
    /// a box, which the translation holds by value; 0 for null.
    fn enum_identity_hash(&mut self, t: usize, e: &'a Expr) -> Code {
        let nullable = self.nullable(e);
        let value = if nullable { format!("{}.as_ref()", self.object_place(e)) } else { self.object_ref(e) };
        let mut arms = Vec::new();
        for c in self.enum_variants(t) {
            let variant = self.variant(t, c);
            let pattern = if nullable { format!("Some({variant}(o))") } else { format!("{variant}(o)") };
            arms.push(format!("{pattern} => {},", self.value_hash(c, "o")));
        }
        if nullable || arms.is_empty() {
            arms.push("_ => 0,".to_string());
        }
        Code::atom(format!("match {value} {{ {} }}", arms.join(" ")))
    }

    /// A `Math` or `StrictMath` function. `ty` is the type of the call, which picks the overload;
    /// `id` is the call's, for the checked form of a function that may throw.
    fn math(&mut self, name: &str, strict: bool, args: &'a [Expr], ty: Type, id: ExprId) -> Code {
        if let Some(code) = self.math_extra(name, args, ty) {
            return code;
        }
        if let Some(code) = self.ceil_division(name, args, ty, id) {
            return code;
        }
        // The operand type of the overloads: `round(float)` returns `int`, `floorMod(long, int)`
        // computes in `long`.
        let operand = match name {
            "round" if ty == Type::Int => Type::Float,
            "signum" | "ulp" | "nextUp" | "fma" => ty,
            "floorDiv" | "floorMod" if args.iter().any(|a| a.ty == Type::Long) => Type::Long,
            "floorDiv" | "floorMod" => Type::Int,
            _ => Type::Double,
        };
        let values: Vec<Code> = args.iter().map(|a| self.value(a, operand)).collect();
        let x = self.recv(&values[0], operand);
        let method = |m: &str| Code::atom(format!("{x}.{m}()"));
        let float = if operand == Type::Float { "_f32" } else { "" };
        let texts: Vec<String> = values.iter().map(|v| v.text.clone()).collect();
        match name {
            // Exact in Rust as in Java, and the same in `Math` and `StrictMath`.
            "floor" | "ceil" => method(name),
            "rint" => method("round_ties_even"),
            "toRadians" => method("to_radians"),
            "toDegrees" => method("to_degrees"),
            "fma" => Code::atom(format!("{x}.mul_add({}, {})", texts[1], texts[2])),
            "round" | "signum" | "ulp" | "nextUp" => {
                let f = match name {
                    "nextUp" => "next_up",
                    other => other,
                };
                Code::atom(format!("{}({})", self.runtime(&format!("{f}{float}")), texts[0]))
            }
            "floorDiv" | "floorMod" => {
                let f = if name == "floorDiv" { "floor_div" } else { "floor_mod" };
                let result = self.may_fail(id, f, "", &format!("{}, {}", texts[0], texts[1]));
                if ty == operand { result } else { self.convert(result, operand, ty) }
            }
            // HotSpot's own code for `Math`, which the platform's libm is closest to.
            "sin" | "cos" | "tan" | "exp" | "cbrt" | "sinh" | "tanh" if !strict => method(name),
            "log" if !strict => method("ln"),
            "pow" if !strict => Code::atom(format!("{x}.powf({})", texts[1])),
            "log10" if !strict => Code::atom(format!("{}({})", self.runtime("log10"), texts[0])),
            // fdlibm: all of `StrictMath`, and `Math` where HotSpot delegates to it.
            _ => {
                let f = if name == "IEEEremainder" { "remainder" } else { name };
                Code::atom(format!("{}({})", self.runtime(&format!("fdlibm::{f}")), texts.join(", ")))
            }
        }
    }

    /// `System.arraycopy(src, srcPos, dest, destPos, length)`. Shared arrays go through
    /// `java::arraycopy`, which also handles a copy within one array; plain arrays are copied
    /// between slices, or within one with `java::copy_within`. Where a `catch` or a `finally` sees
    /// the copy's failure (`Facts::raising`), the checked form of each.
    /// `Unsafe.copyMemory0(srcBase, srcOffset, destBase, destOffset, bytes)` (and with a sixth
    /// argument, the unit size, `copySwapMemory0`): the bases are `Object`s that hold arrays of
    /// primitives, matched by kind, whose bytes `java::copy_memory` copies, or null, where the
    /// offset is an address outside the heap (`java::copy_from_native` and the others). A base
    /// that is no such array ends the program.
    fn copy_memory(&mut self, args: &'a [Expr]) -> Code {
        let [src, src_offset, dest, dest_offset, bytes, rest @ ..] = args else {
            unreachable!("copyMemory0 has five arguments")
        };
        let (s, d) = (self.optional(src).text, self.optional(dest).text);
        let longs: Vec<String> = [src_offset, dest_offset, bytes]
            .into_iter()
            .chain(rest)
            .map(|e| self.value(e, Type::Long).text)
            .collect();
        let swap = longs.get(3).cloned().unwrap_or_else(|| "0".to_string());
        let [p, q, n, k] = ["src_offset", "dest_offset", "bytes", "unit"].map(|base| self.temp(base));
        let copy = self.runtime("copy_memory");
        let variants = self.primitive_array_variants();
        let mut arms = Vec::new();
        for a in &variants {
            for b in &variants {
                arms.push(format!(
                    "(Some({a}(src)), Some({b}(dest))) => {copy}(src, {p}, dest, {q}, {n}, {k}),"
                ));
            }
        }
        let (from, to, within) =
            (self.runtime("copy_from_native"), self.runtime("copy_to_native"), self.runtime("copy_native"));
        for v in &variants {
            arms.push(format!("(None, Some({v}(dest))) => {from}({p}, dest, {q}, {n}, {k}),"));
            arms.push(format!("(Some({v}(src)), None) => {to}(src, {p}, {q}, {n}, {k}),"));
        }
        arms.push(format!("(None, None) => {within}({p}, {q}, {n}, {k}),"));
        arms.push(format!(
            "_ => {}(\"an Unsafe copy of memory that is not between arrays of primitives\"),",
            self.runtime("unsupported_memory")
        ));
        Code::atom(format!(
            "{{ let ({p}, {q}, {n}, {k}) = ({}, {}, {}, {swap}); match (&{s}, &{d}) {{ {} }} }}",
            longs[0],
            longs[1],
            longs[2],
            arms.join(" ")
        ))
    }

    /// `Unsafe.setMemory0(base, offset, bytes, value)`: `bytes` bytes of `value` from `offset` in an
    /// `Object` that holds an array of primitives (`java::set_memory`), or with a null base from
    /// the address `offset` outside the heap (`java::set_native`).
    fn set_memory(&mut self, args: &'a [Expr]) -> Code {
        let base = self.optional(&args[0]).text;
        let [offset, bytes] = [&args[1], &args[2]].map(|e| self.value(e, Type::Long).text);
        let value = self.value(&args[3], Type::Byte).text;
        let [o, n, v] = ["offset", "bytes", "value"].map(|name| self.temp(name));
        let set = self.runtime("set_memory");
        let mut arms: Vec<String> = self
            .primitive_array_variants()
            .iter()
            .map(|variant| format!("Some({variant}(array)) => {set}(array, {o}, {n}, {v}),"))
            .collect();
        arms.push(format!("None => {}({o}, {n}, {v}),", self.runtime("set_native")));
        arms.push(format!(
            "_ => {}(\"an Unsafe fill of memory that is not in an array of primitives\"),",
            self.runtime("unsupported_memory")
        ));
        Code::atom(format!(
            "{{ let ({o}, {n}, {v}) = ({offset}, {bytes}, {value}); match &{base} {{ {} }} }}",
            arms.join(" ")
        ))
    }

    /// The variants of the `Object` enum that hold arrays of primitives.
    fn primitive_array_variants(&self) -> Vec<String> {
        let primitive = |ty: Type| {
            matches!(
                ty,
                Type::Array(e) if matches!(
                    *e,
                    Type::Int | Type::Long | Type::Double | Type::Float | Type::Byte | Type::Short | Type::Char | Type::Boolean
                )
            )
        };
        let Some(t) = self.object_index() else { return Vec::new() };
        let arrays =
            self.enum_variants(t).into_iter().filter(|&c| self.class.array_of(c).is_some_and(primitive));
        arrays.map(|c| self.variant(t, c)).collect()
    }

    /// `Builtin::RawMemory`: `Unsafe`'s read (or with `put`, write) of a `ty` at an offset of an
    /// `Object` that holds an array of primitives, matched by kind (`java::get_memory`).
    pub(super) fn raw_memory(&mut self, ty: Type, put: bool, args: &'a [Expr]) -> Code {
        let base = self.optional(&args[0]).text;
        let offset = self.value(&args[1], Type::Long).text;
        let rust = rust_type(ty);
        let [o, v] = ["offset", "value"].map(|name| self.temp(name));
        let (value, call) = match args.get(2) {
            Some(x) if put => {
                let value = self.value(x, ty).text;
                (
                    format!(", {value}"),
                    format!("{}::<{rust}, _, _>(array, {o}, {v})", self.runtime("put_memory")),
                )
            }
            _ => (String::new(), format!("{}::<{rust}, _, _>(array, {o})", self.runtime("get_memory"))),
        };
        let mut arms: Vec<String> = self
            .primitive_array_variants()
            .iter()
            .map(|variant| format!("Some({variant}(array)) => {call},"))
            .collect();
        // A null base: the offset is an address outside the heap.
        arms.push(if put {
            format!("None => {}::<{rust}>({o}, {v}),", self.runtime("put_native"))
        } else {
            format!("None => {}::<{rust}>({o}),", self.runtime("get_native"))
        });
        arms.push(format!(
            "_ => {}(\"an Unsafe access of memory that is not in an array of primitives\"),",
            self.runtime("unsupported_memory")
        ));
        let bind =
            if put { format!("let ({o}, {v}) = ({offset}{value});") } else { format!("let {o} = {offset};") };
        Code::atom(format!("{{ {bind} match &{base} {{ {} }} }}", arms.join(" ")))
    }

    fn array_copy(&mut self, args: &'a [Expr], id: ExprId) -> Code {
        let [src, src_pos, dest, dest_pos, length] = args else {
            unreachable!("arraycopy has five arguments")
        };
        if matches!(src.ty, Type::Class(name, _) if name == OBJECT) {
            return self.object_array_copy(args, id);
        }
        let plain_name = |g: &Self, e: &Expr| match &e.kind {
            ExprKind::Var(name) if g.plain(name).is_some() => Some(name.clone()),
            _ => None,
        };
        let (src_plain, dest_plain) = (plain_name(self, src), plain_name(self, dest));
        if src_plain.is_none() && dest_plain.is_none() {
            let (s, d) = (self.handle(src), self.handle(dest));
            let ints: Vec<String> =
                [src_pos, dest_pos, length].iter().map(|e| self.value(e, Type::Int).text).collect();
            let args = format!("{s}, {}, {d}, {}, {}", ints[0], ints[1], ints[2]);
            return self.may_fail(id, "arraycopy", "", &args);
        }
        // A `&mut` borrow of a plain destination conflicts with a later argument that reads it,
        // such as `a.length - 1`, so then the positions that are not constants are computed first,
        // in Java's order, as statements before the call (which, being `void`, is a statement).
        let names: Vec<&String> = dest_plain.iter().collect();
        let hoist = [src_pos, dest_pos, length].iter().any(|e| names.iter().any(|n| mentions(e, n)));
        let mut ints = Vec::new();
        for (e, base) in [(src_pos, "src_pos"), (dest_pos, "dest_pos"), (length, "length")] {
            let code = self.value(e, Type::Int);
            if hoist && code.literal.is_none() {
                let temp = self.temp(base);
                self.line(&format!("let {temp} = {};", code.text));
                ints.push(temp);
            } else {
                ints.push(code.text);
            }
        }
        match (&src_plain, &dest_plain) {
            (Some(s), Some(d)) if s == d => {
                let a = self.mut_ref(&self.local(d));
                self.may_fail(id, "copy_within", "", &format!("{a}, {}, {}, {}", ints[0], ints[1], ints[2]))
            }
            _ => {
                let s = self.slice_of(src);
                let d = match &dest_plain {
                    Some(d) => self.mut_ref(&self.local(d)),
                    None => format!("&mut {}.borrow_mut()", self.array_receiver(dest)),
                };
                self.may_fail(
                    id,
                    "copy_slice",
                    "",
                    &format!("{s}, {}, {d}, {}, {}", ints[0], ints[1], ints[2]),
                )
            }
        }
    }

    /// An array as a `&[T]` for reading: a plain array borrowed, a shared one through its cell.
    pub(super) fn slice_of(&mut self, e: &'a Expr) -> String {
        match &e.kind {
            ExprKind::Var(name) if self.plain(name).is_some() => self.borrowed_arg(e, false),
            _ => format!("&{}.borrow()", self.array_receiver(e)),
        }
    }

    /// A shared array as the receiver of `borrow()`: a variable as it is, without a clone. One
    /// that may be null throws `NullPointerException` there.
    pub(super) fn array_receiver(&mut self, e: &'a Expr) -> String {
        if self.nullable(e) {
            return self.handle(e);
        }
        match &e.kind {
            ExprKind::Var(name) => self.local(name),
            _ => self.expr(e).receiver(),
        }
    }
}
