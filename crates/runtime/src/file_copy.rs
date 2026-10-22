//! The natives of the JDK's file copy, which `Files.copy` runs: `LinuxNativeDispatcher`'s
//! `posix_fadvise` and `directCopy0`, and `UnixFileSystem`'s `bufferedCopy0`, which the translation
//! calls as `java::posix_fadvise`, `java::direct_copy` and `java::buffered_copy`. The copy reads
//! and writes the files of the descriptors from `fd.rs` through the channel functions
//! (`file_channel.rs`), so a failed call fails with the same `IOException` a channel's does.
//!
//! Two of them are not exact, and say so. `directCopy0` answers `IOStatus.UNSUPPORTED`, so the JDK
//! copies through `bufferedCopy0`, which moves the same bytes between the same positions;
//! `copy_file_range(2)` is not used. `posix_fadvise` gives the kernel advice that no program can
//! see, so the runtime only checks that the descriptor is open.

use crate::fd::{Failure, last_error, with_file};
use crate::file_channel::{channel_read, channel_write};
use crate::native_memory::read_native;

/// `IOStatus.UNSUPPORTED`: what `directCopy0` answers when the JDK must copy through its buffer.
const UNSUPPORTED: i32 = -4;

/// `LinuxNativeDispatcher.posix_fadvise(fd, offset, len, advice)`: `0` for an open descriptor. The
/// advice is not used, so the arguments are only checked as far as the descriptor goes.
pub fn posix_fadvise(fd: i32, _offset: i64, _len: i64, _advice: i32) -> Result<i32, Failure> {
    with_file(fd, |_| Ok(())).map_err(last_error)?;
    Ok(0)
}

/// `LinuxNativeDispatcher.directCopy0(dst, src, addressToPollForCancel)`: the kernel's copy is
/// not used, so the JDK is told so and copies through `buffered_copy` instead.
pub fn direct_copy(_dst: i32, _src: i32, _cancel: i64) -> i32 {
    UNSUPPORTED
}

/// `UnixFileSystem.bufferedCopy0(dst, src, address, size, addressToPollForCancel)`: reads `src`
/// into the block at `address`, `size` bytes at a time, and writes each chunk to `dst`, until
/// `src` has no more bytes. The copy stops before a chunk once the word at the cancel address is
/// not zero; address `0` is a copy that cannot be cancelled.
pub fn buffered_copy(dst: i32, src: i32, address: i64, size: i32, cancel: i64) -> Result<(), Failure> {
    while !cancelled(cancel) {
        let read = channel_read(src, address, size)?;
        if read < 0 {
            break;
        }
        let mut written = 0;
        while written < read {
            let at = address + i64::from(written);
            written += channel_write(dst, at, read - written)?;
        }
    }
    Ok(())
}

/// Whether the JDK has set the word at `address`, which is polled between chunks.
fn cancelled(address: i64) -> bool {
    address != 0 && i32::from_ne_bytes(read_native(address, 4).try_into().unwrap_or([0; 4])) != 0
}
