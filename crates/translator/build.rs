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
//! `java.lang.reflect.Method`, `Field` and `Constructor`, as `reflection.json`.

use std::process::Command;

/// The packages of javac's implementation that `JavaTree` and `java/BytecodeNames.java`, which
/// reads what javac's code generator decided, use (see `javac.rs`).
const JAVAC_PACKAGES: [&str; 6] = ["api", "code", "tree", "util", "jvm", "main"];

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
}
