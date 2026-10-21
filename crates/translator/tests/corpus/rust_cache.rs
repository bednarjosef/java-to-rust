//! The runs of translated programs kept between runs of the corpus test, in the checks' cache
//! (`check_cache`, under `rust/`), which every worktree shares: a program whose translation is the
//! same as in an earlier run that matched Java is neither built nor run again, as long as all else
//! that decides its binary and its run is the same too: the runtime crate, the toolchain, the
//! build's settings, the program's arguments and its input. Those, with the translation, make
//! the name of its entry. Only a run that matched Java's is stored, so a program that fails is
//! built and run every time, and the stored run is still compared with Java's. `CHECK_FRESH=1`
//! builds and runs every program and stores the runs anew.
//!
//! An entry is a JSON object: the run's exit code and standard output, and the rustc warnings of
//! the build, which the report lists.

use std::path::{Path, PathBuf};

use java_to_rust::check_cache::{self, Fingerprint};
use serde_json::json;

use super::{Program, Run};

pub struct RustCache {
    dir: PathBuf,
    /// The hash of what decides every program's binary besides its source; None if part of it
    /// cannot be read, and then nothing is cached.
    build: Option<String>,
}

/// A stored run of a program.
pub struct Stored {
    pub run: Run,
    pub warnings: Vec<String>,
}

impl RustCache {
    /// The cache in `dir`, for programs built with the runtime crate `runtime` as binaries of the
    /// Cargo project in `work` with the manifest `cargo_toml`.
    pub fn open(dir: &Path, runtime: &Path, work: &Path, cargo_toml: &str) -> Self {
        let build = check_cache::rust_build_key(runtime, work).map(|key| {
            let mut fingerprint = Fingerprint::new();
            fingerprint.field(key.as_bytes());
            // The manifest names the runtime by its path, which is another in each worktree.
            fingerprint.field(cargo_toml.replace(&format!("{runtime:?}"), "<runtime>").as_bytes());
            fingerprint.key()
        });
        RustCache { dir: dir.to_path_buf(), build }
    }

    /// The key of a program translated to `rust`: None if it is not to be cached.
    pub fn key(&self, p: &Program, rust: &str) -> Option<String> {
        let mut fingerprint = Fingerprint::new();
        fingerprint.field(b"corpus rust run");
        fingerprint.field(self.build.as_ref()?.as_bytes());
        fingerprint.field(p.name.as_bytes());
        fingerprint.field(rust.as_bytes());
        fingerprint.field(b"arguments");
        for arg in &p.args {
            fingerprint.field(arg.as_bytes());
        }
        fingerprint.field(b"input");
        if let Some(input) = &p.input {
            fingerprint.field(&std::fs::read(input).ok()?);
        }
        Some(fingerprint.key())
    }

    /// The stored run with this key, if there is one and it is readable.
    pub fn load(&self, key: &str) -> Option<Stored> {
        let entry: serde_json::Value = serde_json::from_slice(&check_cache::load(&self.dir, key)?).ok()?;
        let code = i32::try_from(entry["code"].as_i64()?).ok()?;
        let stdout = entry["stdout"].as_str()?.to_string();
        let warnings: Option<Vec<String>> =
            entry["warnings"].as_array()?.iter().map(|w| w.as_str().map(str::to_string)).collect();
        Some(Stored { run: Run { stdout, code: Some(code) }, warnings: warnings? })
    }

    /// Stores a run that matched Java's, with the build's warnings. A run without an exit code
    /// (the program was killed) is not stored.
    pub fn store(&self, key: &str, run: &Run, warnings: &[String]) {
        let Some(code) = run.code else { return };
        let entry = json!({ "code": code, "stdout": run.stdout, "warnings": warnings });
        check_cache::store(&self.dir, key, entry.to_string().as_bytes());
    }
}
