//! A whole program as several crates (`--crate DIR --crates N`): a header crate, `N` body crates
//! and the program's binary (see `docs/several-crates.md`).
//!
//! Crates cannot name each other in a loop, and a translation's classes do: the enums of class
//! hierarchies name every subclass, and Java's packages use each other. So the split is not along
//! classes but between declarations and code, as C splits headers from sources:
//!
//! - The **header** crate has every module and item of the program, made `pub`: the structs,
//!   enums, statics and traits, and every function and method. A function whose code can move
//!   (`movable`) is only declared there, in an `extern "Rust"` block of its module, by its own
//!   name, with the name of its code (`#[link_name = "j2r_.."]`). A method whose code can move
//!   keeps only its signature; its body calls a function declared in that block, by the name of
//!   its code.
//! - Each **body** crate depends on the header alone and defines those functions, with
//!   `#[unsafe(no_mangle)]`, for the items given to it. A module of a body crate re-exports its
//!   header module (`pub use header::a::b::*`) and has the module's own `use` lines, so the moved
//!   code names everything as it did, and the header's `recursion_limit`. It has only the modules
//!   with code of its own and those around them: the parent's glob names any other. A method's
//!   code moves into a private trait that the body crate implements for the header's type (`impl
//!   Bodies_X for X { fn body_m(&self, ..) {..} }`), where `self` and `Self` mean what they did;
//!   the trait is `Sized`, as `X` is, for a signature that names `Option<Self>`. Its function
//!   calls it. An `impl` with much code is cut into several such traits, and a module's code in
//!   a body crate into modules of about `CODEGEN_UNIT` each, as rustc compiles a module's code on
//!   one thread.
//! - The **app** crate is the binary: it depends on all of them, so that the linker finds the
//!   bodies, and its `main` calls the header's.
//!
//! Each body crate builds by itself, in parallel with the others; the header holds no code but
//! what cannot move: generic functions and impls (Rust needs their code where they are used),
//! `const` functions (statics call them), functions with an `impl Trait` in their signature, and
//! the initializers of statics; and the small functions (`small`), which an optimized build
//! inlines where they are called, as it does the header's calls of the moved ones. The items' moved code goes to the body crates in the order of the
//! modules and their items, in runs of about the same size of code whose ends the items' names
//! choose (`chunks`), so the same program is split the same way every time, a change moves only
//! the cuts around it, and a module with more code than a crate's share is spread over several.
//! Writing the workspace leaves unchanged files as they are, so Cargo builds again only the
//! crates that changed.

use crate::output::{ModuleTree, SourceFile};
use proc_macro2::Span;
use std::path::{Path, PathBuf};
use syn::spanned::Spanned;

/// About how much moved code (`Piece::size`) a module of a body crate holds before the rest goes
/// to another module: rustc compiles a module's code in one codegen unit, on one thread.
const CODEGEN_UNIT: usize = 512 * 1024;

/// The `.cargo/config.toml` of a translation's Cargo project on x86-64, one crate's or a split
/// workspace's: it builds for processors with AVX2 (`x86-64-v3`), and builds a binary that is
/// not position-independent (`relocation-model=static`). A position-independent binary is moved
/// to a random address when it starts, and every pointer in its data is fixed up then: the
/// Minecraft server's has 4.6 million of them (the place of every panic message, every table of
/// functions), 105 MB of fix-ups that the loader reads and 61 MB of data that fixing them writes,
/// 166 MB of its peak memory. A binary at a fixed address keeps those pointers in its file, read
/// only where used.
pub const CARGO_CONFIG: &str =
    "[build]\nrustflags = [\"-C\", \"target-cpu=x86-64-v3\", \"-C\", \"relocation-model=static\"]\n";

/// The crates of a split program, each as its files under `src/`.
pub struct Workspace {
    pub header: Vec<SourceFile>,
    /// The body crates, each by its name (`chunks`).
    pub bodies: Vec<(String, Vec<SourceFile>)>,
    pub app: Vec<SourceFile>,
}

impl Workspace {
    /// The workspace in directory `dir`: the header crate, the body crates by their names, and the
    /// binary, named after the program (`name`), with the profiles and target of a translation's
    /// one crate (`write_crate` in `main.rs`). A file whose text is already there is left as it is,
    /// and the body crates of an earlier split that this one does not have are removed, so that
    /// Cargo builds again only the crates that changed.
    pub fn write(&self, dir: &Path, name: &str) -> std::io::Result<()> {
        let workspace = self;
        let name = name.to_lowercase().replace(|c: char| !c.is_ascii_alphanumeric(), "-");
        let runtime: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../runtime").canonicalize()?;
        let runtime = format!("java-runtime = {{ path = {:?} }}\n", runtime.display().to_string());
        let bodies: Vec<&String> = workspace.bodies.iter().map(|(body, _)| body).collect();
        let members: Vec<String> = ["header".to_string()]
            .into_iter()
            .chain(bodies.iter().map(|b| b.to_string()))
            .chain([name.clone()])
            .collect();
        std::fs::create_dir_all(dir)?;
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let stale = entry
                    .file_name()
                    .to_str()
                    .is_some_and(|f| f.starts_with("body-") && !bodies.iter().any(|b| *b == f));
                if stale && entry.path().is_dir() {
                    std::fs::remove_dir_all(entry.path())?;
                }
            }
        }
        write_if_changed(
            &dir.join("Cargo.toml"),
            &format!(
                "[workspace]\nmembers = [{}]\nresolver = \"3\"\n\n\
                 [profile.dev]\ndebug = \"line-tables-only\"\nincremental = false\n\n[profile.release]\nlto = \"fat\"\n",
                members.iter().map(|m| format!("{m:?}")).collect::<Vec<_>>().join(", ")
            ),
        )?;
        if cfg!(target_arch = "x86_64") {
            std::fs::create_dir_all(dir.join(".cargo"))?;
            write_if_changed(&dir.join(".cargo/config.toml"), CARGO_CONFIG)?;
        }
        let package = |crate_dir: &str, deps: &str, files: &[SourceFile]| -> std::io::Result<()> {
            let root = dir.join(crate_dir);
            std::fs::create_dir_all(root.join("src"))?;
            write_if_changed(
                &root.join("Cargo.toml"),
                &format!(
                    "[package]\nname = \"{crate_dir}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n{runtime}{deps}"
                ),
            )?;
            for file in files {
                write_if_changed(&root.join("src").join(&file.path), &file.text)?;
            }
            Ok(())
        };
        package("header", "", &workspace.header)?;
        for (body, files) in &workspace.bodies {
            package(body, "header = { path = \"../header\" }\n", files)?;
        }
        let app_deps: String = std::iter::once("header = { path = \"../header\" }\n".to_string())
            .chain(bodies.iter().map(|b| format!("{b} = {{ path = \"../{b}\" }}\n")))
            .collect();
        package(&name, &app_deps, &workspace.app)
    }
}

/// Writes `text` to `path` unless the file has it already, which keeps its time of change, by
/// which Cargo decides what to build again.
fn write_if_changed(path: &Path, text: &str) -> std::io::Result<()> {
    if std::fs::read(path).is_ok_and(|old| old == text.as_bytes()) {
        return Ok(());
    }
    std::fs::write(path, text)
}

/// An item's moved code.
struct Piece {
    text: String,
    /// The size of its text, wrappers and traits included: the crate root's many small methods
    /// are more wrapper than code.
    size: usize,
    /// The exported name of its first function, which no other piece has.
    key: String,
}

/// A replacement of the text from `start` to `end`.
struct Edit {
    start: usize,
    end: usize,
    text: String,
}

/// A module's text as the header has it, and the code that moves out of it.
struct Split {
    header: String,
    /// The `use` lines among the module's items, which the moved code has wherever it goes.
    uses: String,
    /// The moved code, item by item, each with its size and the name of its first function, which
    /// decide its body crate (`chunks`).
    pieces: Vec<Piece>,
}

/// The program `tree` as a header crate, `crates` body crates and the binary. The code is held
/// once at a time: each module's items go once their text is split, and each part of a split
/// once it is in its crate (a program as large as the Minecraft server is gigabytes of it).
pub fn split(mut tree: ModuleTree, crates: usize) -> Result<Workspace, String> {
    let mut used = std::collections::HashSet::new();
    let mut splits = Vec::new();
    // The items that are not Rust, all of them, rather than the first: each is a bug of the
    // generator, and a census takes long enough that one run should show every one.
    let mut failures = Vec::new();
    for module in &mut tree.modules {
        // Item by item: a parsed module's syntax tree is many times its text, and the largest
        // modules of a program like the Minecraft server are hundreds of megabytes.
        let mut split = Split { header: String::new(), uses: String::new(), pieces: Vec::new() };
        let mut externs = String::new();
        let mut traits = std::collections::HashSet::new();
        for item in std::mem::take(&mut module.items) {
            let part = match split_module(&item.text, &module.path, &mut used, &mut traits) {
                Ok(part) => part,
                Err(e) => {
                    failures.push(format!("splitting the module of package `{}`: {e}", module.package));
                    continue;
                }
            };
            split.header.push_str(&part.split.header);
            split.uses.push_str(&part.split.uses);
            split.pieces.extend(part.split.pieces);
            externs.push_str(&part.externs);
            proc_macro2::extra::invalidate_current_thread_spans();
        }
        if !externs.is_empty() {
            split.header.push_str(&format!("\nunsafe extern \"Rust\" {{\n{externs}}}\n"));
        }
        splits.push(split);
    }
    if !failures.is_empty() {
        return Err(failures.join("\n"));
    }
    let sizes: Vec<Vec<(usize, &str)>> =
        splits.iter().map(|s| s.pieces.iter().map(|p| (p.size, p.key.as_str())).collect()).collect();
    let (crate_of, names) = chunks(&sizes, crates.max(1));
    let crates = names.len();
    // A body crate has the modules that hold code of its own and the modules around them; any
    // other module is the header's, which the glob re-export of its parent names.
    let index: std::collections::HashMap<&[String], usize> =
        tree.modules.iter().enumerate().map(|(i, m)| (m.path.as_slice(), i)).collect();
    let mut kept = vec![vec![false; tree.modules.len()]; crates];
    for (m, of) in crate_of.iter().enumerate() {
        for &c in of {
            let path = &tree.modules[m].path;
            for len in (0..=path.len()).rev() {
                match index.get(&path[..len]) {
                    Some(&outer) if !kept[c][outer] => kept[c][outer] = true,
                    Some(_) => break,
                    None => {}
                }
            }
        }
    }
    let splits = std::cell::RefCell::new(splits);
    let tree = &tree;
    // No warnings, as in the body crates: the Minecraft server's header had thousands
    // (parentheses, unreachable code after a call that throws, enum variants' names), which rustc
    // renders and Cargo keeps with the crate's build.
    let header = format!(
        "{}#![allow(warnings)]\n\n{}",
        tree.header,
        nest(tree, &|_| true, &|m| {
            let uses = uses_text(&tree.modules[m].uses);
            format!("{uses}{}", std::mem::take(&mut splits.borrow_mut()[m].header))
        })
    );
    // The proofs that statics are `Send` go as deep in the body crates as in the header.
    let limit: String = tree
        .header
        .lines()
        .filter(|l| l.starts_with("#![recursion_limit"))
        .map(|l| format!("{l}\n"))
        .collect();
    let mut bodies = Vec::new();
    for (c, kept) in kept.iter().enumerate() {
        let text = nest(tree, &|m| kept[m], &|m| {
            let path = &tree.modules[m].path;
            let reexport =
                if path.is_empty() { "header".to_string() } else { format!("header::{}", path.join("::")) };
            let mut splits = splits.borrow_mut();
            let split = &mut splits[m];
            // rustc puts a module's code in one codegen unit, which one thread compiles: the
            // module's code is cut into modules of its own of about `CODEGEN_UNIT` each, which
            // rustc compiles in parallel.
            let mut units: Vec<String> = Vec::new();
            let mut size = 0;
            for (piece, _) in split.pieces.iter_mut().zip(&crate_of[m]).filter(|(_, of)| **of == c) {
                if units.is_empty() || (size > 0 && size + piece.size > CODEGEN_UNIT) {
                    units.push(String::new());
                    size = 0;
                }
                units.last_mut().expect("a unit").push_str(&std::mem::take(&mut piece.text));
                size += piece.size;
            }
            let code: String = match units.len() {
                0 => String::new(),
                1 => format!("{}{}", split.uses, units[0]),
                _ => units
                    .iter()
                    .enumerate()
                    .map(|(i, unit)| {
                        format!("\nmod j2r_code_{i} {{\nuse super::*;\n{}{unit}}}\n", split.uses)
                    })
                    .collect(),
            };
            format!("pub use {reexport}::*;\n{}{code}", uses_text(&tree.modules[m].uses))
        });
        let text = format!(
            "//! Code of the translated program (see the header crate).\n\
             #![allow(warnings)]\n{limit}\n{text}"
        );
        bodies.push((names[c].clone(), vec![SourceFile { path: "lib.rs".into(), text }]));
    }
    let links: String = names.iter().map(|name| format!("use {} as _;\n", name.replace('-', "_"))).collect();
    let app = format!("{links}\nfn main() {{\n    header::main()\n}}\n\n{}", tree.footer);
    Ok(Workspace {
        header: vec![SourceFile { path: "lib.rs".into(), text: header }],
        bodies,
        app: vec![SourceFile { path: "main.rs".into(), text: app }],
    })
}

/// The body crate of each piece, module by module, from each piece's size and key, and the names
/// of the crates, about `crates` of them. The pieces, in the order of the modules and their items,
/// are cut into runs of about the same size of code, at places that the pieces' keys choose
/// rather than their positions: a run ends before a piece whose key's hash is a multiple of a
/// power of two, once it has half a share of the code, or at twice a share. So a piece added,
/// removed or changed moves the cuts around it only, the cuts beyond come back to where they
/// were, and each run's crate, named after the hash of its first piece's key, has the same name
/// and code as before: after a change, Cargo builds again only the crates whose code changed. The
/// share and the power of two are rounded to powers of two, so that a program that grows a
/// little keeps them. A module whose code is more than a share (the crate root's, with the
/// `Object` enum's methods and the reflection table) is spread over several crates.
fn chunks(pieces: &[Vec<(usize, &str)>], crates: usize) -> (Vec<Vec<usize>>, Vec<String>) {
    let total: usize = pieces.iter().flatten().map(|(size, _)| size).sum();
    let count = pieces.iter().map(Vec::len).sum::<usize>().max(1);
    let round = |x: usize| 1usize << (x.max(1) as f64).log2().round() as u32;
    let share = round(total / crates);
    // A piece starts a run once in `every` pieces on average: about a share's worth of pieces
    // after the run's first half.
    let every = round(count * share / 2 / total.max(1));
    let mut names: Vec<String> = Vec::new();
    let mut size = 0;
    let crate_of = pieces
        .iter()
        .map(|module| {
            module
                .iter()
                .map(|&(piece, key)| {
                    let hash = stable_hash(key);
                    let cut = names.is_empty()
                        || (size >= share / 2 && hash.is_multiple_of(every as u64))
                        || size + piece > 2 * share;
                    if cut {
                        let mut name = format!("body-{:010x}", hash >> 24);
                        while names.contains(&name) {
                            name.push('x');
                        }
                        names.push(name);
                        size = 0;
                    }
                    size += piece;
                    names.len() - 1
                })
                .collect()
        })
        .collect();
    if names.is_empty() {
        names.push("body-empty".into());
    }
    (crate_of, names)
}

/// FNV-1a: a hash that every build of the translator computes the same.
fn stable_hash(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, b| (hash ^ b as u64).wrapping_mul(0x0100_0000_01b3))
}

/// The `use` lines of a module, a blank line after each group.
fn uses_text(uses: &[String]) -> String {
    uses.iter().map(|group| format!("{group}\n\n")).collect()
}

/// The modules of `tree` that `keep` keeps as inline modules, from the root, each module's text
/// made by `text`.
fn nest(tree: &ModuleTree, keep: &dyn Fn(usize) -> bool, text: &dyn Fn(usize) -> String) -> String {
    fn inline(
        tree: &ModuleTree,
        index: usize,
        keep: &dyn Fn(usize) -> bool,
        text: &dyn Fn(usize) -> String,
    ) -> String {
        let path = &tree.modules[index].path;
        let mut out = text(index);
        for child in 0..tree.modules.len() {
            let inner = &tree.modules[child].path;
            if inner.len() == path.len() + 1 && inner.starts_with(path) && keep(child) {
                let name = inner.last().map_or("", String::as_str);
                out.push_str(&format!("\npub mod {name} {{\n{}}}\n", inline(tree, child, keep, text)));
            }
        }
        out
    }
    match tree.modules.iter().position(|m| m.path.is_empty()) {
        Some(root) => inline(tree, root, keep, text),
        None => String::new(),
    }
}

/// The text of `span` in `text`.
fn at(text: &str, span: Span) -> &str {
    &text[span.byte_range()]
}

/// Whether the code of a function with signature `sig` can move to a body crate.
fn movable(sig: &syn::Signature, text: &str) -> bool {
    sig.generics.params.is_empty()
        && sig.generics.where_clause.is_none()
        && sig.constness.is_none()
        && sig.asyncness.is_none()
        && sig.unsafety.is_none()
        && sig.abi.is_none()
        && sig.variadic.is_none()
        && !has_word(at(text, sig.span()), "impl")
}

/// The most tokens that the code of a function kept in the header has (`small`).
const SMALL_TOKENS: usize = 48;

/// Whether `code`, a function's block, is small enough to stay in the header, `#[inline]`, where
/// every crate that calls it can inline it: a getter, a buffer's `get`, a class's initialization
/// check, `Math.lerp`. A call into a body crate cannot be inlined without link-time optimization,
/// and the call of such a function costs more than its code.
///
/// The arms of a `match` that passes a failure on (`match x { Ok(v) => v, Err(e) => return
/// Err(..) }`, `Some(v) => v, None => ..`, which `codegen::exceptions` writes for rustc's sake
/// where Java code calls a method that throws) count as one token, as the `?` they stand for did.
/// So does a path (`crate::net::minecraft::world::level::biome::ClimateParameter`), one name
/// however deep its package: a cast of an `Object` to a class (`Object::as_x`, a compare of the
/// class number) is as small in a program of many packages as in one of none.
fn small(code: &str) -> bool {
    fn passes_on(stream: &proc_macro2::TokenStream) -> bool {
        let first: String = stream.clone().into_iter().take(6).map(|t| t.to_string()).collect();
        first == "Ok(v)=>v," || first == "Some(v)=>v,"
    }
    fn count(stream: proc_macro2::TokenStream, limit: usize) -> usize {
        let mut n = 0;
        let mut tokens = stream.into_iter().peekable();
        while let Some(token) = tokens.next() {
            // `::` and the name after it continue the path before them.
            if let proc_macro2::TokenTree::Punct(p) = &token
                && p.as_char() == ':'
                && p.spacing() == proc_macro2::Spacing::Joint
                && matches!(tokens.peek(), Some(proc_macro2::TokenTree::Punct(q)) if q.as_char() == ':')
            {
                tokens.next();
                if matches!(tokens.peek(), Some(proc_macro2::TokenTree::Ident(_))) {
                    tokens.next();
                }
                continue;
            }
            n += 1;
            if let proc_macro2::TokenTree::Group(g) = token
                && !(g.delimiter() == proc_macro2::Delimiter::Brace && passes_on(&g.stream()))
            {
                n += count(g.stream(), limit);
            }
            if n > limit {
                break;
            }
        }
        n
    }
    // A token is at least a byte, and most are several: a longer text is not small.
    code.len() <= SMALL_TOKENS * 12
        && code.parse::<proc_macro2::TokenStream>().is_ok_and(|s| count(s, SMALL_TOKENS) <= SMALL_TOKENS)
}

/// Whether a function with attributes `attrs` may be marked `#[inline]`: it has no inline
/// attribute of its own and is not `#[cold]`.
fn inlinable(attrs: &[syn::Attribute]) -> bool {
    !attrs.iter().any(|a| a.path().is_ident("inline") || a.path().is_ident("cold"))
}

/// Whether a function with attributes `attrs` and block `code` stays in the header for its size
/// (`small`): it is neither `#[cold]` nor `#[inline(never)]`. One that the generator marked
/// `#[inline]` itself stays as one that the split marks does: a class's initialization check
/// (`init_x`, whose `init_x_slow` moves), a variant's constructor, the view of an enum.
fn stays(attrs: &[syn::Attribute], code: &str) -> bool {
    let never = |a: &syn::Attribute| {
        a.path().is_ident("cold")
            || a.path().is_ident("inline")
                && matches!(&a.meta, syn::Meta::List(l) if l.tokens.to_string() == "never")
    };
    !attrs.iter().any(never) && small(code)
}

/// An edit that marks a function `#[inline]` in a build that optimizes (without debug
/// assertions), before its visibility or its signature. A build at `opt-level = 0` inlines
/// nothing, and there each crate that calls an `#[inline]` function compiles a copy of it.
fn mark_inline(vis: &syn::Visibility, sig: &syn::Signature) -> Edit {
    let start = match vis {
        syn::Visibility::Inherited => sig_start(sig),
        _ => vis.span().byte_range().start,
    };
    Edit { start, end: start, text: "#[cfg_attr(not(debug_assertions), inline)] ".into() }
}

/// Whether `text` has `word` as a whole word.
fn has_word(text: &str, word: &str) -> bool {
    replace_word(text, word, "\u{0}") != text
}

/// `text` with every whole word `word` replaced by `with`.
fn replace_word(text: &str, word: &str, with: &str) -> String {
    let is_ident = |c: char| c.is_alphanumeric() || c == '_';
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find(word) {
        let before = rest[..i].chars().next_back();
        let after = rest[i + word.len()..].chars().next();
        out.push_str(&rest[..i]);
        if before.is_some_and(is_ident) || after.is_some_and(is_ident) {
            out.push_str(word);
        } else {
            out.push_str(with);
        }
        rest = &rest[i + word.len()..];
    }
    out.push_str(rest);
    out
}

/// A name for the function of `name` in module `path` and impl `owner`, used nowhere else.
fn symbol(path: &[String], owner: &str, name: &str, used: &mut std::collections::HashSet<String>) -> String {
    let clean = |s: &str| s.chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect::<String>();
    let mut base = format!("j2r__{}__{}__{}", clean(&path.join("__")), clean(owner), clean(name));
    base = base.replace("____", "__");
    let mut symbol = base.clone();
    let mut n = 1;
    while !used.insert(symbol.clone()) {
        n += 1;
        symbol = format!("{base}_{n}");
    }
    symbol
}

/// Edits that make `vis` public, before the token at `start` when it has none.
fn publish(vis: &syn::Visibility, start: usize, edits: &mut Vec<Edit>) {
    match vis {
        syn::Visibility::Public(_) => {}
        syn::Visibility::Restricted(r) => {
            let range = r.span().byte_range();
            edits.push(Edit { start: range.start, end: range.end, text: "pub".into() });
        }
        syn::Visibility::Inherited => edits.push(Edit { start, end: start, text: "pub ".into() }),
    }
}

/// The start of a signature: its first qualifier or `fn`.
fn sig_start(sig: &syn::Signature) -> usize {
    [
        sig.constness.as_ref().map(|t| t.span),
        sig.asyncness.as_ref().map(|t| t.span),
        sig.unsafety.as_ref().map(|t| t.span),
        sig.abi.as_ref().map(|a| a.extern_token.span),
        Some(sig.fn_token.span),
    ]
    .into_iter()
    .flatten()
    .map(|s| s.byte_range().start)
    .min()
    .unwrap_or(0)
}

/// One item's split, and the declarations of its moved functions for the module's
/// `extern "Rust"` block.
struct Part {
    split: Split,
    externs: String,
}

/// Splits `text`, items of the module at `path`; `traits` holds the names of the module's traits
/// of moved methods, each named after its type (`Bodies_Circle`), so that a trait keeps its
/// name whatever is added before it.
fn split_module(
    text: &str,
    path: &[String],
    used: &mut std::collections::HashSet<String>,
    traits: &mut std::collections::HashSet<String>,
) -> Result<Part, String> {
    let file = text
        .parse::<proc_macro2::TokenStream>()
        .map_err(syn::Error::from)
        .and_then(|tokens| syn::parse2::<syn::File>(hollow(tokens, true)))
        .map_err(|e| {
            // The code around the error, which a translator's fault in the generated text names.
            let at = e.span().start();
            let line = text.lines().nth(at.line.saturating_sub(1)).unwrap_or_default();
            let around: String = line.chars().skip(at.column.saturating_sub(120)).take(240).collect();
            format!("{e} at line {}, column {}: {around}", at.line, at.column)
        })?;
    let mut edits = Vec::new();
    let mut externs = String::new();
    let mut uses = String::new();
    let mut pieces = Vec::new();
    for item in &file.items {
        let (mut body, mut key) = (String::new(), None);
        match item {
            syn::Item::Struct(s) => {
                publish(&s.vis, s.struct_token.span.byte_range().start, &mut edits);
                for field in &s.fields {
                    let start = match &field.ident {
                        Some(ident) => ident.span().byte_range().start,
                        None => field.ty.span().byte_range().start,
                    };
                    publish(&field.vis, start, &mut edits);
                }
            }
            syn::Item::Enum(e) => publish(&e.vis, e.enum_token.span.byte_range().start, &mut edits),
            syn::Item::Union(u) => publish(&u.vis, u.union_token.span.byte_range().start, &mut edits),
            syn::Item::Static(s) => publish(&s.vis, s.static_token.span.byte_range().start, &mut edits),
            syn::Item::Const(c) => publish(&c.vis, c.const_token.span.byte_range().start, &mut edits),
            syn::Item::Type(t) => publish(&t.vis, t.type_token.span.byte_range().start, &mut edits),
            syn::Item::Trait(t) => {
                let start = [t.unsafety.as_ref().map(|u| u.span), Some(t.trait_token.span)]
                    .into_iter()
                    .flatten()
                    .map(|s| s.byte_range().start)
                    .min()
                    .unwrap_or(0);
                publish(&t.vis, start, &mut edits);
            }
            syn::Item::Macro(m) if m.mac.path.is_ident("thread_local") => {
                // `thread_local! { static X: T = ..; }`: each static made public.
                let syn::MacroDelimiter::Brace(braces) = &m.mac.delimiter else { continue };
                let whole = braces.span.join().byte_range();
                let range = whole.start + 1..whole.end - 1;
                let inner = &text[range.clone()];
                let private = ["pub(crate) static", "pub(super) static", "pub static"]
                    .iter()
                    .fold(inner.to_string(), |text, visible| replace_word(&text, visible, "static"));
                let published = replace_word(&private, "static", "pub static");
                edits.push(Edit { start: range.start, end: range.end, text: published });
            }
            syn::Item::Fn(f) => {
                let block = f.block.span().byte_range();
                let can_move = movable(&f.sig, text);
                // A `#[cold]` function is not inlined, so it moves whatever its size.
                let moves = can_move && !stays(&f.attrs, &text[block.clone()]);
                if !moves {
                    // A function kept in the header for its size is inlined where it is called.
                    // The mark goes before the edit that makes the function public, at the same
                    // place.
                    if can_move && inlinable(&f.attrs) {
                        edits.push(mark_inline(&f.vis, &f.sig));
                    }
                    publish(&f.vis, sig_start(&f.sig), &mut edits);
                } else {
                    let name = f.sig.ident.to_string();
                    let symbol = symbol(path, "", &name, used);
                    key.get_or_insert_with(|| symbol.clone());
                    let (params, _) = rename_params(&f.sig, text, &mut Vec::new(), None);
                    let ret = at_or_empty(text, &f.sig.output);
                    // `#[inline]` does nothing on an exported function, and is not allowed on a
                    // declaration.
                    let kept: Vec<&str> = f
                        .attrs
                        .iter()
                        .map(|a| at(text, a.span()))
                        .filter(|a| {
                            !(a.starts_with("#[inline")
                                || a.starts_with("#[cfg_attr") && has_word(a, "inline"))
                        })
                        .collect();
                    // The function is its declaration, by its own name, which callers name as
                    // they did: a function in the header that called the declaration would cost
                    // rustc three times what the declaration does (`docs/several-crates.md`).
                    let vis = match f.vis {
                        syn::Visibility::Inherited => None,
                        _ => Some(f.vis.span()),
                    };
                    let start = [f.attrs.first().map(|a| a.span()), vis]
                        .into_iter()
                        .flatten()
                        .map(|s| s.byte_range().start)
                        .chain([sig_start(&f.sig)])
                        .min()
                        .unwrap_or(block.start);
                    edits.push(Edit { start, end: block.end, text: String::new() });
                    let decl_attrs: String = kept.iter().map(|a| format!("    {a}\n")).collect();
                    externs.push_str(&format!(
                        "{decl_attrs}    #[link_name = \"{symbol}\"]\n    pub safe fn {name}({params}){ret};\n"
                    ));
                    let attrs: String = kept.iter().map(|a| format!("{a}\n")).collect();
                    let inputs = f.sig.inputs.span().byte_range();
                    let inputs = if f.sig.inputs.is_empty() { "" } else { &text[inputs] };
                    let code = &text[block];
                    body.push_str(&format!(
                        "{attrs}#[unsafe(no_mangle)]\npub fn {symbol}({inputs}){ret} {code}\n\n"
                    ));
                }
            }
            syn::Item::Impl(imp) => {
                let self_ty = at(text, imp.self_ty.span()).to_string();
                let plain = imp.generics.params.is_empty()
                    && imp.generics.where_clause.is_none()
                    && !self_ty.contains('<')
                    && matches!(*imp.self_ty, syn::Type::Path(_));
                let trait_name = imp.trait_.as_ref().map(|(_, p, _)| at(text, p.span()).to_string());
                // Each moved method: its exported name, and its declaration and code in the trait.
                let mut moved: Vec<(String, String, String)> = Vec::new();
                let clean = |s: &str| {
                    s.chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect::<String>()
                };
                let owner_name = match &trait_name {
                    Some(t) => format!("{}__{}", clean(&self_ty), clean(t)),
                    None => clean(&self_ty),
                };
                for impl_item in &imp.items {
                    let syn::ImplItem::Fn(f) = impl_item else {
                        if let syn::ImplItem::Const(c) = impl_item
                            && trait_name.is_none()
                        {
                            publish(&c.vis, c.const_token.span.byte_range().start, &mut edits);
                        }
                        continue;
                    };
                    let sig_text = at(text, f.sig.span());
                    let can_move = plain
                        && movable(&f.sig, text)
                        && !(trait_name.is_some() && sig_text.contains("Self::"));
                    // As for a function: a small method stays, and both it and the header's
                    // call of a moved one are inlined where they are called.
                    if can_move && inlinable(&f.attrs) {
                        edits.push(mark_inline(&f.vis, &f.sig));
                    }
                    if trait_name.is_none() {
                        publish(&f.vis, sig_start(&f.sig), &mut edits);
                    }
                    if !can_move || stays(&f.attrs, at(text, f.block.span())) {
                        continue;
                    }
                    let name = f.sig.ident.to_string();
                    let owner = match &trait_name {
                        Some(t) => format!("{self_ty}__{t}"),
                        None => self_ty.clone(),
                    };
                    let symbol = symbol(path, &owner, &name, used);
                    let receiver = f.sig.receiver();
                    let this_ty = receiver.map(|r| {
                        if r.colon_token.is_some() {
                            replace_word(at(text, r.ty.span()), "Self", &self_ty)
                        } else {
                            let reference = match (&r.reference, &r.mutability) {
                                (Some(_), Some(_)) => "&mut ",
                                (Some(_), None) => "&",
                                (None, _) => "",
                            };
                            format!("{reference}{self_ty}")
                        }
                    });
                    let (params, args) = rename_params(&f.sig, text, &mut edits, Some(&self_ty));
                    let ret = at_or_empty(text, &f.sig.output);
                    let ret_here = replace_word(&ret, "Self", &self_ty);
                    let block = f.block.span().byte_range();
                    let (call_this, this_param) = match &this_ty {
                        Some(ty) => ("self, ", format!("this: {ty}, ")),
                        None => ("", String::new()),
                    };
                    edits.push(Edit {
                        start: block.start,
                        end: block.end,
                        text: format!("{{ {symbol}({call_this}{args}) }}"),
                    });
                    externs.push_str(&format!("    safe fn {symbol}({this_param}{params}){ret_here};\n"));
                    // The trait's method: the receiver as declared, without `mut`, and the
                    // parameters by the header's names.
                    let decl_receiver = match receiver {
                        Some(r) if r.colon_token.is_some() => format!("self: {}, ", at(text, r.ty.span())),
                        Some(r) => match (&r.reference, &r.mutability) {
                            (Some(_), Some(_)) => "&mut self, ".to_string(),
                            (Some(_), None) => "&self, ".to_string(),
                            (None, _) => "self, ".to_string(),
                        },
                        None => String::new(),
                    };
                    let decl_params = trait_params(&f.sig, text);
                    let decl = format!("    fn body_{name}({decl_receiver}{decl_params}){ret};\n");
                    let attrs: String =
                        f.attrs.iter().map(|a| format!("    {}\n", at(text, a.span()))).collect();
                    let inputs =
                        if f.sig.inputs.is_empty() { "" } else { &text[f.sig.inputs.span().byte_range()] };
                    let code = &text[block];
                    // The method itself is the function that the header declares: a method's
                    // `self` is passed as a first parameter of its type is.
                    let code = format!(
                        "{attrs}    #[unsafe(export_name = \"{symbol}\")]\n    fn body_{name}({inputs}){ret} {code}\n"
                    );
                    moved.push((symbol, decl, code));
                }
                // An `impl` with much code (`Object`'s methods over every class) is cut into
                // traits of about `CODEGEN_UNIT` of code each, each a piece of its own, which the
                // split spreads over crates and modules as it does any item.
                let mut start = 0;
                while start < moved.len() {
                    let mut end = start + 1;
                    let mut size = moved[start].2.len();
                    while end < moved.len() && size + moved[end].2.len() <= CODEGEN_UNIT {
                        size += moved[end].2.len();
                        end += 1;
                    }
                    let mut bodies = format!("Bodies_{owner_name}");
                    let mut n = 1;
                    while traits.contains(&bodies) {
                        n += 1;
                        bodies = format!("Bodies_{owner_name}_{n}");
                    }
                    let methods = &moved[start..end];
                    let decls: String = methods.iter().map(|m| m.1.as_str()).collect();
                    let impls: String = methods.iter().map(|m| m.2.as_str()).collect();
                    let text = format!(
                        "trait {bodies}: Sized {{\n{decls}}}\n\nimpl {bodies} for {self_ty} {{\n{impls}}}\n\n"
                    );
                    pieces.push(Piece { size: text.len(), text, key: methods[0].0.clone() });
                    traits.insert(bodies);
                    start = end;
                }
            }
            // A `use` among the items is the moved code's too.
            syn::Item::Use(u) => {
                uses.push_str(at(text, u.span()));
                uses.push('\n');
            }
            _ => {}
        }
        if let Some(key) = key.filter(|_| !body.is_empty()) {
            pieces.push(Piece { size: body.len(), text: body, key });
        }
    }
    edits.sort_by_key(|e| (e.start, e.end));
    let mut header = String::with_capacity(text.len());
    let mut last = 0;
    for edit in &edits {
        if edit.start < last {
            return Err(format!("overlapping edits at byte {}", edit.start));
        }
        header.push_str(&text[last..edit.start]);
        header.push_str(&edit.text);
        last = edit.end;
    }
    header.push_str(&text[last..]);
    Ok(Part { split: Split { header, uses, pieces }, externs })
}

/// `tokens` with the code of every function of the module, or of an `impl`, a trait or any other
/// item's braces at its top (`top`), taken out: the braces after `fn` hold nothing, with the
/// place of the code (its span) kept. The split reads a function's signature and where its code
/// is, and takes the code's text from the module's text: syn parsing every body into a syntax tree,
/// printing it again to find its place (`Spanned::span`) and dropping it took most of the time of
/// the write, and that of a large program's crate root most of all.
fn hollow(tokens: proc_macro2::TokenStream, top: bool) -> proc_macro2::TokenStream {
    use proc_macro2::{Delimiter, Group, TokenStream, TokenTree};
    let mut after_fn = false;
    tokens
        .into_iter()
        .map(|token| match token {
            TokenTree::Ident(ident) => {
                after_fn |= ident == "fn";
                TokenTree::Ident(ident)
            }
            // A `fn` type of a static or a type alias ends with its item.
            TokenTree::Punct(punct) => {
                after_fn &= punct.as_char() != ';';
                TokenTree::Punct(punct)
            }
            TokenTree::Group(group) if group.delimiter() == Delimiter::Brace && after_fn => {
                after_fn = false;
                let mut empty = Group::new(Delimiter::Brace, TokenStream::new());
                empty.set_span(group.span());
                TokenTree::Group(empty)
            }
            TokenTree::Group(group) if group.delimiter() == Delimiter::Brace && top => {
                let mut inner = Group::new(Delimiter::Brace, hollow(group.stream(), false));
                inner.set_span(group.span());
                TokenTree::Group(inner)
            }
            other => other,
        })
        .collect()
}

/// The text of a function's return type (` -> T`), or nothing.
fn at_or_empty(text: &str, output: &syn::ReturnType) -> String {
    match output {
        syn::ReturnType::Default => String::new(),
        syn::ReturnType::Type(..) => format!(" {}", at(text, output.span())),
    }
}

/// Renames the parameters of `sig` other than `self` to `p0`, `p1`, .. in the header (an edit of
/// each pattern), and gives them as `p0: T0, ..` (each `Self` in a type replaced by `self_ty`) and
/// as the arguments `p0, ..`.
fn rename_params(
    sig: &syn::Signature,
    text: &str,
    edits: &mut Vec<Edit>,
    self_ty: Option<&str>,
) -> (String, String) {
    let mut params = Vec::new();
    let mut args = Vec::new();
    for (k, input) in sig
        .inputs
        .iter()
        .filter_map(|i| match i {
            syn::FnArg::Typed(t) => Some(t),
            syn::FnArg::Receiver(_) => None,
        })
        .enumerate()
    {
        let range = input.pat.span().byte_range();
        edits.push(Edit { start: range.start, end: range.end, text: format!("p{k}") });
        let ty = at(text, input.ty.span());
        let ty = match self_ty {
            Some(s) => replace_word(ty, "Self", s),
            None => ty.to_string(),
        };
        params.push(format!("p{k}: {ty}"));
        args.push(format!("p{k}"));
    }
    (params.join(", "), args.join(", "))
}

/// The parameters of `sig` other than `self` as a trait declares them: `p0: T0, ..`.
fn trait_params(sig: &syn::Signature, text: &str) -> String {
    sig.inputs
        .iter()
        .filter_map(|i| match i {
            syn::FnArg::Typed(t) => Some(at(text, t.ty.span())),
            syn::FnArg::Receiver(_) => None,
        })
        .enumerate()
        .map(|(k, ty)| format!("p{k}: {ty}"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The statics of a `thread_local!` in a package's module, which the generator declares
    /// `pub(crate)`, are public in the header, as those of the root module are.
    #[test]
    fn every_item_that_is_not_rust_is_reported() {
        let module = |package: &str, text: &str| crate::output::Module {
            package: package.to_string(),
            path: vec![package.to_string()],
            uses: Vec::new(),
            items: vec![crate::output::Item {
                origin: crate::output::Origin::Statics,
                text: text.to_string(),
                refers: Default::default(),
                names: Default::default(),
            }],
        };
        let tree = ModuleTree {
            header: String::new(),
            modules: vec![
                module("a", "struct S { x: }\n"),
                module("b", "fn ok() {}\n"),
                module("c", "fn h( {}\n"),
            ],
            footer: String::new(),
        };
        let Err(e) = split(tree, 2) else { panic!("items that are not Rust were split") };
        assert!(e.contains("package `a`") && e.contains("package `c`") && !e.contains("package `b`"), "{e}");
    }

    /// A piece that grows, and one added, change the crates around them only: the others keep
    /// their names and their pieces.
    #[test]
    fn a_change_moves_the_cuts_around_it_only() {
        let keys: Vec<String> = (0..20_000).map(|i| format!("j2r__p{}__C{i}__m", i / 500)).collect();
        let sizes = |keys: &[String], grown: usize| -> Vec<Vec<(usize, String)>> {
            keys.chunks(500)
                .map(|m| {
                    m.iter()
                        .map(|k| (if *k == keys[grown] { 90_000 } else { 1_000 + k.len() * 7 }, k.clone()))
                        .collect()
                })
                .collect()
        };
        let crates_of =
            |modules: &[Vec<(usize, String)>]| -> std::collections::BTreeMap<String, Vec<String>> {
                let borrowed: Vec<Vec<(usize, &str)>> =
                    modules.iter().map(|m| m.iter().map(|(s, k)| (*s, k.as_str())).collect()).collect();
                let (of, names) = chunks(&borrowed, 16);
                let mut crates = std::collections::BTreeMap::new();
                for (module, of) in modules.iter().zip(&of) {
                    for ((_, key), &c) in module.iter().zip(of) {
                        crates.entry(names[c].clone()).or_insert_with(Vec::new).push(key.clone());
                    }
                }
                crates
            };
        let before = crates_of(&sizes(&keys, 0));
        assert!((8..=32).contains(&before.len()), "{} crates", before.len());
        let mut added = keys.clone();
        added.insert(12_345, "j2r__p24__New__m".into());
        for after in [crates_of(&sizes(&keys, 7_000)), crates_of(&sizes(&added, 0))] {
            let kept = before.iter().filter(|(name, keys)| after.get(*name) == Some(keys)).count();
            assert!(kept + 3 >= before.len(), "{kept} of {} crates kept", before.len());
        }
    }

    /// A small function or method keeps its code in the header, inlined where an optimized build
    /// calls it; a larger one moves: a function is then declared alone, and the header's call of
    /// a method is inlined so.
    #[test]
    fn small_functions_stay_in_the_header() {
        let large = (0..40).map(|i| format!("    let v{i} = x + {i};\n")).collect::<String>();
        let text = format!(
            "fn get(x: i32) -> i32 {{ x + 1 }}\n\nfn sum(x: i32) -> i32 {{\n{large}    x\n}}\n\n\
             #[cold]\n#[inline(never)]\nfn slow() {{}}\n\n\
             #[inline]\nfn init_a() {{ if A.get() {{ return; }} slow() }}\n\n\
             fn deep(o: &O) -> Option<a::b::c::d::e::f::g::h::i::j::K> {{ match o.tag() {{ 7 => {{ let k: &a::b::c::d::e::f::g::h::i::j::K = o.get(); Some(k.clone()) }} _ => None, }} }}\n\n\
             pub struct S;\n\nimpl S {{\n    fn size(&self) -> i32 {{ 3 }}\n    fn total(&self, x: i32) -> i32 {{\n{large}        x\n    }}\n}}\n"
        );
        let part = split_module(&text, &["p".to_string()], &mut Default::default(), &mut Default::default())
            .unwrap();
        let split = &part.split;
        let inline = "#[cfg_attr(not(debug_assertions), inline)] ";
        let header = &split.header;
        assert!(header.contains(&format!("{inline}pub fn get(x: i32) -> i32 {{ x + 1 }}")), "{header}");
        assert!(!header.contains("fn sum(") && !header.contains("fn slow("), "{header}");
        // So does one that the generator marked `#[inline]` itself, a class's initialization check.
        assert!(header.contains("#[inline]\npub fn init_a() { if A.get() { return; } slow() }"), "{header}");
        assert!(header.contains(&format!("{inline}pub fn size(&self) -> i32 {{ 3 }}")), "{header}");
        // A path is one token, however long.
        assert!(header.contains(&format!("{inline}pub fn deep(")), "{header}");
        assert!(header.contains(&format!("{inline}pub fn total(&self, p0: i32)")), "{header}");
        // A moved function is its declaration, which keeps its attributes but `#[inline]`.
        let externs = &part.externs;
        assert!(
            externs.contains("    #[link_name = \"j2r__p__sum\"]\n    pub safe fn sum(p0: i32) -> i32;"),
            "{externs}"
        );
        assert!(
            externs.contains("    #[cold]\n    #[link_name = \"j2r__p__slow\"]\n    pub safe fn slow();"),
            "{externs}"
        );
        let moved: String = split.pieces.iter().map(|p| p.text.as_str()).collect();
        assert!(
            !moved.contains("fn get") && !moved.contains("body_size") && !moved.contains("init_a"),
            "{moved}"
        );
        assert!(moved.contains("fn j2r__p__sum") && moved.contains("fn body_total"), "{moved}");
        assert!(moved.contains("fn j2r__p__slow"), "{moved}");
    }

    #[test]
    fn thread_local_statics_of_any_visibility_become_public() {
        let text = "thread_local! {\n    pub(crate) static A__INITIALIZED: Cell<bool> = const { Cell::new(false) };\n    static B: Cell<i32> = const { Cell::new(0) };\n    pub static C: Cell<i32> = const { Cell::new(0) };\n}\n";
        let split = split_module(text, &["p".to_string()], &mut Default::default(), &mut Default::default())
            .unwrap()
            .split;
        for name in ["A__INITIALIZED", "B", "C"] {
            assert!(split.header.contains(&format!("    pub static {name}:")), "{}", split.header);
        }
        assert!(!split.header.contains("pub(crate)"), "{}", split.header);
    }
}
