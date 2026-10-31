//! The reflection table of a whole program, cut to the members that a lookup can find.
//!
//! Reachability puts in the table the members of every class that a lookup could look in by
//! what the trees show (`program::reach::reflection`): for a lookup on a `Class` value that is
//! not a class literal, every class whose class literal reachable code has. The points-to facts
//! for dispatch (`object_values::analyze_for_dispatch`) follow `Class` values too, from class
//! literals, `getClass()` (the classes of the objects) and `getSuperclass()`,
//! `getEnclosingClass()` and `getDeclaringClass()` of the classes followed; any other `Class`
//! value may be of any class. They give the `Class` values that each kind of lookup gets: the
//! first parameter of the translator's `Method.find` and `list`, `Field.find` and `list`, and
//! `Constructor.find` and `list` (`ReflectionTable::lookups`), which every lookup of its kind
//! calls. A member of the table is found on a class (the class it is looked up in, which may
//! inherit it); where that class is not among the `Class` values that the lookups of its kind
//! get, no lookup finds it, and it is left out (`ObjectValues::live_slot`): its numbers go from
//! its part's data, and its cases from the `switch`es that call, read, write or create it. The
//! data keeps each member's slot, which `java/Reflection.java` reads, so the slots of the members
//! kept stay as they were. Members that no class finds (`Object`'s, a box's `valueOf`, `Class`'s
//! own) stay.
//!
//! The table's own code gives `Class` values too: a member's classes (its declaring class, its
//! result, its parameters' and exceptions', a field's type) come from its part's `tableClasses`,
//! and `Field.getType()` hands them on. The facts take those of the members that a lookup may
//! find only, and the value that a `switch` case of a member yields only where a lookup may find
//! the member, so that the table does not keep itself: a class that only the table's data names
//! is not looked in for that.
//!
//! The members left out are then no longer called by the table, and the walk of the methods that
//! can run (`live_methods`), which follows, leaves out what only they reached. With `--timings`
//! the translator prints how many members it kept, and `JAVA_TO_RUST_TABLE_DUMP=file` writes how
//! many of each class's of each kind it kept and left out.

use super::object_values::{ObjectValues, STATIC};
use super::{FastMap, FastSet};
use crate::ast::*;

/// The classes, by kind and with their names as javac gives them, that the facts `values` find
/// that a lookup on a `Class` value that is not a class literal gets, but that reachability did
/// not take for candidates of such a lookup (`ReflectionTable::uncovered`), so that the table
/// lacks their members: the translation is made again with them (`Program::flow_classes`). A
/// lookup's `Class` value is the first argument of a call of `find`, `list` or `newInstanceOf`
/// in the program's code (`ReflectionTable::entries`); the calls that the translator's classes
/// of reflection make themselves look in what reachability found for them (a record's
/// accessors), and a class literal is looked in for the names that its lookup spells out. Where
/// the lookups of a kind may get any class, the facts do not tell, and the kind is left as
/// reachability found it.
pub(super) fn missing(class: &Class, values: &ObjectValues) -> Vec<(TableKind, String)> {
    let table = &class.reflection_table;
    if table.entries.is_empty() {
        return Vec::new();
    }
    let entries: FastMap<usize, TableKind> = table.entries.iter().map(|&(kind, m)| (m, kind)).collect();
    let mut gets: FastMap<TableKind, FastSet<usize>> = FastMap::default();
    let mut any: FastSet<TableKind> = FastSet::default();
    let mut look = |m: usize, contexts: &[usize], e: &Expr| {
        visit_expr(e, &mut |x| {
            let ExprKind::Call { resolved: Some(r), args, .. } = &x.kind else { return };
            let (Some(&kind), Some(first)) = (entries.get(r), args.first()) else { return };
            if is_class_literal(first) {
                return;
            }
            for &ctx in contexts {
                let flow = values.eval(class, m, ctx, first);
                if flow.any_class {
                    any.insert(kind);
                } else {
                    gets.entry(kind).or_default().extend(flow.class_values.iter());
                }
            }
        })
    };
    for (m, method) in class.methods.iter().enumerate() {
        if table.internal.contains(&method.class) {
            continue;
        }
        for stmt in &method.body {
            visit_stmt(stmt, &mut |e| look(m, &values.contexts[m], e), &mut |_| {});
        }
    }
    for decl in &class.types {
        for init in decl.fields.iter().chain(&decl.statics).filter_map(|f| f.init.as_ref()) {
            look(usize::MAX, &[STATIC], init);
        }
    }
    let mut out = Vec::new();
    for (kind, classes) in gets.into_iter().filter(|(kind, _)| !any.contains(kind)) {
        let Some(uncovered) = table.uncovered.get(&kind) else { continue };
        out.extend(classes.into_iter().filter_map(|t| uncovered.get(&t)).map(|name| (kind, name.clone())));
    }
    out.sort_by(|a, b| (format!("{:?}", a.0), &a.1).cmp(&(format!("{:?}", b.0), &b.1)));
    out.dedup();
    out
}

/// Whether `e` is a class literal, as converted or cast.
fn is_class_literal(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Cast(_, inner) => is_class_literal(inner),
        ExprKind::Builtin(Builtin::ClassLiteral(_), _) => true,
        _ => false,
    }
}

/// Leaves out of the reflection table the members that no lookup can find, by the facts
/// `values` (see the module's comment); how many members it kept, of how many that a class
/// finds.
pub(super) fn prune(class: &mut Class, values: &ObjectValues) -> (usize, usize) {
    let table = &class.reflection_table;
    let found: Vec<(TableKind, i32)> =
        table.found_on.iter().filter(|(_, on)| on.is_some()).map(|(&slot, _)| slot).collect();
    let dead: FastSet<(TableKind, i32)> =
        found.iter().copied().filter(|&(kind, slot)| !values.live_slot(class, kind, slot)).collect();
    // With `JAVA_TO_RUST_TABLE_DUMP=file`, how many members of each class are kept and left out.
    if let Some(path) = std::env::var_os("JAVA_TO_RUST_TABLE_DUMP") {
        let mut counts: std::collections::BTreeMap<(String, String), (usize, usize)> = Default::default();
        for &(kind, slot) in &found {
            let Some(Some(t)) = table.found_on.get(&(kind, slot)) else { continue };
            let count = counts.entry((format!("{kind:?}"), class.types[*t].name.clone())).or_default();
            if dead.contains(&(kind, slot)) { count.1 += 1 } else { count.0 += 1 }
        }
        let lines: String = counts
            .iter()
            .map(|((kind, name), (kept, out))| format!("{kind} {name}: kept {kept}, left out {out}\n"))
            .collect();
        let _ = std::fs::write(path, lines);
    }
    if dead.is_empty() {
        return (found.len(), found.len());
    }
    let cases: FastSet<ExprId> =
        table.cases.iter().filter(|(_, slot)| dead.contains(slot)).map(|(&id, _)| id).collect();
    // The numbers of each part: what is kept of them, by the id of their first item.
    let numbers: std::collections::HashMap<ExprId, Vec<(usize, usize)>> = table
        .data
        .iter()
        .filter(|(_, data)| data.named_by.is_empty())
        .map(|(&id, data)| {
            let kept = data.members.iter().filter(|(slot, ..)| !dead.contains(&(data.kind, *slot)));
            (id, kept.map(|&(_, from, to)| (from, to)).collect())
        })
        .collect();
    let mut rewrite = |e: &mut Expr| match &mut e.kind {
        ExprKind::Switch { cases: arms, .. } => arms.retain(|arm| match arm.body.last() {
            Some(Stmt::Yield(last)) => !cases.contains(&last.id),
            _ => true,
        }),
        ExprKind::ArrayLit(items) => {
            let Some(kept) = items.first().and_then(|first| numbers.get(&first.id)) else { return };
            let mut old = std::mem::take(items).into_iter().map(Some).collect::<Vec<_>>();
            let mut count = old[0].take().expect("the number of members");
            count.kind = ExprKind::Int(kept.len() as i32);
            items.push(count);
            for &(from, to) in kept {
                items
                    .extend(old[from..to].iter_mut().map(|item| item.take().expect("an item of one member")));
            }
        }
        _ => {}
    };
    for method in &mut class.methods {
        for stmt in &mut method.body {
            rewrite_stmt(stmt, &mut rewrite);
        }
    }
    (found.len() - dead.len(), found.len())
}
