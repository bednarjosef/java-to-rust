//! Compiles `java/JavaTree.java`, the program that runs javac on the input and prints its typed
//! syntax tree (with `java/JdkPatch.java`, which applies the translator's patches of library
//! classes, and `java/MissingClasses.java`, which makes code that names a missing class throw
//! `NoClassDefFoundError`), into the build directory; `javac.rs` runs it from there. Then prints
//! the tree of `java/Standard.java`, the standard exception classes that lowering adds to the
//! programs that use them, as `standard.json` in the build directory, and the tree of
//! `java/Functional.java`, the JDK's functional interfaces for programs translated without the
//! JDK's sources, as `functional.json`, and the tree of `java/Threads.java`, the translator's
//! `java.lang.Thread`, as `threads.json` (compiled with `java/ThreadLocalStub.java`, the name of a
//! JDK class it uses), and the tree of `java/Reflection.java`, the translator's
//! `java.lang.reflect.Method`, `Field` and `Constructor`, as `reflection.json`. Last, runs
//! `java/Charsets.java`, which prints this JDK's charsets and the tables of its single-byte ones,
//! as `charsets.json` (see `jdk_charsets.rs`). It compiles `java/JdkResources.java` too, which the
//! translator runs to read the JDK's data files (`program/reach/jdk_resources.rs`). Those are this JDK's, so the build runs again
//! when the `java` on `PATH` is replaced, as an upgrade replaces it; and the translator asks the
//! JDK it runs with again where that is another JDK, which an upgrade with older file times or a
//! change of `PATH` can leave.

use std::process::Command;

/// The packages of javac's implementation that `JavaTree` and `java/BytecodeNames.java`, which
/// reads what javac's code generator decided, use (see `javac.rs`).
const JAVAC_PACKAGES: [&str; 8] = ["api", "code", "comp", "tree", "util", "jvm", "main", "resources"];

fn main() {
    println!("cargo:rerun-if-changed=java/JavaTree.java");
    println!("cargo:rerun-if-changed=java/BytecodeNames.java");
    println!("cargo:rerun-if-changed=java/JdkPatch.java");
    println!("cargo:rerun-if-changed=java/MissingClasses.java");
    println!("cargo:rerun-if-changed=java/Standard.java");
    println!("cargo:rerun-if-changed=java/Functional.java");
    println!("cargo:rerun-if-changed=java/Threads.java");
    println!("cargo:rerun-if-changed=java/ThreadLocalStub.java");
    println!("cargo:rerun-if-changed=java/Reflection.java");
    println!("cargo:rerun-if-changed=java/Charsets.java");
    println!("cargo:rerun-if-changed=java/JdkResources.java");
    rerun_if_jdk_changes();
    let out = std::env::var("OUT_DIR").expect("cargo sets OUT_DIR");
    let mut javac = Command::new("javac");
    javac.args(["-d", &out, "-Xlint:none"]);
    for package in JAVAC_PACKAGES {
        javac.args(["--add-exports", &format!("jdk.compiler/com.sun.tools.javac.{package}=ALL-UNNAMED")]);
    }
    let status = javac
        .args([
            "java/JavaTree.java",
            "java/BytecodeNames.java",
            "java/JdkPatch.java",
            "java/MissingClasses.java",
            "java/Charsets.java",
            "java/JdkResources.java",
        ])
        .status()
        .expect("building the translator needs javac (a JDK, version 21 or newer) on PATH");
    assert!(
        status.success(),
        "javac failed to compile the translator's Java (java/JavaTree.java and the classes it uses)"
    );
    for (source, json, with) in [
        ("java/Standard.java", "standard.json", None),
        ("java/Functional.java", "functional.json", None),
        ("java/Threads.java", "threads.json", Some("java/ThreadLocalStub.java")),
        ("java/Reflection.java", "reflection.json", None),
    ] {
        let mut tree = Command::new("java");
        for package in JAVAC_PACKAGES {
            tree.args(["--add-exports", &format!("jdk.compiler/com.sun.tools.javac.{package}=ALL-UNNAMED")]);
        }
        let tree = tree
            .args(["-cp", &out, "JavaTree", "--program", source])
            .args(with)
            .output()
            .expect("building the translator needs java (a JDK) on PATH");
        assert!(
            tree.status.success(),
            "JavaTree failed on {source}: {}",
            String::from_utf8_lossy(&tree.stderr)
        );
        std::fs::write(std::path::Path::new(&out).join(json), tree.stdout)
            .expect("the build directory is writable");
    }
    let charsets = Command::new("java")
        .args(["-cp", &out, "Charsets"])
        .output()
        .expect("building the translator needs java (a JDK) on PATH");
    assert!(charsets.status.success(), "Charsets failed: {}", String::from_utf8_lossy(&charsets.stderr));
    std::fs::write(std::path::Path::new(&out).join("charsets.json"), charsets.stdout)
        .expect("the build directory is writable");
    println!("cargo:rerun-if-changed=java/overlay");
    println!("cargo:rerun-if-changed=java/patches");
    write_embedded(std::path::Path::new(&out));
}

/// Writes `embedded.rs` into the build directory: the compiled classes of `java/` and the files
/// of `java/overlay` and `java/patches`, each by its path under its directory, with a key of all
/// their bytes (see `embedded.rs` in `src/`), so that the translator's binary carries them and
/// runs wherever it is copied or installed.
fn write_embedded(out: &std::path::Path) {
    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets it"));
    let mut key: u64 = 0xcbf2_9ce4_8422_2325;
    let mut text = String::new();
    for (name, dir, keep) in [
        ("CLASSES", out.to_path_buf(), "class"),
        ("OVERLAY", manifest.join("java/overlay"), "java"),
        ("PATCHES", manifest.join("java/patches"), "java"),
    ] {
        let mut files = Vec::new();
        list(&dir, &dir, keep, &mut files);
        files.sort();
        text.push_str(&format!("pub static {name}: &[(&str, &[u8])] = &[\n"));
        for relative in &files {
            let path = dir.join(relative);
            for byte in relative.bytes().chain(std::fs::read(&path).expect("a listed file reads")) {
                key = (key ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
            }
            text.push_str(&format!(
                "    ({relative:?}, include_bytes!({:?})),\n",
                path.display().to_string()
            ));
        }
        text.push_str("];\n");
    }
    text.push_str(&format!("pub const KEY: &str = \"{key:016x}\";\n"));
    std::fs::write(out.join("embedded.rs"), text).expect("the build directory is writable");
}

/// The files under `dir` whose extension is `keep`, as paths relative to `root` with `/`.
fn list(root: &std::path::Path, dir: &std::path::Path, keep: &str, files: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).expect("a directory of the build reads").flatten() {
        let path = entry.path();
        if path.is_dir() {
            list(root, &path, keep, files);
        } else if path.extension().is_some_and(|e| e == keep) {
            let relative = path.strip_prefix(root).expect("under its root");
            files.push(
                relative.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/"),
            );
        }
    }
}

/// Makes Cargo run this script again when the `java` that `PATH` finds (through its links) or
/// that JDK's `release` file is replaced, as an upgrade replaces them. Not on a change of `PATH`
/// itself, which differs between shells and would rebuild the translator for nothing.
fn rerun_if_jdk_changes() {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let Some(java) = std::env::split_paths(&path).map(|dir| dir.join("java")).find(|java| java.is_file())
    else {
        return;
    };
    let Ok(java) = std::fs::canonicalize(java) else { return };
    println!("cargo:rerun-if-changed={}", java.display());
    if let Some(home) = java.parent().and_then(|bin| bin.parent()) {
        let release = home.join("release");
        if release.is_file() {
            println!("cargo:rerun-if-changed={}", release.display());
        }
    }
}
