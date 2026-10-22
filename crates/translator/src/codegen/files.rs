//! The file natives (`ast::Native::File*`), which call the runtime's descriptor table
//! (`java_runtime::fd`), and those of `java.io.UnixFileSystem` (`Native::FileSystem`, on
//! `java_runtime::fs`). A call that can fail is wrapped in `or_throw`, which raises the Java
//! exception, or where the program may catch it, passed on as a thrown exception. Every
//! descriptor native but `open0` takes the stream's descriptor (`fd`) as its first argument.

use super::*;

impl<'a> Gen<'a> {
    pub(super) fn file_native(&mut self, native: Native, args: &'a [Expr], id: ExprId) -> Code {
        match native {
            Native::FileOpenRead => {
                let name = self.text_ref(&args[0]);
                self.fallible(id, "fd_open_read", vec![name])
            }
            Native::FileOpenWrite => {
                let name = self.text_ref(&args[0]);
                let append = self.value(&args[1], Type::Boolean).text;
                self.fallible(id, "fd_open_write", vec![name, append])
            }
            Native::FileOpenRandom => {
                let name = self.text_ref(&args[0]);
                let mode = self.value(&args[1], Type::Int).text;
                self.fallible(id, "fd_open_random", vec![name, mode])
            }
            Native::FileSeek | Native::FileSetLength => {
                let fd = self.value(&args[0], Type::Int).text;
                let n = self.value(&args[1], Type::Long).text;
                let f = if native == Native::FileSeek { "fd_seek" } else { "fd_set_length" };
                self.fallible(id, f, vec![fd, n])
            }
            Native::FileRead => {
                let fd = self.value(&args[0], Type::Int).text;
                self.fallible(id, "fd_read", vec![fd])
            }
            Native::FileReadBytes => {
                let [fd, off, len] = self.hoisted_ints(args, [0, 2, 3]);
                let buf = self.byte_array(&args[1], true);
                self.fallible(id, "fd_read_bytes", vec![fd, buf, off, len])
            }
            Native::FileAvailable => {
                let fd = self.value(&args[0], Type::Int).text;
                self.fallible(id, "fd_available", vec![fd])
            }
            Native::FileSkip => {
                let fd = self.value(&args[0], Type::Int).text;
                let n = self.value(&args[1], Type::Long).text;
                self.fallible(id, "fd_skip", vec![fd, n])
            }
            Native::FileClose => {
                let fd = self.value(&args[0], Type::Int).text;
                Code::atom(format!("{}({fd})", self.runtime("fd_close")))
            }
            Native::FileWrite => {
                let fd = self.value(&args[0], Type::Int).text;
                let b = self.value(&args[1], Type::Int).text;
                self.fallible(id, "fd_write", vec![fd, b])
            }
            Native::FileWriteBytes => {
                let [fd, off, len] = self.hoisted_ints(args, [0, 2, 3]);
                let buf = self.byte_array(&args[1], false);
                self.fallible(id, "fd_write_bytes", vec![fd, buf, off, len])
            }
            Native::FileIsRegular => {
                let fd = self.value(&args[0], Type::Int).text;
                Code::atom(format!("{}({fd})", self.runtime("fd_is_regular_file")))
            }
            Native::FileLength => {
                let fd = self.value(&args[0], Type::Int).text;
                self.fallible(id, "fd_length", vec![fd])
            }
            Native::FilePosition => {
                let fd = self.value(&args[0], Type::Int).text;
                self.fallible(id, "fd_position", vec![fd])
            }
            _ => unreachable!("not a file native: {native:?}"),
        }
    }

    /// A native of `java.io.UnixFileSystem` (`java_runtime::fs`), whose `File` arguments the
    /// lowering made their paths. `list0` gives `null` (an `Option`) for a directory it cannot
    /// read; `createFileExclusively0` and `canonicalize0` may throw.
    pub(super) fn file_system_native(&mut self, name: &str, args: &'a [Expr], ty: Type, id: ExprId) -> Code {
        let mut values = Vec::new();
        for arg in args {
            values.push(match arg.ty {
                Type::String => self.text_ref(arg),
                Type::Boolean => self.value(arg, Type::Boolean).text,
                Type::Long => self.value(arg, Type::Long).text,
                _ => self.value(arg, Type::Int).text,
            });
        }
        let f = match name {
            "getBooleanAttributes0" => "fs_attributes",
            "checkAccess0" => "fs_access",
            "getLastModifiedTime0" => "fs_modified",
            "getLength0" => "fs_length",
            "setPermission0" => "fs_set_permission",
            "createFileExclusively0" => return self.fallible(id, "fs_create_file", values),
            "canonicalize0" => return self.fallible(id, "fs_canonicalize", values),
            "delete0" => "fs_delete",
            "list0" => {
                let names = format!("{}({})", self.runtime("fs_list"), values.join(", "));
                let each = self.string_vec_as(ty, "names".to_string());
                let array = format!("{}({each})", self.array_maker("from_vec"));
                return Code::atom(format!("{names}.map(|names| {array})"));
            }
            "createDirectory0" => "fs_create_directory",
            "rename0" => "fs_rename",
            "setLastModifiedTime0" => "fs_set_modified",
            "setReadOnly0" => "fs_set_read_only",
            "getNameMax0" => "fs_name_max",
            _ => unreachable!("not a native of `UnixFileSystem` the runtime implements: {name}"),
        };
        Code::atom(format!("{}({})", self.runtime(f), values.join(", ")))
    }

    /// A native of the translator's `UnixNativeDispatcher` (`Native::Unix`) or `TranslatedNet`
    /// (`Native::Net`), as a call of the runtime's function that `calls` names (`ast::UNIX_CALLS`,
    /// `ast::NET_CALLS`). A path, a host name or an address is the runtime's `&[i8]`; the `long[]` that
    /// `stat0` and `fstat0` fill, and the buffer that `readBytes` reads into, are borrowed
    /// mutably. A `byte[]` that the function returns becomes a new array.
    pub(super) fn unix_native(
        &mut self,
        calls: &[(&str, &str)],
        name: &str,
        args: &'a [Expr],
        ty: Type,
    ) -> Code {
        let f = calls.iter().find(|(n, _)| *n == name).map(|(_, f)| *f);
        let f = f.unwrap_or_else(|| unreachable!("not a native the runtime implements: {name}"));
        let mut values = Vec::new();
        for (i, arg) in args.iter().enumerate() {
            values.push(match arg.ty {
                Type::Array(_) => {
                    let written = arg.ty.element() == Type::Long || (name == "readBytes" && i == 1);
                    self.array_arg(arg, written)
                }
                Type::Boolean => self.value(arg, Type::Boolean).text,
                Type::Long => self.value(arg, Type::Long).text,
                _ => self.value(arg, Type::Int).text,
            });
        }
        let call = format!("{}({})", self.runtime(f), values.join(", "));
        if matches!(ty, Type::Array(_)) {
            let array = format!("{}({call})", self.array_maker("from_vec"));
            return Code::atom(array);
        }
        Code::atom(call)
    }

    /// The natives of the stream under `System.out` and `System.err` (`Native::StandardWrite`
    /// and the others), which the runtime's `out` implements; they cannot fail.
    pub(super) fn standard_native(&mut self, native: Native, args: &'a [Expr]) -> Code {
        let fd = self.value(&args[0], Type::Int).text;
        let call = |g: &mut Self, f: &str, args: Vec<String>| {
            Code::atom(format!("{}({})", g.runtime(f), args.join(", ")))
        };
        match native {
            Native::StandardWrite => {
                let b = self.value(&args[1], Type::Int).text;
                call(self, "standard_write", vec![fd, b])
            }
            Native::StandardWriteBytes => {
                let [off, len] = self.hoisted_ints(args, [2, 3]);
                let buf = self.byte_array(&args[1], false);
                call(self, "standard_write_bytes", vec![fd, buf, off, len])
            }
            Native::StandardFlush => call(self, "standard_flush", vec![fd]),
            Native::StandardClose => call(self, "standard_close", vec![fd]),
            _ => unreachable!("not a native of the standard streams: {native:?}"),
        }
    }

    /// A call of a runtime function that returns `Result`, with the failure raised as Java's
    /// exception: where a `catch` or a `finally` may see it (`Facts::raising`), as a thrown
    /// exception that goes on from here, else as an uncaught one.
    pub(super) fn fallible(&mut self, id: ExprId, f: &str, args: Vec<String>) -> Code {
        let call = format!("{}({})", self.runtime(f), args.join(", "));
        if self.facts.raising.contains(&id) {
            let checked = format!("{call}.map_err({})", self.runtime("Failure::exception"));
            return Code::atom(self.passed_on(Code::atom(checked)));
        }
        Code::atom(format!("{}({call})", self.runtime("or_throw")))
    }

    /// The int arguments at `at`, in Java's order. Each one that is not a constant goes into a
    /// temporary first: the array borrow taken by the call must not overlap the evaluation of a
    /// later argument that reads the array.
    pub(super) fn hoisted_ints<const N: usize>(&mut self, args: &'a [Expr], at: [usize; N]) -> [String; N] {
        at.map(|i| {
            let code = self.value(&args[i], Type::Int);
            if code.literal.is_some() {
                return code.text;
            }
            let temp = self.temp(&format!("arg{i}"));
            self.line(&format!("let {temp} = {};", code.text));
            temp
        })
    }

    /// A byte array as the runtime's slice: borrowed mutably when a read writes to it, else for
    /// reading. A variable is borrowed as it is; any other array goes through its cell.
    fn byte_array(&mut self, e: &'a Expr, written: bool) -> String {
        self.array_arg(e, written)
    }

    /// An array argument as a slice, borrowed mutably when the call writes to it.
    pub(super) fn array_arg(&mut self, e: &'a Expr, written: bool) -> String {
        match &e.kind {
            ExprKind::Var(name) if self.plain(name).is_some() => self.borrowed_arg(e, written),
            _ => {
                let cell = self.array_receiver(e);
                if written { format!("&mut {cell}.borrow_mut()") } else { format!("&{cell}.borrow()") }
            }
        }
    }
}
