//! The stubs of classes that a whole program does not translate but uses as types
//! (`program::reach::stubs`): a class in its package, or nested in a class of it without that
//! class, with no constructors and no fields, whose instance methods are those that reachable
//! code calls, with bodies that never run, since no object of the class exists.

use super::*;
use serde_json::{Value, json};
use std::collections::BTreeSet;

impl<'t> Lowering<'t> {
    /// Declares the stub of the class with qualified name `class`, with the `methods` called on
    /// it, by name and javac signature.
    pub(super) fn declare_stub(
        &mut self,
        class: &str,
        methods: &BTreeSet<(String, String)>,
    ) -> Result<(), Error> {
        let (outer, simple) = class.rsplit_once('.').expect("a stub's class is in a package");
        let members: Vec<Value> =
            methods.iter().filter_map(|(name, sig)| stub_method(class, name, sig)).collect();
        let decl = json!({"kind": "CLASS", "simpleName": simple, "modifiers": [], "members": members,
            "extendsClause": null, "implementsClause": [], "typeParameters": [],
            "symbol": {"kind": "CLASS", "name": simple, "owner": outer, "type": class}});
        self.declare_type(Node(Box::leak(Box::new(decl))), Some(outer))
    }
}

/// The package of the stub `class`: its names up to the first class name (`java.io` of
/// `java.io.ObjectInputStream.GetField`).
pub(super) fn stub_package(class: &str) -> String {
    let parts: Vec<&str> =
        class.split('.').take_while(|part| part.starts_with(|c: char| c.is_lowercase())).collect();
    parts.join(".")
}

/// The declaration of the instance method `name` of the stub `class`, with javac's signature
/// `sig`, such as `(int)java.lang.Object`; `None` for a generic method.
fn stub_method(class: &str, name: &str, sig: &str) -> Option<Value> {
    let (params, ret) = sig.strip_prefix('(')?.rsplit_once(')')?;
    let mut types = Vec::new();
    let (mut depth, mut start) = (0, 0);
    for (i, c) in params.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => depth -= 1,
            ',' if depth == 0 => {
                types.push(&params[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    if !params.is_empty() {
        types.push(&params[start..]);
    }
    let owner = format!("{name}{sig}");
    let parameters: Vec<Value> = types
        .iter()
        .enumerate()
        .map(|(i, ty)| {
            let param = format!("arg{i}");
            json!({"kind": "VARIABLE", "name": param, "modifiers": [], "javaType": ty,
                "symbol": {"kind": "PARAMETER", "name": param, "owner": owner, "static": false, "type": ty}})
        })
        .collect();
    Some(json!({"kind": "METHOD", "name": name, "modifiers": [], "parameters": parameters,
        "typeParameters": [], "thrown": [], "returnType": {"kind": "IDENTIFIER", "javaType": ret},
        "body": {"kind": "BLOCK", "statements": []},
        "symbol": {"kind": "METHOD", "name": name, "owner": class, "static": false, "type": sig}}))
}
