//! The natives of `java.io.UnixFileSystem`, which `java.io.File` asks about files and directories
//! (`ast::Native::FileSystem`), on `std::fs`. Each takes the path that the `File` holds. As the
//! JVM's do, the questions answer 0 or `false` where the file cannot be read, and only
//! `createFileExclusively0` and `canonicalize0` throw.

use crate::fd::{Failure, last_error};
use std::fs;
use std::io;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

/// `FileSystem.BA_EXISTS`, `BA_REGULAR` and `BA_DIRECTORY`.
const BA_EXISTS: i32 = 0x01;
const BA_REGULAR: i32 = 0x02;
const BA_DIRECTORY: i32 = 0x04;

/// `FileSystem.ACCESS_READ`, `ACCESS_WRITE` and `ACCESS_EXECUTE`.
const ACCESS_READ: i32 = 0x04;
const ACCESS_WRITE: i32 = 0x02;
const ACCESS_EXECUTE: i32 = 0x01;

/// `getBooleanAttributes0(f)`: whether the file exists, and whether it is a regular file or a
/// directory, following symbolic links.
pub fn fs_attributes(path: &crate::Wtf8) -> i32 {
    let path = &*path.to_string_lossy();
    match fs::metadata(path) {
        Ok(m) if m.is_dir() => BA_EXISTS | BA_DIRECTORY,
        Ok(m) if m.is_file() => BA_EXISTS | BA_REGULAR,
        Ok(_) => BA_EXISTS,
        Err(_) => 0,
    }
}

/// The C string of a path, for the calls `std` has no counterpart of.
fn c_path(path: &str) -> Option<std::ffi::CString> {
    std::ffi::CString::new(path).ok()
}

unsafe extern "C" {
    fn access(path: *const std::ffi::c_char, mode: i32) -> i32;
    fn pathconf(path: *const std::ffi::c_char, name: i32) -> std::ffi::c_long;
}

/// `checkAccess0(f, access)`: whether this process may read, write or run the file, as
/// `access(2)` tells.
pub fn fs_access(path: &crate::Wtf8, mode: i32) -> bool {
    let path = &*path.to_string_lossy();
    let mut bits = 0;
    if mode & ACCESS_READ != 0 {
        bits |= 4;
    }
    if mode & ACCESS_WRITE != 0 {
        bits |= 2;
    }
    if mode & ACCESS_EXECUTE != 0 {
        bits |= 1;
    }
    // Safety: the path is a valid C string that outlives the call.
    c_path(path).is_some_and(|p| unsafe { access(p.as_ptr(), bits) } == 0)
}

/// `getLastModifiedTime0(f)`: the time the file was last changed, in milliseconds since 1970, or
/// 0 if it cannot be read.
pub fn fs_modified(path: &crate::Wtf8) -> i64 {
    let path = &*path.to_string_lossy();
    fs::metadata(path).map(|m| m.mtime() * 1000 + m.mtime_nsec() / 1_000_000).unwrap_or(0)
}

/// `getLength0(f)`: the size of the file in bytes, or 0 if it cannot be read.
pub fn fs_length(path: &crate::Wtf8) -> i64 {
    let path = &*path.to_string_lossy();
    fs::metadata(path).map(|m| m.len() as i64).unwrap_or(0)
}

/// `setPermission0(f, access, enable, ownerOnly)`: turns the owner's bit for `access` on or off,
/// and everyone's unless `owner_only`.
pub fn fs_set_permission(path: &crate::Wtf8, access: i32, enable: bool, owner_only: bool) -> bool {
    let path = &*path.to_string_lossy();
    let owner = match access {
        ACCESS_READ => 0o400,
        ACCESS_WRITE => 0o200,
        ACCESS_EXECUTE => 0o100,
        _ => return false,
    };
    let bits = if owner_only { owner } else { owner | owner >> 3 | owner >> 6 };
    change_mode(path, |mode| if enable { mode | bits } else { mode & !bits })
}

/// `setReadOnly0(f)`: takes away everyone's write permission.
pub fn fs_set_read_only(path: &crate::Wtf8) -> bool {
    let path = &*path.to_string_lossy();
    change_mode(path, |mode| mode & !0o222)
}

fn change_mode(path: &str, change: impl FnOnce(u32) -> u32) -> bool {
    let Ok(meta) = fs::metadata(path) else { return false };
    let mode = change(meta.permissions().mode() & 0o7777);
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).is_ok()
}

/// `createFileExclusively0(path)`: creates an empty file, `false` if something is there already.
/// Any other failure is an `IOException`, as is the JVM's. The root is never created.
pub fn fs_create_file(path: &crate::Wtf8) -> Result<bool, Failure> {
    let path = &*path.to_string_lossy();
    if path == "/" {
        return Ok(false);
    }
    match fs::OpenOptions::new().read(true).write(true).create_new(true).open(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(e) => Err(last_error(e)),
    }
}

/// `delete0(f)`: removes a file or an empty directory, as C's `remove` does.
pub fn fs_delete(path: &crate::Wtf8) -> bool {
    let path = &*path.to_string_lossy();
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_dir() => fs::remove_dir(path).is_ok(),
        Ok(_) => fs::remove_file(path).is_ok(),
        Err(_) => false,
    }
}

/// `list0(f)`: the names in a directory, without `.` and `..`, in the order the system gives
/// them (`readdir`), or `None` (Java's `null`) if it cannot be read.
pub fn fs_list(path: &crate::Wtf8) -> Option<Vec<crate::JString>> {
    let path = &*path.to_string_lossy();
    let mut names = Vec::new();
    for entry in fs::read_dir(path).ok()? {
        names.push(entry.ok()?.file_name().to_string_lossy().into_owned().into());
    }
    Some(names)
}

/// `createDirectory0(f)`: makes one directory, whose parent must exist.
pub fn fs_create_directory(path: &crate::Wtf8) -> bool {
    let path = &*path.to_string_lossy();
    fs::create_dir(path).is_ok()
}

/// `rename0(f1, f2)`.
pub fn fs_rename(from: &crate::Wtf8, to: &crate::Wtf8) -> bool {
    let from = &*from.to_string_lossy();
    let to = &*to.to_string_lossy();
    fs::rename(from, to).is_ok()
}

/// `setLastModifiedTime0(f, time)`: sets the time the file was last changed, in milliseconds
/// since 1970, and leaves the time it was last read.
pub fn fs_set_modified(path: &crate::Wtf8, millis: i64) -> bool {
    let path = &*path.to_string_lossy();
    let Ok(file) = fs::File::open(path) else { return false };
    let time = if millis >= 0 {
        std::time::UNIX_EPOCH + std::time::Duration::from_millis(millis as u64)
    } else {
        std::time::UNIX_EPOCH - std::time::Duration::from_millis(millis.unsigned_abs())
    };
    file.set_modified(time).is_ok()
}

/// `getNameMax0(path)`: the longest name a file in the directory may have
/// (`pathconf(path, _PC_NAME_MAX)`), or 255 where the system does not say.
pub fn fs_name_max(path: &crate::Wtf8) -> i64 {
    let path = &*path.to_string_lossy();
    const PC_NAME_MAX: i32 = 3;
    // Safety: the path is a valid C string that outlives the call.
    let max = c_path(path).map_or(-1, |p| unsafe { pathconf(p.as_ptr(), PC_NAME_MAX) });
    // `c_long` is `i64` only on 64-bit systems.
    #[allow(clippy::useless_conversion)]
    if max < 0 { 255 } else { i64::from(max) }
}

/// `getSpace0(path, t)`: the size of the file system that holds the file in bytes
/// (`FileSystem.SPACE_TOTAL`, 0), its free bytes (`SPACE_FREE`, 1), or the free bytes that this
/// user may take (`SPACE_USABLE`, 2), each `statvfs`'s count of blocks of `f_frsize` bytes; 0
/// where `statvfs` fails.
pub fn fs_space(path: &crate::Wtf8, t: i32) -> i64 {
    let path = &*path.to_string_lossy();
    let Ok([block, blocks, free, usable]) = crate::unix::file_system_space(path.as_bytes()) else { return 0 };
    let count = match t {
        0 => blocks,
        1 => free,
        2 => usable,
        _ => return 0,
    };
    block.wrapping_mul(count) as i64
}

/// `canonicalize0(path)` of an absolute path: the path with its symbolic links resolved and `.`
/// and `..` taken out. As the JVM's `JDK_Canonicalize` does, a path that does not exist resolves
/// its longest part that does and keeps the rest as written.
pub fn fs_canonicalize(path: &crate::Wtf8) -> Result<crate::JString, Failure> {
    canonical(&path.to_string_lossy()).map(crate::JString::from)
}

fn canonical(path: &str) -> Result<String, Failure> {
    if let Ok(real) = fs::canonicalize(path) {
        return Ok(real.to_string_lossy().into_owned());
    }
    let collapsed = collapse(Path::new(path));
    let mut existing = collapsed.clone();
    let mut rest = Vec::new();
    loop {
        if let Ok(real) = fs::canonicalize(&existing) {
            let mut result = real;
            for part in rest.iter().rev() {
                result.push(part);
            }
            return Ok(result.to_string_lossy().into_owned());
        }
        match (existing.file_name().map(|n| n.to_os_string()), existing.parent()) {
            (Some(name), Some(parent)) => {
                rest.push(name);
                existing = parent.to_path_buf();
            }
            _ => return Ok(collapsed.to_string_lossy().into_owned()),
        }
    }
}

/// The path without `.` and with each `..` taking out the name before it.
fn collapse(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(test: &str) -> String {
        let dir = std::env::temp_dir().join(format!("java-runtime-fs-{test}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir.to_string_lossy().into_owned()
    }

    #[test]
    fn attributes_lengths_and_listing() {
        let dir = temp_dir("attributes");
        let file = format!("{dir}/a.txt");
        assert_eq!(fs_create_file(crate::Wtf8::new(&file)), Ok(true));
        assert_eq!(fs_create_file(crate::Wtf8::new(&file)), Ok(false));
        fs::write(&file, b"four").unwrap();
        assert_eq!(fs_attributes(crate::Wtf8::new(&file)), BA_EXISTS | BA_REGULAR);
        assert_eq!(fs_attributes(crate::Wtf8::new(&dir)), BA_EXISTS | BA_DIRECTORY);
        assert_eq!(fs_attributes(crate::Wtf8::new(&format!("{dir}/none"))), 0);
        assert_eq!(fs_length(crate::Wtf8::new(&file)), 4);
        assert_eq!(fs_length(crate::Wtf8::new(&format!("{dir}/none"))), 0);
        assert!(fs_set_modified(crate::Wtf8::new(&file), 1_000_000_000_000));
        assert_eq!(fs_modified(crate::Wtf8::new(&file)), 1_000_000_000_000);
        assert!(fs_access(crate::Wtf8::new(&file), ACCESS_READ | ACCESS_WRITE));
        assert!(!fs_access(crate::Wtf8::new(&file), ACCESS_EXECUTE));
        assert!(fs_set_permission(crate::Wtf8::new(&file), ACCESS_EXECUTE, true, true));
        assert!(fs_access(crate::Wtf8::new(&file), ACCESS_EXECUTE));
        assert!(fs_create_directory(crate::Wtf8::new(&format!("{dir}/sub"))));
        assert!(!fs_create_directory(crate::Wtf8::new(&format!("{dir}/sub"))));
        let mut names = fs_list(crate::Wtf8::new(&dir)).unwrap();
        names.sort();
        assert_eq!(names, ["a.txt", "sub"]);
        assert_eq!(fs_list(crate::Wtf8::new(&format!("{dir}/none"))), None);
        assert!(fs_rename(crate::Wtf8::new(&file), crate::Wtf8::new(&format!("{dir}/b.txt"))));
        assert!(fs_delete(crate::Wtf8::new(&format!("{dir}/b.txt"))));
        assert!(fs_delete(crate::Wtf8::new(&format!("{dir}/sub"))));
        assert!(!fs_delete(crate::Wtf8::new(&format!("{dir}/sub"))));
        let missing = fs_create_file(crate::Wtf8::new(&format!("{dir}/none/x"))).unwrap_err();
        assert_eq!(
            (missing.class, missing.message.as_str()),
            ("java.io.IOException", "No such file or directory")
        );
        fs::remove_dir(&dir).unwrap();
    }

    #[test]
    fn canonical_paths_resolve_what_exists() {
        let dir = temp_dir("canonical");
        let real = fs::canonicalize(&dir).unwrap().to_string_lossy().into_owned();
        assert_eq!(
            fs_canonicalize(crate::Wtf8::new(&format!("{dir}/./x/../y/z"))),
            Ok(crate::JString::from(format!("{real}/y/z")))
        );
        assert_eq!(
            fs_canonicalize(crate::Wtf8::new(&format!("{dir}/."))),
            Ok(crate::JString::from(real.clone()))
        );
        fs::remove_dir(&dir).unwrap();
        assert!(fs_name_max(crate::Wtf8::new("/")) > 0);
    }
}
