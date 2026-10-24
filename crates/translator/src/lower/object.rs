//! `java.lang.Object`. A program that uses `Object` gets a type of its own for it, named
//! `OBJECT`: an interface that every other type implements, so that the machinery of interfaces
//! (an enum over the classes whose objects become values of it, conversions, dispatch,
//! `instanceof`) covers it. Strings and boxes become `Object`s too, so each runtime class whose
//! values can be one is a type of kind `Runtime` beside it. `Object`'s `equals` and `hashCode` are
//! abstract methods of the type, which the program's `equals(Object)` and `hashCode()` override;
//! a class that overrides neither compares and hashes by identity (see `codegen::dispatch`).
//!
//! `java.lang.Comparable`, when the program uses it as a type (a type variable's bound, mostly),
//! is a type of the same kind, `COMPARABLE`, with an abstract `compareTo(Comparable)`: the
//! program's enums and the classes that implement `Comparable` implement it, and so do strings and
//! the boxes. A class's own `compareTo(C)` implements it through a bridge (see `bridges`).
//!
//! `java.lang.Number`, when the program uses it as a type (`List<? extends Number>`'s elements,
//! `T extends Number`), is a third such type, `NUMBER`, which the numeric boxes implement, with
//! the abstract `byteValue()` to `doubleValue()` that each box answers by converting its value
//! (see `codegen::dispatch`). A class of the program that extends `Number` (the JDK's
//! `AtomicInteger` and `BigInteger`) implements it too, with the `intValue()` to `doubleValue()`
//! that it declares or inherits; `Number`'s own `byteValue()` and `shortValue()`, which narrow
//! `intValue()`, are lowered to that narrowing for every `Number`, since the boxes' agree with it.
//! `java.lang.CharSequence`, when the program uses it as a type (`StringJoiner`'s parameters),
//! is another, `CHAR_SEQUENCE`, which strings and builders implement, with an abstract `length()`,
//! `charAt(int)`, `subSequence(int, int)` and `getChars(int, int, char[], int)` that each answers
//! as the runtime's string or builder does; a class of the program without a `getChars` gets the
//! interface's default, written here as the JDK has it. Its `toString()` is the value's text and
//! its `isEmpty()` a zero length. The JDK's `CharSequence` is not translated, and its other
//! methods are rejected.
//! The JDK's `java.lang.Appendable` is translated, and the runtime's builders implement it too,
//! with its `append` methods answered by the builder's own (see `codegen::dispatch`), so that a
//! builder passed as one (to `Formatter`, to `DateTimeFormatter.formatTo`) is one. The runtime's
//! classes implement `java.lang.reflect.Type` and `Serializable` where the program has them
//! (`CLASS_VALUE_INTERFACES`), with `Type`'s `getTypeName()` answered by the class's own, so
//! that a class held as a `Type` (gson's and guava's type tokens) is one.
//! `java.lang.Enum`, when the program uses it as a type, is a type of the same kind, `ENUM`, with
//! no methods of its own: the program's enums implement it, and `lower::enums` lowers the calls
//! of `Enum`'s methods on its values.

use super::*;
use std::collections::HashSet;

/// Where lowering puts `Object`: the index of its type and of its two methods, which follow the
/// program's types and methods, and `Comparable`'s when the program uses it.
#[derive(Clone, Copy)]
pub(super) struct ObjectIndexes {
    pub(super) decl: usize,
    pub(super) equals: usize,
    pub(super) hash_code: usize,
    pub(super) comparable: Option<ComparableIndexes>,
    pub(super) number: Option<NumberIndexes>,
    /// The type of `CharSequence`, after `Enum`'s, and of the first of its
    /// `CHAR_SEQUENCE_METHODS`, after `Number`'s, when the program uses it.
    pub(super) char_sequence: Option<NumberIndexes>,
    /// The index of `Enum`'s type, after `Number`'s, when the program uses it.
    pub(super) enumeration: Option<usize>,
    /// The index of the JDK's `Appendable`, a type of the program, when the program has it: the
    /// runtime's builders implement it.
    pub(super) appendable: Option<usize>,
}

/// The index of `Number`'s type, after `Comparable`'s or the runtime classes, and of the first of
/// its `NUMBER_METHODS`.
#[derive(Clone, Copy)]
pub(super) struct NumberIndexes {
    pub(super) decl: usize,
    pub(super) methods: usize,
}

/// The abstract methods of `Number`, in order, with their result types.
pub(super) const NUMBER_METHODS: [(&str, Type); 6] = [
    ("byteValue", Type::Byte),
    ("shortValue", Type::Short),
    ("intValue", Type::Int),
    ("longValue", Type::Long),
    ("floatValue", Type::Float),
    ("doubleValue", Type::Double),
];

/// A method of `CharSequence`: its name, its parameters' names and types, and its result type
/// (`None` for `CharSequence` itself).
type SequenceMethod = (&'static str, &'static [(&'static str, Type)], Option<Type>);

/// The methods of `CharSequence` that its values implement, in order. A class without a
/// `getChars` of its own gets the interface's default (`default_get_chars`).
pub(super) const CHAR_SEQUENCE_METHODS: [SequenceMethod; 4] = [
    ("length", &[], Some(Type::Int)),
    ("charAt", &[("index", Type::Int)], Some(Type::Char)),
    ("subSequence", &[("start", Type::Int), ("end", Type::Int)], None),
    (
        "getChars",
        &[
            ("srcBegin", Type::Int),
            ("srcEnd", Type::Int),
            ("dst", Type::Array(&Type::Char)),
            ("dstBegin", Type::Int),
        ],
        Some(Type::Void),
    ),
];

/// The index of `getChars` in `CHAR_SEQUENCE_METHODS`.
const GET_CHARS: usize = 3;

/// Whether the declarations use `java.lang.CharSequence` as the type of a value, as `uses_number`
/// tells for `Number`.
pub(super) fn uses_char_sequence(decls: &[Node]) -> bool {
    decls.iter().any(|d| mentions_char_sequence(d.0))
}

fn mentions_char_sequence(v: &Value) -> bool {
    match v {
        Value::Object(fields) => fields.iter().any(|(key, value)| match key.as_str() {
            "symbol" => false,
            "javaType" => value.as_str().is_some_and(|t| {
                t.trim_end_matches("[]") == "java.lang.CharSequence"
                    || t.ends_with("? extends java.lang.CharSequence")
            }),
            _ => mentions_char_sequence(value),
        }),
        Value::Array(items) => items.iter().any(mentions_char_sequence),
        _ => false,
    }
}

/// The runtime classes whose values are `Number`s: the numeric boxes.
pub(super) fn is_number_box(name: &str) -> bool {
    matches!(
        name,
        "java.lang.Integer"
            | "java.lang.Long"
            | "java.lang.Double"
            | "java.lang.Float"
            | "java.lang.Short"
            | "java.lang.Byte"
    )
}

/// Whether the declarations use `java.lang.Number` as the type of a value: a variable, an
/// expression or a type variable's bound of that type, or a wildcard or a capture bounded by it.
pub(super) fn uses_number(decls: &[Node]) -> bool {
    decls.iter().any(|d| mentions_number(d.0))
}

fn mentions_number(v: &Value) -> bool {
    match v {
        Value::Object(fields) => fields.iter().any(|(key, value)| match key.as_str() {
            "symbol" => false,
            "javaType" => value.as_str().is_some_and(|t| {
                t.trim_end_matches("[]") == "java.lang.Number" || t.ends_with("? extends java.lang.Number")
            }),
            _ => mentions_number(value),
        }),
        Value::Array(items) => items.iter().any(mentions_number),
        _ => false,
    }
}

/// The index of `Comparable`'s type, after the runtime classes, and of its `compareTo`.
#[derive(Clone, Copy)]
pub(super) struct ComparableIndexes {
    pub(super) decl: usize,
    pub(super) compare_to: usize,
}

/// Whether the declarations use `java.lang.Comparable` as a type: anywhere but in the
/// `implements` clause of a class whose `compareTo` the program calls directly.
pub(super) fn uses_comparable(decls: &[Node]) -> bool {
    decls.iter().any(|d| mentions_comparable(d.0))
}

fn mentions_comparable(v: &Value) -> bool {
    match v {
        Value::Object(fields) => fields.iter().any(|(key, value)| match key.as_str() {
            "implementsClause" | "symbol" => false,
            "javaType" => value.as_str().is_some_and(|t| t.starts_with("java.lang.Comparable")),
            _ => mentions_comparable(value),
        }),
        Value::Array(items) => items.iter().any(mentions_comparable),
        _ => false,
    }
}

/// Whether the declarations use `java.lang.Enum` as a type, a type variable's bound included.
fn uses_enum(v: &Value) -> bool {
    match v {
        Value::Object(fields) => fields.iter().any(|(key, value)| match key.as_str() {
            "symbol" => false,
            "javaType" => {
                value.as_str().is_some_and(|t| t == "java.lang.Enum" || t.starts_with("java.lang.Enum<"))
            }
            _ => uses_enum(value),
        }),
        Value::Array(items) => items.iter().any(uses_enum),
        _ => false,
    }
}

/// Whether the declarations mention `java.lang.Object` other than through the constructor that
/// every class calls: a value, a parameter or a call of one of its methods.
pub(super) fn uses_object(decls: &[Node], real: &HashSet<*const Value>) -> bool {
    decls.iter().any(|d| mentions_object(d.0, real))
}

fn mentions_object(v: &Value, real: &HashSet<*const Value>) -> bool {
    if real.contains(&(v as *const Value)) {
        return false;
    }
    let mentions_object = |v| mentions_object(v, real);
    match v {
        // A type variable erases to `Object`.
        Value::Object(fields) if fields.get("kind").and_then(Value::as_str) == Some("TYPE_PARAMETER") => true,
        // A call that the members written for records make (see `javac::records`) on a value that
        // is not an `Object`.
        Value::Object(fields) if fields.get("implicit") == Some(&Value::Bool(true)) => false,
        Value::Object(fields) => {
            if fields.get("kind").and_then(Value::as_str) == Some("CONSTRUCTOR")
                && fields.get("owner").and_then(Value::as_str) == Some("java.lang.Object")
            {
                return false;
            }
            let names = ["javaType", "type", "owner"];
            // An `Enum` is an `Object` too.
            let is_enum = |t: &str| t == "java.lang.Enum" || t.starts_with("java.lang.Enum<");
            names.iter().any(|n| {
                fields.get(n).and_then(Value::as_str).is_some_and(|t| t.contains("java.lang.Object"))
            }) || fields.get("javaType").and_then(Value::as_str).is_some_and(is_enum)
                // `Enum.hashCode()` is `Object`'s (see `enums`).
                || fields.get("owner").and_then(Value::as_str) == Some("java.lang.Enum")
                    && fields.get("name").and_then(Value::as_str) == Some("hashCode")
                || fields.values().any(mentions_object)
        }
        Value::Array(items) => items.iter().any(mentions_object),
        _ => false,
    }
}

/// The class of the objects that `new Object()` creates: a class without fields that implements
/// `Object` like any other, so that such an object is a variant of the `Object` enum, equal only
/// to itself, hashed and printed by its identity (`java.lang.Object@1b6d3586`, see
/// `Origin::Object`). The lock objects and the placeholder values of the JDK's collections
/// (`HashSet`'s `PRESENT`) are such objects.
pub(super) const PLAIN_OBJECT: &str = "PlainObject";

/// Whether the tree `v` creates an object of `java.lang.Object` itself.
fn creates_object(v: &Value) -> bool {
    match v {
        Value::Object(fields) => {
            (fields.get("kind").and_then(Value::as_str) == Some("NEW_CLASS")
                && fields.get("classBody").is_none_or(Value::is_null)
                && v["symbol"]["owner"].as_str() == Some("java.lang.Object"))
                || fields.values().any(creates_object)
        }
        Value::Array(items) => items.iter().any(creates_object),
        _ => false,
    }
}

impl<'t> Lowering<'t> {
    /// Declares the class of `new Object()` (`PLAIN_OBJECT`) when one of these trees creates such
    /// an object.
    pub(super) fn declare_plain_object<'v>(
        &mut self,
        trees: impl IntoIterator<Item = &'v Value>,
    ) -> Result<(), Error> {
        if !trees.into_iter().any(creates_object) {
            return Ok(());
        }
        if let Some(clash) = self.types.iter().find(|t| t.name == PLAIN_OBJECT) {
            return unsupported(clash.node, &format!("a class named `{PLAIN_OBJECT}`"));
        }
        let ctor = crate::value::json!({"kind": "METHOD", "name": "<init>", "modifiers": [],
            "parameters": [], "body": {"kind": "BLOCK", "statements": []},
            "symbol": {"kind": "CONSTRUCTOR", "name": "<init>", "owner": PLAIN_OBJECT, "type": "()void"}});
        let decl = crate::value::json!({"kind": "CLASS", "simpleName": PLAIN_OBJECT, "modifiers": [],
            "members": [ctor], "symbol": {"kind": "CLASS", "name": PLAIN_OBJECT, "owner": "",
            "type": PLAIN_OBJECT}});
        self.plain_object = Some(self.types.len());
        self.declare_type(Node(Box::leak(Box::new(decl))), None)
    }

    /// `new Object()`, `node`: an object of the class `PLAIN_OBJECT` as an `Object`.
    pub(super) fn new_plain_object(&self, node: Node<'t>) -> Option<ExprKind> {
        let t = self.plain_object?;
        let symbol = node.symbol()?;
        if symbol.owner != "java.lang.Object" || node.get("classBody").is_some() {
            return None;
        }
        let ctor = self.methods.iter().position(|m| m.owner == t && m.name == "<init>");
        let new = self.synth(
            ExprKind::New { class: t, args: Vec::new(), ctor },
            node.pos(),
            Type::class(PLAIN_OBJECT),
        );
        Some(ExprKind::Cast(Type::class(OBJECT), Box::new(new)))
    }

    /// Plans `Object` for a program that uses it: its type and methods come after the `types`
    /// and `methods` declared so far.
    pub(super) fn plan_object(&mut self, decls: &[Node<'t>]) {
        let comparable = uses_comparable(decls);
        let number = uses_number(decls);
        let char_sequence = uses_char_sequence(decls);
        let unrelated = decls.iter().any(|d| self.compares_unrelated(d.0));
        let appendable = self.type_index("java.lang.Appendable");
        let class_interface = CLASS_VALUE_INTERFACES.iter().any(|i| self.type_index(i).is_some());
        if uses_object(decls, &self.real_type_parameters())
            || comparable
            || number
            || char_sequence
            || unrelated
            || appendable.is_some()
            || class_interface
        {
            let decl = self.types.len();
            let equals = self.methods.len();
            let runtime = decl + 1 + runtime_values().len();
            let comparable =
                comparable.then_some(ComparableIndexes { decl: runtime, compare_to: equals + 2 });
            let after = usize::from(comparable.is_some());
            let number =
                number.then_some(NumberIndexes { decl: runtime + after, methods: equals + 2 + after });
            let enumeration = decls
                .iter()
                .any(|d| uses_enum(d.0))
                .then_some(runtime + after + usize::from(number.is_some()));
            let char_sequence = char_sequence.then_some(NumberIndexes {
                decl: runtime + after + usize::from(number.is_some()) + usize::from(enumeration.is_some()),
                methods: equals + 2 + after + if number.is_some() { NUMBER_METHODS.len() } else { 0 },
            });
            self.object = Some(ObjectIndexes {
                decl,
                equals,
                hash_code: equals + 1,
                comparable,
                number,
                enumeration,
                char_sequence,
                appendable,
            });
        }
    }

    /// Whether the tree `v` compares with `==` or `!=` two values of the program's types of which
    /// neither is a subtype of the other, such as a class and an interface it does not implement:
    /// both are converted to `Object` for that (see `convert_operands`).
    fn compares_unrelated(&self, v: &Value) -> bool {
        match v {
            Value::Object(fields) => {
                let kind = fields.get("kind").and_then(Value::as_str);
                if matches!(kind, Some("EQUAL_TO" | "NOT_EQUAL_TO")) {
                    let operand = |side: &str| {
                        let ty = v[side]["javaType"].as_str().unwrap_or_default();
                        self.type_index(ty.split('<').next().unwrap_or(ty))
                    };
                    if let (Some(a), Some(b)) = (operand("leftOperand"), operand("rightOperand"))
                        && !self.is_subclass(a, b)
                        && !self.is_subclass(b, a)
                    {
                        return true;
                    }
                }
                fields.values().any(|x| self.compares_unrelated(x))
            }
            Value::Array(items) => items.iter().any(|x| self.compares_unrelated(x)),
            _ => false,
        }
    }

    /// `Object`'s abstract `equals` and `hashCode`, appended to the lowered methods, which must be
    /// exactly the declared ones; and the program's overrides of them.
    pub(super) fn object_methods(&self, methods: &mut Vec<Method>) -> Result<(), Error> {
        self.inherited_implementations(methods);
        let Some(at) = self.object else { return Ok(()) };
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
            is_synchronized: false,
            overrides: Vec::new(),
            type_params: Vec::new(),
            jdk_code: false,
            field_update: None,
        };
        for m in methods.iter_mut().filter(|m| !m.is_static && !m.is_ctor) {
            match (m.name.as_str(), &m.params[..], m.ret) {
                ("equals", [p], Type::Boolean) if p.ty == object => m.overrides.push(at.equals),
                ("hashCode", [], Type::Int) => m.overrides.push(at.hash_code),
                _ => {}
            }
        }
        // The `length()`, `charAt(int)`, `subSequence(int, int)` and `getChars(..)` that a class
        // implementing `CharSequence` (directly, or through an interface of the program, as
        // javac's names do through `javax.lang.model.element.Name`) declares or inherits
        // implement the interface's. A
        // `subSequence` may return a subclass of `CharSequence` (`CharBuffer`'s returns a
        // `CharBuffer`), which the dispatcher converts, or a string (javac's names return theirs),
        // which a bridge converts. A class that has no `getChars` gets the interface's default,
        // and then the `getChars` of its subclasses implement the interface's too.
        let mut defaults: Vec<(usize, [Option<usize>; 2])> = Vec::new();
        if let Some(s) = at.char_sequence {
            let mut sequences = Vec::new();
            for t in 0..self.types.len() {
                let above = self.superclass(t)?;
                if self.is_char_sequence(t)?
                    && !above.map(|s| self.is_char_sequence(s)).transpose()?.unwrap_or(false)
                {
                    sequences.push(t);
                }
            }
            for t in sequences {
                let mut implementations = [None; CHAR_SEQUENCE_METHODS.len()];
                for (i, (name, params, ret)) in CHAR_SEQUENCE_METHODS.into_iter().enumerate() {
                    let mut class = Some(t);
                    while let Some(c) = class {
                        let found = methods.iter().take(at.equals).position(|m| {
                            m.class == c
                                && m.name == name
                                && !m.is_static
                                && !m.is_ctor
                                && m.params.len() == params.len()
                                && m.params.iter().zip(params).all(|(p, (_, ty))| p.ty == *ty)
                        });
                        if let Some(m) = found {
                            let returned = methods[m].ret;
                            let fits = match ret {
                                Some(ret) => returned == ret,
                                None => matches!(returned, Type::Class(..) | Type::String),
                            };
                            if !fits && ret.is_none() {
                                return Err(Error {
                                    pos: methods[m].pos,
                                    message: format!(
                                        "a `CharSequence.{name}` that returns a `{returned}` not supported yet"
                                    ),
                                });
                            }
                            if !fits {
                                class = self.superclass(c)?;
                                continue;
                            }
                            if !methods[m].overrides.contains(&(s.methods + i)) {
                                methods[m].overrides.push(s.methods + i);
                            }
                            implementations[i] = Some(m);
                            break;
                        }
                        class = self.superclass(c)?;
                    }
                }
                if implementations[GET_CHARS].is_none() && self.types[t].node.kind() != "INTERFACE" {
                    defaults.push((t, [implementations[0], implementations[1]]));
                }
            }
            self.link_sequence_subclasses(methods, s.methods, at.equals)?;
            let (_, params, _) = CHAR_SEQUENCE_METHODS[GET_CHARS];
            for method in methods.iter_mut().take(at.equals) {
                let below_default =
                    defaults.iter().any(|&(t, _)| method.class != t && self.is_subclass(method.class, t));
                if below_default
                    && method.name == "getChars"
                    && !method.is_static
                    && method.params.iter().map(|p| p.ty).eq(params.iter().map(|&(_, ty)| ty))
                    && !method.overrides.contains(&(s.methods + GET_CHARS))
                {
                    method.overrides.push(s.methods + GET_CHARS);
                }
            }
        }
        // The `intValue()` to `doubleValue()` of a class that extends `Number`, or of a subclass,
        // implement the interface's.
        if let Some(n) = at.number {
            for method in methods.iter_mut().take(at.equals) {
                if method.is_static
                    || method.is_ctor
                    || !method.params.is_empty()
                    || !self.extends_number(method.class)?
                {
                    continue;
                }
                let Some(i) =
                    NUMBER_METHODS.iter().position(|&(name, ret)| method.name == name && method.ret == ret)
                else {
                    continue;
                };
                // A call through `Number` narrows `intValue()` instead (see `number_call`).
                if i < 2 {
                    return Err(Error {
                        pos: method.pos,
                        message: format!("overriding `Number.{}` not supported yet", method.name),
                    });
                }
                if !method.overrides.contains(&(n.methods + i)) {
                    method.overrides.push(n.methods + i);
                }
            }
        }
        methods.push(method("equals", vec![Param { name: "other".to_string(), ty: object }], Type::Boolean));
        methods.push(method("hashCode", Vec::new(), Type::Int));
        if let Some(c) = at.comparable {
            let param = Param { name: "other".to_string(), ty: Type::class(COMPARABLE) };
            methods.push(Method { class: c.decl, ..method("compareTo", vec![param], Type::Int) });
        }
        if let Some(n) = at.number {
            debug_assert_eq!(methods.len(), n.methods);
            for (name, ret) in NUMBER_METHODS {
                methods.push(Method { class: n.decl, ..method(name, Vec::new(), ret) });
            }
        }
        if let Some(s) = at.char_sequence {
            debug_assert_eq!(methods.len(), s.methods);
            for (name, params, ret) in CHAR_SEQUENCE_METHODS {
                let params = params.iter().map(|&(name, ty)| Param { name: name.to_string(), ty }).collect();
                let ret = ret.unwrap_or(Type::class(CHAR_SEQUENCE));
                methods.push(Method { class: s.decl, ..method(name, params, ret) });
            }
            for (t, mut found) in defaults {
                for (i, slot) in found.iter_mut().enumerate() {
                    if slot.is_none() && self.types[t].node.has_flag("ABSTRACT") {
                        *slot = Some(methods.len());
                        methods.push(self.abstract_sequence_method(t, i, s.methods));
                    }
                }
                methods.push(self.default_get_chars(t, found[0], found[1], s.methods));
            }
        }
        self.bridges(methods)
    }

    /// `CharSequence`'s default `getChars(srcBegin, srcEnd, dst, dstBegin)` in class `t`, as the
    /// JDK writes it:
    ///
    /// ```java
    /// Objects.checkFromToIndex(srcBegin, srcEnd, length());
    /// Objects.checkIndex(dstBegin, dst.length - (srcEnd - srcBegin) + 1);
    /// while (srcBegin < srcEnd)
    ///     dst[dstBegin++] = charAt(srcBegin++);
    /// ```
    ///
    /// `length` and `charAt` are the class's methods, when it has them, else the interface's,
    /// whose first method is `first` (see `CHAR_SEQUENCE_METHODS`).
    fn default_get_chars(
        &self,
        t: usize,
        length: Option<usize>,
        char_at: Option<usize>,
        first: usize,
    ) -> Method {
        let pos = self.types[t].node.pos();
        let (_, params, _) = CHAR_SEQUENCE_METHODS[GET_CHARS];
        let var = |name: &str| {
            self.synth(
                ExprKind::Var(name.to_string()),
                pos,
                params.iter().find(|p| p.0 == name).expect("a parameter").1,
            )
        };
        let int = |kind: ExprKind| self.synth(kind, pos, Type::Int);
        let binary = |op: BinOp, l: Expr, r: Expr, ty: Type| {
            self.synth(ExprKind::Binary(op, Box::new(l), Box::new(r)), pos, ty)
        };
        let call = |name: &str, resolved: usize, args: Vec<Expr>, ty: Type| {
            let receiver = Box::new(self.synth(ExprKind::This, pos, self.self_type(t)));
            let kind = ExprKind::MethodCall {
                receiver,
                name: name.to_string(),
                args,
                resolved: Some(resolved),
                is_super: false,
            };
            self.synth(kind, pos, ty)
        };
        let post_increment = |name: &str| {
            let target = Place::Var(name.to_string());
            int(ExprKind::IncDec { target, increment: true, prefix: false })
        };
        let length = call("length", length.unwrap_or(first), Vec::new(), Type::Int);
        let range = vec![var("srcBegin"), var("srcEnd"), length];
        let check_range = self.synth(ExprKind::Builtin(Builtin::CheckFromToIndex, range), pos, Type::Void);
        let count = binary(BinOp::Sub, var("srcEnd"), var("srcBegin"), Type::Int);
        let room = binary(BinOp::Sub, int(ExprKind::Length(Box::new(var("dst")))), count, Type::Int);
        let room = binary(BinOp::Add, room, int(ExprKind::Int(1)), Type::Int);
        let check_index =
            self.synth(ExprKind::Builtin(Builtin::CheckIndex, vec![var("dstBegin"), room]), pos, Type::Void);
        let char_at =
            call("charAt", char_at.unwrap_or(first + 1), vec![post_increment("srcBegin")], Type::Char);
        let target = Place::Index(Box::new(var("dst")), Box::new(post_increment("dstBegin")));
        let store =
            self.synth(ExprKind::Assign { op: None, target, value: Box::new(char_at) }, pos, Type::Char);
        let body = vec![
            Stmt::Expr(check_range),
            Stmt::Expr(check_index),
            Stmt::While {
                cond: binary(BinOp::Lt, var("srcBegin"), var("srcEnd"), Type::Boolean),
                body: Box::new(Stmt::Expr(store)),
            },
        ];
        Method {
            name: "getChars".to_string(),
            params: params.iter().map(|&(name, ty)| Param { name: name.to_string(), ty }).collect(),
            ret: Type::Void,
            body,
            is_main: false,
            is_static: false,
            is_ctor: false,
            class: t,
            pos,
            is_abstract: false,
            is_synchronized: false,
            overrides: vec![first + GET_CHARS],
            type_params: Vec::new(),
            jdk_code: true,
            field_update: None,
        }
    }

    /// The methods that a class inherits from a superclass that does not implement the interface
    /// of the class whose method they implement (javac lists these with the class): they override
    /// that method too, which gives the class its implementation.
    fn inherited_implementations(&self, methods: &mut [Method]) {
        let find = |owner: &str, name: &str, sig: &str| {
            self.methods
                .iter()
                .position(|m| self.types[m.owner].javac == owner && m.name == name && m.sig == sig)
        };
        for t in &self.types {
            let Some(list) = t.node.get("symbol").and_then(|s| s.0["inherits"].as_array()) else { continue };
            for entry in list {
                let (Some(name), Some(owner), Some(sig)) =
                    (entry["name"].as_str(), entry["owner"].as_str(), entry["type"].as_str())
                else {
                    continue;
                };
                let implemented = &entry["implements"];
                let (Some(i_owner), Some(i_sig)) =
                    (implemented["owner"].as_str(), implemented["type"].as_str())
                else {
                    continue;
                };
                if let (Some(m), Some(x)) = (find(owner, name, sig), find(i_owner, name, i_sig))
                    && !methods[m].overrides.contains(&x)
                {
                    methods[m].overrides.push(x);
                }
            }
        }
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
            type_params: Vec::new(),
            outer: None,
            captures: Vec::new(),
            origin: Origin::Member,
            cloneable: false,
        };
        // The program's enums are comparable, and so are its classes that implement `Comparable`.
        if let Some(c) = at.comparable {
            for (t, decl) in types.iter_mut().enumerate() {
                if decl.kind == TypeKind::Enum || self.implements_comparable(t) {
                    decl.implements.push(c.decl);
                }
            }
        }
        // Its classes that implement `CharSequence`, such as the JDK's `CharBuffer`.
        if let Some(s) = at.char_sequence {
            for (t, decl) in types.iter_mut().enumerate() {
                if self.implements_directly(t, "java.lang.CharSequence") {
                    decl.implements.push(s.decl);
                }
            }
        }
        // Its classes that extend `Number`, such as the JDK's `AtomicInteger`.
        if let Some(n) = at.number {
            for (t, decl) in types.iter_mut().enumerate() {
                if self.extends_directly(t, "java.lang.Number") {
                    decl.implements.push(n.decl);
                }
            }
        }
        types.push(decl(OBJECT, "", TypeKind::Interface, Vec::new()));
        let runtime = std::iter::once(at.decl).chain(at.comparable.map(|c| c.decl)).collect::<Vec<_>>();
        // The strings and boxes are `Serializable` and `Constable` (`Serializable s = 2.5;`) where
        // the program names those.
        let serializable = self.type_index("java.io.Serializable");
        let constable = self.type_index("java.lang.constant.Constable");
        for (name, _) in runtime_values() {
            let mut implements = runtime.clone();
            implements.extend(serializable);
            let constant = matches!(
                name,
                "java.lang.String"
                    | "java.lang.Integer"
                    | "java.lang.Long"
                    | "java.lang.Float"
                    | "java.lang.Double"
            );
            implements.extend(constable.filter(|_| constant));
            if let Some(n) = at.number.filter(|_| is_number_box(name)) {
                implements.push(n.decl);
            }
            if let Some(s) = at.char_sequence.filter(|_| name == "java.lang.String") {
                implements.push(s.decl);
            }
            types.push(decl(name, "", TypeKind::Runtime, implements));
        }
        if let Some(c) = at.comparable {
            debug_assert_eq!(types.len(), c.decl);
            types.push(decl(COMPARABLE, "", TypeKind::Interface, vec![at.decl]));
        }
        if let Some(n) = at.number {
            debug_assert_eq!(types.len(), n.decl);
            types.push(decl(NUMBER, "", TypeKind::Interface, vec![at.decl]));
        }
        if let Some(e) = at.enumeration {
            for t in types.iter_mut().filter(|t| t.kind == TypeKind::Enum) {
                t.implements.push(e);
            }
            debug_assert_eq!(types.len(), e);
            let implements = std::iter::once(at.decl).chain(at.comparable.map(|c| c.decl)).collect();
            types.push(decl(ENUM, "", TypeKind::Interface, implements));
        }
        if let Some(s) = at.char_sequence {
            debug_assert_eq!(types.len(), s.decl);
            types.push(decl(CHAR_SEQUENCE, "", TypeKind::Interface, vec![at.decl]));
        }
        // The arrays that become `Object`s, each named after its type (`int[]`), the
        // `StringBuilder`s and the classes.
        for &ty in self.object_arrays.borrow().iter() {
            let name = match ty {
                Type::StringBuilder => STRING_BUILDER.to_string(),
                Type::JavaClass => CLASS_VALUE.to_string(),
                _ => ty.to_string(),
            };
            let mut implements = vec![at.decl];
            if ty == Type::StringBuilder {
                implements.extend(at.char_sequence.map(|s| s.decl));
                implements.extend(at.appendable);
            }
            if ty == Type::JavaClass {
                implements.extend(CLASS_VALUE_INTERFACES.iter().filter_map(|i| self.type_index(i)));
            }
            types.push(decl(&name, "", TypeKind::Runtime, implements));
        }
    }

    /// Notes that arrays of type `ty`, if it is an array type, `StringBuilder`s or classes become
    /// `Object`s.
    pub(super) fn note_object_array(&self, ty: Type) {
        let mut arrays = self.object_arrays.borrow_mut();
        if matches!(ty, Type::Array(_) | Type::StringBuilder | Type::JavaClass) && !arrays.contains(&ty) {
            arrays.push(ty);
        }
    }

    /// Notes that an `Object` is cast to `ty`, if it is an array type: its arrays are `Object`s.
    /// An array of a Rust generic's instantiation has no runtime class.
    pub(super) fn object_array(&self, node: Node, ty: Type) -> Result<(), Error> {
        let mut innermost = ty;
        while let Type::Array(element) = innermost {
            innermost = *element;
        }
        if innermost != ty && matches!(innermost, Type::Class(_, args) if !args.is_empty()) {
            return unsupported(node, "an array of a generic class's instantiation as an `Object`");
        }
        self.note_object_array(ty);
        Ok(())
    }

    /// Whether class `t` of the program says it implements `java.lang.Comparable`.
    /// A box translated for its static members (`Character`) is not: its objects are the
    /// runtime's, which compare themselves.
    pub(super) fn implements_comparable(&self, t: usize) -> bool {
        self.implements_directly(t, "java.lang.Comparable")
            && super::boxing::boxed_type(&self.types[t].javac).is_none()
    }

    /// Whether type `t` of the program names the JDK interface `interface` in its `implements`
    /// clause (an interface's `extends` clause).
    fn implements_directly(&self, t: usize, interface: &str) -> bool {
        self.types[t]
            .node
            .list("implementsClause")
            .any(|c| c.java_type().split('<').next() == Some(interface))
    }

    /// Whether type `t` of the program implements `java.lang.CharSequence`, directly, through an
    /// interface or through a superclass.
    pub(super) fn is_char_sequence(&self, t: usize) -> Result<bool, Error> {
        if self.implements_directly(t, "java.lang.CharSequence") {
            return Ok(true);
        }
        for i in self.interfaces(t)? {
            if self.is_char_sequence(i)? {
                return Ok(true);
            }
        }
        match self.superclass(t)? {
            Some(s) => self.is_char_sequence(s),
            None => Ok(false),
        }
    }

    /// Whether class `t` of the program names the JDK class `class` in its `extends` clause.
    fn extends_directly(&self, t: usize, class: &str) -> bool {
        self.types[t]
            .node
            .get("extendsClause")
            .is_some_and(|c| c.java_type().split('<').next() == Some(class))
    }

    /// Whether class `t` of the program extends `java.lang.Number`, directly or through a
    /// superclass.
    pub(super) fn extends_number(&self, t: usize) -> Result<bool, Error> {
        let mut class = Some(t);
        while let Some(c) = class {
            if self.extends_directly(c, "java.lang.Number") {
                return Ok(true);
            }
            class = self.superclass(c)?;
        }
        Ok(false)
    }

    /// The bridges erasure needs, appended to `methods`: a method that overrides one whose erased
    /// parameter or return types differ (`compareTo(Point)` implementing `Comparable`'s,
    /// `Integer next()` implementing `T next()` of `Source<Integer>`) no longer overrides it; a
    /// method of its class with the overridden signature does, converting the arguments and
    /// calling it, as the bridge javac writes does. A class whose superclass's method already has
    /// that bridge needs none of its own.
    fn bridges(&self, methods: &mut Vec<Method>) -> Result<(), Error> {
        let Some(at) = self.object else { return Ok(()) };
        let original: Vec<Vec<usize>> = methods.iter().map(|m| m.overrides.clone()).collect();
        let mut wanted: Vec<(usize, usize)> = Vec::new();
        for (i, m) in methods.iter().enumerate().take(at.equals) {
            for &o in &original[i] {
                if !Self::same_signature(m, &methods[o]) {
                    wanted.push((i, o));
                }
            }
        }
        // `compareTo(C)` of a class that implements `Comparable`.
        if let Some(c) = at.comparable {
            for t in (0..self.types.len()).filter(|&t| self.implements_comparable(t)) {
                let declared = methods.iter().take(at.equals).position(|m| {
                    m.class == t && m.name == "compareTo" && !m.is_static && m.params.len() == 1
                });
                match declared {
                    Some(i) if !Self::same_signature(&methods[i], &methods[c.compare_to]) => {
                        wanted.push((i, c.compare_to));
                    }
                    Some(i) => methods[i].overrides.push(c.compare_to),
                    None if self.types[t].node.has_flag("ABSTRACT") => {}
                    // Nor does a class whose objects the program never creates (`Year`, whose
                    // static methods alone are reached): its `compareTo` never runs.
                    None if self.reach.is_some_and(|r| !r.instantiated.contains(&self.types[t].javac)) => {}
                    // In a census, the class goes on without the bridge.
                    None => {
                        let pos = self.types[t].node.pos();
                        let message = "a `Comparable` class that inherits its `compareTo` not supported yet";
                        self.reject_at(t, Some(pos), Error { pos, message: message.to_string() })?;
                    }
                }
            }
        }
        // The bridges written, by the method they call and the signature they have.
        let mut written: Vec<(usize, usize)> = Vec::new();
        for &(i, o) in &wanted {
            methods[i].overrides.retain(|&x| x != o);
            // A method that this one overrides with the same signature has the bridge already.
            if original[i].iter().any(|&k| {
                k != o && original[k].contains(&o) && Self::same_signature(&methods[i], &methods[k])
            }) {
                continue;
            }
            // So has a method that overrides two methods with the same erased signature (an
            // abstract class's `R evaluate(..)` and its interface's): one bridge overrides both.
            if let Some(&(_, b)) =
                written.iter().find(|&&(j, b)| j == i && Self::same_signature(&methods[b], &methods[o]))
            {
                methods[b].overrides.push(o);
                continue;
            }
            let bridge = self.bridge(&methods[i], i, &methods[o], o);
            written.push((i, methods.len()));
            methods.push(bridge);
        }
        Ok(())
    }

    /// Whether two methods take and return the same types.
    fn same_signature(a: &Method, b: &Method) -> bool {
        a.ret == b.ret
            && a.params.len() == b.params.len()
            && a.params.iter().zip(&b.params).all(|(p, q)| p.ty == q.ty)
    }

    /// The bridge in the class of method `m` (index `i`) for the method `o` (index `index`) that it
    /// overrides: `o`'s signature, and a body that calls `m` with the arguments converted.
    fn bridge(&self, m: &Method, i: usize, o: &Method, index: usize) -> Method {
        let pos = m.pos;
        let name = &self.types[m.class].name;
        let this_ty = if self.enum_index(name).is_some() { Type::enum_type(name) } else { Type::class(name) };
        let args = o
            .params
            .iter()
            .zip(&m.params)
            .map(|(p, to)| {
                let var = self.synth(ExprKind::Var(p.name.clone()), pos, p.ty);
                if p.ty == to.ty {
                    return var;
                }
                // An array that the bridge casts to another array type (`Integer[]` for the
                // `Object[]` of `T[]`, as `Node.OfInt.copyInto` takes it) is one array that
                // both methods write: the two types meet, and each is an `Object[]`.
                self.meets_object_array(p.ty, to.ty);
                self.synth(ExprKind::Cast(to.ty, Box::new(var)), pos, to.ty)
            })
            .collect();
        self.meets_object_array(m.ret, o.ret);
        let receiver = Box::new(self.synth(ExprKind::This, pos, this_ty));
        let call =
            ExprKind::MethodCall { receiver, name: m.name.clone(), args, resolved: Some(i), is_super: false };
        let call = self.synth(call, pos, m.ret);
        let body = if o.ret == Type::Void {
            vec![Stmt::Expr(call)]
        } else {
            vec![Stmt::Return(Some(self.convert(call, o.ret)), pos)]
        };
        let params = o.params.iter().map(|p| Param { name: p.name.clone(), ty: p.ty }).collect();
        Method {
            name: o.name.clone(),
            params,
            ret: o.ret,
            body,
            is_main: false,
            is_static: false,
            is_ctor: false,
            class: m.class,
            pos,
            is_abstract: false,
            is_synchronized: false,
            overrides: vec![index],
            type_params: Vec::new(),
            jdk_code: false,
            field_update: None,
        }
    }

    /// Whether `ty` is `Object`.
    pub(super) fn is_object(ty: Type) -> bool {
        matches!(ty, Type::Class(name, _) if name == OBJECT)
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

    /// `a.compareTo(b)` on a value of `Comparable` (a type variable bounded by it): `Comparable`'s
    /// `compareTo`, with the argument converted to `Comparable`.
    pub(super) fn comparable_call(
        &self,
        node: Node<'t>,
        receiver: Option<Node<'t>>,
        args: Vec<Expr>,
    ) -> Result<ExprKind, Error> {
        let Some(c) = self.object.and_then(|at| at.comparable) else {
            return unsupported(node, "`Comparable` as a type");
        };
        let Some(receiver) = receiver else { return unsupported(node, "this call") };
        let receiver = self.convert(self.expr(receiver)?, Type::class(COMPARABLE));
        let args = args.into_iter().map(|a| self.convert(a, Type::class(COMPARABLE))).collect();
        let name = "compareTo".to_string();
        Ok(ExprKind::MethodCall {
            receiver: Box::new(receiver),
            name,
            args,
            resolved: Some(c.compare_to),
            is_super: false,
        })
    }

    /// `super.m()` of `Object`'s `hashCode`, `equals`, `toString` or `finalize` in a class that
    /// overrides it: the identity hash of `this`, identity, `getClass().getName() + "@" +
    /// Integer.toHexString(hashCode())`, and nothing.
    fn object_super_call(&self, node: Node<'t>, symbol: &Symbol, args: Vec<Expr>) -> Result<ExprKind, Error> {
        let at = self.object.expect("`object_call` checked `Object`");
        // In a lambda, `super` is the object of the class whose code holds it.
        let this = || match self.lambda_this(node) {
            Some(holder) => holder,
            None => Ok(self.this(node)),
        };
        let pos = node.pos();
        match symbol.name {
            "hashCode" => Ok(ExprKind::Builtin(Builtin::Native(Native::IdentityHash), vec![this()?])),
            "equals" => {
                let this = self.convert(this()?, Type::class(OBJECT));
                let other = args.into_iter().next().expect("`equals` has an argument");
                let other = self.convert(other, Type::class(OBJECT));
                Ok(ExprKind::Binary(BinOp::Eq, Box::new(this), Box::new(other)))
            }
            "toString" => {
                let name = ExprKind::Builtin(Builtin::ClassName { simple: false }, vec![this()?]);
                let name = self.synth(name, pos, Type::String);
                let at_sign = self.synth(ExprKind::Str("@".to_string()), pos, Type::String);
                let hash = ExprKind::MethodCall {
                    receiver: Box::new(this()?),
                    name: "hashCode".to_string(),
                    args: Vec::new(),
                    resolved: Some(at.hash_code),
                    is_super: false,
                };
                let hash = self.synth(hash, pos, Type::Int);
                let hex = ExprKind::Builtin(Builtin::Bits(BitsOp::ToHexString, Type::Int), vec![hash]);
                let hex = self.synth(hex, pos, Type::String);
                let left = self.synth(
                    ExprKind::Binary(BinOp::Add, Box::new(name), Box::new(at_sign)),
                    pos,
                    Type::String,
                );
                Ok(ExprKind::Binary(BinOp::Add, Box::new(left), Box::new(hex)))
            }
            // `Object`'s `finalize` is empty.
            "finalize" => Ok(ExprKind::Builtin(Builtin::Native(Native::Nothing), Vec::new())),
            name => unsupported(node, &format!("`super.{name}()` of `Object`")),
        }
    }

    /// A call of one of `Number`'s `NUMBER_METHODS`, on a value converted to `Number`.
    pub(super) fn number_call(
        &self,
        node: Node<'t>,
        symbol: &Symbol,
        receiver: Option<Node<'t>>,
    ) -> Result<ExprKind, Error> {
        let Some(n) = self.object.and_then(|at| at.number) else {
            return unsupported(node, "`Number` as a type");
        };
        let Some(i) = NUMBER_METHODS.iter().position(|(name, _)| *name == symbol.name) else {
            return unsupported(node, &format!("method `Number.{}`", symbol.name));
        };
        let receiver = match receiver {
            Some(receiver) => self.expr(receiver)?,
            // In a class that extends `Number`.
            None => self.this(node),
        };
        let receiver = self.convert(receiver, Type::class(NUMBER));
        // `byteValue()` and `shortValue()` narrow `intValue()`: `Number`'s do, and each box's
        // gives the same, as Java narrows a `long`, `float` or `double` through an `int`.
        let narrowed = (i < 2).then_some(NUMBER_METHODS[i].1);
        let i = if narrowed.is_some() { 2 } else { i };
        let call = ExprKind::MethodCall {
            receiver: Box::new(receiver),
            name: NUMBER_METHODS[i].0.to_string(),
            args: Vec::new(),
            resolved: Some(n.methods + i),
            is_super: false,
        };
        Ok(match narrowed {
            Some(ty) => ExprKind::Cast(ty, Box::new(self.synth(call, node.pos(), Type::Int))),
            None => call,
        })
    }

    /// A call of a method of `CharSequence` on a value converted to it: one of its
    /// `CHAR_SEQUENCE_METHODS`, or `toString()` as the value's text, or `isEmpty()` as a zero
    /// `length()`.
    pub(super) fn char_sequence_call(
        &self,
        node: Node<'t>,
        symbol: &Symbol,
        receiver: Option<Node<'t>>,
    ) -> Result<ExprKind, Error> {
        let Some(s) = self.object.and_then(|at| at.char_sequence) else {
            return unsupported(node, "`CharSequence` as a type");
        };
        let Some(receiver) = receiver else { return unsupported(node, "this call") };
        let receiver = self.convert(self.expr(receiver)?, Type::class(CHAR_SEQUENCE));
        let pos = receiver.pos;
        let call = |receiver: Expr, name: &str, args: Vec<Expr>| {
            let i = CHAR_SEQUENCE_METHODS
                .iter()
                .position(|(m, ..)| *m == name)
                .expect("a method of `CharSequence`");
            ExprKind::MethodCall {
                receiver: Box::new(receiver),
                name: name.to_string(),
                args,
                resolved: Some(s.methods + i),
                is_super: false,
            }
        };
        match symbol.name {
            "length" => Ok(call(receiver, "length", Vec::new())),
            "charAt" => {
                let index = self.expr(node.list("arguments").next().expect("`charAt` has an index"))?;
                Ok(call(receiver, "charAt", vec![index]))
            }
            "subSequence" | "getChars" => {
                let args = node.list("arguments").map(|a| self.expr(a)).collect::<Result<_, _>>()?;
                Ok(call(receiver, symbol.name, args))
            }
            "toString" => Ok(self.value_of(receiver).kind),
            "isEmpty" => {
                let length = self.synth(call(receiver, "length", Vec::new()), pos, Type::Int);
                let zero = self.synth(ExprKind::Int(0), pos, Type::Int);
                Ok(ExprKind::Binary(BinOp::Eq, Box::new(length), Box::new(zero)))
            }
            name => unsupported(node, &format!("method `CharSequence.{name}`")),
        }
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
                return self.object_super_call(node, symbol, args);
            }
            Some(r) => self.expr(r)?,
            None => self.this(node),
        };
        // An array's methods are `Object`'s, and so are a builder's `equals` and `hashCode`: it
        // becomes an `Object` to call them.
        let receiver = if matches!(receiver.ty, Type::Array(_) | Type::StringBuilder) {
            self.object_array(node, receiver.ty)?;
            self.convert(receiver, Type::class(OBJECT))
        } else {
            receiver
        };
        // A box's own `toString` and `hashCode`, where javac names `Object`'s (`Object::toString`
        // over a stream of `Integer`s).
        if let Type::Boxed(primitive) = receiver.ty
            && let Some(op) = match symbol.name {
                "toString" => Some(BoxedOp::ToString),
                "hashCode" => Some(BoxedOp::HashCode),
                _ => None,
            }
        {
            return Ok(ExprKind::Builtin(Builtin::Boxed(op, *primitive), vec![receiver]));
        }
        if !matches!(receiver.ty, Type::Class(..)) {
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
