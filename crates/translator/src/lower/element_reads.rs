//! Repeated reads of one array element or one field. `bodies[j].x - ...; bodies[j].vx += ...`
//! reads the element `bodies[j]` again for every use, which in Rust clones its handle out of the
//! shared array each time (a borrow of the array, a count increment and decrement), where Java only
//! loads it. `bi.vx -= dx * bj.mass * mag; bi.vy -= dy * bj.mass * mag;` reads `bj.mass` again
//! after each store, as Rust's compiler cannot tell that the store to `bi.vx`, a cell
//! (`java::Field`) of an object it knows nothing of, is not one to `bj.mass`; Java's tells fields
//! apart by name and reads it once. A field of an object that threads share is an atomic load
//! (`java::SharedField::get`), which the compiler never merges with another: `if (this.vehicle !=
//! null) this.vehicle.turn(..)` loads `vehicle` twice and tests it for null twice, the second time
//! with a branch to a `NullPointerException`, where Java's JIT loads it once and tests it once.
//!
//! Java's memory model lets a read of a field that is not `volatile` take the value of an earlier
//! read of it, or of the thread's own earlier store to it, when nothing between them is a
//! synchronization action or a store to it by the thread: no other thread's store can be ordered
//! between them. So the value is read once into a new local, and the later reads read the local.
//! When the first read is the first thing its statement evaluates but locals and constants (`if
//! (this.vehicle != null)`, `int d = this.leader.value - ..`), it becomes a statement of its own
//! before that one, `this_vehicle = this.vehicle;`, and the test reads the local, which the
//! generated code narrows to the object (`if let Some(..)`). Otherwise the read becomes
//! `(this_vehicle = this.vehicle)` in its place, so the order of evaluation and of any exception
//! stays Java's, and a test of the local after it is one the compiler folds into the first. A
//! store whose value later reads take becomes `(this_seed = (this.seed = ..))`.
//!
//! The pass walks each method's statements in Java's order of evaluation and keeps the reads
//! whose values a later read may take. Taken are the reads of:
//! - an element of an array that is a local, a field of `this` or a static, at an index that is a
//!   local or a constant, when the element is an object or a string (an element that is itself an
//!   array stays a read, as the array analyses decide how each array is held from the names that
//!   reach it);
//! - a field of a local's object or of `this` that is not `final`, primitive, or an object, enum
//!   or string field that the program declares (a `final` field's read is a plain load, which the
//!   compiler merges where it can and which costs little where it cannot).
//!
//! No field of the name may be volatile in any class, as Java reads a volatile field again at
//! every read. What a later read may take is forgotten:
//! - at a call, an object creation, a library method other than `Math`'s `abs`, `min`, `max`,
//!   `sqrt` and `pow`, a switch, a string concatenation with an object, a volatile read or store,
//!   and the first use of another class's static, which may initialize it: any of these may run
//!   code that stores to the field, or synchronize with a thread that does. The receiver and
//!   arguments of a call are read before it runs, so a later read among them still takes the value;
//!   those of a constructor's call of another constructor are read in place;
//! - for a field, at a store to a field of that name of any object, or to the local;
//! - for an element, at a store to the array or the index, or to an element of any object array;
//! - at a loop, `try`, labeled statement or switch statement, whose parts are walked each on
//!   their own; nothing is taken in a loop's header.
//!
//! A read in a part of `&&`, `||` or `?:` that runs only under a condition takes the value of an
//! earlier read but starts none. A read in a branch of an `if` or a loop's body starts one that
//! the rest of the branch takes; the code after the `if` takes only values from before it (`if
//! (this.f == null) return;`), and so after a nested block. So each local is declared, without a
//! value, before the statement of its first read in that read's block, and given its value once
//! there; a branch or body that is a single statement becomes a block that holds the declaration.
//! A first read is never in the value of a compound assignment to an element or to a boxed
//! number, which the generated code computes in a closure; nor, unless it becomes a statement of
//! its own, in the receiver or an argument of a call, the operands of an object's or array's
//! creation or of a string concatenation, which the generated code may run in a closure that
//! catches their failure (`java::catching`). A store's value is taken only for a primitive
//! field, and not when the value assigns the local whose object it stores to.

use super::*;
use std::collections::{HashMap, HashSet};

/// Where the array of a shared read is.
#[derive(Clone, PartialEq, Eq, Hash)]
enum Array {
    Local(String),
    /// A field of `this`, by name.
    Field(String),
    Static(usize, String),
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum Index {
    Local(String),
    Constant(i32),
}

/// What a shared read reads.
#[derive(Clone, PartialEq, Eq, Hash)]
enum Key {
    Element(Array, Index),
    /// A field, by name, of the object of a local (`Some`) or of `this` (`None`).
    Field(Option<String>, String),
}

impl Key {
    /// Whether a store to the local `name` changes what the key reads.
    fn reads_local(&self, name: &str) -> bool {
        match self {
            Key::Element(array, index) => {
                matches!(array, Array::Local(a) if a == name) || matches!(index, Index::Local(i) if i == name)
            }
            Key::Field(object, _) => object.as_deref() == Some(name),
        }
    }
}

/// A value that later reads take: its first read (or the store that gives it), the reads that
/// take it, and where each is (`Walk::path`).
struct Shared {
    key: Key,
    first: ExprId,
    at: Vec<(usize, usize)>,
    later: Vec<(ExprId, Vec<(usize, usize)>)>,
    /// The first read is the first thing that its statement evaluates (but locals and constants),
    /// so it may be read in a statement of its own before it.
    hoist: bool,
}

/// What later reads may take at a point of the walk.
#[derive(Clone, Default)]
struct State {
    /// Per key, the index in `Walk::shared` of the value that a read of it takes.
    taken: HashMap<Key, usize>,
    /// The classes that are initialized on every path to here (besides the method's own).
    initialized: HashSet<usize>,
}

impl State {
    fn forget(&mut self) {
        self.taken.clear();
    }

    /// What two paths that meet both know.
    fn meet(mut self, other: &State) -> State {
        self.taken.retain(|k, v| other.taken.get(k) == Some(v));
        self.initialized.retain(|c| other.initialized.contains(c));
        self
    }

    /// Keeps only the values that `before` has too: those that a branch started stay in it.
    fn earlier(&mut self, before: &State) {
        self.taken.retain(|k, v| before.taken.get(k) == Some(v));
    }
}

/// The walk of one method in evaluation order (see the module comment).
struct Walk<'l, 't> {
    lowering: &'l Lowering<'t>,
    /// The method's class, which is initialized while it runs.
    class: usize,
    shared: Vec<Shared>,
    /// The block (numbered in walk order) and the statement within it of each enclosing block.
    path: Vec<(usize, usize)>,
    blocks: usize,
    /// Above 0 inside a part that runs only under a condition, or that the generated code
    /// computes in a closure: a read there starts no value.
    quiet: usize,
    /// Above 0 inside the operands of an operation that the generated code may run in a closure
    /// (`java::catching` around a call, an array's creation or a concatenation, `java::cold`
    /// around a thrown object's creation): a read there starts a value only if it is read before
    /// its statement, as the closure could not give the local its value.
    closed: usize,
    /// Nothing but locals and constants evaluated yet in the statement being walked.
    fresh: bool,
}

impl<'l, 't> Walk<'l, 't> {
    fn volatile(&self, name: &str) -> bool {
        self.lowering.volatile_field_names().contains(name)
    }

    /// The key that `e` reads, if it is a read this pass can share.
    fn key(&self, e: &Expr) -> Option<Key> {
        match &e.kind {
            ExprKind::Field { object, name } => self.field_key(object, name, e.ty),
            ExprKind::Index(array, index) => {
                if !matches!(e.ty, Type::Class(..) | Type::String) {
                    return None;
                }
                let array = match &array.kind {
                    ExprKind::Var(name) => Array::Local(name.clone()),
                    ExprKind::Static { class, name } => Array::Static(*class, name.clone()),
                    ExprKind::Field { object, name } if matches!(object.kind, ExprKind::This) => {
                        Array::Field(name.clone())
                    }
                    _ => return None,
                };
                let index = match &index.kind {
                    ExprKind::Var(name) => Index::Local(name.clone()),
                    ExprKind::Int(n) => Index::Constant(*n),
                    _ => return None,
                };
                Some(Key::Element(array, index))
            }
            _ => None,
        }
    }

    /// The key of the field `name` of type `ty` of `object`: one that is not volatile or
    /// `final`, of a local's object or of `this`, primitive or (where the program declares it) an
    /// object, enum or string.
    fn field_key(&self, object: &Expr, name: &str, ty: Type) -> Option<Key> {
        if self.volatile(name) || name == "referent" {
            return None;
        }
        let (local, class) = match &object.kind {
            ExprKind::Var(v) => (Some(v.clone()), self.class_of(object.ty)),
            ExprKind::This => (None, Some(self.class)),
            _ => return None,
        };
        let declared_final = class.and_then(|c| self.lowering.final_field(c, name));
        let primitive = (ty.is_numeric() || ty == Type::Boolean) && declared_final != Some(true);
        let object_field =
            matches!(ty, Type::Class(..) | Type::Enum(_) | Type::String) && declared_final == Some(false);
        (primitive || object_field).then(|| Key::Field(local, name.to_string()))
    }

    fn class_of(&self, ty: Type) -> Option<usize> {
        match ty {
            Type::Class(name, _) => self.lowering.type_index(name),
            _ => None,
        }
    }

    /// A read of `key` by `id`: it takes the value that is there, or starts one; `fresh`: whether
    /// the statement evaluates nothing before it.
    fn read(&mut self, key: Key, id: ExprId, fresh: bool, state: &mut State) {
        if let Some(&s) = state.taken.get(&key) {
            self.shared[s].later.push((id, self.path.clone()));
        } else if self.quiet == 0 && (fresh || self.closed == 0) {
            self.start(key, id, fresh, state);
        }
    }

    fn start(&mut self, key: Key, id: ExprId, hoist: bool, state: &mut State) {
        let at = self.path.clone();
        self.shared.push(Shared { key: key.clone(), first: id, at, later: Vec::new(), hoist });
        state.taken.insert(key, self.shared.len() - 1);
    }

    /// Walks `e` with nothing that a read there starts.
    fn quietly(&mut self, e: &Expr, state: &mut State) {
        self.quiet += 1;
        self.expr(e, state);
        self.quiet -= 1;
    }

    /// Walks `parts`, operands of an operation that may be run in a closure (`Walk::closed`).
    fn closed<'e>(&mut self, parts: impl IntoIterator<Item = &'e Expr>, state: &mut State) {
        self.closed += 1;
        for part in parts {
            self.expr(part, state);
        }
        self.closed -= 1;
    }

    /// Something that may run other code or synchronize, with the parts `parts` that it evaluates
    /// first, whose reads take no earlier value either.
    fn opaque(&mut self, parts: &[&Expr], state: &mut State) {
        state.forget();
        for part in parts {
            self.quietly(part, state);
        }
        state.forget();
    }

    /// The first use of a static of `class`, which may initialize it. A use under a condition
    /// leaves it uninitialized for what follows.
    fn initialize(&mut self, class: usize, state: &mut State) {
        if class == self.class || state.initialized.contains(&class) {
            return;
        }
        state.forget();
        if self.quiet == 0 {
            state.initialized.insert(class);
        }
    }

    /// Walks `e` in evaluation order.
    fn expr(&mut self, e: &Expr, state: &mut State) {
        let fresh = self.fresh;
        self.parts(e, fresh, state);
        let plain = matches!(
            e.kind,
            ExprKind::Int(_)
                | ExprKind::Long(_)
                | ExprKind::Double(_)
                | ExprKind::Char(_)
                | ExprKind::Float(_)
                | ExprKind::Bool(_)
                | ExprKind::Str(_)
                | ExprKind::Units(_)
                | ExprKind::Null
                | ExprKind::Var(_)
                | ExprKind::This
        );
        if !plain {
            self.fresh = false;
        }
    }

    /// `expr` of the kind of `e`; `fresh`: whether its statement evaluates nothing before `e`.
    fn parts(&mut self, e: &Expr, fresh: bool, state: &mut State) {
        match &e.kind {
            ExprKind::Int(_)
            | ExprKind::Long(_)
            | ExprKind::Double(_)
            | ExprKind::Char(_)
            | ExprKind::Float(_)
            | ExprKind::Bool(_)
            | ExprKind::Str(_)
            | ExprKind::Units(_)
            | ExprKind::Null
            | ExprKind::Var(_)
            | ExprKind::This => {}
            ExprKind::Static { class, name } => {
                self.initialize(*class, state);
                // A volatile read is an acquire: nothing after it may take a value from before it.
                if self.volatile(name) {
                    state.forget();
                }
            }
            ExprKind::EnumConstant { class, .. } => self.initialize(*class, state),
            ExprKind::Unary(_, x) | ExprKind::Cast(_, x) | ExprKind::Length(x) => self.expr(x, state),
            ExprKind::InstanceOf { value, binding, .. } => {
                self.expr(value, state);
                if let Some(b) = binding {
                    state.taken.retain(|k, _| !k.reads_local(b));
                }
            }
            ExprKind::Field { object, name } => {
                self.expr(object, state);
                if self.volatile(name) {
                    state.forget();
                } else if let Some(key) = self.key(e) {
                    self.read(key, e.id, fresh, state);
                }
            }
            ExprKind::Index(array, index) => {
                self.expr(array, state);
                self.expr(index, state);
                if let Some(key) = self.key(e) {
                    self.read(key, e.id, fresh, state);
                }
            }
            // A concatenation with an object calls its `toString`.
            ExprKind::Binary(_, l, r)
                if e.ty == Type::String
                    && [l, r].iter().any(|o| o.ty.is_reference() && o.ty != Type::String) =>
            {
                self.opaque(&[&**l, &**r], state)
            }
            ExprKind::Binary(BinOp::And | BinOp::Or, l, r) => {
                self.expr(l, state);
                self.quietly(r, state);
            }
            ExprKind::Binary(_, l, r) if e.ty == Type::String => self.closed([&**l, &**r], state),
            ExprKind::Binary(_, l, r) => {
                self.expr(l, state);
                self.expr(r, state);
            }
            ExprKind::Conditional(c, a, b) => {
                self.expr(c, state);
                self.quietly(a, state);
                self.quietly(b, state);
            }
            ExprKind::Assign { op, target, value } => {
                self.place(target, state);
                // A compound assignment reads its place before the value, and a static's place
                // may initialize its class: a read in the value is no longer the first thing.
                if matches!(target, Place::Static { .. })
                    || (op.is_some() && !matches!(target, Place::Var(_)))
                {
                    self.fresh = false;
                }
                // The value of a compound assignment to an element, or to a boxed number, is
                // written inside a closure (`java::update`, `boxed_update`), which cannot set a
                // local of the method: a first read there would be such a setting.
                let primitive = e.ty.is_numeric() || e.ty == Type::Boolean;
                if op.is_some() && (matches!(target, Place::Index(..)) || !primitive) {
                    self.quietly(value, state);
                } else {
                    self.expr(value, state);
                }
                self.store(target, state);
                self.forward(e, target, Some(value), primitive, state);
            }
            ExprKind::IncDec { target, prefix, .. } => {
                self.place(target, state);
                self.store(target, state);
                if *prefix {
                    self.forward(e, target, None, e.ty.is_numeric(), state);
                }
            }
            ExprKind::NewArray(items) | ExprKind::ArrayLit(items) => self.closed(items, state),
            ExprKind::Builtin(
                Builtin::MathAbs | Builtin::MathMax | Builtin::MathMin | Builtin::MathSqrt | Builtin::MathPow,
                args,
            ) => args.iter().for_each(|a| self.expr(a, state)),
            ExprKind::MethodCall { receiver, args, .. } => {
                self.closed(std::iter::once(&**receiver).chain(args), state);
                state.forget();
            }
            ExprKind::Call { args, .. } => {
                self.closed(args, state);
                state.forget();
            }
            // `new` initializes the class before the arguments are evaluated.
            ExprKind::New { class, args, .. } => {
                self.initialize(*class, state);
                self.fresh = false;
                self.closed(args, state);
                state.forget();
            }
            // A constructor's call of another, which code generation places before the object.
            ExprKind::ThisCall { args, .. } | ExprKind::SuperInit { args, .. } => {
                self.opaque(&args.iter().collect::<Vec<_>>(), state)
            }
            ExprKind::Builtin(_, args) => self.opaque(&args.iter().collect::<Vec<_>>(), state),
            ExprKind::Switch { .. } => self.opaque(&[], state),
        }
    }

    /// Walks the parts of an assigned place that are evaluated before the value.
    fn place(&mut self, target: &Place, state: &mut State) {
        match target {
            Place::Var(_) => {}
            Place::Static { class, .. } => self.initialize(*class, state),
            Place::Field { object, .. } => self.expr(object, state),
            Place::Index(array, index) => {
                self.expr(array, state);
                self.expr(index, state);
            }
        }
    }

    /// Forgets what a store to `target` may change.
    fn store(&mut self, target: &Place, state: &mut State) {
        match target {
            Place::Var(name) => state.taken.retain(|k, _| !k.reads_local(name)),
            Place::Static { class, name } => {
                if self.volatile(name) {
                    state.forget();
                }
                let array = Array::Static(*class, name.clone());
                state.taken.retain(|k, _| !matches!(k, Key::Element(a, _) if *a == array));
            }
            Place::Field { name, .. } => {
                if self.volatile(name) {
                    state.forget();
                }
                state.taken.retain(|k, _| match k {
                    Key::Field(_, field) => field != name,
                    Key::Element(Array::Field(field), _) => field != name,
                    Key::Element(..) => true,
                });
            }
            Place::Index(array, _) => {
                let element = match array.ty {
                    Type::Array(_) => array.ty.element(),
                    _ => Type::Null,
                };
                if element.is_reference() || element == Type::Null {
                    state.taken.retain(|k, _| !matches!(k, Key::Element(..)));
                }
            }
        }
    }

    /// After the store `e` of `value` to `target`: later reads of a primitive field take the
    /// stored value. Not when the value assigns the local whose object it stores to, which
    /// then names another object.
    fn forward(
        &mut self,
        e: &Expr,
        target: &Place,
        value: Option<&Expr>,
        primitive: bool,
        state: &mut State,
    ) {
        let Place::Field { object, name } = target else { return };
        if !primitive || self.quiet > 0 || self.closed > 0 {
            return;
        }
        if let ExprKind::Var(v) = &object.kind
            && value.is_some_and(|x| crate::ast::assigns(x, v))
        {
            return;
        }
        if let Some(key) = self.field_key(object, name, e.ty) {
            self.start(key, e.id, false, state);
        }
    }

    /// Walks a block, numbering it; returns whether it always leaves (by `return`, `throw`,
    /// `break` or `continue`).
    fn block(&mut self, stmts: &[Stmt], state: &mut State) -> bool {
        let block = self.blocks;
        self.blocks += 1;
        let mut leaves = false;
        for (i, stmt) in stmts.iter().enumerate() {
            if leaves {
                // Code after a statement that always leaves runs only if something jumps to it.
                state.forget();
            }
            self.path.push((block, i));
            self.fresh = true;
            leaves = self.stmt(stmt, state);
            self.path.pop();
        }
        leaves
    }

    /// Walks the branch or body `stmt`: its own block, or one statement in the place of a block,
    /// which is numbered as one, and made one if a local is declared there (`insert_branch`).
    fn nested(&mut self, stmt: &Stmt, state: &mut State) -> bool {
        match stmt {
            Stmt::Block(stmts) => self.block(stmts, state),
            _ => {
                let block = self.blocks;
                self.blocks += 1;
                self.path.push((block, 0));
                self.fresh = true;
                let leaves = self.stmt(stmt, state);
                self.path.pop();
                leaves
            }
        }
    }

    /// Walks a part that starts with nothing taken: a loop's body, a `try`'s blocks, a case.
    fn apart(&mut self, stmt: &Stmt, state: &State) {
        let mut fresh = State { taken: HashMap::new(), initialized: state.initialized.clone() };
        self.nested(stmt, &mut fresh);
    }

    fn apart_block(&mut self, stmts: &[Stmt], state: &State) {
        let mut fresh = State { taken: HashMap::new(), initialized: state.initialized.clone() };
        self.block(stmts, &mut fresh);
    }

    /// Walks `stmt`; returns whether it always leaves its block.
    fn stmt(&mut self, stmt: &Stmt, state: &mut State) -> bool {
        match stmt {
            Stmt::Local { name, init, .. } => {
                if let Some(e) = init {
                    self.expr(e, state);
                }
                state.taken.retain(|k, _| !k.reads_local(name));
                false
            }
            Stmt::Expr(e) => {
                self.expr(e, state);
                false
            }
            Stmt::Return(e, _) => {
                if let Some(e) = e {
                    self.expr(e, state);
                }
                true
            }
            Stmt::Throw(e) | Stmt::Yield(e) => {
                self.expr(e, state);
                true
            }
            Stmt::Break | Stmt::Continue | Stmt::BreakLabel(_) | Stmt::ContinueLabel(_) => true,
            // After the `if`, the values from before it that neither branch changed, or that the
            // branch which does not always leave kept; one that a branch started stays in it, so
            // each local is given its value in the block that reads it.
            Stmt::If { cond, then, otherwise } => {
                self.expr(cond, state);
                let before = state.clone();
                let mut other = state.clone();
                let then_leaves = self.nested(then, state);
                let other_leaves = match otherwise {
                    Some(o) => self.nested(o, &mut other),
                    None => false,
                };
                match (then_leaves, other_leaves) {
                    (true, true) => return true,
                    (true, false) => *state = other,
                    (false, true) => {}
                    (false, false) => *state = std::mem::take(state).meet(&other),
                }
                state.earlier(&before);
                false
            }
            Stmt::Block(stmts) => {
                let before = state.clone();
                let leaves = self.block(stmts, state);
                state.earlier(&before);
                leaves
            }
            Stmt::While { body, .. }
            | Stmt::DoWhile { body, .. }
            | Stmt::For { body, .. }
            | Stmt::ForEach { body, .. } => {
                state.forget();
                self.apart(body, state);
                state.forget();
                false
            }
            // The labeled loop or block itself, which stays the statement that the label names.
            Stmt::Labeled { body, .. } => {
                state.forget();
                let mut inner = State { taken: HashMap::new(), initialized: state.initialized.clone() };
                match &**body {
                    Stmt::Block(stmts) => self.block(stmts, &mut inner),
                    other => self.stmt(other, &mut inner),
                };
                state.forget();
                false
            }
            Stmt::Switch { cases, .. } => {
                state.forget();
                for case in cases {
                    self.apart_block(&case.body, state);
                }
                state.forget();
                false
            }
            Stmt::Try { body, catches, finally } => {
                state.forget();
                self.apart_block(body, state);
                for c in catches {
                    self.apart_block(&c.body, state);
                }
                if let Some(f) = finally {
                    self.apart_block(f, state);
                }
                state.forget();
                false
            }
        }
    }
}

/// Where each shared value's local is declared: per block, the statement it goes before, with the
/// declaration.
type Declarations = HashMap<usize, Vec<(usize, Stmt)>>;

/// The block and statement before which the local of `shared` is declared: the innermost block
/// that holds its first read and every later one.
fn declared_at(shared: &Shared) -> (usize, usize) {
    let mut level = 0;
    loop {
        let (block, index) = shared.at[level];
        let deeper = shared.at.get(level + 1).map(|&(b, _)| b);
        let same = shared.later.iter().all(|(_, at)| {
            at.get(level).map(|&(b, i)| b == block && i == index).unwrap_or(false)
                && at.get(level + 1).map(|&(b, _)| b) == deeper
        });
        if !same || deeper.is_none() {
            return (block, index);
        }
        level += 1;
    }
}

/// Inserts the declarations into the blocks of `stmts`, numbered as `Walk::block` numbers them.
fn insert(stmts: &mut Vec<Stmt>, next: &mut usize, declarations: &mut Declarations) {
    let block = *next;
    *next += 1;
    for stmt in stmts.iter_mut() {
        insert_nested(stmt, next, declarations);
    }
    if let Some(mut list) = declarations.remove(&block) {
        // From the last statement back, so each index is still the statement's; several before
        // one statement keep the order of their first reads.
        list.sort_by_key(|(i, _)| *i);
        for (i, declaration) in list.into_iter().rev() {
            stmts.insert(i, declaration);
        }
    }
}

/// `insert` for a branch or body, numbered as `Walk::nested` numbers it: one statement that is not
/// a block becomes one that holds the declarations before it.
fn insert_branch(stmt: &mut Stmt, next: &mut usize, declarations: &mut Declarations) {
    if let Stmt::Block(stmts) = stmt {
        return insert(stmts, next, declarations);
    }
    let block = *next;
    *next += 1;
    insert_nested(stmt, next, declarations);
    if let Some(list) = declarations.remove(&block) {
        let single = std::mem::replace(stmt, Stmt::Block(Vec::new()));
        let mut stmts: Vec<Stmt> = list.into_iter().map(|(_, declaration)| declaration).collect();
        stmts.push(single);
        *stmt = Stmt::Block(stmts);
    }
}

/// `insert` in the blocks that `Walk::stmt` walks inside `stmt`, in the same order.
fn insert_nested(stmt: &mut Stmt, next: &mut usize, declarations: &mut Declarations) {
    match stmt {
        Stmt::Block(stmts) => insert(stmts, next, declarations),
        Stmt::If { then, otherwise, .. } => {
            insert_branch(then, next, declarations);
            if let Some(o) = otherwise {
                insert_branch(o, next, declarations);
            }
        }
        Stmt::While { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::For { body, .. }
        | Stmt::ForEach { body, .. } => insert_branch(body, next, declarations),
        Stmt::Labeled { body, .. } => insert_nested(body, next, declarations),
        Stmt::Switch { cases, .. } => {
            for case in cases {
                insert(&mut case.body, next, declarations);
            }
        }
        Stmt::Try { body, catches, finally } => {
            insert(body, next, declarations);
            for c in catches {
                insert(&mut c.body, next, declarations);
            }
            if let Some(f) = finally {
                insert(f, next, declarations);
            }
        }
        _ => {}
    }
}

impl<'t> Lowering<'t> {
    /// Shares the repeated element and field reads of `method` (see the module comment).
    pub(super) fn share_element_reads(&self, method: &mut Method) {
        let mut walk = Walk {
            lowering: self,
            class: method.class,
            shared: Vec::new(),
            path: Vec::new(),
            blocks: 0,
            quiet: 0,
            closed: 0,
            fresh: false,
        };
        walk.block(&method.body, &mut State::default());
        let shared: Vec<Shared> = walk.shared.into_iter().filter(|s| !s.later.is_empty()).collect();
        if shared.is_empty() {
            return;
        }
        let mut taken: HashSet<String> = method.params.iter().map(|p| p.name.clone()).collect();
        let mut declared: HashSet<String> = HashSet::new();
        for stmt in &method.body {
            visit_stmt(
                stmt,
                &mut |e| {
                    visit_expr(e, &mut |x| match &x.kind {
                        ExprKind::Var(v) | ExprKind::InstanceOf { binding: Some(v), .. } => {
                            taken.insert(v.clone());
                        }
                        _ => {}
                    })
                },
                &mut |s| match s {
                    Stmt::Local { name, .. } | Stmt::ForEach { name, .. } => {
                        declared.insert(name.clone());
                    }
                    Stmt::Try { catches, .. } => declared.extend(catches.iter().map(|c| c.name.clone())),
                    _ => {}
                },
            );
        }
        taken.extend(declared);
        // The local of each first read (and whether it is read in a statement of its own), and the
        // read that each later read takes.
        let mut firsts: HashMap<ExprId, (String, bool)> = HashMap::new();
        let mut laters: HashMap<ExprId, String> = HashMap::new();
        let mut places = Vec::new();
        for s in &shared {
            let mut name = match &s.key {
                Key::Element(array, index) => {
                    let array = match array {
                        Array::Local(n) | Array::Field(n) | Array::Static(_, n) => n,
                    };
                    let index = match index {
                        Index::Local(v) => v.clone(),
                        Index::Constant(n) => n.to_string(),
                    };
                    format!("{array}_{index}")
                }
                Key::Field(object, field) => format!("{}_{field}", object.as_deref().unwrap_or("this")),
            };
            while taken.contains(&name) {
                name.push('_');
            }
            taken.insert(name.clone());
            let at = declared_at(s);
            let hoist = s.hoist && s.at.last() == Some(&at);
            firsts.insert(s.first, (name.clone(), hoist));
            laters.extend(s.later.iter().map(|(id, _)| (*id, name.clone())));
            places.push((at, s.first, name));
        }
        // Each first read's type and position, and the read itself where it is one statement.
        let mut found: HashMap<ExprId, (Type, Pos, Option<Expr>)> = HashMap::new();
        for stmt in &mut method.body {
            rewrite_stmt(stmt, &mut |x| {
                if let Some((name, hoist)) = firsts.get(&x.id) {
                    let read = std::mem::replace(&mut x.kind, ExprKind::Var(name.clone()));
                    let value = self.synth(read, x.pos, x.ty);
                    if *hoist {
                        found.insert(x.id, (x.ty, x.pos, Some(value)));
                    } else {
                        found.insert(x.id, (x.ty, x.pos, None));
                        x.kind = ExprKind::Assign {
                            op: None,
                            target: Place::Var(name.clone()),
                            value: Box::new(value),
                        };
                    }
                } else if let Some(name) = laters.get(&x.id) {
                    x.kind = ExprKind::Var(name.clone());
                }
            });
        }
        // Each local's declaration, and after it the statement that reads its value, if any.
        let mut declarations: Declarations = HashMap::new();
        for ((block, index), first, name) in places {
            let (ty, pos, read) = found.remove(&first).expect("a first read");
            let list = declarations.entry(block).or_default();
            list.push((index, Stmt::Local { ty, name: name.clone(), init: None, pos }));
            if let Some(read) = read {
                let assign = ExprKind::Assign { op: None, target: Place::Var(name), value: Box::new(read) };
                list.push((index, Stmt::Expr(self.synth(assign, pos, ty))));
            }
        }
        insert(&mut method.body, &mut 0, &mut declarations);
    }

    /// The names of the volatile fields of every class (`Lowering::volatile_field_names`).
    fn volatile_field_names(&self) -> &HashSet<String> {
        self.volatile_field_names.get_or_init(|| {
            self.types
                .iter()
                .flat_map(|t| t.node.list("members"))
                .filter(|m| m.kind() == "VARIABLE" && m.has_flag("VOLATILE"))
                .map(|m| m.str("name").to_string())
                .collect()
        })
    }

    /// Whether the instance field `name` of an object of class `t` is declared `final`: the first
    /// class from `t` up that declares it decides. `None` where none of the program's does.
    fn final_field(&self, t: usize, name: &str) -> Option<bool> {
        let mut class = Some(t);
        // A bound on the climb, against a cycle that only a broken program's classes could make.
        for _ in 0..self.types.len().min(256) {
            let Some(c) = class else { break };
            let node = self.types[c].node;
            if let Some(field) = node
                .list("members")
                .find(|m| m.kind() == "VARIABLE" && m.str("name") == name && !m.has_flag("STATIC"))
            {
                return Some(field.has_flag("FINAL") || node.kind() == "INTERFACE");
            }
            class = node
                .get("extendsClause")
                .and_then(|clause| self.type_index(&super::captures::raw(clause.java_type())))
                .filter(|&s| s != c && self.types[s].node.kind() == "CLASS");
        }
        None
    }
}
