//! Results kept between runs of the checks (`scripts/check.sh`), shared by every worktree: what
//! Java printed for a test program, javac's tree of a source, and which translated programs were
//! built and passed. Each entry is a file named by a hash of everything that decides it, so an
//! entry is never out of date, only unused; a missing or unreadable entry means the full work.
//!
//! The cache is the directory `CHECK_CACHE` (scripts/check.sh sets it to
//! `~/.cache/java-to-rust-checks`); without it, the tests keep their results there all the same
//! (`test_root`), but the translator does not keep javac's trees. `CHECK_FRESH=1` ignores what is
//! stored, does all the work, and stores the results anew.

use std::path::{Path, PathBuf};

/// The root of the cache, if caching is on.
pub fn root() -> Option<PathBuf> {
    std::env::var_os("CHECK_CACHE").filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// The root of the cache for the tests, which keep their results without `CHECK_CACHE` too:
/// `~/.cache/java-to-rust-checks` (under `XDG_CACHE_HOME` if it is set), as scripts/check.sh sets
/// it.
pub fn test_root() -> PathBuf {
    root().unwrap_or_else(|| {
        let cache = std::env::var_os("XDG_CACHE_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| Path::new(&std::env::var_os("HOME").unwrap_or_default()).join(".cache"));
        cache.join("java-to-rust-checks")
    })
}

/// Whether stored results are ignored (`CHECK_FRESH=1`). New ones are still stored.
pub fn fresh() -> bool {
    std::env::var("CHECK_FRESH").is_ok_and(|v| v == "1")
}

/// The stored entry `key` of the cache `dir`, unless `CHECK_FRESH` is set.
pub fn load(dir: &Path, key: &str) -> Option<Vec<u8>> {
    if fresh() {
        return None;
    }
    std::fs::read(entry(dir, key)).ok()
}

/// Stores an entry, written under a name of its own first so that a reader, in this run or in
/// another worktree's, never sees half of it. A failure to store is ignored: the entry is
/// only missed next time.
pub fn store(dir: &Path, key: &str, bytes: &[u8]) {
    let path = entry(dir, key);
    let Some(parent) = path.parent() else { return };
    if std::fs::create_dir_all(parent).is_err() {
        return;
    }
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = parent.join(format!(".{key}.{}.{n}.tmp", std::process::id()));
    if std::fs::write(&tmp, bytes).is_ok() && std::fs::rename(&tmp, &path).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
}

/// The file of an entry: under a directory named by the key's first two digits, so that no
/// directory holds too many.
fn entry(dir: &Path, key: &str) -> PathBuf {
    dir.join(&key[..2.min(key.len())]).join(key)
}

/// FNV-1a with 128 bits over length-prefixed fields, so that no two different lists of fields
/// hash the same input, and its values do not change between Rust releases (unlike
/// `DefaultHasher`). With 128 bits, two different inputs of the checks do not share a key.
pub struct Fingerprint(u128);

impl Default for Fingerprint {
    fn default() -> Self {
        Fingerprint::new()
    }
}

impl Fingerprint {
    pub fn new() -> Self {
        Fingerprint(0x6c62_272e_07bb_0142_62b8_2175_6295_c58d)
    }

    pub fn field(&mut self, bytes: &[u8]) {
        const PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;
        for byte in (bytes.len() as u64).to_le_bytes().iter().chain(bytes) {
            self.0 ^= u128::from(*byte);
            self.0 = self.0.wrapping_mul(PRIME);
        }
    }

    /// Adds every file under `dir`, in order, with its path relative to `dir`, skipping the
    /// directories named in `skip`. None if one cannot be read.
    pub fn tree(&mut self, dir: &Path, skip: &[&str]) -> Option<()> {
        let mut files = Vec::new();
        walk(dir, skip, &mut files)?;
        files.sort();
        for file in files {
            self.field(file.strip_prefix(dir).ok()?.to_string_lossy().as_bytes());
            self.field(&std::fs::read(&file).ok()?);
        }
        Some(())
    }

    /// The key: 32 hexadecimal digits.
    pub fn key(&self) -> String {
        format!("{:032x}", self.0)
    }
}

fn walk(dir: &Path, skip: &[&str], out: &mut Vec<PathBuf>) -> Option<()> {
    for entry in std::fs::read_dir(dir).ok()? {
        let path = entry.ok()?.path();
        if path.is_dir() {
            if !skip.iter().any(|s| path.file_name().is_some_and(|n| n == *s)) {
                walk(&path, skip, out)?;
            }
        } else {
            out.push(path);
        }
    }
    Some(())
}

/// The `java -version` output (Java writes it to its standard error) and the environment
/// variables that the `java` launcher reads options from: with the JVM's arguments and the
/// classes it runs, what decides what a JVM does.
pub fn java_version() -> &'static str {
    static VERSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    VERSION.get_or_init(|| {
        let mut text = match std::process::Command::new("java").arg("-version").output() {
            Ok(output) => {
                format!(
                    "{}{}",
                    String::from_utf8_lossy(&output.stderr),
                    String::from_utf8_lossy(&output.stdout)
                )
            }
            Err(e) => format!("no java: {e}"),
        };
        for var in ["JAVA_TOOL_OPTIONS", "JDK_JAVA_OPTIONS", "_JAVA_OPTIONS", "CLASSPATH"] {
            if let Some(value) = std::env::var_os(var) {
                text.push_str(&format!("\n{var}={}", value.to_string_lossy()));
            }
        }
        text
    })
}

/// A hash of what, besides a translated program's own crate, decides the binary that cargo builds
/// from it in the directory `work` with the runtime crate in `runtime`: the runtime's files and
/// the manifests it inherits from, the toolchain, cargo's configuration files, and the
/// environment variables that cargo, rustc and the program read. None if part of it cannot be
/// read. Paths are not part of it, so worktrees share it.
pub fn rust_build_key(runtime: &Path, work: &Path) -> Option<String> {
    let mut fingerprint = Fingerprint::new();
    fingerprint.field(b"rust build");
    fingerprint.tree(runtime, &["target"])?;
    for dir in runtime.ancestors().skip(1) {
        if let Ok(manifest) = std::fs::read(dir.join("Cargo.toml")) {
            fingerprint.field(&manifest);
        }
    }
    for (program, arg) in
        [(std::env::var("RUSTC").unwrap_or("rustc".to_string()), "-vV"), ("cargo".to_string(), "-V")]
    {
        let output = std::process::Command::new(program).arg(arg).current_dir(work).output().ok()?;
        fingerprint.field(&output.stdout);
    }
    let cargo_home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".cargo")));
    let configs = work.ancestors().map(|dir| dir.join(".cargo")).chain(cargo_home);
    for dir in configs {
        for name in ["config", "config.toml"] {
            if let Ok(config) = std::fs::read(dir.join(name)) {
                fingerprint.field(&config);
            }
        }
    }
    let mut vars: Vec<(String, String)> = std::env::vars_os()
        .filter_map(|(name, value)| Some((name.into_string().ok()?, value.to_string_lossy().into_owned())))
        .filter(|(name, _)| (name.starts_with("CARGO") || name.starts_with("RUST")) && !ignored_var(name))
        .collect();
    vars.sort();
    for (name, value) in vars {
        fingerprint.field(name.as_bytes());
        fingerprint.field(value.as_bytes());
    }
    Some(fingerprint.key())
}

/// The variables of cargo and Rust that change neither a binary nor what it prints on standard
/// output: where things are built, how many jobs build them, the test harness's settings, the
/// manifest of the test that runs the checks, and backtraces, which go to standard error.
fn ignored_var(name: &str) -> bool {
    const NAMES: [&str; 13] = [
        "CARGO",
        "CARGO_BUILD_JOBS",
        "CARGO_TARGET_DIR",
        "CARGO_TARGET_TMPDIR",
        "CARGO_MANIFEST_DIR",
        "CARGO_MANIFEST_PATH",
        "CARGO_MAKEFLAGS",
        "CARGO_PRIMARY_PACKAGE",
        "CARGO_CRATE_NAME",
        "RUST_TEST_THREADS",
        "RUST_RECURSION_COUNT",
        "RUST_BACKTRACE",
        "RUST_LIB_BACKTRACE",
    ];
    NAMES.contains(&name) || ["CARGO_PKG_", "CARGO_BIN_"].iter().any(|prefix| name.starts_with(prefix))
}
