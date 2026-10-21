//! `java.util.Objects`: `equals(a, b)`, `hashCode(o)`, `hash(values...)`, `isNull` and
//! `nonNull`, which the `equals` and `hashCode` of records use too (see `javac::records`). A
//! string, a box or an enum compares and hashes as a value (`ObjectsEquals`, `ObjectsHashCode`);
//! an object calls its class's `equals` or `hashCode`, which the builtin carries as its last
//! argument, so that the analyses see the call.

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

    /// Whether values of `ty` compare and hash as values: strings, boxes and enums.
    fn compares_as_value(ty: Type) -> bool {
        matches!(ty, Type::String | Type::Boxed(_) | Type::Enum(_))
    }

    /// `Objects.equals(a, b)`: true when both are null, or `a.equals(b)`.
    fn objects_equals(&self, node: Node<'t>, nodes: &[Node<'t>], args: Vec<Expr>) -> Result<ExprKind, Error> {
        let (a, b) = (&args[0], &args[1]);
        let same = a.ty == b.ty || a.ty == Type::Null || b.ty == Type::Null;
        let ty = if a.ty == Type::Null { b.ty } else { a.ty };
        if same && Self::compares_as_value(ty) {
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
