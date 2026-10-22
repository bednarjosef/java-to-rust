//! The file channel natives (`ast::Native::TransferUnsupported`, `MapFile`, `UnmapFile` and
//! `AllocationGranularity`), which the runtime's `file_channels` implements. A transfer's arguments
//! are not evaluated: the runtime answers without them, as `Native::Nothing` drops its own. A
//! mapping can fail, so it goes through `fallible`.

use super::*;

impl<'a> Gen<'a> {
    pub(super) fn file_channel_native(&mut self, native: Native, args: &'a [Expr], id: ExprId) -> Code {
        match native {
            Native::TransferUnsupported => Code::atom(format!("{}()", self.runtime("transfer_unsupported"))),
            Native::MapFile => {
                let fd = self.value(&args[0], Type::Int).text;
                let prot = self.value(&args[1], Type::Int).text;
                let position = self.value(&args[2], Type::Long).text;
                let length = self.value(&args[3], Type::Long).text;
                let is_sync = self.value(&args[4], Type::Boolean).text;
                self.fallible(id, "map_file", vec![fd, prot, position, length, is_sync])
            }
            Native::UnmapFile => {
                let address = self.value(&args[0], Type::Long).text;
                let length = self.value(&args[1], Type::Long).text;
                Code::atom(format!("{}({address}, {length})", self.runtime("unmap_file")))
            }
            Native::AllocationGranularity => {
                Code::atom(format!("{}()", self.runtime("allocation_granularity")))
            }
            _ => unreachable!("not a file channel native: {native:?}"),
        }
    }
}
