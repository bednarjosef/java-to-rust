//! The file natives (`ast::Native::File*`), which call the runtime's descriptor table
//! (`java_runtime::fd`). A call that can fail is wrapped in `or_throw`, which raises the Java
//! exception. Every native but `open0` takes the stream's descriptor (`fd`) as its first argument.

use super::*;

impl<'a> Gen<'a> {
    pub(super) fn file_native(&mut self, native: Native, args: &'a [Expr]) -> Code {
        match native {
            Native::FileOpenRead => {
                let name = self.str_value(&args[0]);
                self.fallible("fd_open_read", vec![format!("&{name}")])
            }
            Native::FileOpenWrite => {
                let name = self.str_value(&args[0]);
                let append = self.value(&args[1], Type::Boolean).text;
                self.fallible("fd_open_write", vec![format!("&{name}"), append])
            }
            Native::FileRead => {
                let fd = self.value(&args[0], Type::Int).text;
                self.fallible("fd_read", vec![fd])
            }
            Native::FileReadBytes => {
                let [fd, off, len] = self.hoisted_ints(args, [0, 2, 3]);
                let buf = self.byte_array(&args[1], true);
                self.fallible("fd_read_bytes", vec![fd, buf, off, len])
            }
            Native::FileAvailable => {
                let fd = self.value(&args[0], Type::Int).text;
                self.fallible("fd_available", vec![fd])
            }
            Native::FileSkip => {
                let fd = self.value(&args[0], Type::Int).text;
                let n = self.value(&args[1], Type::Long).text;
                self.fallible("fd_skip", vec![fd, n])
            }
            Native::FileClose => {
                let fd = self.value(&args[0], Type::Int).text;
                Code::atom(format!("{}({fd})", self.runtime("fd_close")))
            }
            Native::FileWrite => {
                let fd = self.value(&args[0], Type::Int).text;
                let b = self.value(&args[1], Type::Int).text;
                self.fallible("fd_write", vec![fd, b])
            }
            Native::FileWriteBytes => {
                let [fd, off, len] = self.hoisted_ints(args, [0, 2, 3]);
                let buf = self.byte_array(&args[1], false);
                self.fallible("fd_write_bytes", vec![fd, buf, off, len])
            }
            _ => unreachable!("not a file native: {native:?}"),
        }
    }

    /// A call of a runtime function that returns `Result`, with the failure raised as Java's
    /// exception.
    fn fallible(&mut self, f: &str, args: Vec<String>) -> Code {
        let call = format!("{}({})", self.runtime(f), args.join(", "));
        Code::atom(format!("{}({call})", self.runtime("or_throw")))
    }

    /// The int arguments at `at`, in Java's order. Each one that is not a constant goes into a
    /// temporary first: the array borrow taken by the call must not overlap the evaluation of a
    /// later argument that reads the array.
    fn hoisted_ints<const N: usize>(&mut self, args: &'a [Expr], at: [usize; N]) -> [String; N] {
        at.map(|i| {
            let code = self.value(&args[i], Type::Int);
            if code.literal.is_some() {
                return code.text;
            }
            let temp = self.temp("arg");
            self.line(&format!("let {temp} = {};", code.text));
            temp
        })
    }

    /// A byte array as the runtime's slice: borrowed mutably when a read writes to it, else for
    /// reading. A variable is borrowed as it is; any other array goes through its cell.
    fn byte_array(&mut self, e: &'a Expr, written: bool) -> String {
        match &e.kind {
            ExprKind::Var(name) if self.plain(name).is_some() => self.borrowed_arg(e, written),
            _ => {
                let cell = self.array_receiver(e);
                if written { format!("&mut {cell}.borrow_mut()") } else { format!("&{cell}.borrow()") }
            }
        }
    }
}
