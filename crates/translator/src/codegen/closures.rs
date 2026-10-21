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
use crate::analysis::closures;

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
    bound_params: Option<usize>,
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
        closures::closure_method(self.class, t)
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
        // An interface that extends another has its closures: `type UnaryOperator = Function;`.
        let above =
            self.class.types[t].implements.iter().copied().find(|i| self.facts.closure_types.contains(i));
        let ty = match above {
            Some(above) => {
                self.need(Need::Enum(above));
                self.closure_name(above)
            }
            None => format!("Rc<dyn Fn({}){ret}>", params.join(", ")),
        };
        self.gap();
        self.line(&format!("{}type {name} = {ty};", self.vis()));
        self.closure_trait(t, &name);
    }

    /// The default methods of closure type `t` (named `name`) that the program calls, as an
    /// extension trait of the closure type, `trait OpMethods`, whose `self` is the closure.
    fn closure_trait(&mut self, t: usize, name: &str) {
        let mut defaults: Vec<usize> = (self.wanted.iter())
            .filter_map(|need| match *need {
                Need::Body(i) if self.class.methods[i].class == t && !self.class.methods[i].is_static => {
                    Some(i)
                }
                _ => None,
            })
            .collect();
        if defaults.is_empty() {
            return;
        }
        defaults.sort();
        let vis = self.vis();
        let mut signatures = Vec::new();
        let mut bodies = String::new();
        self.indent += 1;
        for i in defaults {
            let mark = self.out.len();
            self.gap();
            self.method(i);
            let text = self.out.split_off(mark);
            // Trait methods have the trait's visibility.
            let text = text.replacen(&format!("{vis}fn "), "fn ", 1);
            if let Some(line) = text.lines().find(|l| l.trim_start().starts_with("fn ")) {
                signatures.push(format!("{};", line.trim().trim_end_matches('{').trim_end()));
            }
            bodies.push_str(&text);
        }
        self.indent -= 1;
        self.gap();
        self.line(&format!("{vis}trait {name}Methods {{"));
        self.indent += 1;
        for signature in &signatures {
            self.line(signature);
        }
        self.indent -= 1;
        self.line("}");
        self.gap();
        self.line(&format!("impl {name}Methods for {name} {{"));
        self.out.push_str(bodies.trim_start_matches('\n'));
        self.line("}");
    }

    /// `f.apply(x)` on a closure: `f(x)`. A closure read from a field is taken out of its object
    /// before the call, which may reach the object again.
    pub(super) fn closure_call(&mut self, receiver: &'a Expr, index: usize, args: &'a [Expr]) -> Code {
        let t = self.closure_type(receiver.ty).expect("a closure's type");
        let callee = match &receiver.kind {
            // In a default method, the closure itself.
            ExprKind::This => self.this_name().to_string(),
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
        if index != self.closure_method(t) {
            // A default method, of the extension trait.
            // The trait of the interface that declares it, which `t` may extend.
            let declaring = self.class.methods[index].class;
            self.need(Need::Body(index));
            self.need(Need::Enum(declaring));
            let path = format!("{}Methods", self.closure_name(declaring));
            if path.contains("::") {
                self.traits.insert(path);
            }
            return Code::atom(format!("{callee}.{}({args})", self.fn_names[index]));
        }
        Code::atom(format!("{callee}({args})"))
    }

    /// A lambda converted to its closure type `t`: its captures bound to locals, and the closure
    /// that moves them in, whose body is the lambda class's method.
    pub(super) fn closure(&mut self, t: usize, ctor: usize, args: &'a [Expr]) -> Code {
        let (prelude, closure) = self.closure_parts(t, ctor, args);
        if prelude.is_empty() {
            return Code::new(closure, AS);
        }
        Code::atom(format!("{{ {} {closure} }}", prelude.join(" ")))
    }

    /// The closure that `value`, the value of a function's final `return`, is, as the lines that
    /// bind its captures and the closure: in tail position they need no block of their own.
    pub(super) fn returned_closure(&mut self, value: &'a Expr) -> Option<(Vec<String>, String)> {
        let ExprKind::Cast(to, lambda) = &value.kind else { return None };
        let ExprKind::New { args, ctor: Some(ctor), .. } = &lambda.kind else { return None };
        let t = self.closure_type(*to)?;
        let plain =
            !self.display && !self.optional_return() && !self.facts.owned_returns.contains(&self.method);
        if !plain || *to != self.ret {
            return None;
        }
        // The function's result type converts the closure without `as`.
        let (prelude, closure) = self.closure_parts(t, *ctor, args);
        let closure =
            closure.rsplit_once(" as ").map_or(closure.as_str(), |(closure, _)| closure).to_string();
        Some((prelude, closure))
    }

    /// The lines that bind the captures of a lambda converted to closure type `t`, and the
    /// closure.
    fn closure_parts(&mut self, t: usize, ctor: usize, args: &'a [Expr]) -> (Vec<String>, String) {
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
        // The lambda's own method behind a bridge that converts the arguments and the result.
        let bridged = match closures::bridged_call(&self.class.methods[m]) {
            Some(ExprKind::MethodCall { resolved: Some(sub), args, .. })
                if self.class.methods[*sub].class == c =>
            {
                Some((*sub, args))
            }
            _ => None,
        };
        let converts_result = !matches!(&self.class.methods[m].body[..],
            [Stmt::Expr(e) | Stmt::Return(Some(e), _)] if matches!(e.kind, ExprKind::MethodCall { .. }));
        let saved = self.set_aside();
        // The names of the captures differ from every local of the lambda class's methods.
        let mut used: HashSet<String> = HashSet::from(["java".to_string()]);
        for (i, method) in self.class.methods.iter().enumerate() {
            if method.class == c && !method.is_ctor {
                self.enter_method(i);
                used.extend(self.locals.values().cloned());
            }
        }
        self.enter_method(m);
        self.this_struct = c;
        self.this_form = ThisForm::SelfRef;
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
        // The parameters of the lambda's method are bound to the converted arguments first.
        // Then its body is the closure's when the result needs no conversion, and the bridge's
        // conversion is around its value when it is one `return`; `inline_call` writes it in
        // the bridge's body otherwise.
        let mut body_method = m;
        if let Some((sub, args)) = bridged {
            let one_return = matches!(&self.class.methods[sub].body[..], [Stmt::Return(Some(_), _)]);
            let direct = !converts_result && self.ret_text(sub) == ret;
            if direct || (one_return && !self.facts.throwing.contains(&sub)) {
                let values = self.call_args(sub, args);
                self.enter_method(sub);
                self.bind_params(sub, &values);
                if direct {
                    body_method = sub;
                } else {
                    self.enter_method(m);
                    self.bound_params = Some(sub);
                }
            }
        }
        let body = &self.class.methods[body_method].body;
        self.stmts(body, true);
        self.bound_params = None;
        if self.throwing && self.class.methods[m].ret == Type::Void && exceptions::completes_all(body) {
            self.line("Ok(())");
        }
        self.resolve_annotations(start);
        let text = std::mem::take(&mut self.out);
        self.restore(saved);
        let moving = if moving { "move " } else { "" };
        let body = self.braced(&text);
        let closure =
            format!("Rc::new({moving}|{}|{ret} {body}) as {}", params.join(", "), self.closure_name(t));
        (prelude, closure)
    }

    /// Whether `this.m(..)` (`receiver`, method `index`) is, in a closure's body, a call of the
    /// lambda's own method by the bridge that converts its result.
    pub(super) fn inlined(&self, receiver: &Expr, index: usize) -> bool {
        let c = self.class.methods[index].class;
        matches!(receiver.kind, ExprKind::This)
            && matches!(self.class.types[c].origin, Origin::Lambda { .. })
            && self.class.types[c].implements.iter().any(|i| self.facts.closure_types.contains(i))
    }

    /// The lambda's own method `index` called by its bridge in a closure's body, written in
    /// place: `{ let x: i32 = <arg>; x * x }`, the parameters bound to the arguments, or, when the
    /// method returns early or throws, a closure called at once.
    pub(super) fn inline_call(&mut self, index: usize, args: &'a [Expr]) -> Code {
        let bound = self.bound_params == Some(index);
        let values = if bound { Vec::new() } else { self.call_args(index, args) };
        let field_locals = self.field_locals.clone();
        let (this_struct, this_form) = (self.this_struct, self.this_form);
        let saved = self.set_aside();
        self.enter_method(index);
        self.this_struct = this_struct;
        self.this_form = this_form;
        self.field_locals = field_locals;
        let params = self.typed_params(index, true);
        let ret = self.ret_text(index);
        let body = &self.class.methods[index].body;
        let block = !self.throwing && only_tail_return(body);
        self.indent = saved.indent + 1;
        let start = self.out.len();
        if block && !bound {
            self.bind_params(index, &values);
        }
        self.stmts(body, true);
        if self.throwing && self.class.methods[index].ret == Type::Void && exceptions::completes_all(body) {
            self.line("Ok(())");
        }
        self.resolve_annotations(start);
        let text = std::mem::take(&mut self.out);
        self.restore(saved);
        if block && text.trim().lines().count() == 1 {
            return Code::new(text.trim().to_string(), LOWEST);
        }
        let body = self.braced(&text);
        if block {
            return Code::atom(body);
        }
        Code::atom(format!("(|{}|{ret} {body})({})", params.join(", "), values.join(", ")))
    }

    /// `let x: i32 = <value>;` for each parameter of method `index` that its value is not already.
    fn bind_params(&mut self, index: usize, values: &[String]) {
        for (param, value) in self.typed_params(index, true).iter().zip(values) {
            let name = param.split_once(": ").map_or(param.as_str(), |(name, _)| name);
            if name != value {
                self.line(&format!("let {param} = {value};"));
            }
        }
    }

    /// The lines `text` as a block at the current indentation: on one line when they are few
    /// and short, simple statements.
    fn braced(&self, text: &str) -> String {
        let lines: Vec<&str> = text.lines().map(str::trim).collect();
        let simple = lines.len() <= 3
            && lines.iter().map(|l| l.len() + 1).sum::<usize>() <= 100
            && lines
                .iter()
                .all(|l| !l.ends_with(['{', '(', '[', ',']) && !l.starts_with(['}', ')', ']', '.']));
        if lines.len() == 1 || simple {
            return format!("{{ {} }}", lines.join(" "));
        }
        format!("{{\n{text}{}}}", "    ".repeat(self.indent))
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
            bound_params: self.bound_params.take(),
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
        self.bound_params = s.bound_params;
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

/// Whether `body` returns only at its end.
fn only_tail_return(body: &[Stmt]) -> bool {
    let mut returns = false;
    for s in body.split_last().map_or(&[][..], |(_, rest)| rest) {
        visit_stmt(s, &mut |_| {}, &mut |s| returns |= matches!(s, Stmt::Return(..)));
    }
    !returns
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
