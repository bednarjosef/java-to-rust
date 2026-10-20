//! `java-to-rust Input.java` prints the Rust translation; `-o out.rs` writes it to a file and
//! `--crate DIR` writes a Cargo project that can be built and run with `cargo run`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "usage: java-to-rust <Input.java> [-o <output.rs> | --crate <directory>]";

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    let (input, output) = match &args[..] {
        [input] => (input, None),
        [input, flag, out] if flag == "-o" || flag == "--crate" => (input, Some((flag.as_str(), out))),
        _ => return Err(USAGE.into()),
    };
    let java = std::fs::read_to_string(input).map_err(|e| format!("{input}: {e}"))?;
    let rust = java_to_rust::translate(&java).map_err(|e| format!("{input}:{e}"))?;
    match output {
        None => print!("{rust}"),
        Some(("-o", path)) => std::fs::write(path, rust).map_err(|e| format!("{path}: {e}"))?,
        Some((_, dir)) => write_crate(Path::new(dir), input, &rust).map_err(|e| format!("{dir}: {e}"))?,
    }
    Ok(())
}

/// A Cargo project around the translation, depending on the runtime crate of this checkout.
fn write_crate(dir: &Path, input: &str, rust: &str) -> std::io::Result<()> {
    let name = Path::new(input)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("translated")
        .to_lowercase()
        .replace(|c: char| !c.is_ascii_alphanumeric(), "-");
    let runtime: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../runtime").canonicalize()?;
    std::fs::create_dir_all(dir.join("src"))?;
    std::fs::write(
        dir.join("Cargo.toml"),
        format!(
            "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n\
             [dependencies]\njava-runtime = {{ path = {:?} }}\n\n[workspace]\n",
            runtime.display().to_string()
        ),
    )?;
    std::fs::write(dir.join("src/main.rs"), rust)
}
