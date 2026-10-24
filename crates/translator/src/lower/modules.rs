//! The modules of a translation with the JDK's sources, which the translator's `java.lang.Module`
//! answers with (`java/patches/java/lang/Module.java`): those of the program's roots and the
//! libraries' (`program::modules::contents`, `Reach::modules`).
//!
//! `c.getModule()`, `c.getClassLoader()` and `loader.getUnnamedModule()` call the static methods
//! of `java.lang.TranslatedModules` that answer them (`program::MODULE_CALLS`), with the receiver
//! as the argument, and `Class.forName(module, name)` its `inModule` (`lower::by_name`). The
//! natives of `Module` are what the translation knows of its modules, each a `switch` over a
//! name:
//!
//! - `modules0()`: the names of the named modules, in the order of their roots;
//! - `moduleOf0(pn)`: the named module with the package `pn`, or null;
//! - `declares0(name)`: what the named module declares and holds, a line of words each, as the
//!   comment of `declares0` says;
//! - `holds0(module, name)`: whether the root of the module (`""` the unnamed one) has the file.
//!
//! The translator's `ClassLoader` (`java/Threads.java`) has one such native, `holder0(loader,
//! name)`: the module that holds the resource `name` that the loader (`bootstrap`, `platform` or
//! `app`) finds, as `module m` or `the class path`, or null (`program::loader_resources`).

use super::*;
use crate::program::ModuleContents as Contents;

const MODULE: &str = "java.lang.Module";
const CLASS_LOADER: &str = "java.lang.ClassLoader";

impl<'t> Lowering<'t> {
    /// A call of `MODULE_CALLS`, as a call of the translator's method with the receiver as its
    /// first argument, before the call's, when the translation has that method; `None` for any
    /// other call, and in a translation without the JDK's sources.
    pub(super) fn module_call(
        &self,
        node: Node<'t>,
        symbol: &Symbol,
        select: Node<'t>,
    ) -> Result<Option<ExprKind>, Error> {
        if symbol.is_static || select.kind() != "MEMBER_SELECT" {
            return Ok(None);
        }
        let Some((to, signature)) = crate::program::module_call(symbol.owner, symbol.name, symbol.ty) else {
            return Ok(None);
        };
        let Some(resolved) = self.translated_modules(to, signature) else { return Ok(None) };
        let mut args = vec![self.expr(select.child("expression")?)?];
        for arg in node.list("arguments") {
            args.push(self.expr(arg)?);
        }
        let args = self.convert_generic_args(node, Some(resolved), args)?;
        Ok(Some(ExprKind::Call { name: to.to_string(), args, resolved: Some(resolved) }))
    }

    /// The method `name` of `java.lang.TranslatedModules` with javac's signature `signature`, if
    /// the translation has it.
    pub(super) fn translated_modules(&self, name: &str, signature: &str) -> Option<usize> {
        self.methods.iter().position(|m| {
            self.types[m.owner].javac == crate::program::TRANSLATED_MODULES
                && m.name == name
                && m.sig == signature
        })
    }

    /// A native of the translator's `Module`, with its arguments (see the module's comment);
    /// `None` for any other call.
    pub(super) fn module_native(
        &self,
        node: Node<'t>,
        symbol: &Symbol,
        args: &mut Vec<Expr>,
    ) -> Result<Option<ExprKind>, Error> {
        if (symbol.owner != MODULE && symbol.owner != CLASS_LOADER) || !symbol.native {
            return Ok(None);
        }
        let Some(reach) = self.reach else {
            let class = symbol.owner.rsplit('.').next().unwrap_or_default();
            return unsupported(node, &format!("`{class}` outside a whole program"));
        };
        let pos = node.pos();
        let args = std::mem::take(args);
        let named: Vec<&Contents> = reach.modules.iter().filter(|m| !m.name.is_empty()).collect();
        let ret = self.ty(node, symbol.ty.rsplit(')').next().unwrap_or_default())?;
        let string = |s: &str| self.synth(ExprKind::Str(s.to_string()), pos, Type::String);
        // A `String[]`, which is an `Object[]` where arrays of strings are covariant (`covariant`).
        let strings = |lines: Vec<String>| {
            let array = if lines.is_empty() {
                ExprKind::NewArray(vec![self.synth(ExprKind::Int(0), pos, Type::Int)])
            } else {
                ExprKind::ArrayLit(lines.iter().map(|l| self.convert(string(l), ret.element())).collect())
            };
            self.synth(array, pos, ret)
        };
        // The arguments in locals, read by the `switch`es. A null finds nothing, as a name that
        // no module nor resource has, `/`: `n != null ? n : "/"`, which the `switch` need not
        // check for null.
        let names: Vec<String> = (0..args.len()).map(|i| self.fresh_name(&format!("module_{i}"))).collect();
        let var = |i: usize| {
            let v = || self.synth(ExprKind::Var(names[i].clone()), pos, Type::String);
            let null = self.synth(ExprKind::Null, pos, Type::Null);
            let cond =
                self.synth(ExprKind::Binary(BinOp::Ne, Box::new(v()), Box::new(null)), pos, Type::Boolean);
            let kind = ExprKind::Conditional(Box::new(cond), Box::new(v()), Box::new(string("/")));
            self.synth(kind, pos, Type::String)
        };
        let switch = |selector: Expr, cases: Vec<(Vec<String>, Expr)>, default: Expr| {
            let mut cases: Vec<Case> = cases
                .into_iter()
                .filter(|(labels, _)| !labels.is_empty())
                .map(|(labels, value)| Case {
                    labels: labels.iter().map(|l| CaseLabel::Value(string(l))).collect(),
                    body: vec![Stmt::Yield(value)],
                })
                .collect();
            cases.push(Case { labels: vec![CaseLabel::Default], body: vec![Stmt::Yield(default)] });
            self.synth(ExprKind::Switch { selector: Box::new(selector), cases }, pos, ret)
        };
        let found = match (symbol.name, args.len()) {
            ("modules0", 0) => strings(named.iter().map(|m| m.name.clone()).collect()),
            ("moduleOf0", 1) => {
                // A package in two roots is the first's, as the class path would have it.
                let mut seen = std::collections::HashSet::new();
                let cases = named
                    .iter()
                    .map(|m| {
                        let packages =
                            m.packages.iter().filter(|p| seen.insert(p.as_str())).cloned().collect();
                        (packages, string(&m.name))
                    })
                    .collect();
                switch(var(0), cases, self.synth(ExprKind::Null, pos, Type::Null))
            }
            ("declares0", 1) => {
                let cases =
                    named.iter().map(|m| (vec![m.name.clone()], strings(declared_lines(m)))).collect();
                switch(var(0), cases, strings(Vec::new()))
            }
            ("holds0", 2) => {
                let boolean = |b: bool| self.synth(ExprKind::Bool(b), pos, Type::Boolean);
                let cases = reach
                    .modules
                    .iter()
                    .filter(|m| !m.resources.is_empty())
                    .map(|m| {
                        let holds =
                            switch(var(1), vec![(m.resources.clone(), boolean(true))], boolean(false));
                        (vec![m.name.clone()], holds)
                    })
                    .collect();
                switch(var(0), cases, boolean(false))
            }
            ("holder0", 2) if symbol.owner == CLASS_LOADER => {
                let null = || self.synth(ExprKind::Null, pos, Type::Null);
                let cases = ["bootstrap", "platform", "app"]
                    .into_iter()
                    .map(|loader| {
                        let mut holders: Vec<(String, Vec<String>)> = Vec::new();
                        for (name, holder) in crate::program::loader_resources(&reach.modules, loader) {
                            match holders.iter_mut().find(|(h, _)| *h == holder) {
                                Some((_, names)) => names.push(name),
                                None => holders.push((holder, vec![name])),
                            }
                        }
                        let cases = holders.into_iter().map(|(h, names)| (names, string(&h))).collect();
                        (vec![loader.to_string()], switch(var(1), cases, null()))
                    })
                    .collect();
                switch(var(0), cases, null())
            }
            _ => {
                let class = symbol.owner.rsplit('.').next().unwrap_or_default();
                return unsupported(node, &format!("`{class}.{}`", symbol.name));
            }
        };
        let mut body: Vec<Stmt> = names
            .iter()
            .zip(args)
            .map(|(name, arg)| {
                let init = Some(self.convert(arg, Type::String));
                Stmt::Local { ty: Type::String, name: name.clone(), init, pos }
            })
            .collect();
        body.push(Stmt::Yield(found));
        Ok(Some(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body }],
        }))
    }
}

/// What the module `m` declares and holds as `declares0` gives it: `open`, `requires m`,
/// `transitive m`, `exports p [m..]`, `opens p [m..]`, `uses s` and `package p`.
fn declared_lines(m: &Contents) -> Vec<String> {
    let mut lines = Vec::new();
    if m.open {
        lines.push("open".to_string());
    }
    for (module, transitive) in &m.requires {
        lines.push(format!("{} {module}", if *transitive { "transitive" } else { "requires" }));
    }
    for (keyword, list) in [("exports", &m.exports), ("opens", &m.opens)] {
        for (package, to) in list {
            lines.push(
                std::iter::once(format!("{keyword} {package}"))
                    .chain(to.iter().cloned())
                    .collect::<Vec<_>>()
                    .join(" "),
            );
        }
    }
    lines.extend(m.uses.iter().map(|s| format!("uses {s}")));
    lines.extend(m.packages.iter().map(|p| format!("package {p}")));
    lines
}

#[cfg(test)]
mod tests {
    use super::{Contents, declared_lines};

    #[test]
    fn a_module_declares_a_line_each() {
        let m = Contents {
            name: "m".into(),
            open: true,
            requires: vec![("a".into(), true), ("b".into(), false)],
            exports: vec![("p".into(), vec![]), ("q".into(), vec!["x".into(), "y".into()])],
            opens: vec![("r".into(), vec![])],
            uses: vec!["p.S".into()],
            packages: vec!["".into(), "p".into()],
            resources: vec![],
            classes: vec![],
        };
        assert_eq!(
            declared_lines(&m),
            [
                "open",
                "transitive a",
                "requires b",
                "exports p",
                "exports q x y",
                "opens r",
                "uses p.S",
                "package ",
                "package p"
            ]
        );
    }
}
