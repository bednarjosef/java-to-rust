//! The natives of the JDK's file copy, which `Files.copy` runs: `LinuxNativeDispatcher`'s
//! `posix_fadvise` and `directCopy0`, and `UnixFileSystem`'s `bufferedCopy0`, which the translation
//! calls as `java::posix_fadvise`, `java::direct_copy` and `java::buffered_copy`. The copy reads
//! and writes the files of the descriptors from `fd.rs` through their `File`s.
//!
//! A failed read or write answers its `errno`, as HotSpot's `bufferedCopy0` throws
//! `UnixException(errno)`: the translator's patch of `UnixFileSystem.bufferedCopy` throws that
//! exception, and `copyFile` turns it into the `IOException` subclass of the JDK's message
//! (`NoSuchFileException`, `AccessDeniedException`, ...).
//!
//! Two of them are not exact, and say so. `directCopy0` answers `IOStatus.UNSUPPORTED`, so the JDK
//! copies through `bufferedCopy0`, which moves the same bytes between the same positions;
//! `copy_file_range(2)` is not used. `posix_fadvise` gives the kernel advice that no program can
//! see, so the runtime only checks that the descriptor is open.

use crate::fd::{Failure, last_error, with_file};
use crate::native_memory::{read_native, write_native};
use std::fs::File;
use std::io::{self, Read, Write};

/// `IOStatus.UNSUPPORTED`: what `directCopy0` answers when the JDK must copy through its buffer.
const UNSUPPORTED: i32 = -4;
/// `EINVAL`, for a block length that is negative.
const EINVAL: i32 = 22;
/// `EIO`, for a failure that has no `errno` of its own.
const EIO: i32 = 5;

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
///
/// The result is `0`, or the `errno` of the read or write that failed.
pub fn buffered_copy(dst: i32, src: i32, address: i64, size: i32, cancel: i64) -> i32 {
    match copy_chunks(dst, src, address, size, cancel) {
        Ok(()) => 0,
        Err(e) => e.raw_os_error().unwrap_or(EIO),
    }
}

fn copy_chunks(dst: i32, src: i32, address: i64, size: i32, cancel: i64) -> io::Result<()> {
    let len = usize::try_from(size).map_err(|_| io::Error::from_raw_os_error(EINVAL))?;
    while !cancelled(cancel) {
        let mut chunk = vec![0; len];
        let read = with_file(src, |mut f: &File| f.read(&mut chunk))?;
        if read == 0 {
            break;
        }
        write_native(address, &chunk[..read]);
        let mut written = 0;
        while written < read {
            let bytes = read_native(address + written as i64, read - written);
            written += with_file(dst, |mut f: &File| f.write(&bytes))?;
        }
    }
    Ok(())
}

/// Whether the JDK has set the word at `address`, which is polled between chunks.
fn cancelled(address: i64) -> bool {
    address != 0 && i32::from_ne_bytes(read_native(address, 4).try_into().unwrap_or([0; 4])) != 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fd::{fd_close, insert_file};
    use crate::native_memory::allocate_native;
    use std::path::{Path, PathBuf};

    /// A file of this test's own, holding `text`, and its directory. The caller removes it.
    fn file(test: &str, name: &str, text: &[u8]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("java-runtime-file-copy-{test}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, text).unwrap();
        path
    }

    /// A descriptor of `path`, open to read, to write, or to both.
    fn open(path: &Path, read: bool, write: bool) -> i32 {
        let file = File::options().read(read).write(write).open(path).unwrap();
        insert_file(file, read, write)
    }

    /// Closes the descriptors and removes the directory of `path`.
    fn close(fds: &[i32], path: &Path) {
        for &fd in fds {
            fd_close(fd);
        }
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn a_copy_moves_the_bytes_in_chunks_and_answers_zero() {
        let src = file("copy", "src", b"hello");
        let dst = file("copy", "dst", b"");
        let (from, to) = (open(&src, true, false), open(&dst, false, true));
        let block = allocate_native(4);
        assert_eq!(buffered_copy(to, from, block, 4, 0), 0);
        assert_eq!(std::fs::read(&dst).unwrap(), b"hello");
        close(&[from, to], &src);
    }

    #[test]
    fn a_read_error_answers_its_errno() {
        let src = file("read-error", "src", b"hello");
        let dst = file("read-error", "dst", b"");
        // The source is open to write only, so reading it fails with `EBADF`.
        let (from, to) = (open(&src, false, true), open(&dst, false, true));
        let block = allocate_native(4);
        assert_eq!(buffered_copy(to, from, block, 4, 0), 9, "EBADF");
        close(&[from, to], &src);
    }

    #[test]
    fn a_write_error_answers_its_errno() {
        let src = file("write-error", "src", b"hello");
        let dst = file("write-error", "dst", b"");
        // The destination is open to read only, so writing it fails with `EBADF`.
        let (from, to) = (open(&src, true, false), open(&dst, true, false));
        let block = allocate_native(4);
        assert_eq!(buffered_copy(to, from, block, 4, 0), 9, "EBADF");
        close(&[from, to], &src);
    }
}
