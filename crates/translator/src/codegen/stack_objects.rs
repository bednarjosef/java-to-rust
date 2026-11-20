//! Objects on the stack: a `new` whose object never leaves its method (`Facts::stack_objects`)
//! makes it in a `java::Slot` that the method declares at its start, `slot_1.put(..)`, rather
//! than in the collected heap, `Arc::new(..)`. The handle is the same `java::Arc`, so nothing
//! else in the code changes (docs/collector.md, "Objects on the stack"). A slot holds an object
//! of at most `BYTES_PER_SLOT` bytes of fields, and a method's slots take at most
//! `BYTES_PER_METHOD`. The body of a lambda written as a closure declares slots of its own at its
//! start (`Gen::enter_inner_slots`), so that every call of the closure has its own.
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
//!
//! A local declared with an object that is sure to be in its slot (`filled`) has the primitive
//! fields that the rest of its block reads before the variable is used otherwise read from the
//! slot right after its declaration, `let p_x = *unsafe { slot_1.object() }.x.fixed();`, and
//! those reads, also by getters, read the locals (`Gen::bind_slot_fields`).

use super::objects::changes_object;
use super::*;
use crate::analysis::Guard;

/// The most bytes that a method's slots take, as `object_bytes` counts them, so that a method
/// that recursion runs deep keeps a small frame: eight objects of the largest size.
const BYTES_PER_METHOD: usize = 8 * (BYTES_PER_SLOT + SLOT_HEADER);

/// The most bytes that the fields of an object that goes in a slot (with those of its
/// superclasses) may take, as `field_bytes` counts them: 16 references, or an iterator's 30
/// numbers and its cursor.
const BYTES_PER_SLOT: usize = 128;

/// What a slot takes beside its object's fields (its monitor and its state), about.
const SLOT_HEADER: usize = 16;

/// About the bytes a field of type `ty` takes in an object: a primitive its size, a reference
/// or an enum a word, a string four (it may own its text, `java::JString`), and any other value
/// (an array, a boxed number) two.
fn field_bytes(ty: Type) -> usize {
    match ty {
        Type::Boolean | Type::Byte => 1,
        Type::Short | Type::Char => 2,
        Type::Int | Type::Float => 4,
        Type::Long | Type::Double | Type::Class(..) | Type::Enum(_) => 8,
        Type::String => 32,
        _ => 16,
    }
}

impl<'a> Gen<'a> {
    /// Whether objects of class `c` can be made in a slot: a class whose objects are handles
    /// (not owned plain values or closures), with a struct that is not a Rust generic, and not
    /// too large for a frame. A lambda that captures nothing is made once per place it is
    /// written and kept (`captures_nothing`), so it is never in a slot.
    pub(super) fn stack_class(&self, c: usize) -> bool {
        let decl = &self.class.types[c];
        decl.kind == TypeKind::Class
            && !decl.is_abstract
            && decl.type_params.is_empty()
            && !self.facts.owned.contains(&decl.name)
            && !decl.implements.iter().any(|i| self.facts.closure_types.contains(i))
            && self.object_bytes(c) <= BYTES_PER_SLOT
            && !(matches!(decl.origin, Origin::Lambda { .. }) && decl.fields.is_empty())
    }

    /// About the bytes the fields of an object of class `c` take (`field_bytes`).
    fn object_bytes(&self, c: usize) -> usize {
        std::iter::once(c)
            .chain(self.class.hierarchy.ancestors(c))
            .flat_map(|t| &self.class.types[t].fields)
            .map(|f| field_bytes(f.ty))
            .sum()
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
        let mut bytes = 0;
        for s in &self.class.methods[m].body {
            visit_stmt(
                s,
                &mut |e| {
                    visit_expr(e, &mut |x| {
                        if self.facts.stack_objects.objects.contains(&x.id)
                            && let Some(c) = self.slot_class(x)
                            && bytes + self.object_bytes(c) + SLOT_HEADER <= BYTES_PER_METHOD
                        {
                            bytes += self.object_bytes(c) + SLOT_HEADER;
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
    /// `x`, or `None` when it is written as usual. It follows the forms of `expr`'s calls. A call
    /// of the factory itself, not of a dispatcher, whose factory always fills the slot
    /// (`fills_slot`), is noted in `filled`.
    pub(super) fn call_on_stack(&mut self, x: &'a Expr) -> Option<Code> {
        let mut among = false;
        let code = self.factory_call_on_stack(x, &mut among)?;
        if !among && self.call_of(x).is_some_and(|t| self.fills_slot(t)) {
            self.filled.insert(x.id);
        }
        Some(code)
    }

    /// Whether the form on the stack of factory `t` always puts its object in the slot it is
    /// given: every `return` of it returns the `new` that makes the object, of a class whose
    /// constructors build a plain value, which its form on the stack writes as `put` in that
    /// slot (`Gen::build_object`). (A factory that also returns other values, or the object of
    /// another factory, which may be made in the heap, may leave the slot empty.)
    fn fills_slot(&self, t: usize) -> bool {
        let Some(p) = self.facts.stack_objects.factories.get(&t) else { return false };
        let made = matches!(
            self.find_expr(t, p.made).map(|x| &x.kind),
            Some(ExprKind::New { class: c, ctor: Some(_), .. })
                if *c == p.class
                    && self.stack_class(*c)
                    && self.facts.plain_constructors.contains(&self.class.types[*c].name)
        );
        let mut returns_made = true;
        for s in &self.class.methods[t].body {
            visit_stmt(s, &mut |_| {}, &mut |s| {
                if let Stmt::Return(value, _) = s {
                    returns_made &= value.as_ref().is_some_and(|e| e.id == p.made);
                }
            });
        }
        made && returns_made
    }

    /// `call_on_stack`, which sets `among` when the call is written as one of a dispatcher's
    /// forms on the stack, which fills the slot only for some classes.
    fn factory_call_on_stack(&mut self, x: &'a Expr, among: &mut bool) -> Option<Code> {
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
            let texts = self.call_args(t, args);
            let suffix = self.entry_suffix(t, args);
            let args = texts;
            return Some(Code::atom(format!("{}{suffix}_on_stack({})", self.static_fn(t), with(args))));
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
                None => {
                    *among = true;
                    return self.among_on_stack(d, receiver, index, t, args, &slot);
                }
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
        // In a closure's body, its captures are locals too (`field_locals`).
        let taken: HashSet<String> =
            self.locals.values().chain(self.field_locals.values()).cloned().collect();
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

    /// Names and declares the slots of method `index` whose body is written inside another
    /// method's, as a closure's (`closures`): its objects that stay are made in slots that the
    /// closure's body declares, so each call of the closure has its own, as each call of the
    /// method would. Returns the enclosing method's slots and moves, which `leave_inner_slots`
    /// puts back.
    pub(super) fn enter_inner_slots(&mut self, index: usize) -> InnerSlots {
        let outer = InnerSlots {
            slots: std::mem::take(&mut self.slots),
            slot_param: self.slot_param.clone(),
            on_stack: std::mem::replace(&mut self.on_stack, false),
            moves: std::mem::take(&mut self.moves),
        };
        let sites = self.stack_sites(index);
        self.name_slots(index, &sites);
        self.moves = self.find_moves(index);
        self.declare_slots(&sites);
        outer
    }

    /// Puts back the enclosing method's slots and moves after a body that `enter_inner_slots`
    /// started. A move the body did not write makes the enclosing method be written again with
    /// that object in the heap, as for its own (`Gen::method_form`).
    pub(super) fn leave_inner_slots(&mut self, outer: InnerSlots) {
        let moves = std::mem::replace(&mut self.moves, outer.moves);
        self.missed.extend(moves.missed());
        self.slots = outer.slots;
        self.slot_param = outer.slot_param;
        self.on_stack = outer.on_stack;
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
            let tag = self.runtime("Thin::tag_or_null");
            format!("matches!({tag}({}.as_ref().map(|o| o.0)), {tags})", self.object_place(receiver))
        } else {
            format!("matches!({}.0.tag(), {tags})", super::dispatch::operand(&self.object(receiver)))
        })
    }

    /// The `new` or call of a factory that `stmt` declares a local with, `C v = <site>;`, when
    /// its object goes in a slot: forgotten in `filled` until the statement is written, which
    /// notes it again if it fills its slot.
    pub(super) fn declared_site(&mut self, stmt: &'a Stmt) -> Option<&'a Expr> {
        let Stmt::Local { init: Some(init), .. } = stmt else { return None };
        let site = match &init.kind {
            ExprKind::Cast(_, inner) => inner,
            _ => init,
        };
        self.slots.contains_key(&site.id).then(|| {
            self.filled.remove(&site.id);
            site
        })
    }

    /// After `stmt`, which declares `v` with the object that `site` made in its slot: reads the
    /// primitive fields of the object that the statements after it in its block, `rest`, read
    /// (`v.f`, or a call of a getter, `v.getF()`, whose body for the object's class is
    /// `return this.f;`) from the slot into locals, `let v_f = *unsafe { slot_1.object() }.f.fixed();`,
    /// and has those reads read the locals instead. Gives back the reads, which the caller
    /// forgets after the block.
    ///
    /// A field read so is the value of the field at every read that takes it. Until the variable
    /// is used for anything but such reads (`Use::Other`), nothing but the variable names the
    /// object: the escape analysis proved that it is held by the method's variables and objects
    /// alone, a constructor that builds a plain value does not hand out `this`, and a factory
    /// that returns it keeps it nowhere. So no code can write its fields before that use, and a
    /// read before the first one, in no loop that also has one, reads what the slot held when it
    /// was made. (A move to the heap comes before a statement that uses the variable, after which
    /// nothing is taken from the slot.) The local takes the field where the compiler still sees
    /// the slot's stores, which it forwards to it: the field is not stored and read back through
    /// a handle whose address, masked of its class number, it cannot follow.
    pub(super) fn bind_slot_fields(
        &mut self,
        stmt: &'a Stmt,
        site: &'a Expr,
        rest: &'a [Stmt],
    ) -> Vec<ExprId> {
        let Stmt::Local { name: var, .. } = stmt else { return Vec::new() };
        let Some((slot, c)) = self.slots.get(&site.id).cloned() else { return Vec::new() };
        if !self.filled.contains(&site.id) || !self.locals.contains_key(var.as_str()) {
            return Vec::new();
        }
        let name = &self.class.types[c].name;
        let celled = self.celled(name);
        if !celled && !self.facts.immutable.contains(name) {
            return Vec::new();
        }
        let mut scan = Scan { var, class: c, uses: Vec::new(), loops: Vec::new(), next_loop: 0 };
        for s in rest {
            self.scan_stmt(s, &mut scan);
        }
        let first_other = scan.uses.iter().position(|u| matches!(u.0, Use::Other)).unwrap_or(scan.uses.len());
        let other_loops: HashSet<u32> = scan
            .uses
            .iter()
            .filter(|u| matches!(u.0, Use::Other))
            .flat_map(|u| u.1.iter().copied())
            .collect();
        let mut bound: Vec<(usize, &'a str, String)> = Vec::new();
        let mut reads = Vec::new();
        for (u, loops) in &scan.uses[..first_other] {
            let Use::Read(read, owner, field) = *u else { continue };
            // The struct that declares the field holds cells as the object's does.
            let same_cells = self.celled(&self.class.types[owner].name) == celled;
            if !same_cells
                || loops.iter().any(|l| other_loops.contains(l))
                || self.loop_fields.contains_key(&read.id)
            {
                continue;
            }
            let local = match bound.iter().find(|(o, f, _)| *o == owner && *f == field) {
                Some((_, _, local)) => local.clone(),
                None => {
                    let rust_field = field_name(&self.class.types[owner], field);
                    let base = format!("{}_{rust_field}", self.local(var).trim_start_matches("r#"));
                    let taken = |n: &str| {
                        self.locals.values().any(|l| l == n)
                            || self.fn_name_set.contains(n)
                            || self.loop_fields.values().any(|l| l == n)
                            || bound.iter().any(|(_, _, l)| l == n)
                    };
                    let local = (0..)
                        .map(|k| if k == 0 { base.clone() } else { format!("{base}{k}") })
                        .find(|n| !taken(n))
                        .unwrap_or_default();
                    let path = hierarchy::base_path(self.facts, self.class, c, owner) + &rust_field;
                    let object = format!("unsafe {{ {slot}.object() }}");
                    let value =
                        if celled { format!("*{object}.{path}.fixed()") } else { format!("{object}.{path}") };
                    self.line(&format!("let {local} = {value};"));
                    bound.push((owner, field, local.clone()));
                    local
                }
            };
            self.loop_fields.insert(read.id, local);
            reads.push(read.id);
        }
        reads
    }

    /// The uses of the variable of `scan` in `stmt`, in the order of the code, with the loops
    /// around each (see `bind_slot_fields`).
    fn scan_stmt(&self, stmt: &'a Stmt, scan: &mut Scan<'a>) {
        match stmt {
            Stmt::Local { init: Some(e), .. }
            | Stmt::Expr(e)
            | Stmt::Return(Some(e), _)
            | Stmt::Yield(e)
            | Stmt::Throw(e) => self.scan_expr(e, scan),
            Stmt::If { cond, then, otherwise } => {
                self.scan_expr(cond, scan);
                self.scan_stmt(then, scan);
                if let Some(o) = otherwise {
                    self.scan_stmt(o, scan);
                }
            }
            Stmt::While { cond, body } | Stmt::DoWhile { body, cond } => {
                scan.enter_loop();
                self.scan_expr(cond, scan);
                self.scan_stmt(body, scan);
                scan.loops.pop();
            }
            Stmt::For { init, cond, update, body } => {
                init.iter().for_each(|s| self.scan_stmt(s, scan));
                scan.enter_loop();
                if let Some(cond) = cond {
                    self.scan_expr(cond, scan);
                }
                self.scan_stmt(body, scan);
                update.iter().for_each(|e| self.scan_expr(e, scan));
                scan.loops.pop();
            }
            Stmt::ForEach { array, body, .. } => {
                self.scan_expr(array, scan);
                scan.enter_loop();
                self.scan_stmt(body, scan);
                scan.loops.pop();
            }
            Stmt::Block(stmts) => stmts.iter().for_each(|s| self.scan_stmt(s, scan)),
            Stmt::Switch { selector, cases } => {
                self.scan_expr(selector, scan);
                for case in cases {
                    for label in &case.labels {
                        if let CaseLabel::Value(e) = label {
                            self.scan_expr(e, scan);
                        }
                    }
                    case.body.iter().for_each(|s| self.scan_stmt(s, scan));
                }
            }
            Stmt::Labeled { body, .. } => self.scan_stmt(body, scan),
            Stmt::Try { body, catches, finally } => {
                body.iter().for_each(|s| self.scan_stmt(s, scan));
                catches.iter().flat_map(|c| &c.body).for_each(|s| self.scan_stmt(s, scan));
                finally.iter().flatten().for_each(|s| self.scan_stmt(s, scan));
            }
            Stmt::Local { init: None, .. }
            | Stmt::Return(None, _)
            | Stmt::Break
            | Stmt::Continue
            | Stmt::BreakLabel(_)
            | Stmt::ContinueLabel(_) => {}
        }
    }

    /// `scan_stmt` for an expression. A switch expression, whose statements may hold loops, counts
    /// as a loop around all of the expression.
    fn scan_expr(&self, e: &'a Expr, scan: &mut Scan<'a>) {
        let mut switch = false;
        visit_expr(e, &mut |x| switch |= matches!(x.kind, ExprKind::Switch { .. }));
        if switch {
            scan.enter_loop();
        }
        // The mentions of the variable that are the objects of the reads found.
        let mut read_on: Vec<*const Expr> = Vec::new();
        visit_expr(e, &mut |x| {
            if let Some((owner, field, on)) = self.slot_field_read(x, scan.var, scan.class) {
                read_on.push(on);
                let loops = scan.loops.clone();
                scan.uses.push((Use::Read(x, owner, field), loops));
                return;
            }
            let other = match &x.kind {
                ExprKind::Var(v) => v == scan.var && !read_on.contains(&(x as *const Expr)),
                ExprKind::Assign { target: Place::Var(v), .. }
                | ExprKind::IncDec { target: Place::Var(v), .. } => v == scan.var,
                _ => false,
            };
            if other {
                let loops = scan.loops.clone();
                scan.uses.push((Use::Other, loops));
            }
        });
        if switch {
            scan.loops.pop();
        }
    }

    /// When `x` reads a primitive field of the object in variable `var`, of class `c`, which a
    /// local may take (`bind_slot_fields`): the class that declares the field, its name, and the
    /// mention of the variable. `v.f`, or `v.m()` where the method that class `c` runs for `m`
    /// is `return this.f;`; `v` may be upcast.
    fn slot_field_read(&self, x: &'a Expr, var: &str, c: usize) -> Option<(usize, &'a str, *const Expr)> {
        let of_var = |o: &'a Expr| -> Option<&'a Expr> {
            let o = match &o.kind {
                ExprKind::Cast(_, inner) => inner,
                _ => o,
            };
            matches!(&o.kind, ExprKind::Var(v) if v == var).then_some(o)
        };
        if !(x.ty.is_numeric() || x.ty == Type::Boolean) {
            return None;
        }
        let (on, read) = match &x.kind {
            ExprKind::Field { object, .. } => (of_var(object)?, x),
            ExprKind::MethodCall { receiver, args, resolved: Some(r), is_super: false, .. }
                if args.is_empty() =>
            {
                let on = of_var(receiver)?;
                let class: &'a Class = self.class;
                let m = &class.methods[class.hierarchy.implementation(*r, c)?];
                if m.is_static || m.is_abstract || m.is_synchronized {
                    return None;
                }
                let [Stmt::Return(Some(read), _)] = &m.body[..] else { return None };
                match &read.kind {
                    ExprKind::Field { object, .. }
                        if matches!(object.kind, ExprKind::This) && read.ty == x.ty => {}
                    _ => return None,
                }
                (on, read)
            }
            _ => return None,
        };
        let ExprKind::Field { object, name } = &read.kind else { return None };
        let owner = self.class.field_owner(object.ty, name)?;
        let of_class = owner == c || self.class.hierarchy.ancestors(c).contains(&owner);
        let declared = self.class.types[owner].fields.iter().any(|f| f.name == *name);
        (of_class && declared).then_some((owner, name.as_str(), on as *const Expr))
    }
}

/// The slots and moves of the method being written, set aside while the body of another method
/// is written inside it (`Gen::enter_inner_slots`).
pub(super) struct InnerSlots {
    slots: HashMap<ExprId, (String, usize)>,
    slot_param: String,
    on_stack: bool,
    moves: Moves,
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

/// What the statements after a local's declaration do with the object on the stack it holds
/// (`Gen::bind_slot_fields`).
enum Use<'a> {
    /// A read of a primitive field of the object: the expression, the class that declares the
    /// field, and its name.
    Read(&'a Expr, usize, &'a str),
    /// Any other mention of the variable, or an assignment of it: other code may hold the object
    /// from then on, and write its fields.
    Other,
}

/// The uses of the variable `var`, which holds an object of class `class` on the stack, found
/// so far in the order of the code, each with the loops around it (numbered from 0 as they are
/// met, `loops` those around the code being looked at).
struct Scan<'a> {
    var: &'a str,
    class: usize,
    uses: Vec<(Use<'a>, Vec<u32>)>,
    loops: Vec<u32>,
    next_loop: u32,
}

impl Scan<'_> {
    fn enter_loop(&mut self) {
        self.loops.push(self.next_loop);
        self.next_loop += 1;
    }
}
