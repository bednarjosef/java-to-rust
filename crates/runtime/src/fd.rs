//! The file descriptors behind the natives of `java.io.FileInputStream`, `FileOutputStream`,
//! `RandomAccessFile` and `FileDescriptor` (`ast::Native::File*`). A descriptor is an index into a table of the process, as
//! the JVM's is: 0, 1 and 2 are standard input, output and error. Standard output
//! goes through `out` so that it stays in order with `println!`. Standard input is descriptor 0
//! itself, read without a buffer in between, so that each read returns what the JVM's would; what
//! standard output has written goes out before it is read, so that a prompt shows. A failure is a
//! Java exception (`Failure`), which the generated code raises with `or_throw`, or passes on as a
//! thrown exception where the program may catch it.

use crate::{out, throw};
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, Read, Seek, SeekFrom, Write};
use std::sync::{LazyLock, Mutex};

pub const STDIN_FD: i32 = 0;
pub const STDOUT_FD: i32 = 1;
pub const STDERR_FD: i32 = 2;

/// A Java exception raised by a native: its class and its message.
#[derive(Debug, PartialEq, Eq)]
pub struct Failure {
    pub class: &'static str,
    pub message: String,
}

/// What a descriptor reads from or writes to.
enum Entry {
    Stdin,
    Stdout,
    Stderr,
    Read(File),
    Write(File),
    /// A `RandomAccessFile` opened with `"rw"`, which reads and writes.
    ReadWrite(File),
    /// A socket, a pipe, an `epoll` instance or an event counter of `sun.nio.ch` (`socket.rs`),
    /// which its natives use by the system's descriptor.
    #[cfg(unix)]
    System(std::os::fd::OwnedFd),
}

/// The descriptors of the process. Tests supply standard input as a reader (`stdin`); the
/// process's is descriptor 0.
pub struct FdTable {
    stdin: Option<Box<dyn BufRead + Send>>,
    /// Indexed by descriptor; `None` once closed.
    entries: Vec<Option<Entry>>,
}

/// The table every thread of the program uses.
static FDS: LazyLock<Mutex<FdTable>> = LazyLock::new(|| {
    let mut table = FdTable::new(Box::new(io::empty()));
    table.stdin = None;
    Mutex::new(table)
});

/// Descriptor 0 as a file, which reads and seeks it directly. Rust's `Stdin` would read through a
/// buffer of its own, and return other counts than the JVM's reads.
#[cfg(unix)]
fn process_stdin() -> std::mem::ManuallyDrop<File> {
    use std::os::unix::io::FromRawFd;
    // Safety: the file is never dropped, so descriptor 0 stays open, and nothing else in the
    // process reads it.
    std::mem::ManuallyDrop::new(unsafe { File::from_raw_fd(0) })
}

#[cfg(not(unix))]
fn process_stdin() -> std::mem::ManuallyDrop<File> {
    panic!("the runtime reads standard input on Unix-like systems only")
}

/// What `available()` of an open file says: the bytes from its position to its end for a regular
/// file, and what a pipe, a terminal or a socket holds (`FIONREAD`) otherwise, as the JVM's
/// `available0` answers.
pub(crate) fn file_available(file: &File) -> io::Result<i32> {
    let meta = file.metadata()?;
    if meta.is_file() {
        let position = (&mut &*file).stream_position()?;
        return Ok(meta.len().saturating_sub(position).min(i32::MAX as u64) as i32);
    }
    Ok(pending_bytes(file))
}

/// The bytes that can be read from a pipe, terminal or socket without waiting.
#[cfg(target_os = "linux")]
fn pending_bytes(file: &File) -> i32 {
    use std::os::unix::io::AsRawFd;
    unsafe extern "C" {
        fn ioctl(fd: i32, request: std::ffi::c_ulong, ...) -> i32;
    }
    const FIONREAD: std::ffi::c_ulong = 0x541B;
    let mut count: i32 = 0;
    // Safety: `FIONREAD` writes one `int` to the address it is given.
    if unsafe { ioctl(file.as_raw_fd(), FIONREAD, &mut count as *mut i32) } == 0 { count.max(0) } else { 0 }
}

#[cfg(not(target_os = "linux"))]
fn pending_bytes(_: &File) -> i32 {
    0
}

/// `skip0(n)` of a regular file: the position moves by `n`, which may go past the end or, for a
/// negative `n`, back, and the distance moved is the answer.
fn file_skip(mut file: &File, n: i64) -> Result<i64, Failure> {
    let seek_error = |e: io::Error| Failure { class: "java.io.IOException", message: reason(&e) };
    let start = file.stream_position().map_err(seek_error)?;
    let end = file.seek(SeekFrom::Current(n)).map_err(seek_error)?;
    Ok(end as i64 - start as i64)
}

fn closed() -> Failure {
    Failure { class: "java.io.IOException", message: "Stream Closed".to_string() }
}

fn bad_descriptor() -> Failure {
    Failure { class: "java.io.IOException", message: "Bad file descriptor".to_string() }
}

fn io_failure(e: io::Error) -> Failure {
    Failure { class: "java.io.IOException", message: e.to_string() }
}

/// An `IOException` whose message is the reason alone, as the JVM's
/// `JNU_ThrowIOExceptionWithLastError` gives it.
pub(crate) fn last_error(e: io::Error) -> Failure {
    Failure { class: "java.io.IOException", message: reason(&e) }
}

/// The reason in an `io::Error` as C's `strerror` gives it: `No such file or directory`, not
/// `No such file or directory (os error 2)`.
pub(crate) fn reason(e: &io::Error) -> String {
    let text = e.to_string();
    match text.find(" (os error") {
        Some(end) => text[..end].to_string(),
        None => text,
    }
}

/// `FileNotFoundException` for a file that cannot be opened, with Java's message.
fn not_found(name: &str, e: &io::Error) -> Failure {
    Failure { class: "java.io.FileNotFoundException", message: format!("{name} ({})", reason(e)) }
}

impl Failure {
    /// The failure as the exception that the program may catch.
    pub fn exception(self) -> crate::Exception {
        crate::Exception::new(self.class, self.message)
    }
}

/// Raises the exception of a failed native, as an uncaught Java exception does.
pub fn or_throw<T>(result: Result<T, Failure>) -> T {
    result.unwrap_or_else(|f| throw(f.class, &f.message))
}

impl FdTable {
    pub fn new(stdin: Box<dyn BufRead + Send>) -> Self {
        let entries = vec![Some(Entry::Stdin), Some(Entry::Stdout), Some(Entry::Stderr)];
        FdTable { stdin: Some(stdin), entries }
    }

    /// The lowest free descriptor, as POSIX allocates them.
    fn insert(&mut self, entry: Entry) -> i32 {
        match self.entries.iter().position(Option::is_none) {
            Some(fd) => {
                self.entries[fd] = Some(entry);
                fd as i32
            }
            None => {
                self.entries.push(Some(entry));
                self.entries.len() as i32 - 1
            }
        }
    }

    /// `FileInputStream.open0`: the descriptor of a file to read.
    pub fn open_read(&mut self, name: &str) -> Result<i32, Failure> {
        if std::path::Path::new(name).is_dir() {
            return Err(Failure {
                class: "java.io.FileNotFoundException",
                message: format!("{name} (Is a directory)"),
            });
        }
        let file = File::open(name).map_err(|e| not_found(name, &e))?;
        Ok(self.insert(Entry::Read(file)))
    }

    /// `FileOutputStream.open0`: the descriptor of a file to write, created or truncated, or
    /// appended to.
    pub fn open_write(&mut self, name: &str, append: bool) -> Result<i32, Failure> {
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .append(append)
            .truncate(!append)
            .open(name)
            .map_err(|e| not_found(name, &e))?;
        Ok(self.insert(Entry::Write(file)))
    }

    /// `RandomAccessFile.open0(name, mode)`: the descriptor of a file to read, or with `O_RDWR`
    /// in `mode` to read and write, created if it does not exist. A directory cannot be opened,
    /// as the JVM's `handleOpen` refuses it. (`O_SYNC` and `O_DSYNC` only make each write wait for
    /// the disk, which no program can see.)
    pub fn open_random(&mut self, name: &str, mode: i32) -> Result<i32, Failure> {
        const O_RDWR: i32 = 2;
        let write = mode & O_RDWR != 0;
        let file = OpenOptions::new()
            .read(true)
            .write(write)
            .create(write)
            .truncate(false)
            .open(name)
            .map_err(|e| not_found(name, &e))?;
        if file.metadata().is_ok_and(|m| m.is_dir()) {
            return Err(Failure {
                class: "java.io.FileNotFoundException",
                message: format!("{name} (Is a directory)"),
            });
        }
        Ok(self.insert(if write { Entry::ReadWrite(file) } else { Entry::Read(file) }))
    }

    /// Reads once from an input descriptor into `bytes`, as one `read` system call does.
    fn read_into(&mut self, fd: i32, bytes: &mut [u8]) -> Result<usize, Failure> {
        let entry = self.entries.get_mut(fd as usize).and_then(Option::as_mut);
        let count = match entry {
            Some(Entry::Stdin) => {
                out::drain_stdout();
                match &mut self.stdin {
                    Some(given) => given.read(bytes),
                    None => process_stdin().read(bytes),
                }
            }
            Some(Entry::Read(file) | Entry::ReadWrite(file)) => file.read(bytes),
            Some(_) => return Err(bad_descriptor()),
            None => return Err(closed()),
        };
        count.map_err(io_failure)
    }

    /// `read0()`: the next byte as `0..=255`, or `-1` at the end.
    pub fn read(&mut self, fd: i32) -> Result<i32, Failure> {
        let mut byte = [0u8];
        match self.read_into(fd, &mut byte)? {
            0 => Ok(-1),
            _ => Ok(byte[0] as i32),
        }
    }

    /// `readBytes(b, off, len)`: fills `buf[off..off + len]` and returns the count, or `-1` at the
    /// end (when `len` is not 0).
    pub fn read_bytes(
        &mut self,
        fd: i32,
        buf: &mut (impl crate::Target<i8> + ?Sized),
        off: i32,
        len: i32,
    ) -> Result<i32, Failure> {
        if len == 0 {
            return Ok(0);
        }
        let (off, len) = (off as usize, len as usize);
        let mut bytes = vec![0u8; len];
        let count = self.read_into(fd, &mut bytes)?;
        if count == 0 {
            return Ok(-1);
        }
        let read: Vec<i8> = bytes[..count].iter().map(|&b| b as i8).collect();
        buf.store(off, &read);
        Ok(count as i32)
    }

    /// `available0()`: the bytes that can be read without waiting (`file_available`). For the
    /// input a test supplies, what its reader holds.
    pub fn available(&mut self, fd: i32) -> Result<i32, Failure> {
        if let Some(Some(Entry::Stdin)) = self.entries.get(fd as usize)
            && let Some(given) = &mut self.stdin
        {
            return Ok(given.fill_buf().map_err(io_failure)?.len().min(i32::MAX as usize) as i32);
        }
        self.with_file(fd, file_available)?.ok_or_else(bad_descriptor)?.map_err(io_failure)
    }

    /// `skip0(n)`, which `FileInputStream.skip` calls for a regular file only (`file_skip`). The
    /// input a test supplies skips up to `n` bytes of what it has.
    pub fn skip(&mut self, fd: i32, n: i64) -> Result<i64, Failure> {
        if let Some(Some(Entry::Stdin)) = self.entries.get(fd as usize)
            && let Some(given) = &mut self.stdin
        {
            let skipped = io::copy(&mut given.take(n.max(0) as u64), &mut io::sink());
            return Ok(skipped.map_err(io_failure)? as i64);
        }
        self.with_file(fd, |f| file_skip(f, n))?.ok_or_else(bad_descriptor)?
    }

    /// `f` of the open file behind a descriptor: `None` for the input a test supplies and for
    /// the standard output streams.
    fn with_file<T>(&self, fd: i32, f: impl FnOnce(&File) -> T) -> Result<Option<T>, Failure> {
        match self.entries.get(fd as usize).and_then(Option::as_ref) {
            Some(Entry::Stdin) if self.stdin.is_none() => Ok(Some(f(&process_stdin()))),
            Some(Entry::Read(file) | Entry::Write(file) | Entry::ReadWrite(file)) => Ok(Some(f(file))),
            Some(_) => Ok(None),
            None => Err(closed()),
        }
    }

    /// `isRegularFile0(fd)`: whether the descriptor is a regular file, which `FileInputStream`
    /// then skips and measures directly. A descriptor that is not open is not one.
    pub fn is_regular_file(&self, fd: i32) -> bool {
        matches!(self.with_file(fd, |f| f.metadata().is_ok_and(|m| m.is_file())), Ok(Some(true)))
    }

    /// `length0()`: the size of the file.
    pub fn length(&self, fd: i32) -> Result<i64, Failure> {
        let length = self.with_file(fd, |f| f.metadata().map(|m| m.len() as i64))?;
        length.unwrap_or(Ok(0)).map_err(io_failure)
    }

    /// `position0()`: the position in the file.
    pub fn position(&self, fd: i32) -> Result<i64, Failure> {
        let position = self.with_file(fd, |mut f| f.stream_position().map(|p| p as i64))?;
        position.unwrap_or(Ok(0)).map_err(io_failure)
    }

    /// `RandomAccessFile.seek0(pos)`: moves the position, which may go past the end.
    pub fn seek(&self, fd: i32, pos: i64) -> Result<(), Failure> {
        if pos < 0 {
            return Err(Failure {
                class: "java.io.IOException",
                message: "Negative seek offset".to_string(),
            });
        }
        let moved = self.with_file(fd, |mut f| f.seek(SeekFrom::Start(pos as u64)).map(|_| ()))?;
        moved.unwrap_or(Ok(())).map_err(last_error)
    }

    /// `RandomAccessFile.setLength0(n)`: truncates or extends the file to `n` bytes. The position
    /// stays where it was, or moves to the new end if that is before it.
    pub fn set_length(&self, fd: i32, length: i64) -> Result<(), Failure> {
        let set = self.with_file(fd, |mut f| {
            let position = f.stream_position()?;
            if length < 0 {
                return Err(io::Error::from_raw_os_error(22));
            }
            f.set_len(length as u64)?;
            if position > length as u64 {
                f.seek(SeekFrom::End(0))?;
            }
            Ok(())
        })?;
        set.unwrap_or(Ok(())).map_err(last_error)
    }

    /// `write(b)` and `writeBytes`: the bytes go to the descriptor's stream. Standard output is
    /// the runtime's buffered `out`.
    pub fn write(&mut self, fd: i32, bytes: &[u8]) -> Result<(), Failure> {
        match self.entries.get_mut(fd as usize).and_then(Option::as_mut) {
            Some(Entry::Stdout) => {
                out::raw_write(STDOUT_FD, bytes);
                Ok(())
            }
            Some(Entry::Stderr) => {
                out::raw_write(STDERR_FD, bytes);
                Ok(())
            }
            Some(Entry::Write(file) | Entry::ReadWrite(file)) => file.write_all(bytes).map_err(io_failure),
            Some(_) => Err(bad_descriptor()),
            None => Err(closed()),
        }
    }

    /// `close0()`. Closing a closed descriptor does nothing, as the Java classes allow.
    pub fn close(&mut self, fd: i32) {
        if let Some(slot) = self.entries.get_mut(fd as usize) {
            *slot = None;
        }
    }

    /// `FileDescriptor.getAppend(fd)`: whether the descriptor writes at the end of its file
    /// (`O_APPEND`), as the JVM's `fcntl(fd, F_GETFL)` tells. Standard output appends when the
    /// shell opened it with `>>`. For a descriptor that is not open `fcntl` fails with -1, whose
    /// bits include `O_APPEND`, so Java answers `true` there, and so does this. Where the flags
    /// cannot be read (no `/proc`, as outside Linux) the answer is `false`.
    pub fn append(&self, fd: i32) -> bool {
        let entry = if fd < 0 { None } else { self.entries.get(fd as usize).and_then(Option::as_ref) };
        match entry {
            Some(entry) => os_descriptor(entry).and_then(open_flags).is_some_and(|f| f & O_APPEND != 0),
            None => true,
        }
    }
}

/// Linux's `O_APPEND`.
const O_APPEND: u32 = 0o2000;

/// The operating system's descriptor behind an entry.
#[cfg(unix)]
fn os_descriptor(entry: &Entry) -> Option<i32> {
    use std::os::unix::io::AsRawFd;
    Some(match entry {
        Entry::Stdin => 0,
        Entry::Stdout => 1,
        Entry::Stderr => 2,
        Entry::Read(file) | Entry::Write(file) | Entry::ReadWrite(file) => file.as_raw_fd(),
        Entry::System(fd) => fd.as_raw_fd(),
    })
}

#[cfg(not(unix))]
fn os_descriptor(_: &Entry) -> Option<i32> {
    None
}

/// The flags an open descriptor of this process was opened with (`F_GETFL`), from Linux's
/// `/proc/self/fdinfo`, where they are written in octal.
fn open_flags(fd: i32) -> Option<u32> {
    let info = std::fs::read_to_string(format!("/proc/self/fdinfo/{fd}")).ok()?;
    let flags = info.lines().find_map(|line| line.strip_prefix("flags:"))?;
    u32::from_str_radix(flags.trim(), 8).ok()
}

fn with<T>(f: impl FnOnce(&mut FdTable) -> T) -> T {
    f(&mut FDS.lock().unwrap_or_else(|e| e.into_inner()))
}

/// A descriptor for `file`, which `sun.nio.fs`'s natives opened (`unix.rs`), to read, to write,
/// or both.
pub(crate) fn insert_file(file: File, read: bool, write: bool) -> i32 {
    let entry = match (read, write) {
        (true, true) => Entry::ReadWrite(file),
        (false, true) => Entry::Write(file),
        _ => Entry::Read(file),
    };
    with(|t| t.insert(entry))
}

/// `f` of the file that descriptor `fd` holds; `EBADF` where it holds none (closed, or a
/// standard stream).
pub(crate) fn with_file<T>(fd: i32, f: impl FnOnce(&File) -> io::Result<T>) -> io::Result<T> {
    with(|t| match t.entries.get(fd as usize).and_then(Option::as_ref) {
        Some(Entry::Read(file) | Entry::Write(file) | Entry::ReadWrite(file)) if fd >= 0 => f(file),
        _ => Err(io::Error::from_raw_os_error(EBADF)),
    })
}

/// `dup(fd)`: a new descriptor of the same open file.
pub(crate) fn duplicate(fd: i32) -> io::Result<i32> {
    with(|t| {
        let entry = match t.entries.get(fd as usize).and_then(Option::as_ref) {
            Some(Entry::Read(file)) if fd >= 0 => Entry::Read(file.try_clone()?),
            Some(Entry::Write(file)) if fd >= 0 => Entry::Write(file.try_clone()?),
            Some(Entry::ReadWrite(file)) if fd >= 0 => Entry::ReadWrite(file.try_clone()?),
            _ => return Err(io::Error::from_raw_os_error(EBADF)),
        };
        Ok(t.insert(entry))
    })
}

/// What a child process gets for one of its standard streams where the parent gives it its
/// descriptor `fd` (`process.rs`): the parent's own standard stream, or a copy of the file or
/// pipe that `fd` holds.
pub(crate) fn child_stdio(fd: i32) -> io::Result<std::process::Stdio> {
    use std::process::Stdio;
    with(|t| match t.entries.get(fd as usize).and_then(Option::as_ref) {
        Some(Entry::Stdin) if fd >= 0 => Ok(Stdio::inherit()),
        Some(Entry::Stdout) if fd >= 0 => Ok(Stdio::from(io::stdout())),
        Some(Entry::Stderr) if fd >= 0 => Ok(Stdio::from(io::stderr())),
        Some(Entry::Read(file) | Entry::Write(file) | Entry::ReadWrite(file)) if fd >= 0 => {
            Ok(Stdio::from(file.try_clone()?))
        }
        #[cfg(unix)]
        Some(Entry::System(system)) if fd >= 0 => Ok(Stdio::from(system.try_clone()?)),
        _ => Err(io::Error::from_raw_os_error(EBADF)),
    })
}

/// A descriptor for `fd`, a socket, pipe, `epoll` instance or event counter (`socket.rs`).
#[cfg(unix)]
pub(crate) fn insert_system(fd: std::os::fd::OwnedFd) -> i32 {
    with(|t| t.insert(Entry::System(fd)))
}

/// The system's descriptor behind descriptor `fd` of a socket (or another entry of
/// `insert_system`), which stays open until `close`; `EBADF` where it holds none.
#[cfg(unix)]
pub(crate) fn system_descriptor(fd: i32) -> io::Result<i32> {
    use std::os::unix::io::AsRawFd;
    with(|t| match t.entries.get(fd as usize).and_then(Option::as_ref) {
        Some(Entry::System(system)) if fd >= 0 => Ok(system.as_raw_fd()),
        _ => Err(io::Error::from_raw_os_error(EBADF)),
    })
}

/// `close(fd)` of the table's descriptor `fd`.
pub(crate) fn close_descriptor(fd: i32) {
    with(|t| t.close(fd));
}

/// Whether descriptor `fd` is open.
pub(crate) fn is_open(fd: i32) -> bool {
    with(|t| fd >= 0 && t.entries.get(fd as usize).is_some_and(Option::is_some))
}

const EBADF: i32 = 9;

/// The descriptor of a file to read, for `FileInputStream.open0`.
pub fn fd_open_read(name: &crate::Wtf8) -> Result<i32, Failure> {
    let name = &*name.to_string_lossy();
    with(|t| t.open_read(name))
}

/// The descriptor of a file to write, for `FileOutputStream.open0`.
pub fn fd_open_write(name: &crate::Wtf8, append: bool) -> Result<i32, Failure> {
    let name = &*name.to_string_lossy();
    with(|t| t.open_write(name, append))
}

/// The descriptor of a file to read, or to read and write, for `RandomAccessFile.open0`.
pub fn fd_open_random(name: &crate::Wtf8, mode: i32) -> Result<i32, Failure> {
    let name = &*name.to_string_lossy();
    with(|t| t.open_random(name, mode))
}

pub fn fd_seek(fd: i32, pos: i64) -> Result<(), Failure> {
    with(|t| t.seek(fd, pos))
}

pub fn fd_set_length(fd: i32, length: i64) -> Result<(), Failure> {
    with(|t| t.set_length(fd, length))
}

pub fn fd_read(fd: i32) -> Result<i32, Failure> {
    with(|t| t.read(fd))
}

pub fn fd_read_bytes(
    fd: i32,
    buf: &mut (impl crate::Target<i8> + ?Sized),
    off: i32,
    len: i32,
) -> Result<i32, Failure> {
    with(|t| t.read_bytes(fd, buf, off, len))
}

pub fn fd_available(fd: i32) -> Result<i32, Failure> {
    with(|t| t.available(fd))
}

pub fn fd_skip(fd: i32, n: i64) -> Result<i64, Failure> {
    with(|t| t.skip(fd, n))
}

pub fn fd_write(fd: i32, b: i32) -> Result<(), Failure> {
    with(|t| t.write(fd, &[b as u8]))
}

pub fn fd_write_bytes(
    fd: i32,
    buf: &(impl crate::Source<i8> + ?Sized),
    off: i32,
    len: i32,
) -> Result<(), Failure> {
    let bytes: Vec<u8> = buf.range(off as usize..(off + len) as usize).iter().map(|&b| b as u8).collect();
    with(|t| t.write(fd, &bytes))
}

pub fn fd_is_regular_file(fd: i32) -> bool {
    with(|t| t.is_regular_file(fd))
}

pub fn fd_length(fd: i32) -> Result<i64, Failure> {
    with(|t| t.length(fd))
}

pub fn fd_position(fd: i32) -> Result<i64, Failure> {
    with(|t| t.position(fd))
}

pub fn fd_close(fd: i32) {
    with(|t| t.close(fd))
}

/// `FileDescriptor.getAppend(fd)`, see `FdTable::append`.
pub fn fd_append(fd: i32) -> bool {
    with(|t| t.append(fd))
}

/// `FileDescriptor.getHandle(fd)`: the Windows handle of a descriptor, which on every other
/// system is -1.
pub fn fd_handle(_fd: i32) -> i64 {
    -1
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// A table whose stdin is `input`.
    fn table(input: &[u8]) -> FdTable {
        FdTable::new(Box::new(Cursor::new(input.to_vec())))
    }

    /// A file in a directory of this test's own, removed by the caller.
    fn temp_file(test: &str, name: &str) -> String {
        let dir = std::env::temp_dir().join(format!("java-runtime-fd-{test}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name).to_string_lossy().into_owned()
    }

    #[test]
    fn stdin_reads_bytes_then_end() {
        let mut t = table(b"hi");
        assert_eq!(t.read(STDIN_FD), Ok(b'h' as i32));
        assert_eq!(t.read(STDIN_FD), Ok(b'i' as i32));
        assert_eq!(t.read(STDIN_FD), Ok(-1));
    }

    #[test]
    fn stdin_read_bytes_fills_at_offset() {
        let mut t = table(&[1, 255, 3]);
        let mut buf = [9i8; 5];
        assert_eq!(t.read_bytes(STDIN_FD, &mut buf, 1, 4), Ok(3));
        assert_eq!(buf, [9, 1, -1, 3, 9]);
        assert_eq!(t.read_bytes(STDIN_FD, &mut buf, 0, 4), Ok(-1));
        assert_eq!(t.read_bytes(STDIN_FD, &mut buf, 0, 0), Ok(0));
    }

    #[test]
    fn stdin_skip_and_available() {
        let mut t = table(b"abcdef");
        assert_eq!(t.available(STDIN_FD), Ok(6));
        assert_eq!(t.skip(STDIN_FD, 4), Ok(4));
        assert_eq!(t.read(STDIN_FD), Ok(b'e' as i32));
        assert_eq!(t.skip(STDIN_FD, 10), Ok(1));
        assert_eq!(t.skip(STDIN_FD, -3), Ok(0));
    }

    #[test]
    fn file_written_then_read_back() {
        let name = temp_file("write_read", "data.bin");
        let mut t = table(b"");
        let w = t.open_write(&name, false).unwrap();
        assert_eq!(w, 3);
        t.write(w, b"abc").unwrap();
        t.write(w, &[0xff]).unwrap();
        t.close(w);

        let r = t.open_read(&name).unwrap();
        assert_eq!(r, 3, "the lowest free descriptor is reused");
        assert_eq!(t.available(r), Ok(4));
        assert_eq!(t.read(r), Ok(b'a' as i32));
        assert_eq!(t.skip(r, 1), Ok(1));
        let mut buf = [0i8; 4];
        assert_eq!(t.read_bytes(r, &mut buf, 0, 4), Ok(2));
        assert_eq!(&buf[..2], &[b'c' as i8, -1]);
        assert_eq!(t.read(r), Ok(-1));
        t.close(r);
    }

    #[test]
    fn append_keeps_the_contents() {
        let name = temp_file("append", "log.txt");
        let mut t = table(b"");
        let w = t.open_write(&name, false).unwrap();
        t.write(w, b"x").unwrap();
        t.close(w);
        let w = t.open_write(&name, true).unwrap();
        t.write(w, b"y").unwrap();
        t.close(w);
        assert_eq!(std::fs::read(&name).unwrap(), b"xy");
    }

    #[test]
    fn missing_file_is_file_not_found() {
        let name = temp_file("missing", "absent.txt");
        let mut t = table(b"");
        let error = t.open_read(&name).unwrap_err();
        assert_eq!(error.class, "java.io.FileNotFoundException");
        assert_eq!(error.message, format!("{name} (No such file or directory)"));
    }

    #[test]
    fn closed_descriptor_is_stream_closed() {
        let name = temp_file("closed", "once.txt");
        std::fs::write(&name, b"z").unwrap();
        let mut t = table(b"");
        let r = t.open_read(&name).unwrap();
        t.close(r);
        t.close(r);
        assert_eq!(t.read(r), Err(closed()));
        assert_eq!(t.read(42), Err(closed()));
    }

    #[test]
    fn append_is_the_mode_the_descriptor_was_opened_in() {
        let name = temp_file("append_mode", "mode.txt");
        let mut t = table(b"");
        let plain = t.open_write(&name, false).unwrap();
        let appending = t.open_write(&name, true).unwrap();
        let reading = t.open_read(&name).unwrap();
        if std::path::Path::new("/proc/self/fdinfo").is_dir() {
            assert!(t.append(appending));
        }
        assert!(!t.append(plain));
        assert!(!t.append(reading));
        t.close(appending);
        // `fcntl` fails on a descriptor that is not open, and Java reads that as `true`.
        assert!(t.append(appending));
        assert!(t.append(42));
        assert!(t.append(-1));
        assert_eq!(fd_handle(STDOUT_FD), -1);
    }

    #[test]
    fn random_access_reads_writes_and_seeks() {
        let name = temp_file("random", "random.bin");
        let _ = std::fs::remove_file(&name);
        let mut t = table(b"");
        let only_read = t.open_random(&name, 1).unwrap_err();
        assert_eq!(only_read.class, "java.io.FileNotFoundException");
        let rw = t.open_random(&name, 2).unwrap();
        t.write(rw, b"hello").unwrap();
        assert_eq!(t.position(rw), Ok(5));
        t.seek(rw, 1).unwrap();
        assert_eq!(t.read(rw), Ok(b'e' as i32));
        t.seek(rw, 8).unwrap();
        t.write(rw, b"!").unwrap();
        assert_eq!(t.length(rw), Ok(9));
        assert_eq!(t.seek(rw, -1).unwrap_err().message, "Negative seek offset");
        t.set_length(rw, 3).unwrap();
        assert_eq!(t.position(rw), Ok(3));
        assert_eq!(t.read(rw), Ok(-1));
        t.close(rw);
        let r = t.open_random(&name, 1).unwrap();
        assert_eq!(t.write(r, b"x"), Err(bad_descriptor()));
        t.close(r);
        let dir = std::path::Path::new(&name).parent().unwrap().to_string_lossy().into_owned();
        assert_eq!(t.open_random(&dir, 1).unwrap_err().message, format!("{dir} (Is a directory)"));
    }

    #[test]
    fn wrong_direction_is_bad_descriptor() {
        let mut t = table(b"");
        assert_eq!(t.write(STDIN_FD, b"x"), Err(bad_descriptor()));
        assert_eq!(t.read(STDOUT_FD), Err(bad_descriptor()));
    }
}
