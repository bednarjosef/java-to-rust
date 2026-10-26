//! A whole program as several crates (`--crate DIR --crates N`): a header crate, `N` body crates
//! and the program's binary (see `docs/several-crates.md`).
//!
//! Crates cannot name each other in a loop, and a translation's classes do: the enums of class
//! hierarchies name every subclass, and Java's packages use each other. So the split is not along
//! classes but between declarations and code, as C splits headers from sources:
//!
//! - The **header** crate has every module and item of the program, made `pub`: the structs,
//!   enums, statics and traits, and every function and method. A function or method whose code
//!   can move (`movable`) keeps only its signature there; its body calls a function declared in an
//!   `extern "Rust"` block of its module, by a name of its own (`j2r_..`).
//! - Each **body** crate depends on the header alone and defines those functions, with
//!   `#[unsafe(no_mangle)]`, for the modules given to it. A module of a body crate re-exports its
//!   header module (`pub use header::a::b::*`) and has the module's own `use` lines, so the moved
//!   code names everything as it did, and the header's `recursion_limit`. A method's code moves
//!   into a private trait that the body crate implements for the header's type (`impl Bodies0 for
//!   X { fn body_m(&self, ..) {..} }`), where `self` and `Self` mean what they did; the trait is
//!   `Sized`, as `X` is, for a signature that names `Option<Self>`. Its function calls it.
//! - The **app** crate is the binary: it depends on all of them, so that the linker finds the
//!   bodies, and its `main` calls the header's.
//!
//! Each body crate builds by itself, in parallel with the others; the header holds no code but
//! what cannot move: generic functions and impls (Rust needs their code where they are used),
//! `const` functions (statics call them), functions with an `impl Trait` in their signature, and
//! the initializers of statics. The modules go to the body crates in their order, in runs of about
//! the same size of code, so the same program is split the same way every time.

use crate::output::{ModuleTree, SourceFile};
use proc_macro2::Span;
use std::path::{Path, PathBuf};
use syn::spanned::Spanned;

/// The crates of a split program, each as its files under `src/`.
pub struct Workspace {
    pub header: Vec<SourceFile>,
    pub bodies: Vec<Vec<SourceFile>>,
    pub app: Vec<SourceFile>,
}

impl Workspace {
    /// The workspace in directory `dir`: the header crate, the body crates `body-0`, .., and the
    /// binary, named after the program (`name`), with the profiles and target of a translation's
    /// one crate (`write_crate` in `main.rs`).
    pub fn write(&self, dir: &Path, name: &str) -> std::io::Result<()> {
        let workspace = self;
        let name = name.to_lowercase().replace(|c: char| !c.is_ascii_alphanumeric(), "-");
        let runtime: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../runtime").canonicalize()?;
        let runtime = format!("java-runtime = {{ path = {:?} }}\n", runtime.display().to_string());
        let bodies: Vec<String> = (0..workspace.bodies.len()).map(|c| format!("body-{c}")).collect();
        let members: Vec<String> =
            ["header".to_string()].into_iter().chain(bodies.iter().cloned()).chain([name.clone()]).collect();
        std::fs::create_dir_all(dir)?;
        std::fs::write(
            dir.join("Cargo.toml"),
            format!(
                "[workspace]\nmembers = [{}]\nresolver = \"3\"\n\n\
                 [profile.dev]\ndebug = \"line-tables-only\"\n\n[profile.release]\nlto = \"fat\"\n",
                members.iter().map(|m| format!("{m:?}")).collect::<Vec<_>>().join(", ")
            ),
        )?;
        if cfg!(target_arch = "x86_64") {
            std::fs::create_dir_all(dir.join(".cargo"))?;
            std::fs::write(
                dir.join(".cargo/config.toml"),
                "[build]\nrustflags = [\"-C\", \"target-cpu=x86-64-v3\"]\n",
            )?;
        }
        let package = |crate_dir: &str, deps: &str, files: &[SourceFile]| -> std::io::Result<()> {
            let root = dir.join(crate_dir);
            std::fs::create_dir_all(root.join("src"))?;
            std::fs::write(
                root.join("Cargo.toml"),
                format!(
                    "[package]\nname = \"{crate_dir}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n{runtime}{deps}"
                ),
            )?;
            for file in files {
                std::fs::write(root.join("src").join(&file.path), &file.text)?;
            }
            Ok(())
        };
        package("header", "", &workspace.header)?;
        for (c, files) in workspace.bodies.iter().enumerate() {
            package(&bodies[c], "header = { path = \"../header\" }\n", files)?;
        }
        let app_deps: String = std::iter::once("header = { path = \"../header\" }\n".to_string())
            .chain(bodies.iter().map(|b| format!("{b} = {{ path = \"../{b}\" }}\n")))
            .collect();
        package(&name, &app_deps, &workspace.app)
    }
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
    body: String,
    /// The size of the code that moved, which decides the body crate.
    moved: usize,
}

/// The program `tree` as a header crate, `crates` body crates and the binary. The code is held
/// once at a time: each module's items go once their text is split, and each part of a split
/// once it is in its crate (a program as large as the Minecraft server is gigabytes of it).
pub fn split(mut tree: ModuleTree, crates: usize) -> Result<Workspace, String> {
    let mut used = std::collections::HashSet::new();
    let mut splits = Vec::new();
    for module in &mut tree.modules {
        let text = module.items_text();
        module.items = Vec::new();
        let split = split_module(&text, &module.path, &mut used)
            .map_err(|e| format!("splitting the module of package `{}`: {e}", module.package))?;
        splits.push(split);
        proc_macro2::extra::invalidate_current_thread_spans();
    }
    // Each body crate takes the next modules until it has its share of the code.
    let total: usize = splits.iter().map(|s| s.moved).sum();
    let crates = crates.max(1);
    let share = total.div_ceil(crates).max(1);
    let mut crate_of = Vec::new();
    let (mut current, mut filled) = (0, 0);
    for split in &splits {
        if filled >= share && current + 1 < crates {
            current += 1;
            filled = 0;
        }
        crate_of.push(current);
        filled += split.moved;
    }
    let splits = std::cell::RefCell::new(splits);
    let tree = &tree;
    let header = format!(
        "{}#![allow(dead_code, unused_imports, unused_mut, unused_variables, non_snake_case)]\n\n{}",
        tree.header,
        nest(tree, &|m| {
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
    for c in 0..crates {
        let text = nest(tree, &|m| {
            let path = &tree.modules[m].path;
            let reexport =
                if path.is_empty() { "header".to_string() } else { format!("header::{}", path.join("::")) };
            let code = if crate_of[m] == c {
                std::mem::take(&mut splits.borrow_mut()[m].body)
            } else {
                String::new()
            };
            format!("pub use {reexport}::*;\n{}{code}", uses_text(&tree.modules[m].uses))
        });
        let text = format!(
            "//! Code of the translated program (see the header crate).\n\
             #![allow(warnings)]\n{limit}\n{text}"
        );
        bodies.push(vec![SourceFile { path: "lib.rs".into(), text }]);
    }
    let links: String = (0..crates).map(|c| format!("use body_{c} as _;\n")).collect();
    let app = format!("{links}\nfn main() {{\n    header::main()\n}}\n\n{}", tree.footer);
    Ok(Workspace {
        header: vec![SourceFile { path: "lib.rs".into(), text: header }],
        bodies,
        app: vec![SourceFile { path: "main.rs".into(), text: app }],
    })
}

/// The `use` lines of a module, a blank line after each group.
fn uses_text(uses: &[String]) -> String {
    uses.iter().map(|group| format!("{group}\n\n")).collect()
}

/// The modules of `tree` as inline modules, from the root, each module's text made by `text`.
fn nest(tree: &ModuleTree, text: &dyn Fn(usize) -> String) -> String {
    fn inline(tree: &ModuleTree, index: usize, text: &dyn Fn(usize) -> String) -> String {
        let path = &tree.modules[index].path;
        let mut out = text(index);
        for child in 0..tree.modules.len() {
            let inner = &tree.modules[child].path;
            if inner.len() == path.len() + 1 && inner.starts_with(path) {
                let name = inner.last().map_or("", String::as_str);
                out.push_str(&format!("\npub mod {name} {{\n{}}}\n", inline(tree, child, text)));
            }
        }
        out
    }
    match tree.modules.iter().position(|m| m.path.is_empty()) {
        Some(root) => inline(tree, root, text),
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

fn split_module(
    text: &str,
    path: &[String],
    used: &mut std::collections::HashSet<String>,
) -> Result<Split, String> {
    let file = syn::parse_file(text).map_err(|e| {
        // The code around the error, which a translator's fault in the generated text names.
        let at = e.span().start();
        let line = text.lines().nth(at.line.saturating_sub(1)).unwrap_or_default();
        let around: String = line.chars().skip(at.column.saturating_sub(120)).take(240).collect();
        format!("{e} at line {}, column {}: {around}", at.line, at.column)
    })?;
    let mut edits = Vec::new();
    let mut externs = String::new();
    let mut body = String::new();
    let mut moved = 0;
    let mut traits = 0;
    for item in &file.items {
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
                publish(&f.vis, sig_start(&f.sig), &mut edits);
                if movable(&f.sig, text) {
                    let name = f.sig.ident.to_string();
                    let symbol = symbol(path, "", &name, used);
                    let (params, args) = rename_params(&f.sig, text, &mut edits, None);
                    let ret = at_or_empty(text, &f.sig.output);
                    let block = f.block.span().byte_range();
                    edits.push(Edit {
                        start: block.start,
                        end: block.end,
                        text: format!("{{ {symbol}({args}) }}"),
                    });
                    externs.push_str(&format!("    safe fn {symbol}({params}){ret};\n"));
                    // `#[inline]` does nothing on an exported function.
                    let attrs: String = f
                        .attrs
                        .iter()
                        .filter(|a| !a.path().is_ident("inline"))
                        .map(|a| format!("{}\n", at(text, a.span())))
                        .collect();
                    let inputs = f.sig.inputs.span().byte_range();
                    let inputs = if f.sig.inputs.is_empty() { "" } else { &text[inputs] };
                    let code = &text[block];
                    body.push_str(&format!(
                        "{attrs}#[unsafe(no_mangle)]\npub fn {symbol}({inputs}){ret} {code}\n\n"
                    ));
                    moved += code.len();
                }
            }
            syn::Item::Impl(imp) => {
                let self_ty = at(text, imp.self_ty.span()).to_string();
                let plain = imp.generics.params.is_empty()
                    && imp.generics.where_clause.is_none()
                    && !self_ty.contains('<')
                    && matches!(*imp.self_ty, syn::Type::Path(_));
                let trait_name = imp.trait_.as_ref().map(|(_, p, _)| at(text, p.span()).to_string());
                let mut decls = String::new();
                let mut impls = String::new();
                let mut functions = String::new();
                let bodies = format!("Bodies{traits}");
                for impl_item in &imp.items {
                    let syn::ImplItem::Fn(f) = impl_item else {
                        if let syn::ImplItem::Const(c) = impl_item
                            && trait_name.is_none()
                        {
                            publish(&c.vis, c.const_token.span.byte_range().start, &mut edits);
                        }
                        continue;
                    };
                    if trait_name.is_none() {
                        publish(&f.vis, sig_start(&f.sig), &mut edits);
                    }
                    let sig_text = at(text, f.sig.span());
                    if !plain
                        || !movable(&f.sig, text)
                        || (trait_name.is_some() && sig_text.contains("Self::"))
                    {
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
                    decls.push_str(&format!("    fn body_{name}({decl_receiver}{decl_params}){ret};\n"));
                    let attrs: String =
                        f.attrs.iter().map(|a| format!("    {}\n", at(text, a.span()))).collect();
                    let inputs =
                        if f.sig.inputs.is_empty() { "" } else { &text[f.sig.inputs.span().byte_range()] };
                    let code = &text[block];
                    impls.push_str(&format!("{attrs}    fn body_{name}({inputs}){ret} {code}\n"));
                    let this_arg = if this_ty.is_some() { "this, " } else { "" };
                    functions.push_str(&format!(
                        "#[unsafe(no_mangle)]\npub fn {symbol}({this_param}{params}){ret_here} {{\n    <{self_ty} as {bodies}>::body_{name}({this_arg}{args})\n}}\n\n"
                    ));
                    moved += code.len();
                }
                if !decls.is_empty() {
                    body.push_str(&format!(
                        "trait {bodies}: Sized {{\n{decls}}}\n\nimpl {bodies} for {self_ty} {{\n{impls}}}\n\n{functions}"
                    ));
                    traits += 1;
                }
            }
            // A `use` among the items is the moved code's too.
            syn::Item::Use(u) => {
                body.push_str(at(text, u.span()));
                body.push('\n');
            }
            _ => {}
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
    if !externs.is_empty() {
        header.push_str(&format!("\nunsafe extern \"Rust\" {{\n{externs}}}\n"));
    }
    Ok(Split { header, body, moved })
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
    fn thread_local_statics_of_any_visibility_become_public() {
        let text = "thread_local! {\n    pub(crate) static A__INITIALIZED: Cell<bool> = const { Cell::new(false) };\n    static B: Cell<i32> = const { Cell::new(0) };\n    pub static C: Cell<i32> = const { Cell::new(0) };\n}\n";
        let split = split_module(text, &["p".to_string()], &mut std::collections::HashSet::new()).unwrap();
        for name in ["A__INITIALIZED", "B", "C"] {
            assert!(split.header.contains(&format!("    pub static {name}:")), "{}", split.header);
        }
        assert!(!split.header.contains("pub(crate)"), "{}", split.header);
    }
}
