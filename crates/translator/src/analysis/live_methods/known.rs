//! The values that the walk knows, and the code that they leave out: a branch on a condition
//! whose value is known runs one side only, so the calls on the other side run nothing.
//!
//! A value is known when it is a literal or a constant (a `static final` one, or the result of a
//! method that nothing overrides and that only returns a literal), when it is the same in every
//! place that gives it, or when it follows from known values by an operation that cannot fail. A
//! parameter is given by the arguments of every call of its method that can run, a local by its
//! initializer and every assignment to it in the method, and an instance field by its default,
//! its initializer and every assignment to it that can run; a parameter is not known when its
//! method is entered other than by such a call (`main`, the runtime's calls, a constructor run
//! for a class without one of its own), a local bound by a loop, a `catch` or a pattern is not,
//! and neither is a field that holds a captured local. `x instanceof T` is known to be false
//! while no object of a class below `T` is created, when `T` is a class that no value of the
//! runtime (a string, a box, a builder, an enum constant) can be.
//!
//! The walk starts from knowing nothing and learns as it goes, so a value may change: when
//! another call passes a different argument, or an object of a class below `T` is created, every
//! method whose code relied on the old value is walked again. A value only goes from unknown to
//! known and then to varying, and a class, once created, stays so, so the walk ends. What it
//! ends with is consistent: every call that can run is one that the walk found, with the
//! arguments it saw, so a parameter holds the value that every such call gives, and so on.
//!
//! In BigInteger, `multiply(val)` passes `parallel = false` on to every recursive multiply and
//! square, and to the field of each `RecursiveOp`, so `forkOrInvoke` never forks, and `join`
//! never helps another worker of a pool: the program starts no thread.

use super::*;

/// What a place is known to hold: one value, or values that differ. A place that no code has
/// given a value yet is not in the maps.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Known {
    Value(i128),
    Varying,
}

impl Known {
    fn of(value: Option<i128>) -> Known {
        value.map_or(Known::Varying, Known::Value)
    }

    /// What a place known as `old` holds once it is also given `new`.
    fn join(old: Option<Known>, new: Known) -> Known {
        match (old, new) {
            (None, new) => new,
            (Some(Known::Value(a)), Known::Value(b)) if a == b => Known::Value(a),
            _ => Known::Varying,
        }
    }

    fn value(self) -> Option<i128> {
        match self {
            Known::Value(v) => Some(v),
            Known::Varying => None,
        }
    }
}

/// The parameters and locals of the method being walked, by name.
pub(super) type Env = HashMap<String, Known>;

/// What the walk has learned, and who relied on it.
#[derive(Clone, Default)]
pub(super) struct Knowledge {
    /// The parameters of each method, by method and position.
    params: HashMap<(usize, usize), Known>,
    /// The instance fields, by declaring class and name. A field not here holds its default.
    fields: HashMap<(usize, String), Known>,
    /// The fields whose class a write did not tell: none of them is known.
    unplaced: HashSet<String>,
    /// The methods whose code read each field.
    field_readers: HashMap<(usize, String), HashSet<usize>>,
    /// The methods whose code relied on there being no object of a class below each type: once
    /// there is one, nothing makes their values known again, so they are walked again once.
    absence_readers: HashMap<usize, HashSet<usize>>,
    /// The methods whose code relied on what `same_literal` gives for a method on the objects
    /// of the classes below a type, by method and type: they are walked again when an object of
    /// another class below the type changes that, not whenever one is created.
    literal_readers: HashMap<(usize, usize), HashSet<usize>>,
    /// What `same_literal` gives for each method and type that it has been asked for, kept up as
    /// objects of the classes below the type are created (`Walk::created`).
    literals: HashMap<(usize, usize), Literals>,
    /// The methods of `literals`, by type.
    literal_methods: HashMap<usize, Vec<usize>>,
    /// The types that no value of the runtime can be and that only classes are below, by type.
    program_only: HashMap<usize, bool>,
    /// Whether some implementation of each method only returns a literal, by method.
    literal_groups: HashMap<usize, bool>,
    /// The parameters and locals of the method being walked (`STATIC`: none).
    pub(super) env: Rc<Env>,
    /// Where what code relies on goes instead of the readers, while the walk looks at arguments
    /// before deciding whether to pass them (`Walk::probe`).
    deferred: Option<Vec<(Reliance, usize)>>,
}

/// What a method's code relied on (`Knowledge::rely`).
#[derive(Clone)]
pub(super) enum Reliance {
    /// A field, by declaring class and name.
    Field((usize, String)),
    /// There being no object of a class below a type.
    Absence(usize),
    /// What `same_literal` gives for a method and a type.
    Literal(usize, usize),
}

/// The literals that a method returns on the objects created so far of the classes below a type
/// (`Walk::same_literal`).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Literals {
    /// No class below the type has objects.
    None,
    /// Each class with objects returns this literal.
    One(i128),
    /// They return different literals, or one of them does not return a literal.
    Not,
}

impl Literals {
    /// What the classes return once objects of a class that returns `literal` are created too.
    fn and(self, literal: Option<i128>) -> Literals {
        match (self, literal) {
            (Literals::None, Some(v)) => Literals::One(v),
            (Literals::One(a), Some(b)) if a == b => Literals::One(a),
            _ => Literals::Not,
        }
    }

    /// The literal that every class with objects returns, when there is one.
    fn value(self) -> Option<i128> {
        match self {
            Literals::One(v) => Some(v),
            Literals::None | Literals::Not => None,
        }
    }
}

impl Knowledge {
    /// Method `m` relied on `on`.
    fn rely(&mut self, on: Reliance, m: usize) {
        match &mut self.deferred {
            Some(deferred) => deferred.push((on, m)),
            None => self.relied(on, m),
        }
    }

    /// Records that method `m` relied on `on`, among the readers.
    pub(super) fn relied(&mut self, on: Reliance, m: usize) {
        let readers = match on {
            Reliance::Field(key) => self.field_readers.entry(key).or_default(),
            Reliance::Absence(t) => self.absence_readers.entry(t).or_default(),
            Reliance::Literal(r, t) => self.literal_readers.entry((r, t)).or_default(),
        };
        readers.insert(m);
    }
}

/// Whether the walk tracks the values of a place of type `ty`: one of an integer type or
/// `boolean`.
pub(super) fn tracked(ty: Type) -> bool {
    matches!(ty, Type::Int | Type::Long | Type::Short | Type::Byte | Type::Char | Type::Boolean)
}

impl Walk<'_> {
    /// The fields that hold captured locals, which constructors set from parameters that the
    /// translation adds: none of them is known.
    pub(super) fn captured_fields(&self) {
        let mut state = self.state.borrow_mut();
        for (t, decl) in self.class.types.iter().enumerate() {
            for c in &decl.captures {
                state.known.fields.insert((t, c.field.clone()), Known::Varying);
            }
        }
    }

    /// Method `target` is entered with `args`, evaluated in method `caller` (`None`: with
    /// arguments the walk does not see). When that changes what a parameter is known to hold,
    /// the method is walked again.
    pub(super) fn pass(&self, caller: usize, target: usize, args: Option<&[Expr]>) {
        self.pass_known(caller, target, args, &mut vec![None; args.map_or(0, <[Expr]>::len)]);
    }

    /// `pass`, with what is known of each argument once found, in `found`: a call's arguments
    /// are the same for each method that it runs.
    pub(super) fn pass_known(
        &self,
        caller: usize,
        target: usize,
        args: Option<&[Expr]>,
        found: &mut [Option<Known>],
    ) {
        let params = &self.class.methods[target].params;
        let args = args.filter(|args| args.len() == params.len());
        let mut changed = false;
        for (i, p) in params.iter().enumerate() {
            // What is known of an argument does not depend on what the parameters hold.
            let v = match args {
                Some(args) if tracked(p.ty) => {
                    *found[i].get_or_insert_with(|| Known::of(self.known(caller, &args[i])))
                }
                _ => Known::Varying,
            };
            let mut state = self.state.borrow_mut();
            let old = state.known.params.get(&(target, i)).copied();
            let new = Known::join(old, v);
            if old != Some(new) {
                state.known.params.insert((target, i), new);
                changed = true;
            }
        }
        if changed {
            self.rescan(target, Rescan::Parameter);
        }
    }

    /// What is known of the arguments `args` of a call in method `caller` at the positions
    /// `order`, in that order, as `pass_known` finds it, with what that relies on, which is not
    /// recorded among the readers (`Knowledge::relied`) but returned in the order found.
    pub(super) fn probe(
        &self,
        caller: usize,
        args: &[Expr],
        order: &[usize],
    ) -> (Vec<Option<Known>>, Vec<(Reliance, usize)>) {
        let mut found = vec![None; args.len()];
        self.state.borrow_mut().known.deferred = Some(Vec::new());
        for &i in order {
            found[i] = Some(Known::of(self.known(caller, &args[i])));
        }
        let relied = self.state.borrow_mut().known.deferred.take().unwrap_or_default();
        (found, relied)
    }

    /// The code of method `m` is given `value` for the field `name` of an object of type `ty`
    /// (`None`: a value the walk does not track).
    pub(super) fn write_field(&self, m: usize, ty: Type, name: &str, value: Option<&Expr>) {
        let known = match value {
            Some(v) => Known::of(self.known(m, v)),
            None => Known::Varying,
        };
        let Some(owner) = self.field_owner(ty, name) else {
            let mut state = self.state.borrow_mut();
            // What every method found that read a field of that name may change.
            if state.known.unplaced.insert(name.to_string()) {
                state.prepared.clear();
            }
            return;
        };
        self.give_field(owner, name, known);
    }

    /// The initializer `init` of the field `name` of class `owner` runs.
    pub(super) fn initialize_field(&self, owner: usize, name: &str, init: &Expr) {
        let known = Known::of(self.known(STATIC, init));
        self.give_field(owner, name, known);
    }

    fn give_field(&self, owner: usize, name: &str, known: Known) {
        let key = (owner, name.to_string());
        let readers = {
            let mut state = self.state.borrow_mut();
            // A field holds its default until it is first written.
            let old = Some(*state.known.fields.get(&key).unwrap_or(&Known::Value(0)));
            let new = Known::join(old, known);
            if old == Some(new) {
                return;
            }
            state.known.fields.insert(key.clone(), new);
            state.known.field_readers.get(&key).cloned().unwrap_or_default()
        };
        for m in readers {
            self.rescan(m, Rescan::Field);
        }
    }

    /// An object of class `c` is created: the methods that relied on there being none of a type
    /// above it are walked again, and those that relied on what `same_literal` gives for a type
    /// above it, where that changes.
    pub(super) fn created(&self, c: usize) {
        let mut readers = HashSet::default();
        let mut state = self.state.borrow_mut();
        for &t in &self.supers[c] {
            readers.extend(state.known.absence_readers.remove(&t).into_iter().flatten());
            let known = &mut state.known;
            for &r in known.literal_methods.get(&t).into_iter().flatten() {
                let literals = known.literals.get_mut(&(r, t)).expect("a type's literals are kept");
                let before = literals.value();
                *literals = literals.and(self.literal_of(r, c));
                if literals.value() != before {
                    readers.extend(known.literal_readers.get(&(r, t)).into_iter().flatten().copied());
                }
            }
        }
        drop(state);
        for m in readers {
            self.rescan(m, Rescan::Object);
        }
    }

    /// Method `m` is walked again, on every object that it was walked on.
    pub(super) fn rescan(&self, m: usize, why: Rescan) {
        let mut state = self.state.borrow_mut();
        state.prepared.remove(&m);
        let contexts = state.contexts.get(&m).cloned().unwrap_or_default();
        for ctx in contexts {
            if state.queued.insert((m, ctx)) {
                state.work.push((m, ctx));
                state.rescans[why as usize] += 1;
            }
        }
    }

    /// The class that declares the instance field `name` of an object of type `ty`.
    fn field_owner(&self, ty: Type, name: &str) -> Option<usize> {
        let mut at = self.class.class_index(ty);
        while let Some(t) = at {
            if self.class.types[t].fields.iter().any(|f| f.name == name) {
                return Some(t);
            }
            at = self.class.types[t].extends;
        }
        None
    }

    /// The parameters and locals of method `m` (see the module's comment).
    pub(super) fn env(&self, m: usize) -> Env {
        let method = &self.class.methods[m];
        let mut env = Env::default();
        {
            let state = self.state.borrow();
            for (i, p) in method.params.iter().enumerate() {
                let known = state.known.params.get(&(m, i)).copied().unwrap_or(Known::Varying);
                env.insert(p.name.clone(), if tracked(p.ty) { known } else { Known::Varying });
            }
        }
        // Each local starts with no value and gains those of what gives it one (an expression,
        // or a value that the walk does not know) until none changes.
        let env = RefCell::new(env);
        let changed = std::cell::Cell::new(true);
        let give = |name: &str, source: Option<&Expr>| {
            let known = match source {
                Some(e) => Known::of(self.known_in(m, e, &env.borrow())),
                None => Known::Varying,
            };
            let old = env.borrow().get(name).copied();
            let new = Known::join(old, known);
            if old != Some(new) {
                env.borrow_mut().insert(name.to_string(), new);
                changed.set(true);
            }
        };
        let note_expr = |x: &Expr| match &x.kind {
            ExprKind::Assign { op: None, target: Place::Var(name), value } => give(name, Some(value)),
            ExprKind::Assign { target: Place::Var(name), .. }
            | ExprKind::IncDec { target: Place::Var(name), .. }
            | ExprKind::InstanceOf { binding: Some(name), .. } => give(name, None),
            _ => {}
        };
        let note_stmt = |s: &Stmt| match s {
            Stmt::Local { ty, name, .. } if !tracked(*ty) => give(name, None),
            Stmt::Local { name, init: Some(init), .. } => give(name, Some(init)),
            Stmt::ForEach { name, .. } => give(name, None),
            Stmt::Try { catches, .. } => catches.iter().for_each(|c| give(&c.name, None)),
            _ => {}
        };
        while changed.replace(false) {
            for s in &method.body {
                visit_stmt(s, &mut |e| visit_expr(e, &mut |x| note_expr(x)), &mut |s| note_stmt(s));
            }
        }
        env.into_inner()
    }

    /// The expressions of method `m` that cannot run, by id: the side of each branch on a known
    /// condition that it does not take, and the right operand of an `&&` or `||` that the left
    /// one decides.
    pub(super) fn dead(&self, m: usize) -> HashSet<ExprId> {
        let dead = RefCell::new(HashSet::default());
        let mark_expr = |e: &Expr| {
            visit_expr(e, &mut |x| {
                dead.borrow_mut().insert(x.id);
            })
        };
        let mark_stmt = |s: &Stmt| visit_stmt(s, &mut |e| mark_expr(e), &mut |_| {});
        let known = |e: &Expr| self.known(m, e);
        for s in &self.class.methods[m].body {
            visit_stmt(
                s,
                &mut |e| {
                    visit_expr(e, &mut |x| match &x.kind {
                        ExprKind::Binary(BinOp::And, l, r) if known(l) == Some(0) => mark_expr(r),
                        ExprKind::Binary(BinOp::Or, l, r) if known(l).is_some_and(|v| v != 0) => mark_expr(r),
                        ExprKind::Conditional(c, a, b) => match known(c) {
                            Some(0) => mark_expr(a),
                            Some(_) => mark_expr(b),
                            None => {}
                        },
                        _ => {}
                    })
                },
                &mut |s| match s {
                    Stmt::If { cond, then, otherwise } => match known(cond) {
                        Some(0) => mark_stmt(then),
                        Some(_) => otherwise.iter().for_each(|o| mark_stmt(o)),
                        None => {}
                    },
                    Stmt::While { cond, body } if known(cond) == Some(0) => mark_stmt(body),
                    Stmt::For { cond: Some(cond), update, body, .. } if known(cond) == Some(0) => {
                        mark_stmt(body);
                        update.iter().for_each(&mark_expr);
                    }
                    _ => {}
                },
            );
        }
        dead.into_inner()
    }

    /// The value of `e` in method `m` (`STATIC`: in an initializer), when it is known.
    pub(super) fn known(&self, m: usize, e: &Expr) -> Option<i128> {
        if m == STATIC {
            return self.known_in(STATIC, e, &Env::default());
        }
        let env = std::mem::take(&mut self.state.borrow_mut().known.env);
        let value = self.known_in(m, e, &env);
        self.state.borrow_mut().known.env = env;
        value
    }

    /// `known` with the parameters and locals `env`. Code in an initializer (`m` is `STATIC`)
    /// is walked once, so its values are only those that never change: literals and constants.
    fn known_in(&self, m: usize, e: &Expr, env: &Env) -> Option<i128> {
        let class = self.class;
        let int = |e: &Expr| self.known_in(m, e, env);
        let v = match &e.kind {
            ExprKind::Int(v) => i128::from(*v),
            ExprKind::Long(v) => i128::from(*v),
            ExprKind::Char(v) => i128::from(*v),
            ExprKind::Bool(b) => i128::from(*b),
            ExprKind::Var(name) => env.get(name.as_str())?.value()?,
            ExprKind::Static { class: owner, name } => {
                let field = class.types.get(*owner)?.statics.iter().find(|f| f.name == *name)?;
                if !field.constant || !tracked(field.ty) {
                    return None;
                }
                match field.init.as_ref()?.kind {
                    ExprKind::Int(v) => i128::from(v),
                    ExprKind::Long(v) => i128::from(v),
                    ExprKind::Char(v) => i128::from(v),
                    ExprKind::Bool(b) => i128::from(b),
                    _ => return None,
                }
            }
            ExprKind::Field { object, name } if m != STATIC && tracked(e.ty) => {
                let owner = self.field_owner(object.ty, name)?;
                let key = (owner, name.clone());
                let mut state = self.state.borrow_mut();
                if state.known.unplaced.contains(name) {
                    return None;
                }
                let known = *state.known.fields.get(&key).unwrap_or(&Known::Value(0));
                state.known.rely(Reliance::Field(key), m);
                known.value()?
            }
            ExprKind::InstanceOf { ty, .. } if m != STATIC => {
                let t = class.class_index(*ty)?;
                if !self.program_only(t) {
                    return None;
                }
                let mut state = self.state.borrow_mut();
                // The types above a class with objects (`LiveMethods::constructed`).
                if state.live.constructed.contains(&t) {
                    return None;
                }
                state.known.rely(Reliance::Absence(t), m);
                0
            }
            // A method that nothing overrides and that only returns a literal.
            ExprKind::MethodCall { resolved: Some(r), .. } | ExprKind::Call { resolved: Some(r), .. }
                if !class.hierarchy.is_overridden(*r) =>
            {
                return self.literal_result(*r);
            }
            // A call without arguments whose implementations, in the classes below the receiver's
            // type that have objects, all return the same literal, as every `ByteBuffer`'s
            // `isDirect()` where no direct buffer is created.
            ExprKind::MethodCall { resolved: Some(r), receiver, is_super: false, args, .. }
                if m != STATIC
                    && args.is_empty()
                    && !class.methods[*r].is_static
                    && self.literal_group(*r) =>
            {
                let t = class.class_index(receiver.ty)?;
                self.state.borrow_mut().known.rely(Reliance::Literal(*r, t), m);
                self.same_literal(*r, t)?
            }
            ExprKind::Assign { op: None, target: Place::Var(_), value } => return int(value),
            ExprKind::Cast(to, x) if tracked(*to) && tracked(x.ty) => {
                let v = int(x)?;
                return in_range(*to, v).then_some(v);
            }
            ExprKind::Unary(UnOp::Not, x) => i128::from(int(x)? == 0),
            ExprKind::Conditional(c, a, b) => return if int(c)? != 0 { int(a) } else { int(b) },
            // Either operand that is false decides an `&&`, and either that is true an `||`.
            ExprKind::Binary(BinOp::And, l, r) => match (int(l), int(r)) {
                (Some(0), _) | (_, Some(0)) => 0,
                (Some(_), Some(_)) => 1,
                _ => return None,
            },
            ExprKind::Binary(BinOp::Or, l, r) => match (int(l), int(r)) {
                (Some(a), _) | (_, Some(a)) if a != 0 => 1,
                (Some(_), Some(_)) => 0,
                _ => return None,
            },
            ExprKind::Binary(op, l, r) => {
                let (a, b) = (int(l)?, int(r)?);
                match op {
                    BinOp::Add => a + b,
                    BinOp::Sub => a - b,
                    BinOp::BitAnd => a & b,
                    BinOp::BitOr => a | b,
                    BinOp::BitXor => a ^ b,
                    BinOp::Eq => i128::from(a == b),
                    BinOp::Ne => i128::from(a != b),
                    BinOp::Lt => i128::from(a < b),
                    BinOp::Le => i128::from(a <= b),
                    BinOp::Gt => i128::from(a > b),
                    BinOp::Ge => i128::from(a >= b),
                    _ => return None,
                }
            }
            _ => return None,
        };
        // Java's arithmetic wraps; within its type's range the value is exact.
        in_range(e.ty, v).then_some(v)
    }

    /// The literal that every implementation of method `r` returns in the classes below type `t`
    /// that have objects, when there are some and they all return the same one. Found once, and
    /// then kept up as objects of other classes below `t` are created (`Knowledge::literals`):
    /// a walk that skips nothing finds it again each time.
    fn same_literal(&self, r: usize, t: usize) -> Option<i128> {
        let kept = self.state.borrow().known.literals.get(&(r, t)).copied();
        if let Some(literals) = kept
            && self.fast
        {
            return literals.value();
        }
        let concretes = self.concretes(t);
        let literals = {
            let state = self.state.borrow();
            let created = concretes.iter().filter(|c| state.live.instantiated.contains(c));
            created.fold(Literals::None, |literals, &c| literals.and(self.literal_of(r, c)))
        };
        if kept.is_none() {
            let known = &mut self.state.borrow_mut().known;
            known.literals.insert((r, t), literals);
            known.literal_methods.entry(t).or_default().push(r);
        }
        literals.value()
    }

    /// The literal that the implementation of method `r` that objects of class `c` run returns,
    /// when it only returns one.
    fn literal_of(&self, r: usize, c: usize) -> Option<i128> {
        // `literal_result` of a literal does not look at the state.
        self.literal_result(self.class.hierarchy.implementation(r, c)?)
    }

    /// The literal that method `r` returns, when its body only returns one.
    fn literal_result(&self, r: usize) -> Option<i128> {
        let [Stmt::Return(Some(value), _)] = &self.class.methods[r].body[..] else { return None };
        if !matches!(value.kind, ExprKind::Int(_) | ExprKind::Long(_) | ExprKind::Bool(_)) {
            return None;
        }
        self.known_in(STATIC, value, &Env::default())
    }

    /// Whether some method of the group of method `r` only returns a literal.
    fn literal_group(&self, r: usize) -> bool {
        if let Some(&some) = self.state.borrow().known.literal_groups.get(&r) {
            return some;
        }
        let some = self.class.hierarchy.group(r).into_iter().any(|g| self.literal_result(g).is_some());
        self.state.borrow_mut().known.literal_groups.insert(r, some);
        some
    }

    /// Whether no value of the runtime can be of type `t`, and every type below it is a class:
    /// then `instanceof t` is true only of objects that some `new` created.
    fn program_only(&self, t: usize) -> bool {
        if let Some(&only) = self.state.borrow().known.program_only.get(&t) {
            return only;
        }
        let class = self.class;
        let only = class.types[t].name != "java.lang.AbstractStringBuilder"
            && (0..class.types.len())
                .filter(|&s| self.supers[s].contains(&t))
                .all(|s| class.types[s].kind == TypeKind::Class);
        self.state.borrow_mut().known.program_only.insert(t, only);
        only
    }
}

/// Whether `v` is a value of type `ty`.
fn in_range(ty: Type, v: i128) -> bool {
    let (low, high): (i128, i128) = match ty {
        Type::Int => (i32::MIN.into(), i32::MAX.into()),
        Type::Long => (i64::MIN.into(), i64::MAX.into()),
        Type::Short => (i16::MIN.into(), i16::MAX.into()),
        Type::Byte => (i8::MIN.into(), i8::MAX.into()),
        Type::Char => (0, u16::MAX.into()),
        Type::Boolean => (0, 1),
        _ => return false,
    };
    (low..=high).contains(&v)
}
