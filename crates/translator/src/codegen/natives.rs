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
            Native::ArrayCopy => self.array_copy(args),
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
            Native::IdentityHash => {
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
            Native::SystemProperty(key) => call(self, "system_property", vec![rust_string(key)]),
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
                call(self, "sleep", vec![millis])
            }
            Native::Sleep { nanos: true } => {
                let millis = self.value(&args[0], Type::Long).text;
                let nanos = self.value(&args[1], Type::Int).text;
                call(self, "sleep_nanos", vec![millis, nanos])
            }
            Native::OnSpinWait => call(self, "on_spin_wait", vec![]),
            Native::ThreadYield => call(self, "thread_yield", vec![]),
            Native::FileOpenRead
            | Native::FileOpenWrite
            | Native::FileRead
            | Native::FileReadBytes
            | Native::FileAvailable
            | Native::FileSkip
            | Native::FileClose
            | Native::FileWrite
            | Native::FileWriteBytes => self.file_native(native, args),
            // The lowering drops its statement; anywhere else it is the unit it returns.
            Native::Nothing => Code::atom("()"),
            Native::CdsConfigStatus => call(self, "cds_config_status", vec![]),
            Native::CdsRandomSeed => call(self, "cds_random_seed_for_dumping", vec![]),
            Native::ContinuationsSupported => call(self, "continuations_supported", vec![]),
            Native::NanoTimeAdjustment => {
                let offset = self.value(&args[0], Type::Long).text;
                call(self, "nano_time_adjustment", vec![offset])
            }
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
        }
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
    /// between slices, or within one with `java::copy_within`.
    fn array_copy(&mut self, args: &'a [Expr]) -> Code {
        let [src, src_pos, dest, dest_pos, length] = args else {
            unreachable!("arraycopy has five arguments")
        };
        let plain_name = |g: &Self, e: &Expr| match &e.kind {
            ExprKind::Var(name) if g.plain(name).is_some() => Some(name.clone()),
            _ => None,
        };
        let (src_plain, dest_plain) = (plain_name(self, src), plain_name(self, dest));
        if src_plain.is_none() && dest_plain.is_none() {
            let (s, d) = (self.handle(src), self.handle(dest));
            let ints: Vec<String> =
                [src_pos, dest_pos, length].iter().map(|e| self.value(e, Type::Int).text).collect();
            let f = self.runtime("arraycopy");
            return Code::atom(format!("{f}({s}, {}, {d}, {}, {})", ints[0], ints[1], ints[2]));
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
        let call = match (&src_plain, &dest_plain) {
            (Some(s), Some(d)) if s == d => {
                let a = self.mut_ref(&self.local(d));
                format!("{}({a}, {}, {}, {})", self.runtime("copy_within"), ints[0], ints[1], ints[2])
            }
            _ => {
                let s = self.slice_of(src);
                let d = match &dest_plain {
                    Some(d) => self.mut_ref(&self.local(d)),
                    None => format!("&mut {}.borrow_mut()", self.array_receiver(dest)),
                };
                format!("{}({s}, {}, {d}, {}, {})", self.runtime("copy_slice"), ints[0], ints[1], ints[2])
            }
        };
        Code::atom(call)
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
