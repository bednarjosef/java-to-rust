//! `java.lang.Object`. A program that uses `Object` gets a type of its own for it, named
//! `OBJECT`: an interface that every other type implements, so that the machinery of interfaces
//! (an enum over the classes whose objects become values of it, conversions, dispatch,
//! `instanceof`) covers it. Strings and boxes become `Object`s too, so each runtime class whose
//! values can be one is a type of kind `Runtime` beside it. `Object`'s `equals` and `hashCode` are
//! abstract methods of the type, which the program's `equals(Object)` and `hashCode()` override;
//! a class that overrides neither compares and hashes by identity (see `codegen::dispatch`).

use super::*;

/// Where lowering puts `Object`: the index of its type and of its two methods, which follow the
/// program's types and methods.
#[derive(Clone, Copy)]
pub(super) struct ObjectIndexes {
    pub(super) decl: usize,
    pub(super) equals: usize,
    pub(super) hash_code: usize,
}

/// Whether the declarations mention `java.lang.Object` other than through the constructor that
/// every class calls: a value, a parameter or a call of one of its methods.
pub(super) fn uses_object(decls: &[Node]) -> bool {
    decls.iter().any(|d| mentions_object(d.0))
}

fn mentions_object(v: &Value) -> bool {
    match v {
        Value::Object(fields) => {
            if fields.get("kind").and_then(Value::as_str) == Some("CONSTRUCTOR")
                && fields.get("owner").and_then(Value::as_str) == Some("java.lang.Object")
            {
                return false;
            }
            let names = ["javaType", "type", "owner"];
            names.iter().any(|n| {
                fields.get(*n).and_then(Value::as_str).is_some_and(|t| t.contains("java.lang.Object"))
            }) || fields.values().any(mentions_object)
        }
        Value::Array(items) => items.iter().any(mentions_object),
        _ => false,
    }
}

impl<'t> Lowering<'t> {
    /// Plans `Object` for a program that uses it: its type and methods come after the `types`
    /// and `methods` declared so far.
    pub(super) fn plan_object(&mut self, decls: &[Node<'t>]) {
        if uses_object(decls) {
            let decl = self.types.len();
            let equals = self.methods.len();
            self.object = Some(ObjectIndexes { decl, equals, hash_code: equals + 1 });
        }
    }

    /// `Object`'s abstract `equals` and `hashCode`, appended to the lowered methods, which must be
    /// exactly the declared ones; and the program's overrides of them.
    pub(super) fn object_methods(&self, methods: &mut Vec<Method>) {
        let Some(at) = self.object else { return };
        debug_assert_eq!(methods.len(), at.equals);
        let object = Type::class(OBJECT);
        let method = |name: &str, params: Vec<Param>, ret: Type| Method {
            name: name.to_string(),
            params,
            ret,
            body: Vec::new(),
            is_main: false,
            is_static: false,
            is_ctor: false,
            class: at.decl,
            pos: Pos::default(),
            is_abstract: true,
            overrides: Vec::new(),
        };
        for m in methods.iter_mut().filter(|m| !m.is_static && !m.is_ctor) {
            match (m.name.as_str(), &m.params[..], m.ret) {
                ("equals", [p], Type::Boolean) if p.ty == object => m.overrides.push(at.equals),
                ("hashCode", [], Type::Int) => m.overrides.push(at.hash_code),
                _ => {}
            }
        }
        methods.push(method("equals", vec![Param { name: "other".to_string(), ty: object }], Type::Boolean));
        methods.push(method("hashCode", Vec::new(), Type::Int));
    }

    /// The type of `Object` and the runtime classes, appended to the program's types, which every
    /// type without a superclass implements.
    pub(super) fn object_types(&self, types: &mut Vec<TypeDecl>) {
        let Some(at) = self.object else { return };
        debug_assert_eq!(types.len(), at.decl);
        for t in types.iter_mut().filter(|t| t.extends.is_none()) {
            t.implements.push(at.decl);
        }
        let decl = |name: &str, package: &str, kind: TypeKind, implements: Vec<usize>| TypeDecl {
            name: name.to_string(),
            package: package.to_string(),
            kind,
            is_abstract: kind == TypeKind::Interface,
            extends: None,
            implements,
            fields: Vec::new(),
            statics: Vec::new(),
            enum_constants: None,
        };
        types.push(decl(OBJECT, "", TypeKind::Interface, Vec::new()));
        for (name, _) in runtime_values() {
            types.push(decl(name, "", TypeKind::Runtime, vec![at.decl]));
        }
    }

    /// Whether `ty` is `Object`.
    pub(super) fn is_object(ty: Type) -> bool {
        matches!(ty, Type::Class(name) if name == OBJECT)
    }

    /// `x.equals(o)` on a string or a box `x` with an argument of `Object`: `Object`'s `equals`
    /// on `x` as an `Object`, which compares it with the string or box that `o` may hold.
    pub(super) fn value_equals(
        &self,
        node: Node<'t>,
        receiver: Option<Node<'t>>,
        args: Vec<Expr>,
    ) -> Result<ExprKind, Error> {
        let (Some(at), Some(receiver)) = (self.object, receiver) else {
            return unsupported(node, "this call");
        };
        let receiver = self.convert(self.expr(receiver)?, Type::class(OBJECT));
        let name = "equals".to_string();
        Ok(ExprKind::MethodCall {
            receiver: Box::new(receiver),
            name,
            args,
            resolved: Some(at.equals),
            is_super: false,
        })
    }

    /// A call of `Object`'s `equals`, `hashCode` or `toString` on a value whose class does not
    /// declare it (javac names `Object` as the method's owner).
    pub(super) fn object_call(
        &self,
        node: Node<'t>,
        symbol: &Symbol,
        receiver: Option<Node<'t>>,
        args: Vec<Expr>,
    ) -> Result<ExprKind, Error> {
        let Some(at) = self.object else {
            return unsupported(node, &format!("method `Object.{}`", symbol.name));
        };
        let receiver = match receiver {
            Some(r) if r.kind() == "IDENTIFIER" && r.str("name") == "super" => {
                return unsupported(node, &format!("`super.{}()` of `Object`", symbol.name));
            }
            Some(r) => self.expr(r)?,
            None => self.this(node),
        };
        if !matches!(receiver.ty, Type::Class(_)) {
            return unsupported(node, &format!("`{}` of a value of this type", symbol.name));
        }
        let (resolved, args) = match symbol.name {
            "equals" => {
                let args: Vec<Expr> =
                    args.into_iter().map(|a| self.convert(a, Type::class(OBJECT))).collect();
                (at.equals, args)
            }
            "hashCode" => (at.hash_code, args),
            "toString" => return Ok(ExprKind::Builtin(Builtin::ObjectText, vec![receiver])),
            "getClass" => return unsupported(node, "`getClass` (reflection)"),
            name => return unsupported(node, &format!("method `Object.{name}`")),
        };
        let name = symbol.name.to_string();
        Ok(ExprKind::MethodCall {
            receiver: Box::new(receiver),
            name,
            args,
            resolved: Some(resolved),
            is_super: false,
        })
    }
}
