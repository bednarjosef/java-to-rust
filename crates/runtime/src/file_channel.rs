//! The natives of `sun.nio.ch` behind `FileChannelImpl` (`UnixFileDispatcherImpl`,
//! `FileDispatcherImpl`, `IOUtil` and `NativeThread`), which the translation calls as
//! `java::channel_*`. A channel reads and writes the file of a descriptor from `fd.rs`; the bytes
//! it moves are in the block of native memory (`native_memory.rs`) at the address it gives. Each
//! function answers what the Linux JVM's C code answers: the count moved, `-1` (`IOStatus.EOF`)
//! for a read that gets no bytes, and an `IOException` with the system's reason for a failed call.
//!
//! File locks are the system's record locks (`fcntl`), as the JVM takes them: they belong to the
//! process, so two channels of one program do not exclude each other, and `FileLockTable` (by the
//! file's `FileKey`) keeps a program from locking a region twice, as in Java.
//!
//! Two natives are not exact, and say so. `setDirect0` always fails, because `O_DIRECT` cannot
//! be set on an open file with the standard library. `signal0` does nothing, because a translated
//! thread is never blocked in a read that another thread could interrupt.

use crate::fd::{Failure, file_available, last_error, with_file};
use crate::native_memory::{read_native, write_native};
use std::cell::Cell;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::FileExt;
use std::sync::atomic::{AtomicI64, Ordering};

/// `IOStatus.EOF`: what a read answers when it gets no bytes.
const EOF: i32 = -1;

const EINVAL: i32 = 22;
const EOPNOTSUPP: i32 = 95;

/// `f` of the file of descriptor `fd`, with a failure as the `IOException` the JVM throws.
fn file_op<T>(fd: i32, f: impl FnOnce(&File) -> io::Result<T>) -> Result<T, Failure> {
    with_file(fd, f).map_err(last_error)
}

fn invalid() -> Failure {
    last_error(io::Error::from_raw_os_error(EINVAL))
}

/// The length of a block the channel moves, which Java never makes negative.
fn block_len(len: i32) -> Result<usize, Failure> {
    usize::try_from(len).map_err(|_| invalid())
}

/// The file position a positional call names, which Java never makes negative.
fn position_of(position: i64) -> Result<u64, Failure> {
    u64::try_from(position).map_err(|_| invalid())
}

/// The `len` bytes at `address`; none need no block, so an empty transfer touches no memory.
fn bytes_at(address: i64, len: usize) -> Vec<u8> {
    if len == 0 { Vec::new() } else { read_native(address, len) }
}

/// Copies `bytes` to the block at `address` and answers how many there are, or `EOF` for none.
fn deliver(address: i64, bytes: &[u8]) -> i32 {
    if bytes.is_empty() {
        return EOF;
    }
    write_native(address, bytes);
    bytes.len() as i32
}

/// `read0`: `read(2)` of up to `len` bytes from the file's position into the block at `address`.
pub fn channel_read(fd: i32, address: i64, len: i32) -> Result<i32, Failure> {
    let mut bytes = vec![0; block_len(len)?];
    let n = file_op(fd, |mut f| f.read(&mut bytes))?;
    Ok(deliver(address, &bytes[..n]))
}

/// `pread0`: `pread(2)` of up to `len` bytes at `position`, which leaves the file's position alone.
pub fn channel_pread(fd: i32, address: i64, len: i32, position: i64) -> Result<i32, Failure> {
    let mut bytes = vec![0; block_len(len)?];
    let at = position_of(position)?;
    let n = file_op(fd, |f| f.read_at(&mut bytes, at))?;
    Ok(deliver(address, &bytes[..n]))
}

/// `write0`: `write(2)` of `len` bytes from the block at `address` at the file's position, and
/// the count written.
pub fn channel_write(fd: i32, address: i64, len: i32) -> Result<i32, Failure> {
    let bytes = bytes_at(address, block_len(len)?);
    let n = file_op(fd, |mut f| f.write(&bytes))?;
    Ok(n as i32)
}

/// `pwrite0`: `pwrite(2)` of `len` bytes from the block at `address` at `position`, which leaves
/// the file's position alone.
pub fn channel_pwrite(fd: i32, address: i64, len: i32, position: i64) -> Result<i32, Failure> {
    let bytes = bytes_at(address, block_len(len)?);
    let at = position_of(position)?;
    let n = file_op(fd, |f| f.write_at(&bytes, at))?;
    Ok(n as i32)
}

/// `seek0`: moves the file's position to `offset`, and answers the position. A negative
/// `offset` moves nothing and only answers the position, as `lseek(fd, 0, SEEK_CUR)` does.
pub fn channel_seek(fd: i32, offset: i64) -> Result<i64, Failure> {
    let to = match u64::try_from(offset) {
        Ok(at) => SeekFrom::Start(at),
        Err(_) => SeekFrom::Current(0),
    };
    Ok(file_op(fd, |mut f| f.seek(to))? as i64)
}

/// `size0`: the length of the file, as `fstat` gives it.
pub fn channel_size(fd: i32) -> Result<i64, Failure> {
    Ok(file_op(fd, |f| f.metadata())?.len() as i64)
}

/// `truncate0`: `ftruncate(2)` to `len` bytes. The position stays where it is, as on Linux.
pub fn channel_truncate(fd: i32, len: i64) -> Result<i32, Failure> {
    let size = u64::try_from(len).map_err(|_| invalid())?;
    file_op(fd, |f| f.set_len(size))?;
    Ok(0)
}

/// `available0`: the bytes from the position to the end of a regular file, or what a pipe or a
/// terminal holds without waiting.
pub fn channel_available(fd: i32) -> Result<i32, Failure> {
    file_op(fd, file_available)
}

/// `isOther0`: whether the file is something other than a regular file or a directory.
pub fn channel_is_other(fd: i32) -> Result<bool, Failure> {
    let kind = file_op(fd, |f| f.metadata())?.file_type();
    Ok(!(kind.is_file() || kind.is_dir()))
}

/// `setDirect0`: the Linux JVM sets `O_DIRECT` on the descriptor and answers 0. The standard
/// library cannot set it on an open file, so this always fails with the system's
/// "Operation not supported", after the descriptor is checked as the JVM checks it.
pub fn channel_set_direct(fd: i32) -> Result<i32, Failure> {
    file_op(fd, |_| Err::<i32, _>(io::Error::from_raw_os_error(EOPNOTSUPP)))
}

/// `FileDispatcherImpl.NO_LOCK`: another process holds a lock on the region.
const NO_LOCK: i32 = -1;
/// `FileDispatcherImpl.INTERRUPTED` and `IOStatus.INTERRUPTED`: a signal ended the call.
const INTERRUPTED: i32 = 2;
const IOS_INTERRUPTED: i32 = -3;

const EINTR: i32 = 4;
const EAGAIN: i32 = 11;
const EACCES: i32 = 13;
const F_SETLK: i32 = 6;
const F_SETLKW: i32 = 7;
const F_RDLCK: i16 = 0;
const F_WRLCK: i16 = 1;
const F_UNLCK: i16 = 2;

/// C's `struct flock` on Linux, whose `off_t` is 64 bits.
#[repr(C)]
struct Flock {
    l_type: i16,
    l_whence: i16,
    l_start: i64,
    l_len: i64,
    l_pid: i32,
}

unsafe extern "C" {
    fn fcntl(fd: i32, cmd: i32, ...) -> i32;
}

/// `fcntl(fd, cmd, lock)` of the region from `position` of `size` bytes, a size of
/// `Long.MAX_VALUE` meaning to the end of the file however it grows, as the JVM asks.
fn record_lock(fd: i32, cmd: i32, kind: i16, position: i64, size: i64) -> Result<(), io::Error> {
    // The system's descriptor is used outside the table's lock, so that a lock that waits for
    // another process does not hold up the program's other files. Closing a copy of it would
    // release the process's locks on the file, so no copy is made.
    let system = system_descriptor(fd)?;
    let lock = Flock {
        l_type: kind,
        l_whence: 0,
        l_start: position,
        l_len: if size == i64::MAX { 0 } else { size },
        l_pid: 0,
    };
    // Safety: `lock` is a `struct flock` that lives across the call.
    if unsafe { fcntl(system, cmd, &lock as *const Flock) } < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// The system's descriptor of the file of `fd`.
fn system_descriptor(fd: i32) -> io::Result<i32> {
    use std::os::fd::AsRawFd;
    with_file(fd, |f| Ok(f.as_raw_fd()))
}

/// What a lock of a region came to.
pub(crate) enum Locked {
    Held,
    /// Another process holds a lock on the region, and the call did not wait.
    Refused,
    /// A signal ended the wait.
    Interrupted,
}

/// A lock of the region, shared or exclusive, that waits for other processes' locks when
/// `blocking`, as the JVM's `lock0` takes it.
pub(crate) fn lock_region(
    fd: i32,
    blocking: bool,
    position: i64,
    size: i64,
    shared: bool,
) -> io::Result<Locked> {
    let cmd = if blocking { F_SETLKW } else { F_SETLK };
    let kind = if shared { F_RDLCK } else { F_WRLCK };
    match record_lock(fd, cmd, kind, position, size) {
        Ok(()) => Ok(Locked::Held),
        Err(e) if !blocking && matches!(e.raw_os_error(), Some(EAGAIN | EACCES)) => Ok(Locked::Refused),
        Err(e) if e.raw_os_error() == Some(EINTR) => Ok(Locked::Interrupted),
        Err(e) => Err(e),
    }
}

/// Releases the lock of the region.
pub(crate) fn unlock_region(fd: i32, position: i64, size: i64) -> io::Result<()> {
    record_lock(fd, F_SETLK, F_UNLCK, position, size)
}

/// `lock0`: 0 once the lock is held, `NO_LOCK` where another process holds one and the call
/// does not wait, `INTERRUPTED` where a signal ended the wait.
pub fn channel_lock(fd: i32, blocking: bool, position: i64, size: i64, shared: bool) -> Result<i32, Failure> {
    Ok(match lock_region(fd, blocking, position, size, shared).map_err(last_error)? {
        Locked::Held => 0,
        Locked::Refused => NO_LOCK,
        Locked::Interrupted => INTERRUPTED,
    })
}

/// `release0`: releases the lock of the region.
pub fn channel_release(fd: i32, position: i64, size: i64) -> Result<(), Failure> {
    unlock_region(fd, position, size).map_err(last_error)
}

/// `force0`: writes the file's data, and its metadata too when `metadata`, to the device
/// (`fsync`, else `fdatasync`), and answers 0.
pub fn channel_force(fd: i32, metadata: bool) -> Result<i32, Failure> {
    match with_file(fd, |f| if metadata { f.sync_all() } else { f.sync_data() }) {
        Ok(()) => Ok(0),
        Err(e) if e.raw_os_error() == Some(EINTR) => Ok(IOS_INTERRUPTED),
        Err(e) => Err(last_error(e)),
    }
}

/// `FileKey.init(fd, finfo)`: the device and the inode of the file, which tell whether two
/// channels have the same file open.
pub fn channel_file_key(fd: i32, info: &mut [i64]) -> Result<(), Failure> {
    use std::os::unix::fs::MetadataExt;
    let metadata = file_op(fd, |f| f.metadata())?;
    info[0] = metadata.dev() as i64;
    info[1] = metadata.ino() as i64;
    Ok(())
}

static NEXT_THREAD: AtomicI64 = AtomicI64::new(1);

thread_local! {
    static THREAD_ID: Cell<i64> = Cell::new(NEXT_THREAD.fetch_add(1, Ordering::Relaxed));
}

/// `NativeThread.current0()`: an id of the calling thread, which is never 0.
pub fn native_thread_current() -> i64 {
    THREAD_ID.with(Cell::get)
}

/// `NativeThread.signal0(thread)`: wakes a thread that is blocked in a native call. A translated
/// thread is never blocked in one, so there is nothing to wake.
pub fn native_thread_signal(_thread: i64) {}

/// `IOUtil.iovMax()`: `IOV_MAX` on Linux.
pub fn io_iov_max() -> i32 {
    1024
}

/// `IOUtil.writevMax()`: the most bytes one `writev` call takes, as Linux's `ssize_t` limit.
pub fn io_writev_max() -> i64 {
    i32::MAX as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fd::{fd_close, insert_file};
    use crate::native_memory::allocate_native;
    use std::path::{Path, PathBuf};

    /// A file of this test's own holding `text`, open to read and write, as a descriptor and
    /// the file's path. The caller closes the descriptor and removes the directory.
    fn open(test: &str, text: &[u8]) -> (i32, PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("java-runtime-file-channel-{test}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("data");
        std::fs::write(&path, text).unwrap();
        let file = File::options().read(true).write(true).open(&path).unwrap();
        (insert_file(file, true, true), path)
    }

    fn close(fd: i32, path: &Path) {
        fd_close(fd);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    /// A block of native memory holding `bytes`.
    fn block(bytes: &[u8]) -> i64 {
        let address = allocate_native(bytes.len() as i64);
        write_native(address, bytes);
        address
    }

    #[test]
    fn read_moves_the_position_and_ends_at_the_end_of_the_file() {
        let (fd, path) = open("read", b"hello world");
        let at = block(&[0; 8]);
        assert_eq!(channel_read(fd, at, 5), Ok(5));
        assert_eq!(read_native(at, 5), b"hello");
        assert_eq!(channel_seek(fd, -1), Ok(5));
        assert_eq!(channel_read(fd, at, 8), Ok(6));
        assert_eq!(read_native(at, 6), b" world");
        assert_eq!(channel_read(fd, at, 8), Ok(EOF));
        close(fd, &path);
    }

    #[test]
    fn pread_reads_at_a_position_and_keeps_the_position() {
        let (fd, path) = open("pread", b"hello world");
        let at = block(&[0; 3]);
        assert_eq!(channel_pread(fd, at, 3, 6), Ok(3));
        assert_eq!(read_native(at, 3), b"wor");
        assert_eq!(channel_seek(fd, -1), Ok(0));
        assert_eq!(channel_pread(fd, at, 3, 20), Ok(EOF));
        close(fd, &path);
    }

    #[test]
    fn write_moves_the_position_and_changes_the_file() {
        let (fd, path) = open("write", b"hello world");
        assert_eq!(channel_write(fd, block(b"HEY"), 3), Ok(3));
        assert_eq!(channel_seek(fd, -1), Ok(3));
        assert_eq!(std::fs::read(&path).unwrap(), b"HEYlo world");
        close(fd, &path);
    }

    #[test]
    fn pwrite_writes_at_a_position_and_keeps_the_position() {
        let (fd, path) = open("pwrite", b"HEYlo world");
        assert_eq!(channel_pwrite(fd, block(b"?"), 1, 10), Ok(1));
        assert_eq!(channel_seek(fd, -1), Ok(0));
        assert_eq!(std::fs::read(&path).unwrap(), b"HEYlo worl?");
        close(fd, &path);
    }

    #[test]
    fn seek_answers_the_position_and_moves_it_past_the_end_too() {
        let (fd, path) = open("seek", b"hello");
        assert_eq!(channel_seek(fd, 3), Ok(3));
        assert_eq!(channel_seek(fd, -1), Ok(3));
        assert_eq!(channel_seek(fd, 9), Ok(9));
        assert_eq!(channel_size(fd), Ok(5));
        close(fd, &path);
    }

    #[test]
    fn size_and_truncate_follow_the_file() {
        let (fd, path) = open("truncate", b"hello world");
        assert_eq!(channel_size(fd), Ok(11));
        assert_eq!(channel_truncate(fd, 5), Ok(0));
        assert_eq!(channel_size(fd), Ok(5));
        assert_eq!(std::fs::read(&path).unwrap(), b"hello");
        close(fd, &path);
    }

    #[test]
    fn available_is_the_bytes_left_in_the_file() {
        let (fd, path) = open("available", b"hello");
        assert_eq!(channel_available(fd), Ok(5));
        assert_eq!(channel_read(fd, block(&[0; 2]), 2), Ok(2));
        assert_eq!(channel_available(fd), Ok(3));
        close(fd, &path);
    }

    #[test]
    fn a_regular_file_is_not_other() {
        let (fd, path) = open("other", b"hello");
        assert_eq!(channel_is_other(fd), Ok(false));
        close(fd, &path);
    }

    #[test]
    fn set_direct_fails_as_unsupported() {
        let (fd, path) = open("direct", b"hello");
        let failure = channel_set_direct(fd).unwrap_err();
        assert_eq!(failure.class, "java.io.IOException");
        assert_eq!(failure.message, "Operation not supported");
        close(fd, &path);
    }

    #[test]
    fn a_descriptor_without_a_file_is_a_bad_descriptor() {
        let failure = channel_size(-1).unwrap_err();
        assert_eq!(failure.class, "java.io.IOException");
        assert_eq!(failure.message, "Bad file descriptor");
    }

    #[test]
    fn a_thread_has_an_id_that_is_never_zero_and_differs_from_another() {
        let here = native_thread_current();
        assert_ne!(here, 0);
        assert_eq!(native_thread_current(), here);
        let there = std::thread::spawn(native_thread_current).join().unwrap();
        assert_ne!(there, 0);
        assert_ne!(there, here);
    }

    #[test]
    fn io_limits_are_the_linux_ones() {
        assert_eq!(io_iov_max(), 1024);
        assert_eq!(io_writev_max(), i32::MAX as i64);
    }

    #[test]
    fn a_file_is_locked_released_forced_and_known_by_its_key() {
        let (fd, path) = open("locks", b"abc");
        assert_eq!(channel_lock(fd, false, 0, i64::MAX, false).unwrap(), 0);
        assert_eq!(channel_lock(fd, true, 1, 2, true).unwrap(), 0);
        channel_release(fd, 0, i64::MAX).unwrap();
        assert_eq!(channel_force(fd, true).unwrap(), 0);
        assert_eq!(channel_force(fd, false).unwrap(), 0);
        let again = insert_file(File::open(&path).unwrap(), true, false);
        let other_path = path.with_file_name("other");
        std::fs::write(&other_path, b"x").unwrap();
        let other = insert_file(File::open(&other_path).unwrap(), true, false);
        let key = |fd| {
            let mut info = [0; 2];
            channel_file_key(fd, &mut info).unwrap();
            info
        };
        assert_eq!(key(fd), key(again));
        assert_ne!(key(fd), key(other));
        fd_close(again);
        fd_close(other);
        close(fd, &path);
        assert_eq!(channel_lock(fd, false, 0, 1, false).unwrap_err().message, "Bad file descriptor");
    }
}
