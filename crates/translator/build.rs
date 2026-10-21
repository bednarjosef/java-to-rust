//! Compiles `java/JavaTree.java`, the program that runs javac on the input and prints its typed
//! syntax tree, into the build directory; `javac.rs` runs it from there. Then prints the tree of
//! `java/Standard.java`, the standard exception classes that lowering adds to the programs that
//! use them, as `standard.json` in the build directory, and the tree of `java/Functional.java`,
//! the JDK's functional interfaces for programs translated without the JDK's sources, as
//! `functional.json`.

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=java/JavaTree.java");
    println!("cargo:rerun-if-changed=java/Standard.java");
    println!("cargo:rerun-if-changed=java/Functional.java");
    let out = std::env::var("OUT_DIR").expect("cargo sets OUT_DIR");
    let status = Command::new("javac")
        .args(["-d", &out, "-Xlint:none", "java/JavaTree.java"])
        .status()
        .expect("building the translator needs javac (a JDK, version 21 or newer) on PATH");
    assert!(status.success(), "javac failed to compile java/JavaTree.java");
    for (source, json) in
        [("java/Standard.java", "standard.json"), ("java/Functional.java", "functional.json")]
    {
        let tree = Command::new("java")
            .args(["-cp", &out, "JavaTree", "--program", source])
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
