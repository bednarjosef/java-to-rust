//! The natives of the JDK's file copy (`ast::Native::PosixFadvise`, `DirectCopy` and
//! `BufferedCopy`), which the runtime's `file_copy` implements. `posix_fadvise` can fail, so it
//! goes through `fallible`; `directCopy0` and `bufferedCopy0` answer a status or an `errno`, which
//! the JDK's code (and the translator's patch of `UnixFileSystem`) turns into the exception.

use super::*;

impl<'a> Gen<'a> {
    pub(super) fn file_copy_native(&mut self, native: Native, args: &'a [Expr], id: ExprId) -> Code {
        match native {
            Native::PosixFadvise => {
                let fd = self.value(&args[0], Type::Int).text;
                let offset = self.value(&args[1], Type::Long).text;
                let len = self.value(&args[2], Type::Long).text;
                let advice = self.value(&args[3], Type::Int).text;
                self.fallible(id, "posix_fadvise", vec![fd, offset, len, advice])
            }
            Native::DirectCopy => {
                let dst = self.value(&args[0], Type::Int).text;
                let src = self.value(&args[1], Type::Int).text;
                let cancel = self.value(&args[2], Type::Long).text;
                Code::atom(format!("{}({dst}, {src}, {cancel})", self.runtime("direct_copy")))
            }
            Native::BufferedCopy => {
                let dst = self.value(&args[0], Type::Int).text;
                let src = self.value(&args[1], Type::Int).text;
                let address = self.value(&args[2], Type::Long).text;
                let size = self.value(&args[3], Type::Int).text;
                let cancel = self.value(&args[4], Type::Long).text;
                Code::atom(format!(
                    "{}({dst}, {src}, {address}, {size}, {cancel})",
                    self.runtime("buffered_copy")
                ))
            }
            _ => unreachable!("not a file copy native: {native:?}"),
        }
    }
}
