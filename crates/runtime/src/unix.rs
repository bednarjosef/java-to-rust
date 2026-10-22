//! The system calls of `sun.nio.fs.UnixNativeDispatcher`, which `java.nio.file` makes its files,
//! directories and attributes with (`ast::Native::Unix`). The translator's dispatcher
//! (`java/patches/sun/nio/fs/UnixNativeDispatcher.java`) gives each path as the bytes of its
//! `UnixPath`, where the JVM's copies them to memory of its own first. None of these raises an
//! exception: each one records the `errno` of its failure, or 0, and the dispatcher reads it with
//! `unix_last_error` and throws the JDK's `UnixException` with it.
//!
//! The descriptors that `unix_open` gives are those of `fd.rs`, which the streams of `java.io`
//! use too, and the handles of directories that `unix_opendir` gives are indexes into a table of
//! their own.

use crate::fd;
use std::cell::Cell;
use std::ffi::{CString, OsStr};
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

thread_local! {
    /// The `errno` of this thread's last call, 0 where it succeeded.
    static LAST_ERROR: Cell<i32> = const { Cell::new(0) };
}

/// `EIO`, for a failure that has no `errno`.
const EIO: i32 = 5;
const EBADF: i32 = 9;
const AT_FDCWD: i32 = -100;
/// `AT_SYMLINK_NOFOLLOW`: the call acts on a symbolic link, not on what it points to.
const AT_SYMLINK_NOFOLLOW: i32 = 0x100;
/// The bits of `open`'s flags that say whether it reads, writes or both.
const O_ACCMODE: i32 = 3;

/// The directories that `unix_opendir` opened, by handle - 1; `None` once closed.
static DIRECTORIES: Mutex<Vec<Option<fs::ReadDir>>> = Mutex::new(Vec::new());

unsafe extern "C" {
    fn open(path: *const std::ffi::c_char, flags: i32, ...) -> i32;
    fn access(path: *const std::ffi::c_char, mode: i32) -> i32;
    fn utimensat(dirfd: i32, path: *const std::ffi::c_char, times: *const Timespec, flags: i32) -> i32;
    fn futimens(fd: i32, times: *const Timespec) -> i32;
    fn strerror(errnum: i32) -> *const std::ffi::c_char;
    fn getpwuid(uid: u32) -> *const *const std::ffi::c_char;
    fn getgrgid(gid: u32) -> *const *const std::ffi::c_char;
}

/// C's `struct timespec` on 64-bit Linux.
#[repr(C)]
struct Timespec {
    tv_sec: i64,
    tv_nsec: i64,
}

/// The value of a call that succeeded, or `failed` with the call's `errno` recorded.
fn record<T>(result: io::Result<T>, failed: T) -> T {
    match result {
        Ok(value) => {
            LAST_ERROR.with(|e| e.set(0));
            value
        }
        Err(e) => {
            LAST_ERROR.with(|last| last.set(e.raw_os_error().unwrap_or(EIO)));
            failed
        }
    }
}

/// The C library's `errno` after a call that failed.
fn errno() -> io::Error {
    io::Error::last_os_error()
}

/// A path that the dispatcher gives as its bytes.
fn path(bytes: &[i8]) -> &OsStr {
    // Safety: `i8` and `u8` have the same size and alignment.
    OsStr::from_bytes(unsafe { std::slice::from_raw_parts(bytes.as_ptr().cast::<u8>(), bytes.len()) })
}

fn c_path(bytes: &[i8]) -> io::Result<CString> {
    CString::new(path(bytes).as_bytes()).map_err(|_| io::Error::from_raw_os_error(2))
}

fn to_bytes(text: &[u8]) -> Vec<i8> {
    text.iter().map(|&b| b as i8).collect()
}

/// `errno` of this thread's last call of the dispatcher's natives, 0 where it succeeded.
pub fn unix_last_error() -> i32 {
    LAST_ERROR.with(Cell::get)
}

/// `getcwd()`: the working directory.
pub fn unix_getcwd() -> Vec<i8> {
    record(std::env::current_dir().map(|d| to_bytes(d.as_os_str().as_bytes())), Vec::new())
}

/// `dup(fd)`.
pub fn unix_dup(fd: i32) -> i32 {
    record(fd::duplicate(fd), -1)
}

/// `open(path, flags, mode)`: a descriptor of `fd.rs`'s table for the file, with exactly the
/// flags the JDK asks for (`O_CREAT`, `O_EXCL`, `O_TRUNC`, `O_APPEND`, `O_NOFOLLOW`, ...).
pub fn unix_open(path: &[i8], flags: i32, mode: i32) -> i32 {
    let opened = c_path(path).and_then(|p| {
        // Safety: the path is a valid C string that outlives the call.
        let os = unsafe { open(p.as_ptr(), flags, mode as std::ffi::c_uint) };
        if os < 0 {
            return Err(errno());
        }
        // Safety: `os` is a descriptor this call opened, which the file now owns.
        let file = unsafe { File::from_raw_fd(os) };
        let access = flags & O_ACCMODE;
        Ok(fd::insert_file(file, access != 1, access != 0))
    });
    record(opened, -1)
}

/// `close(fd)`.
pub fn unix_close(fd: i32) {
    let closed = if fd::is_open(fd) {
        fd::fd_close(fd);
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(EBADF))
    };
    record(closed, ())
}

/// `link(existing, new)`.
pub fn unix_link(existing: &[i8], new: &[i8]) {
    record(fs::hard_link(path(existing), path(new)), ())
}

/// `unlink(path)`, which removes anything but a directory.
pub fn unix_unlink(p: &[i8]) {
    record(fs::remove_file(path(p)), ())
}

/// `rename(from, to)`.
pub fn unix_rename(from: &[i8], to: &[i8]) {
    record(fs::rename(path(from), path(to)), ())
}

/// `mkdir(path, mode)`.
pub fn unix_mkdir(p: &[i8], mode: i32) {
    record(fs::DirBuilder::new().mode(mode as u32).create(path(p)), ())
}

/// `rmdir(path)`.
pub fn unix_rmdir(p: &[i8]) {
    record(fs::remove_dir(path(p)), ())
}

/// `readlink(path)`: what a symbolic link points to.
pub fn unix_readlink(p: &[i8]) -> Vec<i8> {
    record(fs::read_link(path(p)).map(|target| to_bytes(target.as_os_str().as_bytes())), Vec::new())
}

/// `realpath(path)`: the absolute path with every symbolic link, `.` and `..` resolved.
pub fn unix_realpath(p: &[i8]) -> Vec<i8> {
    record(fs::canonicalize(path(p)).map(|real| to_bytes(real.as_os_str().as_bytes())), Vec::new())
}

/// `symlink(target, link)`.
pub fn unix_symlink(target: &[i8], link: &[i8]) {
    record(std::os::unix::fs::symlink(path(target), path(link)), ())
}

/// The fields of a `stat` as `UnixFileAttributes` keeps them, in this order: mode, inode,
/// device, device of a special file, links, user, group, size, then the seconds and nanoseconds
/// of the last access, the last change of the contents and the last change of the attributes,
/// and of the creation, with 1 after them where the system knows when the file was created.
fn stat_fields(m: &fs::Metadata, out: &mut [i64]) {
    let created = m.created().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok());
    let fields = [
        m.mode() as i64,
        m.ino() as i64,
        m.dev() as i64,
        m.rdev() as i64,
        m.nlink() as i64,
        m.uid() as i64,
        m.gid() as i64,
        m.size() as i64,
        m.atime(),
        m.atime_nsec(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
        created.map_or(0, |d| d.as_secs() as i64),
        created.map_or(0, |d| d.subsec_nanos() as i64),
        created.is_some() as i64,
    ];
    out[..fields.len()].copy_from_slice(&fields);
}

/// `stat(path)`, or `lstat(path)` where `flag` has `AT_SYMLINK_NOFOLLOW`, into `out` (see
/// `stat_fields`).
pub fn unix_stat(p: &[i8], flag: i32, out: &mut [i64]) {
    let p = path(p);
    let meta = if flag & AT_SYMLINK_NOFOLLOW != 0 { fs::symlink_metadata(p) } else { fs::metadata(p) };
    record(meta.map(|m| stat_fields(&m, out)), ())
}

/// `fstat(fd)` into `out` (see `stat_fields`).
pub fn unix_fstat(fd: i32, out: &mut [i64]) {
    record(fd::with_file(fd, File::metadata).map(|m| stat_fields(&m, out)), ())
}

/// `chown(path, uid, gid)`, or `lchown` where `flag` has `AT_SYMLINK_NOFOLLOW`.
pub fn unix_chown(p: &[i8], uid: i32, gid: i32, flag: i32) {
    let (uid, gid) = (Some(uid as u32), Some(gid as u32));
    let changed = if flag & AT_SYMLINK_NOFOLLOW != 0 {
        std::os::unix::fs::lchown(path(p), uid, gid)
    } else {
        std::os::unix::fs::chown(path(p), uid, gid)
    };
    record(changed, ())
}

/// `fchown(fd, uid, gid)`.
pub fn unix_fchown(fd: i32, uid: i32, gid: i32) {
    record(fd::with_file(fd, |f| std::os::unix::fs::fchown(f, Some(uid as u32), Some(gid as u32))), ())
}

/// `chmod(path, mode)`.
pub fn unix_chmod(p: &[i8], mode: i32) {
    record(fs::set_permissions(path(p), fs::Permissions::from_mode(mode as u32)), ())
}

/// `fchmod(fd, mode)`.
pub fn unix_fchmod(fd: i32, mode: i32) {
    record(fd::with_file(fd, |f| f.set_permissions(fs::Permissions::from_mode(mode as u32))), ())
}

fn times(access: i64, modified: i64) -> [Timespec; 2] {
    let at = |nanos: i64| Timespec {
        tv_sec: nanos.div_euclid(1_000_000_000),
        tv_nsec: nanos.rem_euclid(1_000_000_000),
    };
    [at(access), at(modified)]
}

/// `futimens(fd, times)`, with the times in nanoseconds since 1970.
pub fn unix_futimens(fd: i32, access: i64, modified: i64) {
    let times = times(access, modified);
    let changed = fd::with_file(fd, |f| {
        // Safety: `times` holds the two `timespec`s the call reads.
        if unsafe { futimens(f.as_raw_fd(), times.as_ptr()) } == 0 { Ok(()) } else { Err(errno()) }
    });
    record(changed, ())
}

/// `utimensat(AT_FDCWD, path, times, flag)`, with the times in nanoseconds since 1970.
pub fn unix_utimensat(p: &[i8], access: i64, modified: i64, flag: i32) {
    let times = times(access, modified);
    let changed = c_path(p).and_then(|p| {
        // Safety: the path is a valid C string and `times` holds two `timespec`s.
        if unsafe { utimensat(AT_FDCWD, p.as_ptr(), times.as_ptr(), flag) } == 0 {
            Ok(())
        } else {
            Err(errno())
        }
    });
    record(changed, ())
}

/// `opendir(path)`: a handle of the directory, which `unix_readdir` reads; 0 where it fails.
pub fn unix_opendir(p: &[i8]) -> i64 {
    let opened = fs::read_dir(path(p)).map(|entries| {
        let mut dirs = DIRECTORIES.lock().unwrap_or_else(|e| e.into_inner());
        dirs.push(Some(entries));
        dirs.len() as i64
    });
    record(opened, 0)
}

/// `closedir(dir)`.
pub fn unix_closedir(dir: i64) {
    let mut dirs = DIRECTORIES.lock().unwrap_or_else(|e| e.into_inner());
    let closed = match dirs.get_mut((dir - 1) as usize) {
        Some(slot @ Some(_)) if dir > 0 => {
            *slot = None;
            Ok(())
        }
        _ => Err(io::Error::from_raw_os_error(EBADF)),
    };
    record(closed, ())
}

/// `readdir(dir)`: the name of the next entry, in the order the system gives them, without `.`
/// and `..`; empty after the last one.
pub fn unix_readdir(dir: i64) -> Vec<i8> {
    let mut dirs = DIRECTORIES.lock().unwrap_or_else(|e| e.into_inner());
    let next = match dirs.get_mut((dir - 1) as usize).and_then(Option::as_mut) {
        Some(entries) if dir > 0 => match entries.next() {
            Some(Ok(entry)) => Ok(to_bytes(entry.file_name().as_bytes())),
            Some(Err(e)) => Err(e),
            None => Ok(Vec::new()),
        },
        _ => Err(io::Error::from_raw_os_error(EBADF)),
    };
    record(next, Vec::new())
}

/// `access(path, mode)`: 0 where the process may, else the `errno` of why not.
pub fn unix_access(p: &[i8], mode: i32) -> i32 {
    let allowed = c_path(p).and_then(|p| {
        // Safety: the path is a valid C string that outlives the call.
        if unsafe { access(p.as_ptr(), mode) } == 0 { Ok(()) } else { Err(errno()) }
    });
    record(allowed, ());
    unix_last_error()
}

/// The first field, a C string, of what `getpwuid` or `getgrgid` found, which is the name.
fn first_name(found: *const *const std::ffi::c_char) -> io::Result<Vec<i8>> {
    if found.is_null() {
        // No such user or group: the JDK's natives answer with an `errno` of 0 then.
        return Err(io::Error::from_raw_os_error(0));
    }
    // Safety: the record's first field, `pw_name` or `gr_name`, is a C string.
    let name = unsafe { std::ffi::CStr::from_ptr(*found) };
    Ok(to_bytes(name.to_bytes()))
}

/// `getpwuid(uid)->pw_name`: the name of a user.
pub fn unix_getpwuid(uid: i32) -> Vec<i8> {
    // Safety: `getpwuid` returns null or a record that stays valid until the next call.
    record(first_name(unsafe { getpwuid(uid as u32) }), Vec::new())
}

/// `getgrgid(gid)->gr_name`: the name of a group.
pub fn unix_getgrgid(gid: i32) -> Vec<i8> {
    // Safety: `getgrgid` returns null or a record that stays valid until the next call.
    record(first_name(unsafe { getgrgid(gid as u32) }), Vec::new())
}

/// `strerror(errnum)`: the message of an `errno`.
pub fn unix_strerror(errnum: i32) -> Vec<i8> {
    // Safety: `strerror` returns a C string that stays valid until the next call.
    let text = unsafe { std::ffi::CStr::from_ptr(strerror(errnum)) };
    to_bytes(text.to_bytes())
}

/// Reads up to `len` bytes into `buf[off..]` from descriptor `fd`, at its position (moving it)
/// or, where `position` is not negative, at that position (leaving it): the count, or -1 at the
/// end of the file.
pub fn unix_read(fd: i32, buf: &mut [i8], off: i32, len: i32, position: i64) -> i32 {
    let read = fd::with_file(fd, |mut f| {
        let mut bytes = vec![0u8; len as usize];
        let n = if position < 0 {
            f.read(&mut bytes)?
        } else {
            std::os::unix::fs::FileExt::read_at(f, &mut bytes, position as u64)?
        };
        for (to, from) in buf[off as usize..].iter_mut().zip(&bytes[..n]) {
            *to = *from as i8;
        }
        Ok(if n == 0 && len > 0 { -1 } else { n as i32 })
    });
    record(read, -1)
}

/// Writes `buf[off..off + len]` to descriptor `fd`, at its position (moving it) or, where
/// `position` is not negative, at that position (leaving it): the count written.
pub fn unix_write(fd: i32, buf: &[i8], off: i32, len: i32, position: i64) -> i32 {
    let bytes: Vec<u8> = buf[off as usize..(off + len) as usize].iter().map(|&b| b as u8).collect();
    let written = fd::with_file(fd, |mut f| {
        let n = if position < 0 {
            f.write(&bytes)?
        } else {
            std::os::unix::fs::FileExt::write_at(f, &bytes, position as u64)?
        };
        Ok(n as i32)
    });
    record(written, -1)
}

/// The position of descriptor `fd`, or -1.
pub fn unix_position(fd: i32) -> i64 {
    record(fd::with_file(fd, |mut f| f.stream_position()).map(|p| p as i64), -1)
}

/// Moves the position of descriptor `fd` to `position`.
pub fn unix_seek(fd: i32, position: i64) {
    record(fd::with_file(fd, |mut f| f.seek(SeekFrom::Start(position as u64))).map(|_| ()), ())
}

/// The size of the file of descriptor `fd`, or -1.
pub fn unix_size(fd: i32) -> i64 {
    record(fd::with_file(fd, File::metadata).map(|m| m.len() as i64), -1)
}

/// `ftruncate(fd, size)`.
pub fn unix_truncate(fd: i32, size: i64) {
    record(fd::with_file(fd, |f| f.set_len(size as u64)), ())
}

/// `fsync(fd)`, or `fdatasync(fd)` where `metadata` is false.
pub fn unix_sync(fd: i32, metadata: bool) {
    record(fd::with_file(fd, |f| if metadata { f.sync_all() } else { f.sync_data() }), ())
}

/// `statvfs(path)` into `out`: `f_frsize`, `f_blocks`, `f_bfree` and `f_bavail`, the four fields
/// `UnixFileStoreAttributes` keeps.
pub fn unix_statvfs(p: &[i8], out: &mut [i64]) {
    let path = CString::new(p.iter().map(|&b| b as u8).collect::<Vec<u8>>()).unwrap_or_default();
    // `struct statvfs` on 64-bit Linux (glibc and musl): eleven words, then six spare ints. Its
    // fields are `f_bsize, f_frsize, f_blocks, f_bfree, f_bavail, ...`, read by index.
    let mut buf = [0u64; 14];
    let result = match unsafe { statvfs(path.as_ptr(), &mut buf) } {
        0 => Ok(()),
        _ => Err(errno()),
    };
    record(
        result.map(|()| {
            let fields = [buf[1], buf[2], buf[3], buf[4]];
            for (slot, field) in out.iter_mut().zip(fields) {
                *slot = field as i64;
            }
        }),
        (),
    );
}

unsafe extern "C" {
    fn statvfs(path: *const std::ffi::c_char, buf: *mut [u64; 14]) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(text: &str) -> Vec<i8> {
        to_bytes(text.as_bytes())
    }

    #[test]
    fn files_directories_and_attributes() {
        let dir = std::env::temp_dir().join(format!("java-runtime-unix-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let dir = bytes(dir.to_str().unwrap());
        unix_mkdir(&dir, 0o755);
        assert_eq!(unix_last_error(), 0);
        unix_mkdir(&dir, 0o755);
        assert_eq!(unix_last_error(), 17, "EEXIST");
        let file = [dir.clone(), bytes("/a.txt")].concat();
        const O_RDWR: i32 = 2;
        const O_CREAT: i32 = 0o100;
        const O_EXCL: i32 = 0o200;
        let fd = unix_open(&file, O_RDWR | O_CREAT | O_EXCL, 0o644);
        assert!(fd >= 3);
        assert_eq!(unix_open(&file, O_RDWR | O_CREAT | O_EXCL, 0o644), -1);
        assert_eq!(unix_last_error(), 17);
        assert_eq!(unix_write(fd, &bytes("hello"), 0, 5, -1), 5);
        assert_eq!(unix_size(fd), 5);
        assert_eq!(unix_position(fd), 5);
        unix_seek(fd, 1);
        let mut buf = [0i8; 8];
        assert_eq!(unix_read(fd, &mut buf, 2, 3, -1), 3);
        assert_eq!(&buf[2..5], &bytes("ell")[..]);
        assert_eq!(unix_read(fd, &mut buf, 0, 8, 4), 1);
        assert_eq!(unix_read(fd, &mut buf, 0, 8, -1), 1);
        assert_eq!(unix_read(fd, &mut buf, 0, 8, -1), -1);
        let mut stat = [0i64; 17];
        unix_fstat(fd, &mut stat);
        assert_eq!(stat[7], 5);
        unix_close(fd);
        unix_close(fd);
        assert_eq!(unix_last_error(), EBADF);
        unix_stat(&file, 0, &mut stat);
        assert_eq!((stat[0] & 0o170000, stat[0] & 0o777, stat[7]), (0o100000, 0o644, 5));
        unix_utimensat(&file, 1_000_000_000_500, 2_000_000_000_250, 0);
        unix_stat(&file, 0, &mut stat);
        assert_eq!((stat[8], stat[9], stat[10], stat[11]), (1000, 500, 2000, 250));
        assert_eq!(unix_access(&file, 4), 0);
        assert_eq!(unix_access(&[dir.clone(), bytes("/none")].concat(), 0), 2);
        let mut vfs = [0i64; 4];
        unix_statvfs(&dir, &mut vfs);
        assert_eq!(unix_last_error(), 0);
        assert!(vfs[0] > 0 && vfs[2] <= vfs[1], "block size and blocks: {vfs:?}");
        unix_statvfs(&[dir.clone(), bytes("/none")].concat(), &mut vfs);
        assert_eq!(unix_last_error(), 2, "ENOENT");
        let handle = unix_opendir(&dir);
        assert!(handle > 0);
        assert_eq!(unix_readdir(handle), bytes("a.txt"));
        assert_eq!(unix_readdir(handle), Vec::<i8>::new());
        assert_eq!(unix_last_error(), 0);
        unix_closedir(handle);
        unix_rmdir(&dir);
        assert_eq!(unix_last_error(), 39, "ENOTEMPTY");
        unix_unlink(&file);
        unix_rmdir(&dir);
        assert_eq!(unix_last_error(), 0);
        assert_eq!(unix_strerror(2), bytes("No such file or directory"));
    }
}
