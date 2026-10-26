//! The natives of `java.lang.ref.Reference` and `PhantomReference` that read and write the
//! referent: `get0`, `refersTo0` and `clear0`. The referent is an ordinary field, which the
//! collector (`runtime/src/gc.rs`) traces as any other, so a reference keeps its referent alive
//! and nothing clears it behind the program's back. So a reference behaves as on a JVM whose
//! collector never needs the memory: `get` gives the referent until `clear` sets it to null,
//! `refersTo` compares it, and a reference is enqueued only by its `enqueue`, so a
//! `ReferenceQueue` gets nothing else. The rest of `java.lang.ref` is translated from the JDK's
//! source.
//!
//! Each native is lowered as the Java code it stands for, on the field `referent`:
//! `this.referent`, `this.referent == o` and `this.referent = null`.

use super::*;
use crate::value::{Value, json};

impl<'t> Lowering<'t> {
    /// The call `node` of a referent native, as the code it stands for; `None` for anything else.
    /// The code keeps its own type: `clear0()` is void, but the assignment it stands for has the
    /// referent's type, which the analyses read.
    pub(super) fn reference_native(&self, node: Node<'t>) -> Result<Option<Expr>, Error> {
        let owners = ["java.lang.ref.Reference", "java.lang.ref.PhantomReference"];
        let symbol = match node.symbol() {
            Some(symbol) if node.kind() == "METHOD_INVOCATION" => symbol,
            _ => return Ok(None),
        };
        if !symbol.native || !owners.contains(&symbol.owner) {
            return Ok(None);
        }
        let (line, column) = (&node.0["line"], &node.0["column"]);
        let referent = json!({"kind": "IDENTIFIER", "line": line, "column": column, "javaType": "T",
            "name": "referent", "symbol": {"kind": "FIELD", "name": "referent",
            "owner": "java.lang.ref.Reference", "static": false, "type": "T"}});
        let code = match symbol.name {
            "get0" => referent,
            "refersTo0" => json!({"kind": "EQUAL_TO", "line": line, "column": column,
                "javaType": "boolean", "leftOperand": referent,
                "rightOperand": node.0["arguments"][0].clone()}),
            "clear0" => json!({"kind": "ASSIGNMENT", "line": line, "column": column, "javaType": "T",
                "variable": referent, "expression": {"kind": "NULL_LITERAL", "line": line,
                "column": column, "javaType": "<nulltype>"}}),
            _ => return Ok(None),
        };
        let code: &'t Value = Box::leak(Box::new(code));
        self.expr(Node(code)).map(Some)
    }
}
