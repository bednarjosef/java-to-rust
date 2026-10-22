//! The generated program as a tree of modules, and the layouts that write it as files.
//!
//! The generator (`codegen`) makes a [`ModuleTree`]: one [`Module`] per Java package, each with
//! the [`Item`]s of that package (a class's struct and `impl`s, a static method, the statics of
//! the package) and the `use` lines they need. A [`Layout`] then decides the files:
//!
//! - [`Layout::OneFile`]: everything in one `main.rs`, packages as inline `mod a { .. }` blocks.
//!   A translated single file, and any program with no package or one, is always one file.
//! - [`Layout::FilePerModule`]: a file per package that has items, at the path of the package
//!   (`com.example.util` is `src/com/example/util.rs`), the way a Rust project is laid out.
//!
//! The text of an item does not depend on the layout: names in other modules are paths that
//! the name resolver (`codegen/modules.rs`) chose for the module the item is in, and Rust gives
//! an inline module and a module in its own file the same meaning. A layout that splits the
//! program into several crates would be one more variant here, with a rule that puts each module
//! in a crate (see `docs/large-projects.md`).

use std::collections::BTreeSet;
use std::path::PathBuf;

/// The whole generated program: the modules in the order of their Java packages (a package
/// before the packages inside it), the crate root first.
#[derive(Clone, Debug, Default)]
pub struct ModuleTree {
    /// The comment that starts the crate root.
    pub header: String,
    /// Every module, the root (the unnamed package) first; a package's module comes before the
    /// modules of the packages inside it, and siblings are in the order of their Java names.
    pub modules: Vec<Module>,
    /// What ends the crate root (the global allocator).
    pub footer: String,
}

/// The module of one Java package.
#[derive(Clone, Debug, Default)]
pub struct Module {
    /// The Java package, such as `com.example.util`; empty for the crate root.
    pub package: String,
    /// The module's path from the crate root, as Rust names (`["com", "example", "util"]`; the
    /// JDK's `java.util` is `["java_", "util"]`); empty for the crate root.
    pub path: Vec<String>,
    /// The `use` lines of the module, in groups that are written with a blank line between them.
    pub uses: Vec<String>,
    /// The module's items, in the order they are written.
    pub items: Vec<Item>,
}

/// One item of a module, or a few that belong together (a class's struct with its `impl`s).
#[derive(Clone, Debug)]
pub struct Item {
    /// What the item was made from.
    pub origin: Origin,
    /// The Rust text, at the module's own indentation (none).
    pub text: String,
    /// The Java packages of the other modules that the item names ("" for the crate root).
    pub refers: BTreeSet<String>,
}

/// What an item was made from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    /// A class's struct, its `impl`s and its methods, by the Java class's qualified name.
    Class(String),
    /// The enum that stands for a class and its subclasses (see `codegen/dispatch.rs`).
    ClassEnum(String),
    /// A Java `enum`'s Rust enum.
    Enum(String),
    /// The statics of the module's classes.
    Statics,
    /// The monitors of the module's classes.
    Monitors,
    /// A static method, a function of its class's module, by its class.
    Function(String),
    /// The crate's `fn main`.
    Main,
}

/// How the modules are laid out in files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// One `main.rs`, with every package an inline module.
    OneFile,
    /// One file per package that has items, mirroring the package (`src/com/example/util.rs`);
    /// a package without items of its own is an inline module of its parent's file, which only
    /// declares the modules inside it.
    FilePerModule,
}

/// One source file of the crate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFile {
    /// The path under the crate's `src/`, such as `main.rs` or `com/example/util.rs`.
    pub path: PathBuf,
    pub text: String,
}

impl Module {
    /// The text of the module's items.
    pub fn items_text(&self) -> String {
        self.items.iter().map(|item| item.text.as_str()).collect()
    }

    fn name(&self) -> &str {
        self.path.last().map_or("", String::as_str)
    }

    /// The `use` lines, each group followed by a blank line, with `pad` before each line.
    fn uses_text(&self, pad: &str) -> String {
        let mut text = String::new();
        for group in &self.uses {
            for line in group.lines() {
                text.push_str(&format!("{pad}{line}\n"));
            }
            text.push('\n');
        }
        text
    }
}

impl ModuleTree {
    /// The source files of the program in `layout`. A program without packages, or with one, is
    /// one `main.rs` in every layout.
    pub fn files(&self, layout: Layout) -> Vec<SourceFile> {
        match layout {
            Layout::OneFile => vec![SourceFile { path: "main.rs".into(), text: self.one_file() }],
            Layout::FilePerModule => self.file_per_module(),
        }
    }

    /// The whole program as the text of one `main.rs`.
    pub fn one_file(&self) -> String {
        let body = self.inline(0);
        format!("{}{body}\n{}", self.header, self.footer)
    }

    /// The direct children of module `index`, by index.
    fn children(&self, index: usize) -> impl Iterator<Item = usize> + '_ {
        let parent = &self.modules[index].path;
        (0..self.modules.len()).filter(move |&m| {
            let path = &self.modules[m].path;
            path.len() == parent.len() + 1 && path.starts_with(parent)
        })
    }

    /// Module `index` and the modules inside it as inline modules, indented to its depth.
    fn inline(&self, index: usize) -> String {
        let module = &self.modules[index];
        let depth = module.path.len();
        let pad = "    ".repeat(depth);
        let mut text = module.uses_text(&pad);
        text.push_str(&indented(&module.items_text(), &pad));
        for child in self.children(index) {
            let inner = self.inline(child);
            if !text.is_empty() && !text.ends_with("\n\n") {
                text.push('\n');
            }
            // A module at the root is visible everywhere; a nested one must be made so.
            let vis = if depth == 0 { "" } else { "pub(crate) " };
            text.push_str(&format!("{pad}{vis}mod {} {{\n{inner}{pad}}}\n", self.modules[child].name()));
        }
        text
    }

    /// Whether module `index` or a module inside it has items.
    fn has_items(&self, index: usize) -> bool {
        !self.modules[index].items.is_empty() || self.children(index).any(|c| self.has_items(c))
    }

    fn file_per_module(&self) -> Vec<SourceFile> {
        let Some(root) = self.modules.iter().position(|m| m.path.is_empty()) else {
            return Vec::new();
        };
        // A program of one package is one file, like a translated file.
        if self.modules.iter().filter(|m| !m.path.is_empty() && !m.items.is_empty()).count() <= 1 {
            return self.files(Layout::OneFile);
        }
        let mut files = Vec::new();
        for (index, module) in self.modules.iter().enumerate() {
            if index != root && module.items.is_empty() {
                continue;
            }
            let mut text = String::new();
            if index == root {
                text.push_str(&self.header);
            } else {
                text.push_str(&format!("//! The Java package `{}`.\n\n", module.package));
            }
            let declarations = self.declarations(index, "");
            if !declarations.is_empty() {
                text.push_str(&declarations);
                text.push('\n');
            }
            text.push_str(&module.uses_text(""));
            text.push_str(&module.items_text());
            if index == root {
                text.push('\n');
                text.push_str(&self.footer);
            }
            let path = if index == root { "main.rs".to_string() } else { module.path.join("/") + ".rs" };
            files.push(SourceFile { path: path.into(), text });
        }
        files
    }

    /// The declarations of the modules inside module `index` that have items: `mod name;` for one
    /// in its own file, and an inline module that declares those inside it for one without items
    /// of its own.
    fn declarations(&self, index: usize, pad: &str) -> String {
        let vis = if self.modules[index].path.is_empty() { "" } else { "pub(crate) " };
        let mut text = String::new();
        for child in self.children(index).filter(|&c| self.has_items(c)) {
            let name = self.modules[child].name();
            if self.modules[child].items.is_empty() {
                let inner = self.declarations(child, &format!("{pad}    "));
                text.push_str(&format!("{pad}{vis}mod {name} {{\n{inner}{pad}}}\n"));
            } else {
                text.push_str(&format!("{pad}{vis}mod {name};\n"));
            }
        }
        text
    }
}

/// `text` with `pad` before each line that is not empty.
fn indented(text: &str, pad: &str) -> String {
    if pad.is_empty() {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len() + text.len() / 8);
    for line in text.split_inclusive('\n') {
        if line != "\n" {
            out.push_str(pad);
        }
        out.push_str(line);
    }
    out
}
