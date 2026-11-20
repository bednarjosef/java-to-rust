//! `java.util.Objects`: `equals(a, b)`, `hashCode(o)`, `hash(values...)`, `isNull` and
//! `nonNull`, which the `equals` and `hashCode` of records use too (see `javac::records`). A
//! string, a box or an enum compares and hashes as a value (`ObjectsEquals`, `ObjectsHashCode`);
//! an object calls its class's `equals` or `hashCode`, which the builtin carries as its last
//! argument, so that the analyses see the call. This is for programs without the JDK's sources.
//! With them, `Objects` is translated from its own source like any class; the calls of the
//! members written for records stay here, and the program's calls of `equals` and `hashCode` are
//! written in place (`typed_objects_call`).

use super::*;

impl<'t> Lowering<'t> {
    /// A call of a static method of `java.util.Objects`.
    pub(super) fn objects_call(
        &self,
        node: Node<'t>,
        name: &str,
        args: Vec<Expr>,
    ) -> Result<ExprKind, Error> {
        let arg_nodes: Vec<Node<'t>> = node.list("arguments").collect();
        match (name, args.as_slice()) {
            ("equals", [_, _]) => self.objects_equals(node, &arg_nodes, args),
            ("hashCode", [_]) => self.objects_hash_code(node, arg_nodes[0], args),
            ("hash", _) => self.objects_hash(node, &arg_nodes, args),
            ("isNull" | "nonNull", [_]) => {
                let op = if name == "isNull" { BinOp::Eq } else { BinOp::Ne };
                let null = self.synth(ExprKind::Null, node.pos(), Type::Null);
                let value = args.into_iter().next().expect("one argument");
                Ok(ExprKind::Binary(op, Box::new(value), Box::new(null)))
            }
            _ => unsupported(node, &format!("method `java.util.Objects.{name}`")),
        }
    }

    /// Whether a call of `Objects.equals`, `Objects.hashCode` or `Objects.hash`, where the JDK's
    /// `Objects` is translated, is written in place (`typed_objects_call`): `equals` on strings,
    /// boxes of one kind or enums, or with an object of a class or interface type first;
    /// `hashCode` of a string, a box, an enum or an object; `hash` of those and of numbers, where
    /// javac packs them into the array (`hashed_values`). A type variable or a class of a Rust
    /// generic (`lower::real_generics`) keeps the call, whose arguments are `Object`s.
    pub(super) fn typed_objects_call_fits(&self, node: Node<'t>, name: &str, args: &[Expr]) -> bool {
        let hashed = |ty: Type| {
            ty.is_numeric()
                || matches!(
                    ty,
                    Type::Boolean | Type::String | Type::Boxed(_) | Type::Enum(_) | Type::Class(..)
                )
        };
        fn generic(ty: Type) -> bool {
            match ty {
                Type::Array(element) => generic(*element),
                Type::Class(name, args) => name.contains('#') || !args.is_empty(),
                _ => false,
            }
        }
        if self.object.is_none() {
            return false;
        }
        match (name, args) {
            ("equals", [a, b]) => {
                !generic(a.ty)
                    && !generic(b.ty)
                    && (Self::equal_as_values(a, b) || matches!(a.ty, Type::Class(..)))
            }
            ("hashCode", [o]) => {
                !generic(o.ty) && hashed(o.ty) && !o.ty.is_numeric() && o.ty != Type::Boolean
            }
            ("hash", _) => Self::hashed_values(node, args).is_some_and(|values| {
                values.iter().map(|v| unconverted(v).ty).all(|ty| hashed(ty) && !generic(ty))
            }),
            _ => false,
        }
    }

    /// The values of a call of `Objects.hash(a, b, ..)` that javac packs into its array, which
    /// lowering has made the one argument (`varargs::packed_args`), each converted to `Object`:
    /// none where the call passes an array of its own.
    fn hashed_values<'e>(node: Node<'t>, args: &'e [Expr]) -> Option<&'e [Expr]> {
        match args {
            [Expr { kind: ExprKind::ArrayLit(values), .. }]
                if !node.str("varargsElement").is_empty()
                    && node.list("arguments").count() == values.len() =>
            {
                Some(values)
            }
            _ => None,
        }
    }

    /// `Objects.equals(a, b)`, `Objects.hashCode(o)` or `Objects.hash(values..)` of the JDK's
    /// `Objects`, in place, as its source has them, where `typed_objects_call_fits`: `a == b ||
    /// a != null && a.equals(b)`, `o != null ? o.hashCode() : 0`, and `31 * h + hash(v)` over the
    /// values from 1, as `Arrays.hashCode` of the array the call would make. The object's own
    /// method then runs on the static type the call site knows, so that a call that only a few
    /// classes can receive tests only those, where the JDK's method, of `Object`s, dispatches over
    /// every class of the program; a value passed to it no longer reaches every `equals` there
    /// is; and `hash` makes no array and no boxes. Each value is read once and in order, as the
    /// call's arguments are, before any method runs: `null` in place, another literal too but in
    /// `equals`, a local or `this` when no argument has effects, anything else into a local of its
    /// own first. Strings and boxes compare and hash as values (`ObjectsEquals`, `ObjectsHashCode`),
    /// as their own methods do, and enums compare by identity.
    pub(super) fn typed_objects_call(
        &self,
        node: Node<'t>,
        name: &str,
        args: Vec<Expr>,
    ) -> Result<ExprKind, Error> {
        if let ("equals", [a, b]) = (name, args.as_slice())
            && Self::equal_as_values(a, b)
        {
            return self.objects_call(node, name, args);
        }
        let at = self.object.expect("`typed_objects_call_fits` holds");
        let pos = node.pos();
        let nodes: Vec<Node<'t>> = node.list("arguments").collect();
        // `hash`'s values as the call writes them, out of the array that javac packs.
        let args: Vec<Expr> = if name == "hash" {
            let Some(Expr { kind: ExprKind::ArrayLit(values), .. }) = args.into_iter().next() else {
                unreachable!("`typed_objects_call_fits` holds")
            };
            values.into_iter().map(unconvert).collect()
        } else {
            args
        };
        let effects = args.iter().any(has_effects);
        let mut locals = Vec::new();
        let mut reads = Vec::new();
        for (i, arg) in args.into_iter().enumerate() {
            let literal = matches!(
                arg.kind,
                ExprKind::Int(_)
                    | ExprKind::Long(_)
                    | ExprKind::Double(_)
                    | ExprKind::Float(_)
                    | ExprKind::Char(_)
                    | ExprKind::Bool(_)
                    | ExprKind::Str(_)
            );
            // A literal has no value to change. `null` has no type to give a local either; any other
            // literal is read in place where it is read once, which is in `hash` and `hashCode`: in
            // `equals`, the shortcut `a == b || .. a.equals(b)` that compares strings and boxes by
            // value (`analysis::patterns`) knows the two values only as variables.
            let in_place = matches!(arg.kind, ExprKind::Null)
                || literal && name != "equals"
                || !effects && matches!(arg.kind, ExprKind::Var(_) | ExprKind::This);
            if in_place {
                reads.push(ObjectsRead::InPlace(nodes[i], Some(arg)));
                continue;
            }
            let local = self.fresh_name(&format!("objects_{}_{}_{i}", pos.line, pos.column));
            reads.push(ObjectsRead::Local(local.clone(), arg.ty));
            locals.push(Stmt::Local { ty: arg.ty, name: local, init: Some(arg), pos });
        }
        let object = Type::class(OBJECT);
        let null = || self.synth(ExprKind::Null, pos, Type::Null);
        let boolean = |kind| self.synth(kind, pos, Type::Boolean);
        let int = |kind| self.synth(kind, pos, Type::Int);
        let (value, ty) = match name {
            "equals" => {
                let (a, b) = (self.objects_read(&mut reads[0], pos)?, self.objects_read(&mut reads[1], pos)?);
                let (a, b) = if a.ty == b.ty || b.ty == Type::Null {
                    (a, b)
                } else {
                    (self.convert(a, object), self.convert(b, object))
                };
                let same = boolean(ExprKind::Binary(BinOp::Eq, Box::new(a), Box::new(b)));
                let a = self.objects_read(&mut reads[0], pos)?;
                let given = boolean(ExprKind::Binary(BinOp::Ne, Box::new(a), Box::new(null())));
                let other = self.convert(self.objects_read(&mut reads[1], pos)?, object);
                let call = boolean(ExprKind::MethodCall {
                    receiver: Box::new(self.objects_read(&mut reads[0], pos)?),
                    name: "equals".to_string(),
                    args: vec![other],
                    resolved: Some(at.equals),
                    is_super: false,
                });
                let equal = boolean(ExprKind::Binary(BinOp::And, Box::new(given), Box::new(call)));
                (ExprKind::Binary(BinOp::Or, Box::new(same), Box::new(equal)), Type::Boolean)
            }
            "hashCode" => (self.objects_hash_of(node, &mut reads[0])?, Type::Int),
            _ => {
                let mut hash = ExprKind::Int(1);
                for read in &mut reads {
                    let scaled =
                        ExprKind::Binary(BinOp::Mul, Box::new(int(ExprKind::Int(31))), Box::new(int(hash)));
                    let h = self.objects_hash_of(node, read)?;
                    hash = ExprKind::Binary(BinOp::Add, Box::new(int(scaled)), Box::new(int(h)));
                }
                (hash, Type::Int)
            }
        };
        if locals.is_empty() {
            return Ok(value);
        }
        // `switch (0) { default -> { T a = ..; U b = ..; yield ..; } }`, as a block that has a value.
        locals.push(Stmt::Yield(self.synth(value, pos, ty)));
        Ok(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body: locals }],
        })
    }

    /// The hash of a value of `typed_objects_call`, as `Objects.hashCode` gives it (a number as its
    /// box): a string's or a box's as a value, 0 for null, else the object's own `hashCode`, an
    /// enum's being `Object`'s.
    fn objects_hash_of(&self, node: Node<'t>, read: &mut ObjectsRead<'t>) -> Result<ExprKind, Error> {
        let pos = node.pos();
        let value = self.objects_read(read, pos)?;
        let ty = value.ty;
        if ty.is_numeric() || ty == Type::Boolean {
            return Ok(ExprKind::Builtin(Builtin::Boxed(BoxedOp::HashCode, ty), vec![value]));
        }
        if matches!(ty, Type::String | Type::Boxed(_)) {
            return Ok(ExprKind::Builtin(Builtin::ObjectsHashCode, vec![value]));
        }
        let null = self.synth(ExprKind::Null, pos, Type::Null);
        let given =
            self.synth(ExprKind::Binary(BinOp::Ne, Box::new(value), Box::new(null)), pos, Type::Boolean);
        let receiver = self.objects_read(read, pos)?;
        let receiver =
            if matches!(ty, Type::Enum(_)) { self.convert(receiver, Type::class(OBJECT)) } else { receiver };
        let hash = self.hash_code_call(node, receiver)?;
        let zero = self.synth(ExprKind::Int(0), pos, Type::Int);
        Ok(ExprKind::Conditional(Box::new(given), Box::new(hash), Box::new(zero)))
    }

    /// A read of a value of `typed_objects_call`: the value itself the first time, then the same
    /// node lowered again, or the local it was put in.
    fn objects_read(&self, read: &mut ObjectsRead<'t>, pos: Pos) -> Result<Expr, Error> {
        match read {
            ObjectsRead::InPlace(node, first) => match first.take() {
                Some(value) => Ok(value),
                None => self.expr(*node),
            },
            ObjectsRead::Local(name, ty) => Ok(self.synth(ExprKind::Var(name.clone()), pos, *ty)),
        }
    }

    /// Whether `Objects.equals(a, b)` compares the two as values: strings, boxes of one kind or
    /// enums (or one of them null).
    fn equal_as_values(a: &Expr, b: &Expr) -> bool {
        let same = a.ty == b.ty || a.ty == Type::Null || b.ty == Type::Null;
        let ty = if a.ty == Type::Null { b.ty } else { a.ty };
        same && Self::compares_as_value(ty)
    }

    /// `Objects.requireNonNull(cs)` or `requireNonNull(cs, "message")` of a charset, which is the
    /// runtime's value rather than an object of the translation:
    /// `switch (0) { default -> { Charset c = cs; yield c != null ? c : throw new
    /// NullPointerException("message"); } }`.
    pub(super) fn required_charset(&self, node: Node<'t>, args: Vec<Expr>) -> Result<ExprKind, Error> {
        let pos = node.pos();
        let message = match args.get(1).map(|a| &a.kind) {
            None => None,
            Some(ExprKind::Str(message)) => Some(message.clone()),
            Some(_) => {
                return unsupported(node, "`Objects.requireNonNull` of a charset with a computed message");
            }
        };
        let Some(throw) = self.null_thrown(message.as_deref(), Type::Charset, pos) else {
            return unsupported(node, "`Objects.requireNonNull` of a charset without `NullPointerException`");
        };
        let name = self.fresh_name("charset");
        let var = || self.synth(ExprKind::Var(name.clone()), pos, Type::Charset);
        let null = self.synth(ExprKind::Null, pos, Type::Null);
        let given =
            self.synth(ExprKind::Binary(BinOp::Ne, Box::new(var()), Box::new(null)), pos, Type::Boolean);
        let value = ExprKind::Conditional(Box::new(given), Box::new(var()), Box::new(throw));
        let charset = args.into_iter().next().expect("a charset");
        let block = vec![
            Stmt::Local { ty: Type::Charset, name: name.clone(), init: Some(charset), pos },
            Stmt::Yield(self.synth(value, pos, Type::Charset)),
        ];
        Ok(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body: block }],
        })
    }

    /// Whether values of `ty` compare and hash as values: strings, boxes and enums.
    fn compares_as_value(ty: Type) -> bool {
        matches!(ty, Type::String | Type::Boxed(_) | Type::Enum(_))
    }

    /// `Objects.equals(a, b)`: true when both are null, or `a.equals(b)`.
    fn objects_equals(&self, node: Node<'t>, nodes: &[Node<'t>], args: Vec<Expr>) -> Result<ExprKind, Error> {
        let (a, b) = (&args[0], &args[1]);
        let ty = if a.ty == Type::Null { b.ty } else { a.ty };
        if Self::equal_as_values(a, b) {
            if matches!(ty, Type::Enum(_)) {
                let mut args = args.into_iter();
                let (a, b) = (args.next().expect("two arguments"), args.next().expect("two arguments"));
                return Ok(ExprKind::Binary(BinOp::Eq, Box::new(a), Box::new(b)));
            }
            return Ok(ExprKind::Builtin(Builtin::ObjectsEquals, args));
        }
        let Some(at) = self.object else { return unsupported(node, "`Objects.equals` here") };
        if args.iter().any(has_effects) {
            return unsupported(node, "`Objects.equals` of values with effects");
        }
        // The call that runs when `a` is not null: `a.equals(b)`, which `Object` dispatches to the
        // class of `a`.
        let object = Type::class(OBJECT);
        let receiver = self.expr(nodes[0])?;
        let other = self.convert(self.expr(nodes[1])?, object);
        let call = ExprKind::MethodCall {
            receiver: Box::new(receiver),
            name: "equals".to_string(),
            args: vec![other],
            resolved: Some(at.equals),
            is_super: false,
        };
        let call = self.synth(call, node.pos(), Type::Boolean);
        let mut args: Vec<Expr> = args.into_iter().map(|a| self.convert(a, object)).collect();
        args.push(call);
        Ok(ExprKind::Builtin(Builtin::ObjectsEquals, args))
    }

    /// `Objects.hashCode(o)`: 0 for null, else `o.hashCode()`.
    fn objects_hash_code(&self, node: Node<'t>, arg: Node<'t>, args: Vec<Expr>) -> Result<ExprKind, Error> {
        let value = &args[0];
        if matches!(value.kind, ExprKind::Null) {
            return Ok(ExprKind::Int(0));
        }
        if Self::compares_as_value(value.ty) {
            return Ok(ExprKind::Builtin(Builtin::ObjectsHashCode, args));
        }
        if !matches!(value.ty, Type::Class(..)) {
            return unsupported(node, "`Objects.hashCode` of a value of this type");
        }
        if has_effects(value) {
            return unsupported(node, "`Objects.hashCode` of a value with effects");
        }
        let call = self.hash_code_call(node, self.expr(arg)?)?;
        let mut args = args;
        args.push(call);
        Ok(ExprKind::Builtin(Builtin::ObjectsHashCode, args))
    }

    /// `receiver.hashCode()` on an object: `Object`'s, dispatched to its class's, or without
    /// `Object` in the program, the class's own (a record's) or the identity hash.
    fn hash_code_call(&self, node: Node<'t>, receiver: Expr) -> Result<Expr, Error> {
        let pos = node.pos();
        let kind = match self.object {
            Some(at) => ExprKind::MethodCall {
                receiver: Box::new(receiver),
                name: "hashCode".to_string(),
                args: Vec::new(),
                resolved: Some(at.hash_code),
                is_super: false,
            },
            None => {
                let Type::Class(name, _) = receiver.ty else { return unsupported(node, "`hashCode` here") };
                let own = self.type_index(name).and_then(|t| {
                    self.methods.iter().position(|m| m.owner == t && m.name == "hashCode" && m.sig == "()int")
                });
                match own {
                    Some(resolved) => ExprKind::MethodCall {
                        receiver: Box::new(receiver),
                        name: "hashCode".to_string(),
                        args: Vec::new(),
                        resolved: Some(resolved),
                        is_super: false,
                    },
                    None => ExprKind::Builtin(Builtin::Native(Native::IdentityHash), vec![receiver]),
                }
            }
        };
        Ok(self.synth(kind, pos, Type::Int))
    }

    /// `Objects.hash(a, b, ..)`, which is `Arrays.hashCode` of the values: `31 * h + hash(v)` over
    /// them, starting at 1, each value hashed as `Objects.hashCode` hashes it (a primitive as its
    /// box).
    fn objects_hash(&self, node: Node<'t>, nodes: &[Node<'t>], args: Vec<Expr>) -> Result<ExprKind, Error> {
        // `Objects.hash(array)` hashes the array's elements, which this does not do.
        if nodes.len() == 1 && args[0].ty.is_reference() && matches!(args[0].ty, Type::Array(_) | Type::Null)
        {
            return unsupported(node, "`Objects.hash` of an array");
        }
        let pos = node.pos();
        let int = |i: i32| self.synth(ExprKind::Int(i), pos, Type::Int);
        let mut hash = int(1);
        for (value, &arg) in args.into_iter().zip(nodes) {
            let h = match value.ty {
                ty if ty.is_numeric() || ty == Type::Boolean => {
                    ExprKind::Builtin(Builtin::Boxed(BoxedOp::HashCode, ty), vec![value])
                }
                _ => self.objects_hash_code(node, arg, vec![value])?,
            };
            let h = self.synth(h, pos, Type::Int);
            let scaled =
                self.synth(ExprKind::Binary(BinOp::Mul, Box::new(int(31)), Box::new(hash)), pos, Type::Int);
            hash = self.synth(ExprKind::Binary(BinOp::Add, Box::new(scaled), Box::new(h)), pos, Type::Int);
        }
        Ok(hash.kind)
    }
}

/// `e` without the conversion to `Object` that lowering gives a value packed into an `Object...`
/// array (`Lowering::convert`).
fn unconverted(e: &Expr) -> &Expr {
    match &e.kind {
        ExprKind::Cast(Type::Class(name, _), inner) if *name == OBJECT => inner,
        _ => e,
    }
}

/// `unconverted`, of a value that the caller owns.
fn unconvert(e: Expr) -> Expr {
    match e.kind {
        ExprKind::Cast(Type::Class(name, _), inner) if name == OBJECT => *inner,
        kind => Expr { kind, ..e },
    }
}

/// How `typed_objects_call` reads one of its values: a local, `this` or a literal lowered again
/// from its node (but for the first read, which is the value lowered already), or the local it is
/// put in.
enum ObjectsRead<'t> {
    InPlace(Node<'t>, Option<Expr>),
    Local(String, Type),
}
