//! `System.out` and `System.err`.
//!
//! Java's `System.out` is a `PrintStream` with automatic flushing over a `BufferedOutputStream`
//! of 128 bytes over descriptor 1, and `System.err` the same over descriptor 2. Each stream here
//! (`Stream`) keeps the two stages apart: `buffered` is what the `BufferedOutputStream` holds and
//! `written` what it has passed on to the descriptor. Whatever prints text (`print`, `println`,
//! `printf` and the `PrintStream` methods that write an array) flushes the buffer, as Java's
//! does, so its bytes and everything before them are written. `write(int)` of anything but a
//! newline only fills the buffer, which a later flush passes on or which is lost when the program
//! ends, and lost here too. The translated `PrintStream` of a program that uses `System.out` as a
//! value (`java/overlay/java/io/StandardOutput.java` in the translator) comes down to the same
//! streams through `standard_write`, `standard_write_bytes`, `standard_flush` and
//! `standard_close`, so the two ways of printing stay in order.
//!
//! What standard output has written is held in memory rather than written at once, in a program
//! without threads. Rust's
//! `println!` locks standard output and, because standard output is line buffered even when it is
//! a file or a pipe, makes one `write` system call per line. Java's `System.out` also writes on
//! every `println`, but a translated program has no reason to: what it prints is the same bytes in
//! the same order either way. So `written` goes out in 8 KiB writes. A translated program
//! printing two million lines takes 0.10 s instead of 0.36 s with `println!` when its output goes
//! to a file, and 0.09 s instead of 0.61 s into a pipe (Java: 0.58 s and 0.85 s). It goes out
//! when it is full, when the program ends (`main` returns, `System.exit`, `Runtime.halt` or an
//! uncaught exception), before anything goes to standard error so that a terminal shows the two
//! streams in program order, before the program reads standard input so that a prompt shows, on
//! `System.out.flush()`, and before a panic message. Standard error writes what it passes on at
//! once. Write errors are ignored, as Java's `PrintStream` ignores them (it only sets
//! `checkError`).
//!
//! Standard output lives in a thread-local whose destructor writes it out when the main thread
//! ends; glibc runs those destructors on `exit`, and the corpus test checks every program's
//! output. Once the program starts a thread, the stream is the process's (`SHARED`) instead,
//! which writes what it passes on at once, as Java does: a program with threads, such as a
//! server, may go on for as long as it likes after it prints, or be killed, and what it printed
//! is out by then. Each `print` or `println` collects its text in its `Out` and appends it whole
//! when the `Out` is dropped, at the end of the statement, so that two threads' lines never
//! interleave inside a line, as `PrintStream`'s synchronized methods guarantee. A `toString` that prints while its
//! object is being printed comes out first, as in Java, which converts the object before it
//! locks the stream.

use std::cell::RefCell;
use std::io::{self, Write};
use std::sync::{Mutex, MutexGuard};

/// How many written bytes standard output holds before it writes them out.
const CAPACITY: usize = 8192;

/// The size of the `BufferedOutputStream` that Java's `System.out` and `System.err` write through.
const BUFFER: usize = 128;

/// One standard stream: descriptor 1 or 2.
struct Stream {
    fd: i32,
    /// What the stream's `BufferedOutputStream` has passed on to the descriptor and the runtime
    /// has not written yet.
    written: Vec<u8>,
    /// What the `BufferedOutputStream` holds.
    buffered: Vec<u8>,
    /// Whether the program has closed the stream (`System.out.close()`): what it prints after
    /// that goes nowhere, as Java's closed `PrintStream` drops it.
    closed: bool,
    /// Whether what the stream passes on is written at once, rather than once it has 8 KiB.
    at_once: bool,
}

impl Stream {
    const fn new(fd: i32) -> Stream {
        Stream { fd, written: Vec::new(), buffered: Vec::new(), closed: false, at_once: false }
    }

    const fn at_once(fd: i32) -> Stream {
        Stream { fd, written: Vec::new(), buffered: Vec::new(), closed: false, at_once: true }
    }

    /// Writes out what the stream has written.
    fn drain(&mut self) {
        if self.written.is_empty() {
            return;
        }
        if self.fd == 1 {
            let mut stdout = io::stdout().lock();
            let _ = stdout.write_all(&self.written);
            let _ = stdout.flush();
        } else {
            let _ = io::stderr().write_all(&self.written);
        }
        self.written.clear();
    }

    /// Standard error writes at once, after what standard output has written; so does the
    /// standard output of a program with threads.
    fn settle(&mut self) {
        if self.fd == 2 {
            drain_stdout();
            self.drain();
        } else if self.at_once || self.written.len() >= CAPACITY {
            self.drain();
        }
    }

    /// `BufferedOutputStream.flushBuffer`: the buffer goes on to the descriptor.
    fn pass(&mut self) {
        self.written.append(&mut self.buffered);
    }

    /// Text that `PrintStream` prints, which it flushes after: the buffer and the text are
    /// written. Nothing is, not even the buffer, for no text.
    fn text(&mut self, bytes: &[u8]) {
        if self.closed || bytes.is_empty() {
            return;
        }
        self.pass();
        self.written.extend_from_slice(bytes);
        self.settle();
    }

    /// `PrintStream.write(int)`: the byte goes into the buffer, which a newline flushes.
    fn byte(&mut self, b: i32) {
        if self.closed {
            return;
        }
        self.buffer_byte(b as u8);
        if b == '\n' as i32 {
            self.pass();
        }
        self.settle();
    }

    /// `BufferedOutputStream.write(int)`.
    fn buffer_byte(&mut self, b: u8) {
        if self.buffered.len() >= BUFFER {
            self.pass();
        }
        self.buffered.push(b);
    }

    /// `BufferedOutputStream.write(bytes, off, len)`: an array as large as the buffer goes on at
    /// once, after what the buffer holds; a smaller one joins the buffer, which is passed on
    /// first if the array does not fit.
    fn buffer_bytes(&mut self, bytes: &[u8]) {
        if bytes.len() >= BUFFER {
            self.pass();
            self.written.extend_from_slice(bytes);
        } else {
            if bytes.len() > BUFFER - self.buffered.len() {
                self.pass();
            }
            self.buffered.extend_from_slice(bytes);
        }
    }

    /// `BufferedOutputStream.close()`, which flushes before it closes its descriptor.
    fn close(&mut self) {
        if !self.closed {
            self.pass();
            self.drain();
            self.closed = true;
        }
    }
}

impl Drop for Stream {
    /// What was written goes out when the main thread ends; what is still buffered is lost.
    fn drop(&mut self) {
        self.drain();
    }
}

thread_local! {
    static OUT: RefCell<Stream> = {
        install_panic_flush();
        let mut s = Stream::new(1);
        s.written.reserve(CAPACITY);
        RefCell::new(s)
    };
}

/// Standard output of a program that has started a thread, which every thread prints to.
static SHARED: Mutex<Stream> = Mutex::new(Stream::at_once(1));

/// Standard error.
static ERR: Mutex<Stream> = Mutex::new(Stream::new(2));

fn lock(stream: &'static Mutex<Stream>) -> MutexGuard<'static, Stream> {
    stream.lock().unwrap_or_else(|e| e.into_inner())
}

/// Runs `f` on standard output: the main thread's until the program starts a thread, the
/// process's after.
fn with_stdout<T>(f: impl FnOnce(&mut Stream) -> T) -> T {
    if crate::thread::threads_started() { f(&mut lock(&SHARED)) } else { OUT.with_borrow_mut(f) }
}

/// Runs `f` on descriptor 1 or 2.
fn with_stream<T>(fd: i32, f: impl FnOnce(&mut Stream) -> T) -> T {
    if fd == 2 { f(&mut lock(&ERR)) } else { with_stdout(f) }
}

/// Makes standard output the process's, when the program starts its first thread: what the main
/// thread wrote so far goes out first, and the buffer and whether it is closed go over.
pub(crate) fn share_output() {
    let _ = OUT.try_with(|s| {
        if let Ok(mut s) = s.try_borrow_mut() {
            s.drain();
            let mut shared = lock(&SHARED);
            shared.buffered.append(&mut s.buffered);
            shared.closed |= s.closed;
        }
    });
}

/// A panic is a bug in the translation, not Java behavior, but what the program printed before
/// it should still come out, ahead of the panic message. The hook is the process's, installed
/// once, by the first thread that prints (a panicking thread cannot install one).
fn install_panic_flush() {
    static INSTALLED: std::sync::Once = std::sync::Once::new();
    if std::thread::panicking() {
        return;
    }
    INSTALLED.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = OUT.try_with(|s| s.try_borrow_mut().map(|mut s| s.drain()));
            if let Ok(mut s) = SHARED.try_lock() {
                s.drain();
            }
            previous(info);
        }));
    });
}

/// Writes out what standard output has written, and leaves its buffer as it is: before standard
/// error, before reading standard input, and when the program ends, where Java loses the buffer.
pub fn drain_stdout() {
    let _ = OUT.try_with(|s| s.try_borrow_mut().map(|mut s| s.drain()));
    if crate::thread::threads_started() {
        lock(&SHARED).drain();
    }
}

/// `System.out` or `System.err` as a handle for `write!` and `writeln!`: what one `print` or
/// `println` prints, which it flushes after. Standard output in a program without threads takes
/// the text as it comes; otherwise the handle holds it until it is dropped.
pub struct Out {
    fd: i32,
    pending: Vec<u8>,
}

/// `System.out`.
pub fn out() -> Out {
    Out { fd: 1, pending: Vec::new() }
}

/// `System.err`.
pub fn err() -> Out {
    Out { fd: 2, pending: Vec::new() }
}

impl Write for Out {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.fd == 1 && !crate::thread::threads_started() {
            OUT.with_borrow_mut(|s| s.text(bytes));
        } else {
            self.pending.extend_from_slice(bytes);
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for Out {
    fn drop(&mut self) {
        if !self.pending.is_empty() {
            with_stream(self.fd, |s| s.text(&self.pending));
        }
    }
}

/// `System.out.flush()`: the buffer is passed on, and what was written goes out.
pub fn flush() {
    with_stdout(|s| {
        s.pass();
        s.drain();
    });
}

/// `System.err.flush()`.
pub fn err_flush() {
    with_stream(2, |s| {
        s.pass();
        s.settle();
    });
}

/// `System.out.write(b)`: the low eight bits of `b`, as one byte.
pub fn write_byte(b: i32) {
    with_stdout(|s| s.byte(b));
}

/// `write(b)` of the stream on descriptor `fd` (1 or 2) under a translated `PrintStream`: its
/// `BufferedOutputStream`'s.
pub fn standard_write(fd: i32, b: i32) {
    with_stream(fd, |s| {
        if !s.closed {
            s.buffer_byte(b as u8);
            s.settle();
        }
    });
}

/// `write(bytes, off, len)` of the stream on descriptor `fd`, whose bounds the caller checked.
pub fn standard_write_bytes(fd: i32, bytes: &(impl crate::Source<i8> + ?Sized), off: i32, len: i32) {
    let bytes: Vec<u8> = bytes.range(off as usize..(off + len) as usize).iter().map(|&b| b as u8).collect();
    with_stream(fd, |s| {
        if !s.closed {
            s.buffer_bytes(&bytes);
            s.settle();
        }
    });
}

/// `flush()` of the stream on descriptor `fd`.
pub fn standard_flush(fd: i32) {
    with_stream(fd, |s| {
        s.pass();
        s.settle();
    });
}

/// `close()` of the stream on descriptor `fd`: it is flushed, and prints nothing after.
pub fn standard_close(fd: i32) {
    if fd == 2 {
        drain_stdout();
    }
    with_stream(fd, Stream::close);
}

/// Bytes that a `FileOutputStream` of the program writes to descriptor 1 or 2 itself, around
/// `System.out`'s buffer.
pub(crate) fn raw_write(fd: i32, bytes: &[u8]) {
    with_stream(fd, |s| {
        s.written.extend_from_slice(bytes);
        s.settle();
    });
}

/// Text that the runtime prints to standard error, such as the message of an uncaught exception,
/// which Java prints through `System.err`.
pub(crate) fn eprint_text(text: &str) {
    drain_stdout();
    with_stream(2, |s| s.text(text.as_bytes()));
}

/// The UTF-16 units as text, a lone surrogate as `?` the way Java's UTF-8 output writes it.
fn chars_text(chars: &[u16]) -> String {
    char::decode_utf16(chars.iter().copied()).map(|c| c.unwrap_or('?')).collect()
}

/// `System.out.print(char[])`.
pub fn print_chars(chars: &[u16]) {
    let _ = out().write_all(chars_text(chars).as_bytes());
}

/// `System.out.println(char[])`.
pub fn println_chars(chars: &[u16]) {
    let mut text = chars_text(chars);
    text.push('\n');
    let _ = out().write_all(text.as_bytes());
}

/// `System.err.print(char[])`.
pub fn eprint_chars(chars: &[u16]) {
    let _ = err().write_all(chars_text(chars).as_bytes());
}

/// `System.err.println(char[])`.
pub fn eprintln_chars(chars: &[u16]) {
    let mut text = chars_text(chars);
    text.push('\n');
    let _ = err().write_all(text.as_bytes());
}

/// `System.out.println(...)`; used as `java::println!` with `println!`'s arguments.
#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {{
        use ::std::io::Write as _;
        let _ = ::std::writeln!($crate::out(), $($arg)*);
    }};
}

/// `System.out.print(...)`; used as `java::print!`.
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        use ::std::io::Write as _;
        let _ = ::std::write!($crate::out(), $($arg)*);
    }};
}

/// `System.err.println(...)`.
#[macro_export]
macro_rules! eprintln {
    ($($arg:tt)*) => {{
        use ::std::io::Write as _;
        let _ = ::std::writeln!($crate::err(), $($arg)*);
    }};
}

/// `System.err.print(...)`.
#[macro_export]
macro_rules! eprint {
    ($($arg:tt)*) => {{
        use ::std::io::Write as _;
        let _ = ::std::write!($crate::err(), $($arg)*);
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream() -> Stream {
        // A descriptor that `drain` never writes to: the tests look at the stages only.
        Stream::new(1)
    }

    #[test]
    fn a_byte_waits_in_the_buffer_until_a_newline() {
        let mut s = stream();
        s.buffer_byte(b'a');
        assert_eq!((s.written.as_slice(), s.buffered.as_slice()), (&b""[..], &b"a"[..]));
        s.buffer_byte(b'\n');
        s.pass();
        assert_eq!((s.written.as_slice(), s.buffered.as_slice()), (&b"a\n"[..], &b""[..]));
        s.written.clear();
    }

    #[test]
    fn the_buffer_passes_on_when_full() {
        let mut s = stream();
        for _ in 0..BUFFER {
            s.buffer_byte(b'x');
        }
        assert!(s.written.is_empty());
        s.buffer_byte(b'y');
        assert_eq!(s.written.len(), BUFFER);
        assert_eq!(s.buffered, b"y");
        s.written.clear();
    }

    #[test]
    fn arrays_join_the_buffer_or_go_past_it() {
        let mut s = stream();
        s.buffer_bytes(b"ab");
        s.buffer_bytes(&[b'c'; BUFFER - 2]);
        assert!(s.written.is_empty());
        s.buffer_bytes(b"d");
        assert_eq!(s.written.len(), BUFFER);
        assert_eq!(s.buffered, b"d");
        s.buffer_bytes(&[b'e'; BUFFER]);
        assert_eq!(s.written.len(), 2 * BUFFER + 1);
        assert!(s.buffered.is_empty());
        s.written.clear();
    }

    #[test]
    fn the_output_of_a_program_with_threads_goes_out_at_once() {
        // A newline, which is all this writes to the test's standard output.
        let mut s = Stream::at_once(1);
        s.text(b"\n");
        assert!(s.written.is_empty());
        let mut s = stream();
        s.text(b"\n");
        assert_eq!(s.written, b"\n");
        s.written.clear();
    }

    #[test]
    fn text_flushes_and_empty_text_does_not() {
        let mut s = stream();
        s.buffer_byte(b'a');
        s.text(b"");
        assert_eq!(s.buffered, b"a");
        s.text(b"bc");
        assert_eq!(s.written, b"abc");
        assert!(s.buffered.is_empty());
        s.written.clear();
    }

    #[test]
    fn a_closed_stream_prints_nothing() {
        let mut s = stream();
        s.closed = true;
        s.text(b"lost");
        s.byte(b'\n' as i32);
        assert!(s.written.is_empty() && s.buffered.is_empty());
    }
}
