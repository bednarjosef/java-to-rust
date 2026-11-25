//! The files the translator needs at run time, carried in its binary: the compiled classes of
//! `java/` (`JavaTree`, `BytecodeNames`, `JdkPatch` and the rest, which run on the JVM), and the
//! Java files of `java/overlay` and `java/patches`, which javac reads as source roots. `build.rs`
//! lists them. They are written once, on first use, to a directory of the user's cache named by
//! a key of their bytes, so a binary copied or installed anywhere finds them, and two builds of
//! the translator never share a directory unless their files are the same.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

mod files {
    include!(concat!(env!("OUT_DIR"), "/embedded.rs"));
}

/// The directory of the compiled classes, for `java -cp`.
pub fn classes() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| written("classes", files::CLASSES))
}

/// The translator's own classes that replace or add to the JDK's (docs/jdk-overlays.md).
pub fn overlay() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| written("overlay", files::OVERLAY))
}

/// The translator's patches of the JDK's classes (docs/jdk-overlays.md).
pub fn patches() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| written("patches", files::PATCHES))
}

/// The translator's cache: `JAVA_TO_RUST_CACHE` when it is set, otherwise `java-to-rust` in the
/// user's cache (`XDG_CACHE_HOME`, or `~/.cache`), or in the system's temporary directory without
/// a home. It holds these files (`files/`) and the JDK's unpacked sources (`jdk/`, see
/// `jdk_sources.rs`).
pub fn cache() -> PathBuf {
    if let Some(dir) = std::env::var_os("JAVA_TO_RUST_CACHE") {
        return PathBuf::from(dir);
    }
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .unwrap_or_else(std::env::temp_dir);
    cache.join("java-to-rust")
}

fn base() -> PathBuf {
    cache().join("files")
}

/// The directory `name` of this build's files, written first if it is not there: into a
/// directory of this process's own, then renamed into place, so that translators started at the
/// same time never read a half-written one.
fn written(name: &str, files: &[(&str, &[u8])]) -> PathBuf {
    let dir = base().join(files::KEY).join(name);
    if dir.is_dir() {
        return dir;
    }
    let partial = base().join(files::KEY).join(format!("{name}.partial-{}", std::process::id()));
    let write = || -> std::io::Result<()> {
        for (path, bytes) in files {
            let path = partial.join(path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, bytes)?;
        }
        std::fs::create_dir_all(&partial)?;
        match std::fs::rename(&partial, &dir) {
            // Another translator put the same files in place first.
            Err(_) if dir.is_dir() => std::fs::remove_dir_all(&partial),
            result => result,
        }
    };
    if let Err(e) = write() {
        let _ = std::fs::remove_dir_all(&partial);
        panic!(
            "cannot write the translator's files to {}: {e} (JAVA_TO_RUST_CACHE names another place)",
            dir.display()
        );
    }
    dir
}
