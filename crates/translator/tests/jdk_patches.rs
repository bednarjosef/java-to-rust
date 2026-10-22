//! The translator's patches of library classes (`java/patches`, applied by `java/JdkPatch.java`;
//! see docs/jdk-overlays.md): how a patch changes a file, that a patch naming what the file does
//! not have is refused, and that every patch applies to the JDK's sources that
//! `JAVA_BASE_SOURCES` names (without the variable, that test says it skipped and passes).

use java_to_rust::javac::check_patches;
use std::path::{Path, PathBuf};

/// A library root with one file, and a patches root with one patch of it, under a directory of
/// their own; returns the two roots and where the patched file is written.
fn roots(name: &str, library: &str, patch: &str) -> (PathBuf, PathBuf, PathBuf) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("jdk_patches").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    let (lib, patches, out) = (dir.join("lib"), dir.join("patches"), dir.join("out"));
    for (root, text) in [(&lib, library), (&patches, patch)] {
        std::fs::create_dir_all(root.join("p")).unwrap();
        std::fs::write(root.join("p/Greeter.java"), text).unwrap();
    }
    (lib, patches, out)
}

const LIBRARY: &str = r#"package p;

import java.util.List;

public class Greeter {
    private static final String PREFIX = Lookup.prefix();

    static {
        System.loadLibrary("greeter");
    }

    private final String name;

    public Greeter(String name) {
        this.name = java.util.Objects.requireNonNull(name);
    }

    public String greet() {
        return PREFIX + name;
    }

    public String greet(List<String> others) {
        return greet() + " and " + String.join(", ", others);
    }

    private String secret() {
        return "native";
    }

    static class Lookup {
        static String prefix() {
            return "Hello, ";
        }
    }
}
"#;

#[test]
fn a_patch_replaces_adds_removes_and_edits_members() {
    let patch = r#"package p;

import java.util.Locale;

@Edit(find = "private final String name;", replace = "private final String name; // ours")
class Greeter {
    @Remove
    private static final String PREFIX;

    static {}

    @Edit(find = "java.util.Objects.requireNonNull(name)", replace = "name")
    public Greeter(String name);

    @Replace
    public String greet() {
        return prefix() + name.toUpperCase(Locale.ROOT);
    }

    @Add
    private static String prefix() {
        return "Hi, ";
    }

    @Remove
    private String secret();

    @Patch
    static class Lookup {
        @Edit(from = "return", to = ";", replace = "return \"Hey, \";")
        static String prefix();
    }
}
"#;
    let (lib, patches, out) = roots("applies", LIBRARY, patch);
    check_patches(&patches, &[lib], Some(&out)).unwrap();
    let patched = std::fs::read_to_string(out.join("p/Greeter.java")).unwrap();
    for wanted in [
        "import java.util.Locale;",
        "private final String name; // ours",
        "this.name = name;",
        "return prefix() + name.toUpperCase(Locale.ROOT);",
        "private static String prefix() {",
        "return \"Hey, \";",
        // The overload of the same name is not the replaced method.
        "return greet() + \" and \"",
    ] {
        assert!(patched.contains(wanted), "{wanted:?} is not in the patched file:\n{patched}");
    }
    for gone in ["PREFIX =", "loadLibrary", "secret()", "@Replace", "@Add", "@Remove", "@Edit", "@Patch"] {
        assert!(!patched.contains(gone), "{gone:?} is still in the patched file:\n{patched}");
    }
}

#[test]
fn a_patch_that_names_what_the_file_does_not_have_is_refused() {
    for (patch, message) in [
        ("@Replace\n    public String greet(String other) { return other; }", "greet(String)"),
        ("@Remove\n    private int count;", "the field count"),
        ("@Add\n    public String greet() { return \"\"; }", "has it already"),
        ("@Edit(find = \"Goodbye\", replace = \"\")\n    public String greet();", "\"Goodbye\" is not in"),
        ("public String greet() { return \"\"; }", "says nothing"),
    ] {
        let text = format!("package p;\n\nclass Greeter {{\n    {patch}\n}}\n");
        let (lib, patches, _) = roots("refused", LIBRARY, &text);
        let error = check_patches(&patches, &[lib], None).expect_err(patch);
        assert!(error.contains(message), "{patch}: {error}");
    }
}

#[test]
fn overlay_classes_and_patches_do_not_overlap() {
    let java = Path::new(env!("CARGO_MANIFEST_DIR")).join("java");
    let mut overlay = Vec::new();
    files(&java.join("overlay"), Path::new(""), &mut overlay);
    let mut patches = Vec::new();
    files(&java.join("patches"), Path::new(""), &mut patches);
    let both: Vec<_> = overlay.iter().filter(|f| patches.contains(f)).collect();
    assert!(both.is_empty(), "both a class of java/overlay and a patch: {both:?}");
}

#[test]
fn every_patch_applies_to_the_jdk() {
    let Some(java_base) = std::env::var_os("JAVA_BASE_SOURCES").map(PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to apply the patches");
        return;
    };
    let patches = Path::new(java_to_rust::program::PATCHES);
    if let Err(error) = check_patches(patches, &[java_base], None) {
        panic!("{error}");
    }
}

/// The `.java` files under `dir`, relative to the root `at` started from.
fn files(dir: &Path, at: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries {
        let path = entry.unwrap().path();
        let name = at.join(path.file_name().unwrap());
        if path.is_dir() {
            files(&path, &name, out);
        } else if path.extension().is_some_and(|e| e == "java") {
            out.push(name);
        }
    }
}
