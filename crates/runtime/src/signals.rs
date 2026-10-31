//! `SIGTERM`, `SIGINT` and `SIGHUP`, which end a Java program as `System.exit` does: the JDK's
//! `Terminator` handles them with `Shutdown.exit(128 + signal)`, so the shutdown hooks run and the
//! status is 143 for `SIGTERM`. A program that adds a hook (`Thread.installShutdown0()`) gets the
//! handlers; one without hooks ends as the signal ends any process, which Java's does no
//! differently but for what `System.out` holds, which a program without threads loses here.
//! `SIGHUP` keeps its action where it is ignored, as under `nohup`, as Java leaves it.
//!
//! The handler only writes the signal's number into a pipe. A thread of the runtime waits on the
//! pipe and, for the first signal, starts the Java thread that exits (`SIGTERM handler`, as the
//! JDK names it); a later signal finds the program already exiting and changes nothing, as the
//! JDK's `Shutdown.exit` holds the second caller.

use std::sync::Once;
use std::sync::atomic::{AtomicI32, Ordering};

#[repr(C)]
struct SigAction {
    handler: usize,
    mask: [u64; 16],
    flags: i32,
    restorer: usize,
}

unsafe extern "C" {
    fn sigaction(sig: i32, act: *const SigAction, old: *mut SigAction) -> i32;
    fn sigemptyset(set: *mut [u64; 16]) -> i32;
    fn pipe2(fds: *mut i32, flags: i32) -> i32;
    fn read(fd: i32, buf: *mut std::ffi::c_void, len: usize) -> isize;
    fn write(fd: i32, buf: *const std::ffi::c_void, len: usize) -> isize;
}

const SIGHUP: i32 = 1;
const SIGINT: i32 = 2;
const SIGTERM: i32 = 15;
const SIG_IGN: usize = 1;
const SA_RESTART: i32 = 0x1000_0000;
const O_CLOEXEC: i32 = 0o2000000;

/// The end of the pipe that the handler writes to.
static WRITE_END: AtomicI32 = AtomicI32::new(-1);

/// Installs the handlers, once.
pub(crate) fn install() {
    static INSTALLED: Once = Once::new();
    INSTALLED.call_once(|| {
        let mut fds = [0; 2];
        if unsafe { pipe2(fds.as_mut_ptr(), O_CLOEXEC) } != 0 {
            return;
        }
        WRITE_END.store(fds[1], Ordering::Relaxed);
        let read_end = fds[0];
        // No std name: std keeps a thread's name where the collector does not look (`gc::keep_spans`).
        if std::thread::Builder::new().spawn(move || wait(read_end)).is_err() {
            return;
        }
        for sig in [SIGTERM, SIGINT, SIGHUP] {
            let mut action = SigAction {
                handler: handler as *const () as usize,
                mask: [0; 16],
                flags: SA_RESTART,
                restorer: 0,
            };
            let mut old = SigAction { handler: 0, mask: [0; 16], flags: 0, restorer: 0 };
            unsafe {
                sigemptyset(&mut action.mask);
                sigaction(sig, &action, &mut old);
                if sig == SIGHUP && old.handler == SIG_IGN {
                    sigaction(sig, &old, std::ptr::null_mut());
                }
            }
        }
    });
}

extern "C" fn handler(sig: i32) {
    let byte = sig as u8;
    unsafe { write(WRITE_END.load(Ordering::Relaxed), (&raw const byte).cast(), 1) };
}

/// Waits for the signals, and ends the program on the first.
fn wait(read_end: i32) {
    let mut exiting = false;
    loop {
        let mut byte = 0u8;
        let n = unsafe { read(read_end, (&raw mut byte).cast(), 1) };
        if n == 1 && !exiting {
            exiting = true;
            let sig = byte as i32;
            let name = match sig {
                SIGHUP => "HUP",
                SIGINT => "INT",
                _ => "TERM",
            };
            let tid = crate::thread::next_thread_id();
            crate::thread::start_thread(tid, format!("SIG{name} handler"), true, move || {
                crate::exit(128 + sig)
            });
        } else if n == 0 {
            return;
        }
    }
}
