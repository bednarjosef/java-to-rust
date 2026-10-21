//! Functional interfaces that are closure types (`analysis::closures`): the interface is a type
//! alias, `type Op = Rc<dyn Fn(i32, i32) -> i32>;`, a lambda or method reference converted to it
//! is a closure, and a call of the interface's method calls the closure.
//!
//! The closure's body is the lambda class's method, written where the lambda is created with the
//! method's own names and facts, and its captures are bound before it: the fields that held them
//! in the lambda's object are locals that the closure moves in (`field_locals`), so that
//! `this.k` in the method is `k` and `this.outer` is `outer`.
//!
//! ```text
//! { let k = k; Rc::new(move |x: i32| -> i32 { x + k }) as IntOp }
//! ```

use super::*;

/// The method-level state of the generator, set aside while a closure's body is written inside
/// another method.
struct Saved {
    method: usize,
    ret: Type,
    throwing: bool,
    this_struct: usize,
    this_form: ThisForm,
    display: bool,
    borrowed: HashSet<String>,
    borrowed_optional: HashSet<String>,
    int_receiver: bool,
    double_receiver: bool,
    locals: HashMap<String, String>,
    boxed: HashMap<String, Type>,
    field_locals: HashMap<String, String>,
    ctor_result: Option<&'static str>,
    attempts: Vec<exceptions::Attempt>,
    finallies: Vec<exceptions::Finally>,
    attempt_count: usize,
    try_assigned: HashSet<String>,
    annotations: Vec<(String, Type, String)>,
    elements: HashMap<ExprId, String>,
    sliced: HashMap<String, (String, usize)>,
    sliced_accesses: HashSet<ExprId>,
    out: String,
    indent: usize,
    guarded: bool,
    chain: bool,
    formatting: bool,
    printing: bool,
    lazy_text: bool,
    chars: bool,
    exhaustive: bool,
}

impl<'a> Gen<'a> {
    /// Whether values of class type `ty` are closures.
    pub(super) fn closure_type(&self, ty: Type) -> Option<usize> {
        self.class.class_index(ty).filter(|t| self.facts.closure_types.contains(t))
    }

    /// The method of closure type `t` that its values run.
    fn closure_method(&self, t: usize) -> usize {
        self.class.methods.iter().position(|m| m.class == t && !m.is_static).expect("a closure type's method")
    }

    /// `type Op = Rc<dyn Fn(i32, i32) -> i32>;`: the parameters and the result of the interface's
    /// method as the dispatcher would take them.
    pub(super) fn closure_alias(&mut self, t: usize) {
        let m = self.closure_method(t);
        self.enter_method(m);
        let params: Vec<String> = self
            .typed_params(m, false)
            .iter()
            .map(|p| p.split_once(": ").map_or(p.as_str(), |(_, ty)| ty).to_string())
            .collect();
        let ret = self.ret_text(m);
        let name = struct_name(&self.class.types[t].name);
        self.gap();
        self.line(&format!("{}type {name} = Rc<dyn Fn({}){ret}>;", self.vis(), params.join(", ")));
    }

    /// `f.apply(x)` on a closure: `f(x)`. A closure read from a field is taken out of its object
    /// before the call, which may reach the object again.
    pub(super) fn closure_call(&mut self, receiver: &'a Expr, index: usize, args: &'a [Expr]) -> Code {
        let callee = match &receiver.kind {
            ExprKind::Var(_) => self.object(receiver),
            ExprKind::Field { object, name }
                if matches!(object.kind, ExprKind::This) && self.field_locals.contains_key(name) =>
            {
                self.object(receiver)
            }
            // A field of a plain object, which no `RefCell` holds.
            ExprKind::Field { object, .. } if self.is_plain(object) && !self.nullable(receiver) => {
                format!("({})", self.object(receiver))
            }
            // The borrow of the object that holds the field ends before the call.
            ExprKind::Field { .. } => {
                let value = self.object_handle(receiver);
                let temp = self.temp("f");
                format!("({{ let {temp} = {value}; {temp} }})")
            }
            _ => self.expr(receiver).receiver(),
        };
        let args = self.call_args(index, args).join(", ");
        Code::atom(format!("{callee}({args})"))
    }

    /// A lambda converted to its closure type `t`: its captures bound to locals, and the closure
    /// that moves them in, whose body is the lambda class's method.
    pub(super) fn closure(&mut self, t: usize, ctor: usize, args: &'a [Expr]) -> Code {
        let c = self.class.methods[ctor].class;
        let Some(m) = self.class.hierarchy.implementation(self.closure_method(t), c) else {
            unreachable!("a closure type's lambda implements its method")
        };
        // The values the constructor stores, by field, in the creating method's terms; the
        // closure owns them, where the constructor would borrow a string or an object.
        let mut values = self.call_args(ctor, args);
        for (i, arg) in args.iter().enumerate() {
            let key = (ctor, i);
            let f = self.facts;
            if f.borrowed_params.contains(&key)
                || f.borrowed_objects.contains(&key)
                || f.borrowed_optional_params.contains(&key)
                || f.borrowed_arrays.contains_key(&key)
            {
                values[i] = self.value(arg, self.class.methods[ctor].params[i].ty).text;
            }
        }
        let fields = captured_fields(&self.class.methods[ctor]);
        let saved = self.set_aside();
        self.enter_method(m);
        self.this_struct = c;
        self.this_form = ThisForm::SelfRef;
        let mut used: HashSet<String> = self.locals.values().cloned().collect();
        used.insert("java".into());
        let mut prelude = Vec::new();
        let mut moving = false;
        for (field, value) in fields.iter().zip(&values) {
            let Some(field) = field else { continue };
            let name = unique(snake_case(field), &mut used);
            // A local passed as it is is moved in by its own name.
            if *value != name {
                prelude.push(format!("let {name} = {value};"));
            }
            moving = true;
            self.field_locals.insert(field.clone(), name);
        }
        let params = self.typed_params(m, true);
        let ret = self.ret_text(m);
        self.indent = saved.indent + 1;
        let start = self.out.len();
        let body = &self.class.methods[m].body;
        self.stmts(body, true);
        if self.throwing && self.class.methods[m].ret == Type::Void && exceptions::completes_all(body) {
            self.line("Ok(())");
        }
        self.resolve_annotations(start);
        let text = std::mem::take(&mut self.out);
        self.restore(saved);
        let pad = "    ".repeat(self.indent);
        let moving = if moving { "move " } else { "" };
        // A body of one line stays on the line of the closure.
        let body = match text.trim_end().lines().collect::<Vec<_>>()[..] {
            [line] => format!("{{ {} }}", line.trim()),
            _ => format!("{{\n{text}{pad}}}"),
        };
        let closure =
            format!("Rc::new({moving}|{}|{ret} {body}) as {}", params.join(", "), self.closure_name(t));
        if prelude.is_empty() {
            return Code::new(closure, AS);
        }
        Code::atom(format!("{{ {} {closure} }}", prelude.join(" ")))
    }

    /// The Rust name of closure type `t`, which its values are written as.
    fn closure_name(&self, t: usize) -> String {
        self.object_type(&self.class.types[t].name)
    }

    fn set_aside(&mut self) -> Saved {
        Saved {
            method: self.method,
            ret: self.ret,
            throwing: self.throwing,
            this_struct: self.this_struct,
            this_form: self.this_form,
            display: std::mem::replace(&mut self.display, false),
            borrowed: std::mem::take(&mut self.borrowed),
            borrowed_optional: std::mem::take(&mut self.borrowed_optional),
            int_receiver: self.int_receiver,
            double_receiver: self.double_receiver,
            locals: std::mem::take(&mut self.locals),
            boxed: std::mem::take(&mut self.boxed),
            field_locals: std::mem::take(&mut self.field_locals),
            ctor_result: self.ctor_result.take(),
            attempts: std::mem::take(&mut self.attempts),
            finallies: std::mem::take(&mut self.finallies),
            attempt_count: self.attempt_count,
            try_assigned: std::mem::take(&mut self.try_assigned),
            annotations: std::mem::take(&mut self.annotations),
            elements: std::mem::take(&mut self.elements),
            sliced: std::mem::take(&mut self.sliced),
            sliced_accesses: std::mem::take(&mut self.sliced_accesses),
            out: std::mem::take(&mut self.out),
            indent: self.indent,
            guarded: std::mem::replace(&mut self.guarded, false),
            chain: std::mem::replace(&mut self.chain, false),
            formatting: std::mem::replace(&mut self.formatting, false),
            printing: std::mem::replace(&mut self.printing, false),
            lazy_text: std::mem::replace(&mut self.lazy_text, false),
            chars: std::mem::replace(&mut self.chars, false),
            exhaustive: std::mem::replace(&mut self.exhaustive, false),
        }
    }

    fn restore(&mut self, s: Saved) {
        self.method = s.method;
        self.ret = s.ret;
        self.throwing = s.throwing;
        self.this_struct = s.this_struct;
        self.this_form = s.this_form;
        self.display = s.display;
        self.borrowed = s.borrowed;
        self.borrowed_optional = s.borrowed_optional;
        self.int_receiver = s.int_receiver;
        self.double_receiver = s.double_receiver;
        self.locals = s.locals;
        self.boxed = s.boxed;
        self.field_locals = s.field_locals;
        self.ctor_result = s.ctor_result;
        self.attempts = s.attempts;
        self.finallies = s.finallies;
        self.attempt_count = s.attempt_count;
        self.try_assigned = s.try_assigned;
        self.annotations = s.annotations;
        self.elements = s.elements;
        self.sliced = s.sliced;
        self.sliced_accesses = s.sliced_accesses;
        self.out = s.out;
        self.indent = s.indent;
        self.guarded = s.guarded;
        self.chain = s.chain;
        self.formatting = s.formatting;
        self.printing = s.printing;
        self.lazy_text = s.lazy_text;
        self.chars = s.chars;
        self.exhaustive = s.exhaustive;
    }
}

/// The field that each parameter of a lambda class's constructor is stored in, by parameter.
fn captured_fields(ctor: &Method) -> Vec<Option<String>> {
    ctor.params
        .iter()
        .map(|p| {
            ctor.body.iter().find_map(|s| match s {
                Stmt::Expr(Expr {
                    kind: ExprKind::Assign { op: None, target: Place::Field { object, name }, value },
                    ..
                }) if matches!(object.kind, ExprKind::This)
                    && matches!(&value.kind, ExprKind::Var(v) if *v == p.name) =>
                {
                    Some(name.clone())
                }
                _ => None,
            })
        })
        .collect()
}
