//! Objects on the stack: a `new` whose object never leaves its method (`Facts::stack_objects`)
//! makes it in a `java::Slot` that the method declares at its start, `slot_1.put(..)`, rather
//! than in the collected heap, `Arc::new(..)`. The handle is the same `java::Arc`, so nothing
//! else in the code changes (docs/collector.md, "Objects on the stack").
//!
//! Two kinds of functions get a second form, `<name>_on_stack`, which takes a slot after its
//! receiver and makes its object there:
//!
//! - a constructor that makes its object itself (one that uses `this` as a value,
//!   docs/object-model.md 1.4), which a `new` on the stack calls;
//! - a factory (`analysis::escape`), a method that returns a new object that does not otherwise
//!   leave it, such as an `iterator()`. A call of it whose object stays in the calling method
//!   passes a slot of the caller's, in which the factory makes its object, or which it passes on
//!   to the factory whose object it returns.
//!
//! A call that dispatches, among a few classes that the points-to facts allow or that a profiled
//! run saw (`Gen::among_on_stack`), passes its slot to a form of its small dispatcher, whose arm
//! for the classes that run the factory passes it on; the whole dispatcher, for any other class,
//! makes its object in the heap.
//!
//! A call that the generator cannot write in its form on the stack is written as usual, and its
//! object goes in the heap: the slot is then left empty, which is always correct.
//!
//! An object that leaves only at some statements (`StackObjects::moves`) is moved to the heap
//! before each of them, `unsafe { slot_1.to_heap(&mut pos) };`, and its local is `mut` (see
//! `Moves`). A guarded move (`analysis::Guard`) tests the receiver's class first,
//! `if !matches!(p.0.tag(), 3) { unsafe { slot_1.to_heap(&mut argument_0) }; }`, and is written
//! unguarded when that test cannot be (`Gen::receiver_of_class`). A method in which a move cannot
//! be written is written again with the object in the heap (`Gen::method_form`).

use super::objects::changes_object;
use super::*;
use crate::analysis::Guard;

/// The most slots a method declares, so that a method that recursion runs deep keeps a small
/// frame.
const SLOTS_PER_METHOD: usize = 8;

/// The most fields (with those of its superclasses) of an object that goes in a slot.
const FIELDS_PER_SLOT: usize = 16;

impl<'a> Gen<'a> {
    /// Whether objects of class `c` can be made in a slot: a class whose objects are handles
    /// (not owned plain values or closures), with a struct that is not a Rust generic, and not
    /// too large for a frame. A lambda that captures nothing is made once per place it is
    /// written and kept (`captures_nothing`), so it is never in a slot.
    pub(super) fn stack_class(&self, c: usize) -> bool {
        let decl = &self.class.types[c];
        let fields: usize = std::iter::once(c)
            .chain(self.class.hierarchy.ancestors(c))
            .map(|t| self.class.types[t].fields.len())
            .sum();
        decl.kind == TypeKind::Class
            && !decl.is_abstract
            && decl.type_params.is_empty()
            && !self.facts.owned.contains(&decl.name)
            && !decl.implements.iter().any(|i| self.facts.closure_types.contains(i))
            && fields <= FIELDS_PER_SLOT
            && !(matches!(decl.origin, Origin::Lambda { .. }) && decl.fields.is_empty())
    }

    /// The `new`s and calls of factories of method `m` whose objects it makes in slots, with
    /// their classes, in the order of the code: found once for the program, with the setup
    /// (`find_stack_sites`).
    pub(super) fn stack_sites(&self, m: usize) -> Vec<(ExprId, usize)> {
        let mut sites = self.stack_sites.get(&m).cloned().unwrap_or_default();
        sites.retain(|(id, _)| !self.unmoved.contains(id));
        sites
    }

    /// `stack_sites`, found in the code of `m`.
    pub(super) fn find_stack_sites(&self, m: usize) -> Vec<(ExprId, usize)> {
        if self.facts.stack_objects.objects.is_empty() {
            return Vec::new();
        }
        let mut sites = Vec::new();
        for s in &self.class.methods[m].body {
            visit_stmt(
                s,
                &mut |e| {
                    visit_expr(e, &mut |x| {
                        if sites.len() < SLOTS_PER_METHOD
                            && self.facts.stack_objects.objects.contains(&x.id)
                            && let Some(c) = self.slot_class(x)
                        {
                            sites.push((x.id, c));
                        }
                    })
                },
                &mut |_| {},
            );
        }
        sites
    }

    /// The class of the object that `x`, a `new` or a call of a factory, makes, when it can be
    /// made in a slot.
    fn slot_class(&self, x: &Expr) -> Option<usize> {
        match &x.kind {
            ExprKind::New { class: c, ctor: Some(_), .. } => self.stack_class(*c).then_some(*c),
            ExprKind::Call { resolved: Some(r), .. } => {
                let t = self.call_of(x)?;
                let c = self.facts.stack_objects.factories.get(&t)?.class;
                (*r == t && self.class.methods[t].is_static && self.stack_class(c)).then_some(c)
            }
            ExprKind::MethodCall { receiver, is_super: false, .. } => {
                let t = self.call_of(x)?;
                let c = self.facts.stack_objects.factories.get(&t)?.class;
                let home = self.class.methods[t].is_static
                    || matches!(self.homes[t], Home::Class(_) | Home::Enum(_));
                (home && self.closure_type(receiver.ty).is_none() && self.stack_class(c)).then_some(c)
            }
            _ => None,
        }
    }

    /// The factory that the call `x` runs, when its object may go in a slot: a call whose
    /// object stays in its method, or the call whose object a factory returns.
    fn call_of(&self, x: &Expr) -> Option<usize> {
        let found = &self.facts.stack_objects;
        found.calls.get(&x.id).copied().or_else(|| found.factories.values().find(|p| p.made == x.id)?.call)
    }

    /// The call `x` of a factory in its form on the stack, which makes its object in the slot of
    /// `x`, or `None` when it is written as usual. It follows the forms of `expr`'s calls.
    pub(super) fn call_on_stack(&mut self, x: &'a Expr) -> Option<Code> {
        let slot = self.slot_of(x.id)?;
        let t = self.call_of(x)?;
        if !self.stack_forms.contains(&t) {
            return None;
        }
        let name = format!("{}_on_stack", self.fn_names[t]);
        let with = |args: Vec<String>| {
            std::iter::once(format!("&{slot}")).chain(args).collect::<Vec<_>>().join(", ")
        };
        let (index, receiver, args) = match &x.kind {
            ExprKind::Call { resolved: Some(r), args, .. } => (*r, None, args),
            ExprKind::MethodCall { receiver, resolved: Some(r), args, is_super: false, .. } => {
                (*r, Some(&**receiver), args)
            }
            _ => return None,
        };
        let Some(receiver) = receiver.filter(|_| !self.class.methods[index].is_static) else {
            // `static_call`, without its block for an array it lends.
            if index != t || (0..args.len()).any(|i| self.facts.borrowed_arrays.contains_key(&(t, i))) {
                return None;
            }
            let args = self.call_args(t, args);
            return Some(Code::atom(format!("{}_on_stack({})", self.static_fn(t), with(args))));
        };
        if self.closure_type(receiver.ty).is_some() || self.inlined(receiver, index) {
            return None;
        }
        // `enum_call`, calling the body itself.
        if let Some(d) = self.enum_of(receiver) {
            match self.enum_body(d, index) {
                Some(b) if b == t => {}
                Some(_) => return None,
                // A call that dispatches, which runs the factory for the classes it tests for
                // first.
                None => return self.among_on_stack(d, receiver, index, t, args, &slot),
            }
            self.need(Need::Body(t));
            let obj = self.thin_receiver(d, receiver);
            let args = self.call_args(t, args);
            return Some(Code::atom(self.enum_fn_call(d, &name, &obj, &with(args))));
        }
        if self.dispatch(receiver, index, false) != t || self.is_object_method(t) {
            return None;
        }
        // `method_call`, on the enum that has the method, then on the receiver's class.
        match self.homes[t] {
            Home::Enum(d) => {
                self.need(Need::Body(t));
                let obj = self.thin_as_enum(d, receiver);
                let args = self.call_args(t, args);
                return Some(Code::atom(self.enum_fn_call(d, &name, &obj, &with(args))));
            }
            Home::Class(_) => {}
            _ => return None,
        }
        if !self.facts.self_methods.contains(&t) {
            let obj = self.receiver_ref(receiver);
            let args = self.call_args(t, args);
            return Some(Code::atom(format!("{}_on_stack({obj}, {})", self.qualified_fn(t), with(args))));
        }
        if self.is_plain(receiver) {
            if args.iter().any(|a| changes_object(a, receiver, &self.facts.mutating)) {
                return None;
            }
            let args = self.call_args(t, args);
            let obj = self.object(receiver);
            let path = self.method_path(receiver, t);
            return Some(Code::atom(format!("{obj}.{path}{name}({})", with(args))));
        }
        let Type::Class(class, _) = receiver.ty else { return None };
        if args.iter().any(|a| has_effects(a) || reads_class(self.class, a, class)) {
            return None;
        }
        let borrow = if self.facts.mutating.contains(&t) { ".borrow_mut()" } else { ".borrow()" };
        let obj = self.object(receiver);
        let args = self.call_args(t, args);
        let path = self.method_path(receiver, t);
        let call = format!("{obj}{borrow}.{path}{name}({})", with(args));
        Some(Code::atom(if self.guarded { format!("{{ {call} }}") } else { call }))
    }

    /// The type of the value that a slot holds for an object of class `c`: what its handle
    /// points to.
    pub(super) fn slot_content(&self, c: usize) -> String {
        let name = &self.class.types[c].name;
        let structure = self.struct_of(c);
        if self.celled(name) && self.monitored(name) {
            format!("java::Monitored<{structure}>")
        } else if self.celled(name) || self.facts.immutable.contains(name) {
            structure
        } else {
            format!("{}<{structure}>", self.cell(c))
        }
    }

    /// The functions that get a form on the stack (see the module's comment): the factories and
    /// the constructors that make their object themselves, that a `new` or a call whose object
    /// goes in a slot runs, directly, through a factory or through `this(..)`.
    pub(super) fn stack_forms(&self, sites: &HashMap<usize, Vec<(ExprId, usize)>>) -> HashSet<usize> {
        let mut found = HashSet::new();
        let mut made: Vec<&'a Expr> = Vec::new();
        let mut with_sites: Vec<usize> = sites.keys().copied().collect();
        with_sites.sort_unstable();
        for m in with_sites {
            made.extend(sites[&m].iter().filter_map(|&(id, _)| self.find_expr(m, id)));
        }
        let mut ctors = Vec::new();
        while let Some(x) = made.pop() {
            match x.kind {
                ExprKind::New { class: c, ctor: Some(k), .. } => {
                    if !self.builds_plain(&self.class.types[c].name) {
                        ctors.push(k);
                    }
                }
                _ => {
                    if let Some(t) = self.call_of(x)
                        && found.insert(t)
                        && let Some(p) = self.facts.stack_objects.factories.get(&t)
                        && let Some(inner) = self.find_expr(t, p.made)
                        && self.slot_class(inner).is_some()
                    {
                        made.push(inner);
                    }
                }
            }
        }
        while let Some(k) = ctors.pop() {
            if !found.insert(k) {
                continue;
            }
            if let Some(Stmt::Expr(Expr { kind: ExprKind::ThisCall { resolved: Some(d), .. }, .. })) =
                explicit_call(&self.class.methods[k].body).map(|i| &self.class.methods[k].body[i])
            {
                ctors.push(*d);
            }
        }
        found
    }

    /// The expression `id` in the body of method `m`.
    fn find_expr(&self, m: usize, id: ExprId) -> Option<&'a Expr> {
        let mut found = None;
        for s in &self.class.methods[m].body {
            visit_stmt(
                s,
                &mut |e| {
                    visit_expr(e, &mut |x| {
                        if x.id == id {
                            found = Some(x);
                        }
                    })
                },
                &mut |_| {},
            );
        }
        found
    }

    /// Names the slots of the `new`s and calls `sites` of method `index`, distinct from its
    /// locals and the free functions, and the slot parameter of its form on the stack. In a
    /// factory's form on the stack, the object it returns goes in the slot it is given.
    pub(super) fn name_slots(&mut self, index: usize, sites: &[(ExprId, usize)]) {
        let taken: HashSet<String> = self.locals.values().cloned().collect();
        let free = |name: &str| !taken.contains(name) && !self.static_fn_names.contains(name);
        let mut next = 1;
        let mut slots = HashMap::new();
        for &(id, c) in sites {
            let name = loop {
                let name = format!("slot_{next}");
                next += 1;
                if free(&name) {
                    break name;
                }
            };
            slots.insert(id, (name, c));
        }
        let mut param = "obj_slot".to_string();
        while !free(&param) {
            param.push('_');
        }
        if self.on_stack
            && let Some(p) = self.facts.stack_objects.factories.get(&index)
        {
            slots.insert(p.made, (param.clone(), p.class));
        }
        self.slots = slots;
        self.slot_param = param;
    }

    /// The type of the slot parameter of the form on the stack of method `index`.
    pub(super) fn slot_param_type(&self, index: usize) -> String {
        let m = &self.class.methods[index];
        let c = match self.facts.stack_objects.factories.get(&index) {
            Some(p) if !m.is_ctor => p.class,
            _ => m.class,
        };
        format!("&java::Slot<{}>", self.slot_content(c))
    }

    /// Declares the slots named by `name_slots`, at the start of the method's body.
    pub(super) fn declare_slots(&mut self, sites: &[(ExprId, usize)]) {
        for &(id, c) in sites {
            let name = self.slots[&id].0.clone();
            let content = self.slot_content(c);
            self.line(&format!("let {name} = java::Slot::<{content}>::new();"));
        }
    }

    /// The slot that the `new` or call `id` makes its object in, if it is on the stack.
    pub(super) fn slot_of(&self, id: ExprId) -> Option<String> {
        self.slots.get(&id).map(|(name, _)| name.clone())
    }

    /// The moves to the heap of method `index` (`StackObjects::moves`) of the objects that it
    /// makes in its slots, once `name_slots` named them.
    pub(super) fn find_moves(&self, index: usize) -> Moves {
        let mut moves = Moves::default();
        for mv in self.facts.stack_objects.moves.get(&index).into_iter().flatten() {
            let Some(slot) = self.slot_of(mv.made) else { continue };
            for &(before, guard) in &mv.before {
                moves.before.entry(before).or_default().push((mv.var.clone(), slot.clone(), mv.made, guard));
            }
            moves.declared.push(mv.declared);
        }
        moves
    }

    /// Whether the local declared at `pos` holds an object that the method moves to the heap,
    /// which makes it `mut`.
    pub(super) fn moved_local(&self, pos: Pos) -> bool {
        self.moves.declared.contains(&pos)
    }

    /// Writes the moves to the heap that come before `stmt` (`Slot::to_heap`): of each object in
    /// a slot that may leave the method in it, held by a local. A guarded move (`Guard`) is made
    /// only when the receiver of its call is not of the guard's class.
    pub(super) fn move_to_heap(&mut self, stmt: &'a Stmt) {
        let (Stmt::Local { init: Some(e), .. } | Stmt::Expr(e) | Stmt::Return(Some(e), _) | Stmt::Yield(e)) =
            stmt
        else {
            return;
        };
        let Some(moves) = self.moves.before.get(&e.id).cloned() else { return };
        for (var, slot, made, guard) in moves {
            // A variable that is not a plain local here is not moved: the method is then written
            // again with the object in the heap (`missed`).
            if !self.locals.contains_key(&var) || self.borrowed.contains(&var) || self.plain(&var).is_some() {
                continue;
            }
            let rust = self.local(&var);
            let line = format!("unsafe {{ {slot}.to_heap(&mut {rust}) }};");
            // A guard that cannot be written here leaves the move unguarded, which is only slower.
            match guard.and_then(|g| self.receiver_of_class(e, g)) {
                Some(test) => self.line(&format!("if !{test} {{ {line} }}")),
                None => self.line(&line),
            }
            self.moves.written.insert((e.id, made));
        }
    }

    /// The test that the receiver of the guard's call in `e`, a local, is of the guard's class
    /// exactly: its class number in the receiver's enum. `None` when it is not an enum's.
    fn receiver_of_class(&mut self, e: &'a Expr, guard: Guard) -> Option<String> {
        let mut receiver = None;
        visit_expr(e, &mut |x| {
            if x.id == guard.call
                && let ExprKind::MethodCall { receiver: r, .. } = &x.kind
            {
                receiver = Some(&**r);
            }
        });
        let receiver: &'a Expr = receiver?;
        if !matches!(receiver.kind, ExprKind::Var(_)) {
            return None;
        }
        let t = self.enum_of(receiver)?;
        if !self.is_enum_variant(t, guard.class) {
            return None;
        }
        let tags = self.tags(&[guard.class], &self.enum_variants(t));
        Some(if self.nullable(receiver) {
            format!("{}.as_ref().is_some_and(|o| matches!(o.0.tag(), {tags}))", self.object_place(receiver))
        } else {
            format!("matches!({}.0.tag(), {tags})", super::dispatch::operand(&self.object(receiver)))
        })
    }
}

/// A move before a statement: the local that holds the object, the slot, the `new` or call that
/// makes it, and the move's guard, if any.
type MoveAt = (String, String, ExprId, Option<Guard>);

/// The moves to the heap of the objects of the method being written that may leave it
/// (`StackObjects::moves`): an object made in a slot is moved before each statement where it may
/// leave, by `java::Slot::to_heap`, which does nothing once its local names the heap. A move that
/// is not written would let the object leave from the stack, so a method whose moves are not all
/// written is written again with those objects in the heap.
#[derive(Default)]
pub(super) struct Moves {
    /// By statement (the id of its expression, `Move::before`): the moves before it.
    before: HashMap<ExprId, Vec<MoveAt>>,
    /// The moves written, by statement and object.
    written: HashSet<(ExprId, ExprId)>,
    /// Where the locals that hold the objects are declared.
    declared: Vec<Pos>,
}

impl Moves {
    /// The objects with a move that was not written.
    pub(super) fn missed(&self) -> Vec<ExprId> {
        let mut missed: Vec<ExprId> = self
            .before
            .iter()
            .flat_map(|(&at, moves)| moves.iter().map(move |&(_, _, made, _)| (at, made)))
            .filter(|key| !self.written.contains(key))
            .map(|(_, made)| made)
            .collect();
        missed.sort_unstable();
        missed.dedup();
        missed
    }
}
