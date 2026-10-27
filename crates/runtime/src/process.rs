//! The natives behind `java.lang.Process` on Linux (`ast::Native::Spawn`, `WaitForProcess`,
//! `DestroyProcess`): the JDK's `ProcessImpl` and `ProcessHandleImpl` are translated from its
//! sources, and only starting a process, waiting for one and signalling one are the runtime's.
//! A process starts through `std::process::Command`, and the ends of its pipes that the parent
//! keeps are descriptors of the table of `fd.rs`, which the JDK's `FileInputStream` and
//! `FileOutputStream` read and write.

use crate::fd::{self, Failure, child_stdio, insert_file};
use crate::{Source, Target};
use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt;
use std::process::{Command, Stdio};

unsafe extern "C" {
    fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
    fn waitid(idtype: i32, id: u32, info: *mut i32, options: i32) -> i32;
    fn kill(pid: i32, signal: i32) -> i32;
}

const SIGKILL: i32 = 9;
const SIGTERM: i32 = 15;
const EINTR: i32 = 4;
const ENOTDIR: i32 = 20;

/// The bytes of a C string that Java made (`ProcessImpl.toCString`), without its final NUL.
fn c_string(bytes: &[i8]) -> Vec<u8> {
    let bytes: Vec<u8> = bytes.iter().map(|&b| b as u8).collect();
    match bytes.iter().position(|&b| b == 0) {
        Some(end) => bytes[..end].to_vec(),
        None => bytes,
    }
}

/// The `count` strings of a block of NUL-terminated strings (`ProcessImpl`'s `argBlock`,
/// `ProcessEnvironment.toEnvironmentBlock`).
fn strings(block: &[i8], count: i32) -> Vec<Vec<u8>> {
    let bytes: Vec<u8> = block.iter().map(|&b| b as u8).collect();
    bytes.split(|&b| b == 0).take(count.max(0) as usize).map(<[u8]>::to_vec).collect()
}

/// `ProcessImpl.forkAndExec`, as the translator's patch of `ProcessImpl` calls it: starts `prog`
/// with the arguments of `args` (`argc` of them, after the program) in `dir` (empty for the
/// parent's) and the environment of `env` (`envc` entries `name=value`; -1 for the parent's).
/// `fds` holds, for standard input, output and error, -1 for a pipe to the parent, or the
/// parent's descriptor that the child gets; on return, the parent's end of each pipe, or -1
/// where there is none. With `redirect_error`, standard error goes where standard output does.
/// The answer is the process's id. A program that cannot start is an `IOException` with the
/// message of the JDK's launcher on Linux.
#[allow(clippy::too_many_arguments)]
pub fn spawn_process(
    prog: &(impl Source<i8> + ?Sized),
    args: &(impl Source<i8> + ?Sized),
    argc: i32,
    env: &(impl Source<i8> + ?Sized),
    envc: i32,
    dir: &(impl Source<i8> + ?Sized),
    fds: &mut (impl Target<i32> + ?Sized),
    redirect_error: bool,
) -> Result<i32, Failure> {
    let failure = |e: io::Error| launch_failure("Exec failed", &e);
    let mut command = Command::new(OsStr::from_bytes(&c_string(&prog.whole())));
    for arg in strings(&args.whole(), argc) {
        command.arg(OsStr::from_bytes(&arg));
    }
    if envc >= 0 {
        command.env_clear();
        for entry in strings(&env.whole(), envc) {
            // As the JDK's block, an entry without `=` is left out.
            if let Some(eq) = entry.iter().position(|&b| b == b'=') {
                command.env(OsStr::from_bytes(&entry[..eq]), OsStr::from_bytes(&entry[eq + 1..]));
            }
        }
    }
    let dir = c_string(&dir.whole());
    if !dir.is_empty() {
        // The JDK's launcher reports a directory it cannot enter apart from a program it cannot
        // run.
        let entered = std::fs::metadata(OsStr::from_bytes(&dir))
            .and_then(|m| if m.is_dir() { Ok(()) } else { Err(io::Error::from_raw_os_error(ENOTDIR)) });
        if let Err(e) = entered {
            return Err(launch_failure("Failed to access working directory", &e));
        }
        command.current_dir(OsStr::from_bytes(&dir));
    }
    let given = fds.whole().into_owned();
    let stdio = |fd: i32| if fd == -1 { Ok(Stdio::piped()) } else { child_stdio(fd) };
    command.stdin(stdio(given[0]).map_err(failure)?);
    // Standard error into the pipe of standard output takes a pipe of our own, whose writing end
    // both streams get.
    let mut merged = None;
    if redirect_error && given[1] == -1 {
        let (reader, writer) = io::pipe().map_err(failure)?;
        command.stderr(writer.try_clone().map_err(failure)?);
        command.stdout(writer);
        merged = Some(reader);
    } else {
        command.stdout(stdio(given[1]).map_err(failure)?);
        command
            .stderr(if redirect_error { child_stdio(given[1]) } else { stdio(given[2]) }.map_err(failure)?);
    }
    // What the program printed comes before what the child prints to the same streams.
    crate::out::flush();
    let mut child = command.spawn().map_err(failure)?;
    // The command holds the writing end of the merged pipe, which must close for the parent to
    // see the end of the stream.
    drop(command);
    let kept = |end: Option<OwnedFd>, write: bool| match end {
        Some(end) => insert_file(File::from(end), !write, write),
        None => -1,
    };
    let stdin = kept(child.stdin.take().map(OwnedFd::from), true);
    let stdout = match merged {
        Some(reader) => kept(Some(OwnedFd::from(reader)), false),
        None => kept(child.stdout.take().map(OwnedFd::from), false),
    };
    let stderr = kept(child.stderr.take().map(OwnedFd::from), false);
    fds.store(0, &[stdin, stdout, stderr]);
    // `waitForProcessExit0` waits for it by its id; a `Child` dropped neither waits nor kills.
    Ok(child.id() as i32)
}

/// The `IOException` of the JDK's launcher on Linux for what failed, `what`, with its error.
fn launch_failure(what: &str, e: &io::Error) -> Failure {
    Failure {
        class: "java.io.IOException",
        message: format!("{what}, error: {} ({}) ", e.raw_os_error().unwrap_or(0), fd::reason(e)),
    }
}

/// `ProcessHandleImpl.waitForProcessExit0(pid, reap)`: waits for process `pid` to end, and its
/// exit status as the JDK gives it: the status it exited with, or 0x80 plus the signal that
/// ended it. A process that is not a child of this one gives 0, as in the JDK. Without `reap`
/// the process stays to be waited for again.
pub fn wait_for_process_exit(pid: i64, reap: bool) -> i32 {
    loop {
        let waited = if reap { reaped(pid as i32) } else { exited(pid as i32) };
        match waited {
            Ok(status) => return status,
            Err(e) if e.raw_os_error() == Some(EINTR) => continue,
            Err(_) => return 0,
        }
    }
}

/// `waitpid(pid)`: the exit status of child `pid` once it ends, which then is gone.
fn reaped(pid: i32) -> io::Result<i32> {
    let mut status = 0;
    // Safety: `waitpid` writes one `int` to the address it is given.
    if unsafe { waitpid(pid, &mut status, 0) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(if status & 0x7f == 0 { (status >> 8) & 0xff } else { 0x80 + (status & 0x7f) })
}

/// `waitid(P_PID, pid, WEXITED | WNOWAIT)`: the exit status of child `pid` once it ends, which
/// stays to be waited for.
fn exited(pid: i32) -> io::Result<i32> {
    const P_PID: i32 = 1;
    const WEXITED: i32 = 4;
    const WNOWAIT: i32 = 0x0100_0000;
    const CLD_EXITED: i32 = 1;
    // A `siginfo_t`: 128 bytes, with `si_code` at byte 8 and `si_status` at byte 24 on Linux.
    let mut info = [0i32; 32];
    // Safety: `waitid` writes one `siginfo_t` to the address it is given.
    if unsafe { waitid(P_PID, pid as u32, info.as_mut_ptr(), WEXITED | WNOWAIT) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(if info[2] == CLD_EXITED { info[6] } else { 0x80 + info[6] })
}

/// `ProcessHandleImpl.destroy0(pid, startTime, force)`: sends process `pid` `SIGKILL` (`force`)
/// or `SIGTERM`, if it is the process that started at `start_time` (or that time is unknown, 0),
/// and whether the signal was sent.
pub fn destroy_process(pid: i64, start_time: i64, force: bool) -> bool {
    if start_time != 0 && crate::vm::process_start_time(pid) != start_time {
        return false;
    }
    // Safety: `kill` takes plain integers.
    unsafe { kill(pid as i32, if force { SIGKILL } else { SIGTERM }) == 0 }
}
