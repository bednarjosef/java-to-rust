//! The second constructor form, for the class trees where the first cannot be right.
//!
//! A constructor of a class with subclasses normally builds the class's fields struct as a plain
//! value, which its subclass's constructor embeds as `base` (see `docs/object-model.md` 1.4). The
//! superclass's constructor then runs before the object exists, so it cannot use `this` as a
//! value (store it, pass it, compare it) or call a method that a subclass may override. When a
//! constructor of such a class does, its whole class tree is rewritten so that Java's order holds
//! on the finished object: the object is allocated first, with every field zero, null or false,
//! then the superclass's constructor body runs on it, then the subclass's.
//!
//! - Each constructor of a class with subclasses in the tree becomes an instance method,
//!   `construct<Name>`, whose `super(..)` or `this(..)` becomes a call of the matching method on
//!   `this`, followed (unless it calls `this(..)`) by the class's field initializers. The class
//!   gets one constructor with no parameters, which builds the fields struct with every field
//!   zero, null or false and whose `super()` is that of its superclass.
//! - A constructor of a class without subclasses that extends one of them starts with that
//!   `super()`, sets the enclosing instance and captured locals (which javac sets before
//!   `super(..)` too), calls the superclass's `construct<Name>` with the arguments of its
//!   `super(..)`, runs its own field initializers, then its body.
//! - `new C(..)` of a class with subclasses in the tree calls a static method `new<Name>`, which
//!   builds the zero object, calls its `construct<Name>` and returns it.
//!
//! The field initializers of the classes rewritten move into their constructors (into a method
//! `initializeFields` when the class has several constructors that run them), after the
//! superclass's part: an overridable method that the superclass's constructor calls sees them
//! unset, as in Java.
//!
//! Every other class keeps the first form, so a program whose classes all build plain fields
//! structs is translated as before.

use super::whole_object_classes;
use crate::ast::*;
use std::collections::{BTreeSet, HashMap, HashSet};

pub fn whole_objects(class: &mut Class) -> Result<(), Error> {
    let first = whole_object_classes(class);
    if first.is_empty() {
        return Ok(());
    }
    let h = &class.hierarchy;
    let fields_structs = super::hierarchy::fields_structs(class);
    // The classes with subclasses above and below those, whose constructors call each other.
    let mut whole: BTreeSet<usize> = BTreeSet::new();
    for &t in &first {
        whole.insert(t);
        whole.extend(h.ancestors(t).into_iter().filter(|a| fields_structs.contains(a)));
    }
    let mut work: Vec<usize> = whole.iter().copied().collect();
    while let Some(t) = work.pop() {
        for &s in &h.subtypes[t] {
            if fields_structs.contains(&s) && whole.insert(s) {
                work.push(s);
            }
        }
    }
    for &t in &whole {
        let decl = &class.types[t];
        if decl.outer.is_some() || !decl.captures.is_empty() {
            let pos =
                class.methods.iter().find(|m| m.is_ctor && m.class == t).map_or(Pos::default(), |m| m.pos);
            return Err(Error {
                pos,
                message: "constructors of a class with subclasses that use `this` as a value or call overridable \
                          methods, in a class with an enclosing instance or captured locals, not supported yet"
                    .to_string(),
            });
        }
    }
    // The classes without subclasses whose superclass is rewritten.
    let leaves: BTreeSet<usize> = (0..class.types.len())
        .filter(|t| !whole.contains(t) && class.types[*t].extends.is_some_and(|e| whole.contains(&e)))
        .collect();
    let mut rewrite = Rewrite { next_id: next_id(class), zero: HashMap::new(), construct: HashMap::new() };
    let original: Vec<usize> = (0..class.methods.len())
        .filter(|&m| class.methods[m].is_ctor && whole.contains(&class.methods[m].class))
        .collect();
    for &m in &original {
        rewrite.construct.insert(m, construct_name(class, class.methods[m].class));
    }
    // The constructors with no parameters that build the zero objects, superclasses first.
    let mut order: Vec<usize> = whole.iter().copied().collect();
    order.sort_by_key(|&t| class.hierarchy.ancestors(t).len());
    for &t in &order {
        let zero = class.methods.len();
        rewrite.zero.insert(t, zero);
        let pos = class.methods.iter().find(|m| m.is_ctor && m.class == t).map_or(Pos::default(), |m| m.pos);
        let body = match class.types[t].extends.and_then(|e| rewrite.zero.get(&e)) {
            Some(&above) => {
                let kind = ExprKind::SuperInit { args: Vec::new(), resolved: Some(above) };
                vec![Stmt::Expr(rewrite.expr(kind, pos, Type::Void))]
            }
            None => Vec::new(),
        };
        class.methods.push(Method {
            name: "<init>".to_string(),
            params: Vec::new(),
            ret: Type::Void,
            body,
            is_main: false,
            is_static: false,
            is_ctor: true,
            class: t,
            pos,
            is_abstract: false,
            is_synchronized: false,
            overrides: Vec::new(),
            type_params: Vec::new(),
        });
    }
    // The constructors of the classes with subclasses become `construct<Name>` methods, and those
    // of their subclasses without subclasses start with the zero `super()` and call them.
    for &t in whole.iter().chain(&leaves) {
        let ctors: Vec<usize> = (0..class.methods.len())
            .filter(|&m| {
                class.methods[m].is_ctor && class.methods[m].class == t && rewrite.zero.get(&t) != Some(&m)
            })
            .collect();
        let starting: Vec<usize> = ctors.iter().copied().filter(|&m| !delegates(&class.methods[m])).collect();
        let inits = rewrite.field_inits(class, t);
        // When several constructors run the initializers, they call a method that does.
        let shared = (!inits.is_empty() && starting.len() > 1).then(|| {
            class.methods.push(Method {
                name: "initializeFields".to_string(),
                params: Vec::new(),
                ret: Type::Void,
                body: Vec::new(),
                is_main: false,
                is_static: false,
                is_ctor: false,
                class: t,
                pos: class.methods[starting[0]].pos,
                is_abstract: false,
                is_synchronized: false,
                overrides: Vec::new(),
                type_params: Vec::new(),
            });
            class.methods.len() - 1
        });
        let mut inline = Some(inits);
        if let Some(f) = shared {
            class.methods[f].body = inline.take().unwrap_or_default();
        }
        for &m in &ctors {
            let own = match shared {
                _ if !starting.contains(&m) => Vec::new(),
                Some(f) => {
                    let pos = class.methods[m].pos;
                    let receiver = Box::new(rewrite.this(class, t, pos));
                    let name = class.methods[f].name.clone();
                    let kind = ExprKind::MethodCall {
                        receiver,
                        name,
                        args: Vec::new(),
                        resolved: Some(f),
                        is_super: false,
                    };
                    vec![Stmt::Expr(rewrite.expr(kind, pos, Type::Void))]
                }
                None => inline.take().unwrap_or_default(),
            };
            if whole.contains(&t) {
                rewrite.construct(class, m, own);
            } else {
                rewrite.leaf(class, m, own);
            }
        }
    }
    // `new C(..)` of a class with subclasses calls `new<Name>`.
    let mut factories = HashMap::new();
    for &m in &original {
        let t = class.methods[m].class;
        if !class.hierarchy.is_concrete(t) {
            continue;
        }
        let pos = class.methods[m].pos;
        let ty = self_type(class, t);
        let zero = rewrite.zero[&t];
        let new = rewrite.expr(ExprKind::New { class: t, args: Vec::new(), ctor: Some(zero) }, pos, ty);
        let obj = || "obj".to_string();
        let params: Vec<Param> =
            class.methods[m].params.iter().map(|p| Param { name: p.name.clone(), ty: p.ty }).collect();
        let args = params.iter().map(|p| rewrite.expr(ExprKind::Var(p.name.clone()), pos, p.ty)).collect();
        let receiver = Box::new(rewrite.expr(ExprKind::Var(obj()), pos, ty));
        let call = ExprKind::MethodCall {
            receiver,
            name: class.methods[m].name.clone(),
            args,
            resolved: Some(m),
            is_super: false,
        };
        let body = vec![
            Stmt::Local { ty, name: obj(), init: Some(new), pos },
            Stmt::Expr(rewrite.expr(call, pos, Type::Void)),
            Stmt::Return(Some(rewrite.expr(ExprKind::Var(obj()), pos, ty)), pos),
        ];
        let mut type_params = class.types[t].type_params.clone();
        type_params.extend(class.methods[m].type_params.iter().copied());
        let name = format!("new{}", simple_name(&class.types[t].name));
        factories.insert(m, (class.methods.len(), name.clone()));
        class.methods.push(Method {
            name,
            params,
            ret: ty,
            body,
            is_main: false,
            is_static: true,
            is_ctor: false,
            class: t,
            pos,
            is_abstract: false,
            is_synchronized: false,
            overrides: Vec::new(),
            type_params,
        });
    }
    let mut to_factory = |e: &mut Expr| {
        let ExprKind::New { ctor: Some(ctor), args, .. } = &mut e.kind else { return };
        let Some(&(factory, ref name)) = factories.get(ctor) else { return };
        let args = std::mem::take(args);
        e.kind = ExprKind::Call { name: name.clone(), args, resolved: Some(factory) };
    };
    for method in &mut class.methods {
        method.body.iter_mut().for_each(|s| rewrite_stmt(s, &mut to_factory));
    }
    for decl in &mut class.types {
        for f in decl.fields.iter_mut().chain(decl.statics.iter_mut()) {
            if let Some(init) = &mut f.init {
                rewrite_expr(init, &mut to_factory);
            }
        }
    }
    class.raised.iter_mut().for_each(|e| rewrite_expr(e, &mut to_factory));
    class.hierarchy = Hierarchy::new(&class.types, &class.methods);
    Ok(())
}

/// Whether a constructor calls another constructor of its class.
fn delegates(m: &Method) -> bool {
    crate::ast::delegates(&m.body)
}

/// The name of a class without its package and enclosing classes: `Head` for
/// `java.util.stream.ReferencePipeline$Head`.
fn simple_name(name: &str) -> &str {
    name.rsplit(['.', '$']).next().unwrap_or(name)
}

fn construct_name(class: &Class, t: usize) -> String {
    format!("construct{}", simple_name(&class.types[t].name))
}

/// The type of `this` in class `t`.
fn self_type(class: &Class, t: usize) -> Type {
    let decl = &class.types[t];
    Type::generic(&decl.name, &decl.type_params)
}

/// The first expression id that no expression of the class has.
fn next_id(class: &Class) -> ExprId {
    let mut max = 0;
    let mut see = |e: &Expr| max = max.max(e.id);
    for m in &class.methods {
        for s in &m.body {
            visit_stmt(s, &mut |e| visit_expr(e, &mut see), &mut |_| {});
        }
    }
    for decl in &class.types {
        for f in decl.fields.iter().chain(&decl.statics) {
            if let Some(init) = &f.init {
                visit_expr(init, &mut see);
            }
        }
        for c in decl.enum_constants.iter().flatten() {
            c.values.iter().for_each(|v| visit_expr(v, &mut see));
        }
    }
    class.raised.iter().for_each(|e| visit_expr(e, &mut see));
    max + 1
}

struct Rewrite {
    next_id: ExprId,
    /// The constructor with no parameters of each class with subclasses that is rewritten.
    zero: HashMap<usize, usize>,
    /// The name of the `construct<Name>` method each constructor becomes.
    construct: HashMap<usize, String>,
}

impl Rewrite {
    fn expr(&mut self, kind: ExprKind, pos: Pos, ty: Type) -> Expr {
        let id = self.next_id;
        self.next_id += 1;
        Expr { kind, pos, ty, id }
    }

    fn this(&mut self, class: &Class, t: usize, pos: Pos) -> Expr {
        self.expr(ExprKind::This, pos, self_type(class, t))
    }

    /// Takes the field initializers of class `t` out of its fields, as assignments to `this`.
    fn field_inits(&mut self, class: &mut Class, t: usize) -> Vec<Stmt> {
        let ty = self_type(class, t);
        let mut out = Vec::new();
        for i in 0..class.types[t].fields.len() {
            let Some(value) = class.types[t].fields[i].init.take() else { continue };
            let f = &class.types[t].fields[i];
            let (name, fty, pos) = (f.name.clone(), f.ty, f.pos);
            let object = Box::new(self.expr(ExprKind::This, pos, ty));
            let kind =
                ExprKind::Assign { op: None, target: Place::Field { object, name }, value: Box::new(value) };
            out.push(Stmt::Expr(self.expr(kind, pos, fty)));
        }
        out
    }

    /// A call of the `construct<Name>` method that constructor `ctor` became, on `this` of class
    /// `t`.
    fn construct_call(&mut self, class: &Class, t: usize, ctor: usize, args: Vec<Expr>, pos: Pos) -> Stmt {
        let receiver = Box::new(self.this(class, t, pos));
        let name = self.construct[&ctor].clone();
        let kind = ExprKind::MethodCall { receiver, name, args, resolved: Some(ctor), is_super: false };
        Stmt::Expr(self.expr(kind, pos, Type::Void))
    }

    /// Turns constructor `m` of a class with subclasses into its `construct<Name>` method, which
    /// runs `inits` after the superclass's part.
    fn construct(&mut self, class: &mut Class, m: usize, inits: Vec<Stmt>) {
        let t = class.methods[m].class;
        let mut body = std::mem::take(&mut class.methods[m].body);
        // The statements before `this(..)` or `super(..)` run first, on the zero object, and so
        // do the stores of the fields that they assign (see `lower::prologues`), before the
        // superclass's part, which may call a method that reads them.
        let mut start: Vec<Stmt> = body.drain(..explicit_call(&body).unwrap_or(0)).collect();
        let mut call = Vec::new();
        let rest = match body.first_mut() {
            Some(Stmt::Expr(Expr { kind: ExprKind::ThisCall { args, resolved: Some(r) }, pos, .. }))
            | Some(Stmt::Expr(Expr { kind: ExprKind::SuperInit { args, resolved: Some(r) }, pos, .. }))
                if self.construct.contains_key(r) =>
            {
                let (r, pos, args) = (*r, *pos, std::mem::take(args));
                call.push(self.construct_call(class, t, r, args, pos));
                1
            }
            Some(Stmt::Expr(Expr { kind: ExprKind::SuperInit { args, .. }, .. })) => {
                // A superclass without a fields struct, whose constructor does nothing: only the
                // arguments' effects remain.
                call.extend(std::mem::take(args).into_iter().filter(has_effects).map(Stmt::Expr));
                1
            }
            _ => 0,
        };
        let early = body[rest..].iter().take_while(|s| is_early_field_assignment(s)).count();
        start.extend(body.drain(rest..rest + early));
        start.extend(call);
        start.extend(inits);
        start.extend(body.drain(rest..));
        let method = &mut class.methods[m];
        method.body = start;
        method.is_ctor = false;
        method.name = self.construct[&m].clone();
    }

    /// Starts constructor `m` of a class without subclasses whose superclass is rewritten with
    /// the zero `super()`, then sets the enclosing instance and captured locals, calls the
    /// superclass's `construct<Name>` and runs `inits`.
    fn leaf(&mut self, class: &mut Class, m: usize, inits: Vec<Stmt>) {
        let t = class.methods[m].class;
        let zero = class.types[t].extends.and_then(|e| self.zero.get(&e)).copied();
        let at = explicit_call(&class.methods[m].body);
        let start = match (at.map(|i| &mut class.methods[m].body[i]), zero) {
            (
                Some(Stmt::Expr(Expr { kind: ExprKind::SuperInit { args, resolved: Some(r) }, pos, .. })),
                Some(zero),
            ) => Some((*r, *pos, std::mem::take(args), zero)),
            _ => None,
        };
        let Some((r, pos, args, zero)) = start else {
            let at = at.unwrap_or(0);
            class.methods[m].body.splice(at..at, inits);
            return;
        };
        let synthetic: HashSet<&str> = class.types[t]
            .outer
            .map(|_| "outer")
            .into_iter()
            .chain(class.types[t].captures.iter().map(|c| c.field.as_str()))
            .collect();
        let mut body = std::mem::take(&mut class.methods[m].body);
        // The statements before `super(..)` (see `construct`) run on the zero object.
        let prologue: Vec<Stmt> = body.drain(..at.unwrap_or(0)).collect();
        let first = body.remove(0);
        let mut out = vec![first];
        out.extend(prologue);
        if let Stmt::Expr(Expr { kind: ExprKind::SuperInit { resolved, .. }, .. }) = &mut out[0] {
            *resolved = Some(zero);
        }
        let leading = body
            .iter()
            .take_while(|s| {
                is_early_field_assignment(s)
                    || matches!(s, Stmt::Expr(Expr {
                        kind: ExprKind::Assign { op: None, target: Place::Field { object, name }, .. },
                        ..
                    }) if matches!(object.kind, ExprKind::This) && synthetic.contains(name.as_str()))
            })
            .count();
        out.extend(body.drain(..leading));
        out.push(self.construct_call(class, t, r, args, pos));
        out.extend(inits);
        out.extend(body);
        class.methods[m].body = out;
    }
}
