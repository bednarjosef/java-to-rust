//! The file descriptors behind the natives of `java.io.FileInputStream`, `FileOutputStream` and
//! `FileDescriptor` (`ast::Native::File*`). A descriptor is an index into a table of the process, as
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
fn file_available(file: &File) -> io::Result<i32> {
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

/// The reason in an `io::Error` as C's `strerror` gives it: `No such file or directory`, not
/// `No such file or directory (os error 2)`.
fn reason(e: &io::Error) -> String {
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
            Some(Entry::Read(file)) => file.read(bytes),
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
    pub fn read_bytes(&mut self, fd: i32, buf: &mut [i8], off: i32, len: i32) -> Result<i32, Failure> {
        if len == 0 {
            return Ok(0);
        }
        let (off, len) = (off as usize, len as usize);
        let mut bytes = vec![0u8; len];
        let count = self.read_into(fd, &mut bytes)?;
        if count == 0 {
            return Ok(-1);
        }
        for (dest, &byte) in buf[off..off + count].iter_mut().zip(&bytes) {
            *dest = byte as i8;
        }
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
            Some(Entry::Read(file) | Entry::Write(file)) => Ok(Some(f(file))),
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

    /// `write(b)` and `writeBytes`: the bytes go to the descriptor's stream. Standard output is
    /// the runtime's buffered `out`.
    pub fn write(&mut self, fd: i32, bytes: &[u8]) -> Result<(), Failure> {
        match self.entries.get_mut(fd as usize).and_then(Option::as_mut) {
            Some(Entry::Stdout) => Ok(out::raw_write(STDOUT_FD, bytes)),
            Some(Entry::Stderr) => Ok(out::raw_write(STDERR_FD, bytes)),
            Some(Entry::Write(file)) => file.write_all(bytes).map_err(io_failure),
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
        Entry::Read(file) | Entry::Write(file) => file.as_raw_fd(),
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

/// The descriptor of a file to read, for `FileInputStream.open0`.
pub fn fd_open_read(name: &str) -> Result<i32, Failure> {
    with(|t| t.open_read(name))
}

/// The descriptor of a file to write, for `FileOutputStream.open0`.
pub fn fd_open_write(name: &str, append: bool) -> Result<i32, Failure> {
    with(|t| t.open_write(name, append))
}

pub fn fd_read(fd: i32) -> Result<i32, Failure> {
    with(|t| t.read(fd))
}

pub fn fd_read_bytes(fd: i32, buf: &mut [i8], off: i32, len: i32) -> Result<i32, Failure> {
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

pub fn fd_write_bytes(fd: i32, buf: &[i8], off: i32, len: i32) -> Result<(), Failure> {
    let bytes: Vec<u8> = buf[off as usize..(off + len) as usize].iter().map(|&b| b as u8).collect();
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
    fn wrong_direction_is_bad_descriptor() {
        let mut t = table(b"");
        assert_eq!(t.write(STDIN_FD, b"x"), Err(bad_descriptor()));
        assert_eq!(t.read(STDOUT_FD), Err(bad_descriptor()));
    }
}
