//! Java's results kept between runs of the corpus test. A program's stored result is reused while
//! its Java sources, main class, JVM arguments and Java version are unchanged; `CORPUS_JAVA_FRESH=1`
//! runs every program on Java again and stores the results anew.
//!
//! Each program has one file `<name>.txt` in the cache directory: its key on the first line, its
//! exit code on the second, and the rest of the file is the standard output as Java printed it.

use std::path::{Path, PathBuf};

use super::{JVM_ARGS, Program, Run, java_files, run, run_java};

pub struct JavaCache {
    dir: PathBuf,
    /// The `java -version` output, read once per run and part of every key.
    version: String,
    /// Set by `CORPUS_JAVA_FRESH=1`: stored results are ignored, but new ones are stored.
    fresh: bool,
}

impl JavaCache {
    /// Opens the cache in `dir` and removes the entries of programs that left the corpus.
    pub fn open(dir: &Path, programs: &[Program]) -> Self {
        std::fs::create_dir_all(dir).unwrap();
        let names: Vec<String> = programs.iter().map(|p| format!("{}.txt", p.name)).collect();
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if !names.contains(&path.file_name().unwrap().to_string_lossy().into_owned()) {
                let _ = std::fs::remove_file(path);
            }
        }
        let version = java_version();
        let fresh = std::env::var("CORPUS_JAVA_FRESH").is_ok_and(|v| v == "1");
        JavaCache { dir: dir.to_path_buf(), version, fresh }
    }

    /// Java's run of a program: its stored result if the key is unchanged, otherwise a run on
    /// Java, whose result is stored unless Java did not finish.
    pub fn run(&self, p: &Program, classes: &Path) -> Run {
        let key = self.key(p);
        if let Some(stored) = key.as_ref().and_then(|key| self.load(&p.name, key)) {
            return stored;
        }
        let result = run(run_java(p, classes));
        if let Some(key) = key {
            self.store(&p.name, &key, &result);
        }
        result
    }

    /// The key of a program: a hash of everything that decides its Java result. None if a source
    /// cannot be read, in which case the program is not cached.
    fn key(&self, p: &Program) -> Option<String> {
        let mut fingerprint = Fingerprint::new();
        fingerprint.field(self.version.as_bytes());
        // The sources of a project, in order, with their paths relative to the source root; a
        // single file is its own root.
        let (root, files) = match &p.main {
            None => {
                fingerprint.field(b"single file");
                (p.java.parent()?, vec![p.java.clone()])
            }
            Some(main) => {
                fingerprint.field(b"project");
                fingerprint.field(main.as_bytes());
                let mut files = Vec::new();
                java_files(&p.java, &mut files);
                files.sort();
                (p.java.as_path(), files)
            }
        };
        for arg in JVM_ARGS {
            fingerprint.field(arg.as_bytes());
        }
        for file in files {
            let path = file.strip_prefix(root).ok()?;
            fingerprint.field(path.to_string_lossy().as_bytes());
            fingerprint.field(&std::fs::read(&file).ok()?);
        }
        Some(format!("{:016x}", fingerprint.0))
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.txt"))
    }

    /// The stored result of a program with this key, if there is one and it is readable.
    fn load(&self, name: &str, key: &str) -> Option<Run> {
        if self.fresh {
            return None;
        }
        let text = std::fs::read_to_string(self.path(name)).ok()?;
        let (stored_key, rest) = text.split_once('\n')?;
        let (code, stdout) = rest.split_once('\n')?;
        if stored_key != key {
            return None;
        }
        Some(Run { stdout: stdout.to_string(), code: Some(code.parse().ok()?) })
    }

    /// Stores a result, written under a temporary name first so that a reader never sees half of
    /// it. A result without an exit code (Java was killed) is not stored.
    fn store(&self, name: &str, key: &str, result: &Run) {
        let Some(code) = result.code else { return };
        let path = self.path(name);
        let tmp = path.with_extension("tmp");
        let text = format!("{key}\n{code}\n{}", result.stdout);
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

/// The `java -version` output: Java writes it to its standard error.
fn java_version() -> String {
    let output = std::process::Command::new("java")
        .arg("-version")
        .output()
        .expect("running `java` failed; the corpus test needs a JDK on PATH");
    format!("{}{}", String::from_utf8_lossy(&output.stderr), String::from_utf8_lossy(&output.stdout))
}

/// FNV-1a over length-prefixed fields, so that no two different lists of fields hash the same
/// input. Unlike `DefaultHasher`, its values do not change between Rust releases.
struct Fingerprint(u64);

impl Fingerprint {
    fn new() -> Self {
        Fingerprint(0xcbf2_9ce4_8422_2325)
    }

    fn field(&mut self, bytes: &[u8]) {
        for byte in (bytes.len() as u64).to_le_bytes().iter().chain(bytes) {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}
