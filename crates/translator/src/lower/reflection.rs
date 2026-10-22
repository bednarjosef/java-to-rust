//! Reflection by name: `Class.getMethod`, `getField`, `getConstructor`, their `Declared` forms
//! and the lists of them, `Class.newInstance()` and `asSubclass`, over the translation's
//! reflection table.
//!
//! `java.lang.reflect.Method`, `Field` and `Constructor` are the translator's own classes
//! (`java/Reflection.java`), standard classes like the exceptions (`exceptions.rs`), and the
//! lookups of `Class` are calls of their static methods (`Method.find`, `Field.list`, ...),
//! which search the table. The table is made here, for each program, by its natives: `table0`
//! gives the members, each with its slot, and `invoke0`, `get0`, `set0` and `newInstance0` are a
//! `switch` over the slots that calls the method, reads or writes the field or creates the object
//! of each, with its arguments unboxed and its result boxed. The members are those of the
//! program's classes that reflection can reach: in a whole program, those that reachability
//! found (`program::reach::reflection`, `Reach::reflected`); in a file, those whose names the
//! file's lookups spell out, or any string literal of it for a lookup by a computed name, or
//! every member for `getMethods()` and its relatives. A generic class or method, an inner,
//! local or anonymous class's constructor, and a member whose types a value cannot be boxed to
//! (`Class` itself) are not in the table, and looking them up throws as for a name that is not.

use super::*;
use crate::program::{LookupKind, lookup};
use std::collections::HashSet;

const SERVICE_LOADER: &str = "java.util.ServiceLoader";
const METHOD: &str = "java.lang.reflect.Method";
const FIELD: &str = "java.lang.reflect.Field";
const CONSTRUCTOR: &str = "java.lang.reflect.Constructor";

const FIND_METHOD: &str =
    "(java.lang.Class<?>,java.lang.String,java.lang.Class<?>[],boolean)java.lang.reflect.Method";
const LIST_METHODS: &str = "(java.lang.Class<?>,boolean)java.lang.reflect.Method[]";
const FIND_FIELD: &str = "(java.lang.Class<?>,java.lang.String,boolean)java.lang.reflect.Field";
const LIST_FIELDS: &str = "(java.lang.Class<?>,boolean)java.lang.reflect.Field[]";
const FIND_CONSTRUCTOR: &str =
    "(java.lang.Class<?>,java.lang.Class<?>[],boolean)java.lang.reflect.Constructor<?>";
const LIST_CONSTRUCTORS: &str = "(java.lang.Class<?>,boolean)java.lang.reflect.Constructor<?>[]";
const NEW_INSTANCE_OF: &str = "(java.lang.Class<?>)java.lang.Object";
const SUBCLASS: &str = "(java.lang.Class<?>,boolean)java.lang.Class<?>";

/// The constructors of the three classes, which `table0` calls.
const MEMBER_CONSTRUCTORS: [(&str, &str); 3] = [
    (
        METHOD,
        "(java.lang.Class<?>,java.lang.Class<?>,java.lang.String,java.lang.Class<?>[],java.lang.Class<?>,int,int)void",
    ),
    (FIELD, "(java.lang.Class<?>,java.lang.Class<?>,java.lang.String,java.lang.Class<?>,int,int)void"),
    (CONSTRUCTOR, "(java.lang.Class<T>,java.lang.Class<?>[],int,int)void"),
];

/// The methods of `Class` that the translator's classes answer: the method of `Class`, the
/// static method of the translator's class that answers it, and whether it asks for the
/// declared members (`None`: it takes no such flag).
const LOOKUPS: &[(&str, &str, &str, &str, Option<bool>)] = &[
    ("getMethod", METHOD, "find", FIND_METHOD, Some(false)),
    ("getDeclaredMethod", METHOD, "find", FIND_METHOD, Some(true)),
    ("getMethods", METHOD, "list", LIST_METHODS, Some(false)),
    ("getDeclaredMethods", METHOD, "list", LIST_METHODS, Some(true)),
    ("getField", FIELD, "find", FIND_FIELD, Some(false)),
    ("getDeclaredField", FIELD, "find", FIND_FIELD, Some(true)),
    ("getFields", FIELD, "list", LIST_FIELDS, Some(false)),
    ("getDeclaredFields", FIELD, "list", LIST_FIELDS, Some(true)),
    ("getConstructor", CONSTRUCTOR, "find", FIND_CONSTRUCTOR, Some(false)),
    ("getDeclaredConstructor", CONSTRUCTOR, "find", FIND_CONSTRUCTOR, Some(true)),
    ("getConstructors", CONSTRUCTOR, "list", LIST_CONSTRUCTORS, Some(false)),
    ("getDeclaredConstructors", CONSTRUCTOR, "list", LIST_CONSTRUCTORS, Some(true)),
    ("newInstance", CONSTRUCTOR, "newInstanceOf", NEW_INSTANCE_OF, None),
    ("asSubclass", CONSTRUCTOR, "subclass", SUBCLASS, None),
    // Not reflection's, but answered the same way (`java/Threads.java`).
    ("getClassLoader", "java.lang.ClassLoader", "of", "(java.lang.Class<?>)java.lang.ClassLoader", None),
    ("descriptorString", CONSTRUCTOR, "descriptor", "(java.lang.Class<?>)java.lang.String", None),
];

/// The members of the translator's classes that a call of `Class`'s method `name` uses: the
/// static method that answers it, and the constructor of the objects it finds (see
/// `exceptions::Uses`).
pub(super) fn lookup_members(name: &str) -> Vec<(String, String, String)> {
    let Some(&(_, class, method, sig, _)) = LOOKUPS.iter().find(|l| l.0 == name) else { return Vec::new() };
    let mut out = vec![(class.to_string(), method.to_string(), sig.to_string())];
    if let Some((_, init)) = MEMBER_CONSTRUCTORS.iter().find(|(c, _)| *c == class)
        && !matches!(method, "subclass" | "descriptor")
    {
        out.push((class.to_string(), "<init>".to_string(), init.to_string()));
    }
    out
}

/// What a lookup of a file finds by name, of one kind: `None` for every member.
type Names = Option<HashSet<String>>;

/// A member of the table, with the class it is looked up in.
struct Entry<'t> {
    found_on: usize,
    member: Member<'t>,
}

enum Member<'t> {
    /// A method or constructor, by index in `Class::methods`.
    Method(usize),
    /// A field: the class that declares it, and its declaration.
    Field(usize, Node<'t>),
}

impl<'t> Lowering<'t> {
    /// A lookup of `Class` by name (see the module's comment), or `None` for any other call of
    /// `Class`.
    pub(super) fn reflection_lookup(
        &self,
        node: Node<'t>,
        symbol: &Symbol,
        receiver: Node<'t>,
    ) -> Result<Option<ExprKind>, Error> {
        let Some(&(name, class, method, sig, declared)) = LOOKUPS.iter().find(|l| l.0 == symbol.name) else {
            return Ok(None);
        };
        let Some(resolved) = self.template_method(class, method, sig) else {
            return unsupported(node, &format!("`Class.{name}` here (reflection)"));
        };
        let pos = node.pos();
        let params = self.param_types(resolved)?;
        let args: Vec<Node<'t>> = node.list("arguments").collect();
        let value = self.expr(receiver)?;
        if name == "asSubclass" {
            let [of] = args[..] else { return unsupported(node, "this `asSubclass`") };
            if !super::classes::is_literal(of) {
                return unsupported(node, "`asSubclass` of a class other than a class literal's");
            }
            let local = self.fresh_name("class");
            let var = || self.synth(ExprKind::Var(local.clone()), pos, Type::JavaClass);
            let assignable = self.is_assignable_from(node, of, var())?;
            let assignable = self.synth(assignable, pos, Type::Boolean);
            let call = ExprKind::Call {
                name: method.to_string(),
                args: vec![var(), assignable],
                resolved: Some(resolved),
            };
            let body = vec![
                Stmt::Local { ty: Type::JavaClass, name: local.clone(), init: Some(value), pos },
                Stmt::Yield(self.synth(call, pos, Type::JavaClass)),
            ];
            return Ok(Some(block(self, pos, body)));
        }
        let mut call_args = vec![value];
        let mut lowered: Vec<Expr> = args.iter().map(|a| self.expr(*a)).collect::<Result<_, _>>()?;
        // `getMethod(name, types...)` and `getConstructor(types...)`: the types as an array.
        if symbol.varargs {
            let fixed = usize::from(matches!(name, "getMethod" | "getDeclaredMethod"));
            let array = params[fixed + 1];
            let direct =
                lowered.len() == fixed + 1 && matches!(lowered[fixed].ty, Type::Null | Type::Array(_));
            if !direct {
                // Each type as an element of the parameter's array, an `Object[]` where `Class[]`
                // is covariant.
                let rest: Vec<Expr> =
                    lowered.split_off(fixed).into_iter().map(|a| self.convert(a, array.element())).collect();
                let packed = if rest.is_empty() {
                    ExprKind::NewArray(vec![self.synth(ExprKind::Int(0), pos, Type::Int)])
                } else {
                    ExprKind::ArrayLit(rest)
                };
                lowered.push(self.synth(packed, pos, array));
            }
        }
        call_args.extend(lowered);
        if let Some(declared) = declared {
            call_args.push(self.synth(ExprKind::Bool(declared), pos, Type::Boolean));
        }
        let ret = self.ty(node, sig.rsplit(')').next().unwrap_or_default())?;
        let call = ExprKind::Call { name: method.to_string(), args: call_args, resolved: Some(resolved) };
        let call = self.synth(call, pos, ret);
        let ty = self.ty(node, node.java_type())?;
        if ty == ret || name != "newInstance" {
            return Ok(Some(call.kind));
        }
        Ok(Some(ExprKind::Cast(ty, Box::new(call))))
    }

    /// A call of a native of the translator's `Method`, `Field` or `Constructor` (see the
    /// module's comment), with its arguments; `None` for any other call.
    pub(super) fn reflection_native(
        &self,
        node: Node<'t>,
        symbol: &Symbol,
        args: &mut Vec<Expr>,
    ) -> Result<Option<ExprKind>, Error> {
        if symbol.owner == SERVICE_LOADER && symbol.native {
            return Ok(Some(self.service_native(node, symbol, std::mem::take(args))?));
        }
        if !matches!(symbol.owner, METHOD | FIELD | CONSTRUCTOR) || !symbol.native {
            return Ok(None);
        }
        let args = std::mem::take(args);
        let kind = match symbol.owner {
            METHOD => LookupKind::Method,
            FIELD => LookupKind::Field,
            _ => LookupKind::Constructor,
        };
        let entries = self.table(node, kind)?;
        let pos = node.pos();
        Ok(Some(match symbol.name {
            "table0" => self.table_array(node, symbol.owner, &entries)?,
            "invoke0" | "newInstance0" | "get0" | "set0" => {
                // The arguments in locals, then a `switch` over the slot.
                let names: Vec<String> =
                    (0..args.len()).map(|i| self.fresh_name(&format!("reflected_{i}"))).collect();
                let types: Vec<Type> = args.iter().map(|a| a.ty).collect();
                let var = |i: usize| self.synth(ExprKind::Var(names[i].clone()), pos, types[i]);
                let mut cases = Vec::new();
                for (slot, entry) in entries.iter().enumerate() {
                    let Some(body) = self.slot_case(node, symbol.name, entry, &var)? else { continue };
                    let label = self.synth(ExprKind::Int(slot as i32), pos, Type::Int);
                    cases.push(Case { labels: vec![CaseLabel::Value(label)], body });
                }
                let null = self.synth(ExprKind::Null, pos, Type::Null);
                cases.push(Case { labels: vec![CaseLabel::Default], body: vec![Stmt::Yield(null)] });
                let object = Type::class(OBJECT);
                let switch = ExprKind::Switch { selector: Box::new(var(0)), cases };
                let mut body: Vec<Stmt> = args
                    .into_iter()
                    .zip(&names)
                    .map(|(a, name)| Stmt::Local { ty: a.ty, name: name.clone(), init: Some(a), pos })
                    .collect();
                body.push(Stmt::Yield(self.synth(switch, pos, object)));
                block(self, pos, body)
            }
            _ => return unsupported(node, &format!("`{}.{}`", symbol.owner, symbol.name)),
        }))
    }

    /// A native of the translator's `ServiceLoader` (`java/patches/java/util/ServiceLoader.java`)
    /// over the providers that reachability found (`Reach::services`): `providers0(service)`, a
    /// `switch` over the service's binary name that gives the class literals of its providers,
    /// and `create0(type)`, a chain over the providers that makes one with its constructor
    /// without parameters, or a module's with its public static `provider()` if it has one.
    fn service_native(&self, node: Node<'t>, symbol: &Symbol, args: Vec<Expr>) -> Result<ExprKind, Error> {
        let pos = node.pos();
        let Some(reach) = self.reach else {
            return unsupported(node, "`ServiceLoader` outside a whole program");
        };
        let [arg] = <[Expr; 1]>::try_from(args).map_err(|_| Error {
            pos,
            message: format!("`ServiceLoader.{}` with other arguments", symbol.name),
        })?;
        let literal = |t: usize| {
            self.synth(
                ExprKind::Builtin(Builtin::ClassLiteral(self.self_type(t)), Vec::new()),
                pos,
                Type::JavaClass,
            )
        };
        let ret = self.ty(node, symbol.ty.rsplit(')').next().unwrap_or_default())?;
        let local = self.fresh_name("service");
        let var = || self.synth(ExprKind::Var(local.clone()), pos, arg.ty);
        let found = match symbol.name {
            "providers0" => {
                let mut cases = Vec::new();
                for (service, providers) in &reach.services {
                    let providers: Vec<usize> =
                        providers.iter().filter_map(|(p, _)| self.type_index(p)).collect();
                    let name =
                        self.type_index(service).and_then(|t| self.binary_name(t)).unwrap_or(service.clone());
                    let label = self.synth(ExprKind::Str(name), pos, Type::String);
                    let classes = if providers.is_empty() {
                        ExprKind::NewArray(vec![self.synth(ExprKind::Int(0), pos, Type::Int)])
                    } else {
                        // `Object`s where `Class[]` is covariant.
                        let element = |t| self.convert(literal(t), ret.element());
                        ExprKind::ArrayLit(providers.into_iter().map(element).collect())
                    };
                    let body = vec![Stmt::Yield(self.synth(classes, pos, ret))];
                    cases.push(Case { labels: vec![CaseLabel::Value(label)], body });
                }
                let none = ExprKind::NewArray(vec![self.synth(ExprKind::Int(0), pos, Type::Int)]);
                let body = vec![Stmt::Yield(self.synth(none, pos, ret))];
                cases.push(Case { labels: vec![CaseLabel::Default], body });
                self.synth(ExprKind::Switch { selector: Box::new(var()), cases }, pos, ret)
            }
            "create0" => {
                let object = Type::class(OBJECT);
                let mut chain = self.synth(ExprKind::Null, pos, Type::Null);
                let providers: Vec<(usize, bool)> = reach
                    .services
                    .values()
                    .flatten()
                    .filter_map(|(p, modular)| Some((self.type_index(p)?, *modular)))
                    .collect();
                for (t, modular) in providers.into_iter().rev() {
                    let by_method = self.methods.iter().position(|m| {
                        modular
                            && m.owner == t
                            && m.name == "provider"
                            && m.sig.starts_with("()")
                            && m.node.has_flag("STATIC")
                    });
                    let made = match by_method {
                        Some(m) => {
                            let ret = self.return_type(node, m)?;
                            self.synth(
                                ExprKind::Call {
                                    name: "provider".to_string(),
                                    args: Vec::new(),
                                    resolved: Some(m),
                                },
                                pos,
                                ret,
                            )
                        }
                        None => {
                            let Some(ctor) = self
                                .methods
                                .iter()
                                .position(|m| m.owner == t && m.name == "<init>" && m.sig == "()void")
                            else {
                                continue;
                            };
                            self.synth(
                                ExprKind::New { class: t, args: Vec::new(), ctor: Some(ctor) },
                                pos,
                                self.self_type(t),
                            )
                        }
                    };
                    let equals =
                        ExprKind::Builtin(Builtin::ClassMethod(ClassOp::Equals), vec![var(), literal(t)]);
                    let test = self.synth(equals, pos, Type::Boolean);
                    let made = self.convert(made, object);
                    chain = self.synth(
                        ExprKind::Conditional(Box::new(test), Box::new(made), Box::new(chain)),
                        pos,
                        object,
                    );
                }
                chain
            }
            _ => return unsupported(node, &format!("`ServiceLoader.{}`", symbol.name)),
        };
        let body =
            vec![Stmt::Local { ty: arg.ty, name: local.clone(), init: Some(arg), pos }, Stmt::Yield(found)];
        Ok(block(self, pos, body))
    }

    /// The body of the case of one slot of `invoke0`, `newInstance0`, `get0` or `set0`, each of
    /// whose arguments `var` reads by position; `None` when the member has none (a final field's
    /// `set0`).
    fn slot_case(
        &self,
        node: Node<'t>,
        native: &str,
        entry: &Entry<'t>,
        var: &dyn Fn(usize) -> Expr,
    ) -> Result<Option<Vec<Stmt>>, Error> {
        let pos = node.pos();
        let object = Type::class(OBJECT);
        // The argument at `index` of the `Object[]` argument `array`, as a `ty`.
        let argument = |array: usize, index: usize, ty: Type| -> Result<Expr, Error> {
            let at = self.synth(ExprKind::Int(index as i32), pos, Type::Int);
            let value = self.synth(ExprKind::Index(Box::new(var(array)), Box::new(at)), pos, object);
            self.unboxed(node, value, ty)
        };
        match (&entry.member, native) {
            (Member::Method(m), "invoke0" | "newInstance0") => {
                let m = *m;
                let info = &self.methods[m];
                let creates = native == "newInstance0";
                // A constructor of an abstract class has slot -1 (`table0`): nothing creates one.
                if creates && self.types[info.owner].node.has_flag("ABSTRACT") {
                    return Ok(None);
                }
                // The receiver (`invoke0` of an instance method) and the `Object[]` of arguments.
                let (receiver, array) = if creates { (None, 1) } else { (Some(1), 2) };
                let receiver = receiver.filter(|_| !info.node.has_flag("STATIC"));
                let params = self.param_types(m)?;
                // `Method.invoke` and `Constructor.newInstance` checked the receiver and the
                // arguments before (`Constructor.arguments`): a primitive's is not null. Each is
                // tested again, `if (r != null && a0 != null ..)`, so that the call runs where
                // they are known not null, and only an exception of the member itself reaches the
                // template's `catch`, which wraps it in an `InvocationTargetException`.
                let mut body = Vec::new();
                let mut tested: Vec<Expr> = Vec::new();
                let mut args = Vec::new();
                if !params.is_empty() {
                    tested.push(var(array));
                }
                for (i, ty) in params.iter().enumerate() {
                    if Self::is_object(*ty) || !(ty.is_numeric() || *ty == Type::Boolean) {
                        args.push(argument(array, i, *ty)?);
                        continue;
                    }
                    let name = self.fresh_name(&format!("argument_{i}"));
                    let at = self.synth(ExprKind::Int(i as i32), pos, Type::Int);
                    let element =
                        self.synth(ExprKind::Index(Box::new(var(array)), Box::new(at)), pos, object);
                    body.push(Stmt::Local { ty: object, name: name.clone(), init: Some(element), pos });
                    tested.push(self.synth(ExprKind::Var(name.clone()), pos, object));
                    let local = self.synth(ExprKind::Var(name), pos, object);
                    args.push(self.unboxed(node, local, *ty)?);
                }
                let call = if creates {
                    let class = info.owner;
                    let created = ExprKind::New { class, args, ctor: Some(m) };
                    self.convert(self.synth(created, pos, self.self_type(class)), object)
                } else {
                    let ret = self.return_type(node, m)?;
                    let name = info.name.to_string();
                    let call = match receiver {
                        None => {
                            let mut args = args;
                            args.extend(self.caller_arg(node, Some(m))?);
                            ExprKind::Call { name, args, resolved: Some(m) }
                        }
                        Some(r) => {
                            tested.push(var(r));
                            let receiver = self.unboxed(node, var(r), self.self_type(info.owner))?;
                            let receiver = Box::new(receiver);
                            ExprKind::MethodCall { receiver, name, args, resolved: Some(m), is_super: false }
                        }
                    };
                    let call = self.synth(call, pos, ret);
                    if ret == Type::Void {
                        body.push(Stmt::Expr(call));
                        self.synth(ExprKind::Null, pos, Type::Null)
                    } else {
                        self.convert(call, object)
                    }
                };
                let null = || self.synth(ExprKind::Null, pos, Type::Null);
                let not_null = |e: Expr| {
                    self.synth(ExprKind::Binary(BinOp::Ne, Box::new(e), Box::new(null())), pos, Type::Boolean)
                };
                let cond = tested.into_iter().map(not_null).reduce(|a, b| {
                    self.synth(ExprKind::Binary(BinOp::And, Box::new(a), Box::new(b)), pos, Type::Boolean)
                });
                // The call after the locals, under the test.
                let at = body.iter().position(|s| !matches!(s, Stmt::Local { .. })).unwrap_or(body.len());
                let mut guarded: Vec<Stmt> = body.split_off(at);
                guarded.push(Stmt::Yield(call));
                match cond {
                    None => body.extend(guarded),
                    Some(cond) => {
                        body.push(Stmt::If { cond, then: Box::new(Stmt::Block(guarded)), otherwise: None });
                        body.push(Stmt::Yield(null()));
                    }
                }
                Ok(Some(body))
            }
            (Member::Field(class, decl), "get0" | "set0") => {
                let decl = *decl;
                let ty = self.reflected_field_type(decl)?;
                let name = decl.str("name").to_string();
                let place = if decl.has_flag("STATIC") {
                    Place::Static { class: *class, name }
                } else {
                    let object = self.unboxed(node, var(1), self.self_type(*class))?;
                    Place::Field { object: Box::new(object), name }
                };
                if native == "get0" {
                    let read = self.synth(super::objects::read(place), pos, ty);
                    return Ok(Some(vec![Stmt::Yield(self.convert(read, object))]));
                }
                if decl.has_flag("FINAL") || self.types[*class].node.kind() == "INTERFACE" {
                    return Ok(None);
                }
                let value = self.unboxed(node, var(2), ty)?;
                let assign = ExprKind::Assign { op: None, target: place, value: Box::new(value) };
                Ok(Some(vec![Stmt::Expr(self.synth(assign, pos, ty)), Stmt::Yield(var(2))]))
            }
            _ => Ok(None),
        }
    }

    /// `value`, an `Object`, as a `ty`: unboxed for a primitive, cast for any other type.
    fn unboxed(&self, node: Node<'t>, value: Expr, ty: Type) -> Result<Expr, Error> {
        let pos = value.pos;
        if Self::is_object(ty) {
            return Ok(value);
        }
        if ty.is_numeric() || ty == Type::Boolean {
            let boxed = self.synth(ExprKind::Cast(Type::boxed(ty), Box::new(value)), pos, Type::boxed(ty));
            return Ok(self.synth(ExprKind::Cast(ty, Box::new(boxed)), pos, ty));
        }
        if matches!(ty, Type::Array(_)) {
            self.object_array(node, ty)?;
        }
        Ok(self.synth(ExprKind::Cast(ty, Box::new(value)), pos, ty))
    }

    /// `table0()`: a new array of the table's members, each made with its slot.
    fn table_array(&self, node: Node<'t>, class: &str, entries: &[Entry<'t>]) -> Result<ExprKind, Error> {
        let pos = node.pos();
        let t = self.type_index(class).expect("the class of a native is declared");
        let (_, init) = MEMBER_CONSTRUCTORS.iter().find(|(c, _)| *c == class).expect("each class has one");
        let Some(ctor) = self.template_method(class, "<init>", init) else {
            return unsupported(node, &format!("`{class}` without its constructor"));
        };
        let params = self.param_types(ctor)?;
        let literal = |ty: Type| {
            self.synth(ExprKind::Builtin(Builtin::ClassLiteral(ty), Vec::new()), pos, Type::JavaClass)
        };
        let int = |i: i32| self.synth(ExprKind::Int(i), pos, Type::Int);
        let classes = |types: Vec<Type>, at: usize| {
            let kind = if types.is_empty() {
                ExprKind::NewArray(vec![int(0)])
            } else {
                // `Object`s where `Class[]` is covariant.
                ExprKind::ArrayLit(
                    types.into_iter().map(|t| self.convert(literal(t), params[at].element())).collect(),
                )
            };
            self.synth(kind, pos, params[at])
        };
        let mut made = Vec::new();
        for (slot, entry) in entries.iter().enumerate() {
            let found_on = literal(self.self_type(entry.found_on));
            let args = match &entry.member {
                Member::Method(m) if class == CONSTRUCTOR => {
                    let owner = self.methods[*m].owner;
                    let slot = if self.types[owner].node.has_flag("ABSTRACT") { -1 } else { slot as i32 };
                    vec![found_on, classes(self.param_types(*m)?, 1), int(self.modifiers(*m)), int(slot)]
                }
                Member::Method(m) => {
                    let declaring = literal(self.self_type(self.methods[*m].owner));
                    let name =
                        self.synth(ExprKind::Str(self.methods[*m].name.to_string()), pos, Type::String);
                    let ret = literal(self.return_type(node, *m)?);
                    let types = classes(self.param_types(*m)?, 3);
                    vec![found_on, declaring, name, types, ret, int(self.modifiers(*m)), int(slot as i32)]
                }
                Member::Field(declaring, decl) => {
                    let decl = *decl;
                    let name = self.synth(ExprKind::Str(decl.str("name").to_string()), pos, Type::String);
                    let ty = literal(self.reflected_field_type(decl)?);
                    let modifiers = flags(decl, self.types[*declaring].node.kind() == "INTERFACE", false);
                    let declaring = literal(self.self_type(*declaring));
                    vec![found_on, declaring, name, ty, int(modifiers), int(slot as i32)]
                }
            };
            let new = ExprKind::New { class: t, args, ctor: Some(ctor) };
            made.push(self.synth(new, pos, self.self_type(t)));
        }
        if made.is_empty() {
            return Ok(ExprKind::NewArray(vec![int(0)]));
        }
        Ok(ExprKind::ArrayLit(made))
    }

    /// The members of the table of one kind, in the order of their slots (see the module's
    /// comment).
    fn table(&self, node: Node<'t>, kind: LookupKind) -> Result<Vec<Entry<'t>>, Error> {
        let names = match self.reach {
            Some(_) => None,
            None => Some(self.file_names(kind)),
        };
        let reflected = |t: usize, name: &str| match (self.reach, &names) {
            (Some(reach), _) => reach.reflected.contains(&(self.types[t].javac.clone(), name.to_string())),
            (None, Some(None)) => true,
            (None, Some(Some(names))) => names.contains(name),
            (None, None) => false,
        };
        let mut out = Vec::new();
        for t in (0..self.types.len()).filter(|&t| self.reflectable(t)) {
            match kind {
                LookupKind::Constructor => {
                    // Without synthetic parameters: no enclosing instance and no captured locals,
                    // whose types are read where the class is declared.
                    let creatable = matches!(self.types[t].node.kind(), "CLASS" | "RECORD")
                        && self.nesting(t).is_none_or(|n| !n.outer && n.captures.is_empty());
                    if !creatable || !reflected(t, "<init>") {
                        continue;
                    }
                    for m in (0..self.methods.len()).filter(|&m| self.methods[m].owner == t) {
                        if self.methods[m].name == "<init>" && self.callable(node, m)? {
                            out.push(Entry { found_on: t, member: Member::Method(m) });
                        }
                    }
                }
                LookupKind::Method => {
                    let mut seen: HashSet<(&str, &str)> = HashSet::new();
                    for (depth, owner) in self.supertypes_of(t)?.into_iter().enumerate() {
                        for m in (0..self.methods.len()).filter(|&m| self.methods[m].owner == owner) {
                            let info = &self.methods[m];
                            let public = info.node.has_flag("PUBLIC")
                                || self.types[owner].node.kind() == "INTERFACE"
                                    && !info.node.has_flag("PRIVATE");
                            if info.name.starts_with('<')
                                || !reflected(t, info.name)
                                || depth > 0 && !public
                                || !seen.insert((info.name, info.sig))
                                || !self.callable(node, m)?
                            {
                                continue;
                            }
                            out.push(Entry { found_on: t, member: Member::Method(m) });
                        }
                    }
                }
                LookupKind::Field => {
                    let mut seen: HashSet<&str> = HashSet::new();
                    for (depth, owner) in self.supertypes_of(t)?.into_iter().enumerate() {
                        let javac = &self.types[owner].javac;
                        let interface = self.types[owner].node.kind() == "INTERFACE";
                        for decl in self.types[owner].node.list("members") {
                            let name = decl.str("name");
                            if decl.kind() != "VARIABLE"
                                || super::enums::is_enum_constant(decl)
                                || !self.keeps(decl, javac)
                                || !reflected(t, name)
                                || depth > 0 && !(interface || decl.has_flag("PUBLIC"))
                                || !seen.insert(name)
                                || !self.reflected_field_type(decl).is_ok_and(boxable)
                            {
                                continue;
                            }
                            out.push(Entry { found_on: t, member: Member::Field(owner, decl) });
                        }
                    }
                }
            }
        }
        Ok(out)
    }

    /// Whether the class `t` can have members in the table: a class, interface, enum or record
    /// of the program, not generic, nor one of the translator's standard classes.
    fn reflectable(&self, t: usize) -> bool {
        let info = &self.types[t];
        matches!(info.node.kind(), "CLASS" | "INTERFACE" | "ENUM" | "RECORD")
            && info.node.list("typeParameters").next().is_none()
            && !is_standard(&info.javac)
            && !self.functional.has(&info.javac)
            && self.plain_object != Some(t)
    }

    /// Whether the method or constructor `m` can be called through the table: not generic, and
    /// with parameters and a result that are boxed to and from `Object`.
    fn callable(&self, node: Node<'t>, m: usize) -> Result<bool, Error> {
        let info = &self.methods[m];
        if info.node.list("typeParameters").next().is_some() {
            return Ok(false);
        }
        let Ok(params) = self.param_types(m) else { return Ok(false) };
        let ret = if info.name == "<init>" { Ok(Type::Void) } else { self.return_type(node, m) };
        // The program's `main` takes its arguments from the runtime, not as a parameter
        // (`method_in_scope`), so the table has no call of it to make.
        let declared: Vec<Node> = info.node.list("parameters").collect();
        let main = info.name == "main"
            && info.node.has_flag("STATIC")
            && matches!(ret, Ok(Type::Void))
            && matches!(&declared[..], [p] if p.symbol().is_some_and(|s| s.ty == "java.lang.String[]"))
            && self.entry.is_none_or(|entry| entry == info.owner);
        Ok(!main && params.into_iter().all(boxable) && ret.is_ok_and(|r| r == Type::Void || boxable(r)))
    }

    /// The type that method `m` returns, read where `m` is declared: the `E` of the anonymous
    /// iterator that `Collections.singletonIterator` returns is the method's type variable.
    fn return_type(&self, node: Node<'t>, m: usize) -> Result<Type, Error> {
        let sig = self.methods[m].sig;
        self.declared_type(node, sig.rsplit(')').next().unwrap_or_default(), self.methods[m].owner, Some(m))
    }

    /// The type of the field that `decl` declares.
    fn reflected_field_type(&self, decl: Node<'t>) -> Result<Type, Error> {
        self.ty(decl, decl.symbol().map_or("", |s| s.ty))
    }

    /// The bits of `Modifier` of method or constructor `m`.
    fn modifiers(&self, m: usize) -> i32 {
        let info = &self.methods[m];
        let interface = self.types[info.owner].node.kind() == "INTERFACE";
        let varargs = info.node.symbol().is_some_and(|s| s.varargs);
        let mut bits = flags(info.node, false, varargs);
        if interface && !info.node.has_flag("PRIVATE") {
            bits |= 1;
            if info.node.get("body").is_none() {
                bits |= 0x400;
            }
        }
        bits
    }

    /// The class `t` and its supertypes of the program, directly or not, superclasses first.
    fn supertypes_of(&self, t: usize) -> Result<Vec<usize>, Error> {
        let mut out = Vec::new();
        let mut at = Some(t);
        while let Some(c) = at {
            out.push(c);
            at = self.superclass(c)?;
        }
        let mut i = 0;
        while i < out.len() {
            for interface in self.interfaces(out[i])? {
                if !out.contains(&interface) {
                    out.push(interface);
                }
            }
            i += 1;
        }
        Ok(out)
    }

    /// The names that the lookups of this kind in a file can find (see the module's comment).
    fn file_names(&self, kind: LookupKind) -> Names {
        let mut names = HashSet::new();
        let mut strings = HashSet::new();
        let mut computed = false;
        for info in self.types.iter().filter(|t| t.nested.is_none() && !is_standard(&t.javac)) {
            let mut found = |v: &Value| {
                if let Some(l) = lookup(v).filter(|l| l.kind == kind) {
                    match l.name {
                        _ if l.every => computed = true,
                        Some(name) => _ = names.insert(name),
                        None => computed = true,
                    }
                }
                if v["kind"] == "STRING_LITERAL"
                    && let Some(s) = v["value"].as_str()
                {
                    strings.insert(s.to_string());
                }
            };
            walk_values(info.node.0, &mut found);
        }
        if kind == LookupKind::Constructor || computed && strings.is_empty() {
            return None;
        }
        if computed {
            names.extend(strings);
        }
        Some(names)
    }

    /// The method `name` with signature `sig` of the translator's class `class`, which the
    /// program has when it uses it.
    fn template_method(&self, class: &str, name: &str, sig: &str) -> Option<usize> {
        self.methods.iter().position(|m| self.types[m.owner].javac == class && m.name == name && m.sig == sig)
    }
}

/// `switch (0) { default -> { body } }`, a block as an expression.
fn block(lowering: &Lowering, pos: Pos, body: Vec<Stmt>) -> ExprKind {
    ExprKind::Switch {
        selector: Box::new(lowering.synth(ExprKind::Int(0), pos, Type::Int)),
        cases: vec![Case { labels: vec![CaseLabel::Default], body }],
    }
}

/// Whether a value of type `ty` goes to and from an `Object`.
fn boxable(ty: Type) -> bool {
    match ty {
        Type::Class(_, args) => args.is_empty(),
        Type::Array(element) => boxable(*element),
        Type::Enum(_) | Type::String | Type::Boxed(_) | Type::StringBuilder | Type::Boolean => true,
        _ => ty.is_numeric(),
    }
}

/// The bits of `Modifier` of a declaration, with those that an interface's field has without
/// saying so, and the bit of a variable arity.
fn flags(node: Node, interface_field: bool, varargs: bool) -> i32 {
    const BITS: [(&str, i32); 11] = [
        ("PUBLIC", 1),
        ("PRIVATE", 2),
        ("PROTECTED", 4),
        ("STATIC", 8),
        ("FINAL", 0x10),
        ("SYNCHRONIZED", 0x20),
        ("VOLATILE", 0x40),
        ("TRANSIENT", 0x80),
        ("NATIVE", 0x100),
        ("ABSTRACT", 0x400),
        ("STRICTFP", 0x800),
    ];
    let mut bits = BITS.iter().filter(|(flag, _)| node.has_flag(flag)).map(|(_, bit)| bit).sum::<i32>();
    if interface_field {
        bits |= 1 | 8 | 0x10;
    }
    if varargs {
        bits |= 0x80;
    }
    bits
}

/// Calls `f` on `v` and every value in it.
fn walk_values(v: &Value, f: &mut dyn FnMut(&Value)) {
    f(v);
    match v {
        Value::Array(items) => items.iter().for_each(|i| walk_values(i, f)),
        Value::Object(map) => map.values().for_each(|i| walk_values(i, f)),
        _ => {}
    }
}
