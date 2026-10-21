//! Compiles `java/JavaTree.java`, the program that runs javac on the input and prints its typed
//! syntax tree, into the build directory; `javac.rs` runs it from there. Then prints the tree of
//! `java/Standard.java`, the standard exception classes that lowering adds to the programs that
//! use them, as `standard.json` in the build directory, and the tree of `java/Functional.java`,
//! the JDK's functional interfaces for programs translated without the JDK's sources, as
//! `functional.json`, and the tree of `java/Threads.java`, the translator's `java.lang.Thread`, as
//! `threads.json`.

use std::process::Command;

/// javac's implementation, whose `analyze` of chosen classes `JavaTree` calls (see `javac.rs`).
const JAVAC_API: &str = "jdk.compiler/com.sun.tools.javac.api=ALL-UNNAMED";

/// The packages of javac's implementation that `JavaTree` and `java/BytecodeNames.java`, which
/// reads what javac's code generator decided, use (see `javac.rs`).
const JAVAC_PACKAGES: [&str; 5] = ["api", "code", "tree", "util", "jvm"];

fn main() {
    println!("cargo:rerun-if-changed=java/JavaTree.java");
    println!("cargo:rerun-if-changed=java/BytecodeNames.java");
    println!("cargo:rerun-if-changed=java/Standard.java");
    println!("cargo:rerun-if-changed=java/Functional.java");
    println!("cargo:rerun-if-changed=java/Threads.java");
    let out = std::env::var("OUT_DIR").expect("cargo sets OUT_DIR");
    let mut javac = Command::new("javac");
    javac.args(["-d", &out, "-Xlint:none"]);
    for package in JAVAC_PACKAGES {
        javac.args(["--add-exports", &format!("jdk.compiler/com.sun.tools.javac.{package}=ALL-UNNAMED")]);
    }
    let status = javac
        .args(["java/JavaTree.java", "java/BytecodeNames.java"])
        .status()
        .expect("building the translator needs javac (a JDK, version 21 or newer) on PATH");
    assert!(status.success(), "javac failed to compile java/JavaTree.java and java/BytecodeNames.java");
    for (source, json) in [
        ("java/Standard.java", "standard.json"),
        ("java/Functional.java", "functional.json"),
        ("java/Threads.java", "threads.json"),
    ] {
        let tree = Command::new("java")
            .args(["--add-exports", JAVAC_API, "-cp", &out, "JavaTree", "--program", source])
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
}
