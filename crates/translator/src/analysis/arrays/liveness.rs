//! Which array variables are live after each simple statement (a local's declaration, an
//! expression statement or a `return`): read again on some path before they are assigned. An
//! array read by a statement after which it is not live can be moved there instead of shared:
//! `a = b; b = c;` hands `b`'s array to `a`, since `b` is assigned before anything reads it again.
//!
//! The analysis is the classic backward one over the structured statements; loops are solved by
//! iterating to a fixed point. A `switch` and a `try` are not looked into: every array they
//! mention counts as live throughout them, and so does everything live where a jump inside them
//! could lead. Anything this module cannot follow is live, which only keeps an array shared.

use super::super::{FastMap, FastSet};
use crate::ast::*;

type Live = FastSet<String>;

/// The arrays among `arrays` that are live after each simple statement of `method`, keyed by
/// the statement's expression (the initializer, the expression, or the returned value).
pub(super) fn live_after(method: &Method, arrays: &FastSet<String>) -> FastMap<ExprId, Live> {
    let mut walk = Walk {
        arrays,
        after: FastMap::default(),
        breaks: Vec::new(),
        continues: Vec::new(),
        labels: Vec::new(),
        pending: None,
    };
    walk.block(&method.body, Live::default());
    walk.after
}

struct Walk<'s> {
    arrays: &'s FastSet<String>,
    after: FastMap<ExprId, Live>,
    /// Where an unlabeled `break` and `continue` lead, innermost loop last.
    breaks: Vec<Live>,
    continues: Vec<Live>,
    /// The labeled statements around the current one: label, where `break label` leads, and
    /// where `continue label` leads (the labeled loop's next iteration).
    labels: Vec<(String, Live, Live)>,
    /// The label of the loop about to be walked.
    pending: Option<String>,
}

impl Walk<'_> {
    /// The arrays that `e` mentions anywhere, including in the statements of a switch expression.
    fn uses(&self, e: &Expr) -> Live {
        let mut live = Live::default();
        visit_expr(e, &mut |x| {
            if let ExprKind::Var(v) | ExprKind::Assign { target: Place::Var(v), .. } = &x.kind
                && self.arrays.contains(v)
            {
                live.insert(v.clone());
            }
        });
        live
    }

    fn block(&mut self, stmts: &[Stmt], out: Live) -> Live {
        stmts.iter().rev().fold(out, |live, s| self.stmt(s, live))
    }

    fn stmt(&mut self, s: &Stmt, out: Live) -> Live {
        match s {
            Stmt::Local { name, init, .. } => {
                let mut live = out.clone();
                live.remove(name);
                if let Some(e) = init {
                    self.after.insert(e.id, out);
                    live.extend(self.uses(e));
                }
                live
            }
            Stmt::Expr(e) => {
                self.after.insert(e.id, out.clone());
                match &e.kind {
                    ExprKind::Assign { op: None, target: Place::Var(target), value } => {
                        let mut live = out;
                        live.remove(target);
                        live.extend(self.uses(value));
                        live
                    }
                    _ => union(&[&out, &self.uses(e)]),
                }
            }
            Stmt::Return(Some(e), _) => {
                self.after.insert(e.id, Live::default());
                self.uses(e)
            }
            Stmt::Return(None, _) => Live::default(),
            Stmt::If { cond, then, otherwise } => {
                let mut live = self.stmt(then, out.clone());
                live.extend(match otherwise {
                    Some(o) => self.stmt(o, out),
                    None => out,
                });
                live.extend(self.uses(cond));
                live
            }
            Stmt::Block(stmts) => self.block(stmts, out),
            Stmt::While { cond, body } => {
                let cond_uses = self.uses(cond);
                self.fixed_point(&out, Live::clone, |w, head| {
                    let body_in = w.stmt(body, head.clone());
                    let head = union(&[&cond_uses, &body_in, &out]);
                    (head.clone(), head)
                })
            }
            Stmt::DoWhile { body, cond } => {
                // The head is the condition; the loop is entered at the body.
                let cond_uses = self.uses(cond);
                self.fixed_point(&out, Live::clone, |w, head| {
                    let body_in = w.stmt(body, head.clone());
                    (union(&[&cond_uses, &out, &body_in]), body_in)
                })
            }
            Stmt::For { init, cond, update, body } => {
                let cond_uses = cond.as_ref().map(|c| self.uses(c)).unwrap_or_default();
                let exit = if cond.is_some() { out.clone() } else { Live::default() };
                let update_uses: Live = update.iter().flat_map(|u| self.uses(u)).collect();
                let next = |head: &Live| union(&[&update_uses, head]);
                let head = self.fixed_point(&out, next, |w, head| {
                    let body_in = w.stmt(body, next(head));
                    let head = union(&[&cond_uses, &body_in, &exit]);
                    (head.clone(), head)
                });
                self.block(init, head)
            }
            Stmt::ForEach { array, body, .. } => {
                let array_uses = self.uses(array);
                self.fixed_point(&out, Live::clone, |w, head| {
                    let body_in = w.stmt(body, head.clone());
                    let head = union(&[&body_in, &out]);
                    (head.clone(), union(&[&head, &array_uses]))
                })
            }
            Stmt::Switch { selector, cases } => {
                // Every array the switch mentions, and wherever a jump out of it may lead.
                let live = union(&[&out, &self.uses(selector)]);
                self.opaque(live, cases.iter().flat_map(|c| &c.body))
            }
            Stmt::Try { body, catches, finally } => {
                // Every array the `try` mentions, and wherever a jump out of it may lead.
                let stmts =
                    body.iter().chain(catches.iter().flat_map(|c| &c.body)).chain(finally.iter().flatten());
                self.opaque(out, stmts)
            }
            // Nothing of the method is live after a `throw` that leaves it; one inside a `try` is
            // not walked.
            Stmt::Throw(e) => self.uses(e),
            Stmt::Labeled { label, body } => {
                self.labels.push((label.clone(), out.clone(), Live::default()));
                let is_loop = matches!(
                    **body,
                    Stmt::While { .. } | Stmt::DoWhile { .. } | Stmt::For { .. } | Stmt::ForEach { .. }
                );
                self.pending = is_loop.then(|| label.clone());
                let live = self.stmt(body, out);
                self.pending = None;
                self.labels.pop();
                live
            }
            Stmt::Break => self.breaks.last().cloned().unwrap_or_default(),
            Stmt::Continue => self.continues.last().cloned().unwrap_or_default(),
            Stmt::BreakLabel(label) => self.label(label).1.clone(),
            Stmt::ContinueLabel(label) => self.label(label).2.clone(),
            // A `yield` belongs to a switch expression, which is not walked into.
            Stmt::Yield(e) => union(&[&out, &self.uses(e)]),
        }
    }

    /// The live set before a statement that is not looked into, made of `stmts`: `live`, every
    /// array they mention, and wherever a jump out of them may lead.
    fn opaque<'s>(&self, mut live: Live, stmts: impl Iterator<Item = &'s Stmt>) -> Live {
        for stmt in stmts {
            visit_stmt(stmt, &mut |e| live.extend(self.uses(e)), &mut |_| {});
        }
        let targets =
            self.breaks.iter().chain(&self.continues).chain(self.labels.iter().flat_map(|(_, b, c)| [b, c]));
        for target in targets {
            live.extend(target.iter().cloned());
        }
        live
    }

    fn label(&self, label: &str) -> &(String, Live, Live) {
        self.labels.iter().rev().find(|(l, ..)| l == label).expect("javac resolves labels")
    }

    /// Solves a loop by walking it with `step` from an empty live set at its head until the head
    /// is stable, so that the facts recorded in the last walk hold. Given the head, `step`
    /// returns the new head and the live set where the loop is entered; `continue` leads to
    /// `next(head)` and `break` to `out`.
    fn fixed_point(
        &mut self,
        out: &Live,
        next: impl Fn(&Live) -> Live,
        mut step: impl FnMut(&mut Self, &Live) -> (Live, Live),
    ) -> Live {
        let label = self.pending.take();
        let mut head = Live::default();
        loop {
            let continue_to = next(&head);
            if let Some(label) = &label
                && let Some(entry) = self.labels.iter_mut().rev().find(|(l, ..)| l == label)
            {
                entry.2 = continue_to.clone();
            }
            self.breaks.push(out.clone());
            self.continues.push(continue_to);
            let (new_head, entry) = step(self, &head);
            self.breaks.pop();
            self.continues.pop();
            if new_head == head {
                return entry;
            }
            head = new_head;
        }
    }
}

fn union(sets: &[&Live]) -> Live {
    sets.iter().flat_map(|s| s.iter().cloned()).collect()
}
