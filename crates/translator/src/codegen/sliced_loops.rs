//! Counting loops whose array accesses are checked once, before the loop (see
//! `analysis/arrays/slices.rs`):
//!
//! ```text
//! if (n as usize) <= c[i as usize].len() && (k as usize) < b.len() && (n as usize) <= b[k as usize].len() {
//!     let c_row = &mut c[i as usize][..n as usize];
//!     let b_row = &b[k as usize][..n as usize];
//!     for j in 0..n {
//!         c_row[j as usize] = c_row[j as usize].wrapping_add(aik.wrapping_mul(b_row[j as usize]));
//!     }
//! } else {
//!     for j in 0..n { ... every access checked ... }
//! }
//! ```
//!
//! The slices end at the loop's bound, so Rust knows every index is in range and drops its own
//! checks too. When the check fails, the loop runs with Java's checks and throws where Java does.

use super::*;
use crate::analysis::RowIndex;

impl<'a> Gen<'a> {
    /// Writes the counting loop `counted` with its condition `cond` as a sliced loop, if the
    /// analysis found it to be one.
    pub(super) fn sliced_loop(&mut self, cond: &'a Expr, counted: &Counting<'a>, body: &'a Stmt) -> bool {
        let facts = self.facts;
        let Some(sliced) = facts.sliced_loops.get(&cond.id) else { return false };
        if counted.step != 1 || counted.inclusive || counted.descending || counted.ty != Type::Int {
            return false;
        }
        let end = self.value(counted.to, Type::Int).at(AS);
        let mut checks = Vec::new();
        if !matches!(counted.from.kind, ExprKind::Int(k) if k >= 0) {
            let start = self.value(counted.from, Type::Int).at(CMP + 1);
            checks.push(format!("{start} >= 0"));
        }
        let mut bindings = Vec::new();
        for slice in &sliced.slices {
            let root = self.local(&slice.root);
            let mut place = root.clone();
            for row in &slice.rows {
                let k = match row {
                    RowIndex::Int(k) => k.to_string(),
                    RowIndex::Var(v) => format!("{} as usize", self.local(v)),
                };
                checks.push(format!("{} < {place}.len()", paren(&k)));
                place = format!("{place}[{k}]");
            }
            checks.push(format!("({end} as usize) <= {place}.len()"));
            let name = self.temp(&format!("{root}_{}", if slice.rows.is_empty() { "part" } else { "row" }));
            let mutably = if slice.written { "mut " } else { "" };
            bindings.push((
                slice.root.clone(),
                name,
                slice.rows.len(),
                format!("&{mutably}{place}[..{end} as usize]"),
            ));
        }
        self.line(&format!("if {} {{", checks.join(" && ")));
        self.indent += 1;
        for (root, name, depth, text) in bindings {
            self.line(&format!("let {name} = {text};"));
            self.sliced.insert(root, (name, depth));
        }
        self.sliced_accesses.extend(sliced.accesses.iter().copied());
        // Both copies of the loop take its Java label.
        let label = self.pending.clone();
        self.range_loop(counted, body);
        self.pending = label;
        self.sliced.clear();
        self.sliced_accesses.clear();
        self.indent -= 1;
        self.line("} else {");
        self.indent += 1;
        self.range_loop(counted, body);
        self.indent -= 1;
        self.line("}");
        true
    }
}

/// A row index in a comparison: `i as usize` needs parentheses before `<`, which Rust would
/// read as the start of generic arguments.
fn paren(k: &str) -> String {
    if k.contains(' ') { format!("({k})") } else { k.to_string() }
}
