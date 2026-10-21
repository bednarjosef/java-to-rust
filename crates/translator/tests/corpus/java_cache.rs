//! Java's results kept between runs of the corpus test, in the checks' cache (`check_cache`,
//! under `java/`), which every worktree shares. A stored result is reused while the program's Java
//! sources, main class, JVM arguments, arguments, input and Java version are unchanged: those make
//! the name of its entry. `CORPUS_JAVA_FRESH=1` (or `CHECK_FRESH=1`) runs every program on Java
//! again and stores the results anew.
//!
//! An entry holds the exit code on its first line, and the rest of it is the standard output as
//! Java printed it.

use std::path::{Path, PathBuf};

use java_to_rust::check_cache::{self, Fingerprint};

use super::{JVM_ARGS, Program, Run, java_files, run, run_java};

pub struct JavaCache {
    dir: PathBuf,
    /// Set by `CORPUS_JAVA_FRESH=1`: stored results are ignored, but new ones are stored.
    fresh: bool,
}

impl JavaCache {
    /// The cache in `dir`.
    pub fn open(dir: &Path) -> Self {
        let fresh = std::env::var("CORPUS_JAVA_FRESH").is_ok_and(|v| v == "1");
        JavaCache { dir: dir.to_path_buf(), fresh }
    }

    /// Java's run of a program, and whether it was stored: its stored result if there is one,
    /// otherwise a run on Java, whose result is stored unless Java did not finish.
    pub fn run(&self, p: &Program, classes: &Path) -> (Run, bool) {
        let key = key(p);
        if let Some(stored) = key.as_ref().and_then(|key| self.load(key)) {
            return (stored, true);
        }
        let result = run(run_java(p, classes));
        if let (Some(key), Some(code)) = (key, result.code) {
            check_cache::store(&self.dir, &key, format!("{code}\n{}", result.stdout).as_bytes());
        }
        (result, false)
    }

    /// The stored result with this key, if there is one and it is readable.
    fn load(&self, key: &str) -> Option<Run> {
        if self.fresh {
            return None;
        }
        let text = String::from_utf8(check_cache::load(&self.dir, key)?).ok()?;
        let (code, stdout) = text.split_once('\n')?;
        Some(Run { stdout: stdout.to_string(), code: Some(code.parse().ok()?) })
    }
}

/// The key of a program: a hash of everything that decides its Java result. None if a source
/// cannot be read, in which case the program is not cached.
fn key(p: &Program) -> Option<String> {
    let mut fingerprint = Fingerprint::new();
    fingerprint.field(b"corpus java run");
    fingerprint.field(check_cache::java_version().as_bytes());
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
    // The program's own arguments and what it reads, which change its output.
    fingerprint.field(b"arguments");
    for arg in &p.args {
        fingerprint.field(arg.as_bytes());
    }
    fingerprint.field(b"input");
    if let Some(input) = &p.input {
        fingerprint.field(&std::fs::read(input).ok()?);
    }
    for file in files {
        let path = file.strip_prefix(root).ok()?;
        fingerprint.field(path.to_string_lossy().as_bytes());
        fingerprint.field(&std::fs::read(&file).ok()?);
    }
    Some(fingerprint.key())
}
