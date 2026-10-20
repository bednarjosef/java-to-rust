//! Compiles `java/JavaTree.java`, the program that runs javac on the input and prints its typed
//! syntax tree, into the build directory; `javac.rs` runs it from there.

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=java/JavaTree.java");
    let out = std::env::var("OUT_DIR").expect("cargo sets OUT_DIR");
    let status = Command::new("javac")
        .args(["-d", &out, "-Xlint:none", "java/JavaTree.java"])
        .status()
        .expect("building the translator needs javac (a JDK, version 21 or newer) on PATH");
    assert!(status.success(), "javac failed to compile java/JavaTree.java");
}
