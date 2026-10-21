//! The runs of `JavaTree` and `BytecodeNames` on one source given on standard input, kept in the
//! checks' cache (`check_cache`, under `javac/`) when it is on: what such a run prints is decided
//! by the source, the classes of `java/` that `build.rs` compiled, the JVM's arguments and the
//! Java version, which make its key. Only a run that succeeded is kept; a compile error runs
//! javac every time.

use crate::check_cache::{self, Fingerprint};
use std::io::Write;
use std::process::{Command, Stdio};

/// What a run printed, and whether it succeeded.
pub(super) struct Ran {
    pub success: bool,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Runs `command` with `input` on its standard input, or returns what the same run printed
/// before.
pub(super) fn run(mut command: Command, input: &str) -> std::io::Result<Ran> {
    let cache = check_cache::root().map(|root| root.join("javac"));
    let key = cache.as_ref().and_then(|_| key(&command, input));
    if let (Some(dir), Some(key)) = (&cache, &key)
        && let Some(stdout) = check_cache::load(dir, key)
    {
        return Ok(Ran { success: true, stdout, stderr: Vec::new() });
    }
    let ran = run_uncached(&mut command, input)?;
    if let (Some(dir), Some(key), true) = (&cache, &key, ran.success) {
        check_cache::store(dir, key, &ran.stdout);
    }
    Ok(ran)
}

/// Runs `command` with `input` on its standard input.
pub(super) fn run_uncached(command: &mut Command, input: &str) -> std::io::Result<Ran> {
    let mut child = command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    let mut stdin = child.stdin.take().expect("stdin is piped");
    let source = input.to_string();
    let writer = std::thread::spawn(move || stdin.write_all(source.as_bytes()));
    let output = child.wait_with_output()?;
    let _ = writer.join();
    Ok(Ran { success: output.status.success(), stdout: output.stdout, stderr: output.stderr })
}

/// The key of a run: the Java version, the classes in the build directory (which the command
/// names by its path, the same for every source), the program and arguments of the command, its
/// environment, and the input. None if a class cannot be read.
fn key(command: &Command, input: &str) -> Option<String> {
    let classes = env!("OUT_DIR");
    let mut fingerprint = Fingerprint::new();
    fingerprint.field(b"javac run");
    fingerprint.field(check_cache::java_version().as_bytes());
    fingerprint.field(classes_key()?.as_bytes());
    fingerprint.field(command.get_program().as_encoded_bytes());
    for arg in command.get_args() {
        fingerprint.field(if arg == classes { b"<classes>" } else { arg.as_encoded_bytes() });
    }
    for (name, value) in command.get_envs() {
        fingerprint.field(name.as_encoded_bytes());
        fingerprint.field(value.map_or(b"<removed>".as_slice(), |v| v.as_encoded_bytes()));
    }
    fingerprint.field(input.as_bytes());
    Some(fingerprint.key())
}

/// A hash of the files in the build directory: the classes of `java/` and the trees `build.rs`
/// printed. Computed once per process.
fn classes_key() -> Option<&'static str> {
    static KEY: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    KEY.get_or_init(|| {
        let mut fingerprint = Fingerprint::new();
        fingerprint.tree(std::path::Path::new(env!("OUT_DIR")), &[])?;
        Some(fingerprint.key())
    })
    .as_deref()
}
