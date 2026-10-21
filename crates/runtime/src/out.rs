//! `System.out` and `System.err`.
//!
//! Rust's `println!` locks standard output and, because standard output is line buffered even
//! when it is a file or a pipe, makes one `write` system call per line. Java's `System.out` also
//! flushes on every `println`, but a translated program has no reason to: what it prints is the
//! same bytes in the same order either way. So the generated code prints through `java::println!`
//! and `java::print!`, which fill one buffer that goes out in 8 KiB writes. A translated program
//! printing two million lines takes 0.10 s instead of 0.36 s with `println!` when its output goes
//! to a file, and 0.09 s instead of 0.61 s into a pipe (Java: 0.58 s and 0.85 s).
//!
//! The buffer lives in a thread-local whose destructor writes it out when the main thread ends;
//! glibc runs those destructors on `exit`, and the corpus test checks every program's output.
//!
//! The buffer is written out when it is full, when the program ends (`main` returns, `System.exit`
//! or an uncaught exception), before anything goes to standard error so that a terminal shows the
//! two streams in program order, on `System.out.flush()`, and before a panic message. Write
//! errors are ignored, as Java's `PrintStream` ignores them (it only sets `checkError`).

use std::cell::RefCell;
use std::io::{self, Write};

/// How many bytes the buffer holds before it is written out.
const CAPACITY: usize = 8192;

/// The bytes printed and not yet written; dropping it, which happens when the main thread ends,
/// writes them.
struct Buffer(Vec<u8>);

impl Buffer {
    fn drain(&mut self) {
        if self.0.is_empty() {
            return;
        }
        let mut stdout = io::stdout().lock();
        let _ = stdout.write_all(&self.0);
        let _ = stdout.flush();
        self.0.clear();
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        self.drain();
    }
}

thread_local! {
    static OUT: RefCell<Buffer> = {
        install_panic_flush();
        RefCell::new(Buffer(Vec::with_capacity(CAPACITY)))
    };
}

/// A panic is a bug in the translation, not Java behavior, but what the program printed before
/// it should still come out, ahead of the panic message.
fn install_panic_flush() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = OUT.try_with(|b| b.try_borrow_mut().map(|mut b| b.drain()));
        previous(info);
    }));
}

/// `System.out`: a handle on the buffered standard output, for `write!` and `writeln!`.
pub struct Out;

/// `System.out`.
pub fn out() -> Out {
    Out
}

impl Write for Out {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        OUT.with_borrow_mut(|b| {
            b.0.extend_from_slice(bytes);
            if b.0.len() >= CAPACITY {
                b.drain();
            }
        });
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        flush();
        Ok(())
    }
}

/// `System.out.flush()`: writes out what has been printed.
pub fn flush() {
    let _ = OUT.try_with(|b| b.borrow_mut().drain());
}

/// `System.out.write(b)`: the low eight bits of `b`, as one byte.
pub fn write_byte(b: i32) {
    let _ = out().write_all(&[b as u8]);
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
    print_chars(chars);
    let _ = out().write_all(b"\n");
}

/// `System.err.print(char[])`: what `System.out` printed before goes out first.
pub fn eprint_chars(chars: &[u16]) {
    flush();
    let _ = io::stderr().write_all(chars_text(chars).as_bytes());
}

/// `System.err.println(char[])`.
pub fn eprintln_chars(chars: &[u16]) {
    eprint_chars(chars);
    let _ = io::stderr().write_all(b"\n");
}

/// `System.out.println(...)`, buffered; used as `java::println!` with `println!`'s arguments.
#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {{
        use ::std::io::Write as _;
        let _ = ::std::writeln!($crate::out(), $($arg)*);
    }};
}

/// `System.out.print(...)`, buffered; used as `java::print!`.
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        use ::std::io::Write as _;
        let _ = ::std::write!($crate::out(), $($arg)*);
    }};
}

/// `System.err.println(...)`: standard error is not buffered, and what `System.out` printed
/// before goes out first.
#[macro_export]
macro_rules! eprintln {
    ($($arg:tt)*) => {{
        use ::std::io::Write as _;
        $crate::flush();
        let _ = ::std::writeln!(::std::io::stderr(), $($arg)*);
    }};
}

/// `System.err.print(...)`.
#[macro_export]
macro_rules! eprint {
    ($($arg:tt)*) => {{
        use ::std::io::Write as _;
        $crate::flush();
        let _ = ::std::write!(::std::io::stderr(), $($arg)*);
    }};
}
