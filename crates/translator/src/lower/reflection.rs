//! Reflection by name: `Class.getMethod`, `getField`, `getConstructor`, their `Declared` forms
//! and the lists of them, `Class.newInstance()` and `asSubclass`, over the translation's
//! reflection table.
//!
//! `java.lang.reflect.Method`, `Field` and `Constructor`, with their supertypes
//! `AccessibleObject`, `Executable` and `Member`, are the translator's own classes
//! (`java/Reflection.java`), standard classes like the exceptions (`exceptions.rs`), and the
//! lookups of `Class` are calls of their static methods (`Method.find`, `Field.list`, ...), which
//! search the table. The table is made here, for each program, by its natives: `tableClasses`,
//! `tableInts` and `tableNames` give the members of a part of the slots as data, constant arrays
//! of their classes, numbers and names that `java/Reflection.java` makes the objects from
//! (`table_data`), and `invokePart`, `getPart`, `setPart` and `newInstancePart` are a `switch`
//! over the slots of a part that calls the method, reads or writes the field or creates the object
//! of each, with its arguments unboxed and its result boxed. The slots are in 32 parts (`PARTS`),
//! each with a method of its own in `java/Reflection.java`, and a part's `switch` of more than a
//! few hundred cases is cut into methods of that many (`added_methods::chunked_switch`), so that a
//! big program's table of 100,000 methods makes many functions of a few hundred cases each, which
//! rustc and the analyses take one by one, not one function with all of them. The members are
//! those of the program's classes that reflection can reach: in a whole program, those that
//! reachability found (`program::reach::reflection`, `Reach::reflected`); in a file, those whose
//! names the file's lookups spell out, or any string literal of it for a lookup by a computed name,
//! or every member for `getMethods()` and its relatives. A generic class or method, an inner, local
//! or anonymous class's constructor, and a member whose types a value cannot be boxed to
//! (`Constable`, which a class held as an `Object` is not in a translation) are not in the table,
//! and looking them up throws as for a name that is not. `Class`'s own `isRecord` and
//! `getRecordComponents` are in the table when reachability finds them, and so are the methods
//! of a record's component, the translator's `RecordComponent`, whose native `components` lists
//! the components of the records in `Reach::records`.

use super::*;
use crate::program::{LookupKind, lookup};
use std::collections::HashSet;

const SERVICE_LOADER: &str = "java.util.ServiceLoader";
const METHOD: &str = "java.lang.reflect.Method";
const FIELD: &str = "java.lang.reflect.Field";
const CONSTRUCTOR: &str = "java.lang.reflect.Constructor";
pub(super) const RECORD_COMPONENT: &str = "java.lang.reflect.RecordComponent";
/// The parts that `java/Reflection.java` divides the slots of the table into, by `slot & 31`, for
/// the natives that call, read, write or create its members.
const PARTS: i32 = 32;

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
const IS_RECORD: &str = "(java.lang.Class<?>)boolean";
const RECORD_COMPONENTS: &str = "(java.lang.Class<?>)java.lang.reflect.RecordComponent[]";

/// The members of the translator's `RecordComponent` that a program with records' components
/// has: what answers `Class.isRecord()` and `getRecordComponents()`, the constructor that its
/// native `components` makes them with, and the methods of a component, which reflection may
/// call too (`program::reach::reflection::RECORD_COMPONENT_METHODS`).
const RECORD_MEMBERS: &[(&str, &str)] = &[
    ("isRecord", IS_RECORD),
    ("of", RECORD_COMPONENTS),
    ("<init>", "(java.lang.Class<?>,java.lang.String,java.lang.Class<?>)void"),
    ("getName", "()java.lang.String"),
    ("getType", "()java.lang.Class<?>"),
    ("getGenericType", "()java.lang.reflect.Type"),
    ("getGenericSignature", "()java.lang.String"),
    ("getAccessor", "()java.lang.reflect.Method"),
    ("getDeclaringRecord", "()java.lang.Class<?>"),
    ("toString", "()java.lang.String"),
];

/// `Class`'s own methods that the reflection table has, when reachability finds them
/// (`Reach::reflected`): each with its result and the static method of `RecordComponent` that
/// answers it for the class it is called on.
const CLASS_METHODS: [(&str, &str, &str); 2] = [
    ("isRecord", "boolean", "isRecord"),
    ("getRecordComponents", "java.lang.reflect.RecordComponent[]", "of"),
];

/// The methods that the JDK hides from reflection (`Reflection.registerMethodsToFilter`), by
/// class and name: `sun.misc.Unsafe`'s initialization registers its `getUnsafe`, and the
/// translator's patch of it leaves that out (`java/patches/sun/misc/Unsafe.java`).
const FILTERED_METHODS: &[(&str, &str)] = &[("sun.misc.Unsafe", "getUnsafe")];

/// The constructors of the three classes, which `java/Reflection.java`'s `part` calls.
const MEMBER_CONSTRUCTORS: [(&str, &str); 3] = [
    (
        METHOD,
        "(java.lang.Class<?>,java.lang.Class<?>,java.lang.String,java.lang.Class<?>[],java.lang.Class<?>,java.lang.Class<?>[],int,int)void",
    ),
    (FIELD, "(java.lang.Class<?>,java.lang.Class<?>,java.lang.String,java.lang.Class<?>,int,int)void"),
    (CONSTRUCTOR, "(java.lang.Class<T>,java.lang.Class<?>[],java.lang.Class<?>[],int,int)void"),
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
    ("isRecord", RECORD_COMPONENT, "isRecord", IS_RECORD, None),
    ("getRecordComponents", RECORD_COMPONENT, "of", RECORD_COMPONENTS, None),
];

/// The members of the translator's classes that a call of `Class`'s method `name` uses: the
/// static method that answers it, and the constructor of the objects it finds (see
/// `exceptions::Uses`).
pub(super) fn lookup_members(name: &str) -> Vec<(String, String, String)> {
    let Some(&(_, class, method, sig, _)) = LOOKUPS.iter().find(|l| l.0 == name) else { return Vec::new() };
    if class == RECORD_COMPONENT {
        return record_members();
    }
    let mut out = vec![(class.to_string(), method.to_string(), sig.to_string())];
    if let Some((_, init)) = MEMBER_CONSTRUCTORS.iter().find(|(c, _)| *c == class)
        && !matches!(method, "subclass" | "descriptor")
    {
        out.push((class.to_string(), "<init>".to_string(), init.to_string()));
    }
    out
}

/// The members of the translator's `RecordComponent` that a program that reads records'
/// components has (`RECORD_MEMBERS`).
pub(super) fn record_members() -> Vec<(String, String, String)> {
    RECORD_MEMBERS
        .iter()
        .map(|(name, sig)| (RECORD_COMPONENT.into(), name.to_string(), sig.to_string()))
        .collect()
}

/// The slot that the `Constructor` at `slot` of the table holds: for a constructor of an
/// abstract class, which nothing creates, a negative one, `-1 - slot`.
fn constructor_slot(of_abstract: bool, slot: usize) -> i32 {
    if of_abstract { -1 - slot as i32 } else { slot as i32 }
}

/// What a lookup of a file finds by name, of one kind: `None` for every member.
type Names = Option<HashSet<String>>;

/// What the reflection table of a lowering depends on besides its kind (`Lowering::table`):
/// whether a census's stub is being lowered, whose classes extend nothing they cannot
/// (`Lowering::superclass`); whether the call is a node whose intersection type is an `Object`
/// (`Lowering::ty`); and for fields, the class and method whose type variables are in scope, if
/// any are.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct TableKey {
    kind: LookupKind,
    stubbing: bool,
    joins: bool,
    type_variables: Option<(usize, Option<usize>)>,
}

/// A member of the table, with the class it is looked up in (`None` for `Object`).
#[derive(Clone, Copy)]
pub(super) struct Entry<'t> {
    found_on: Option<usize>,
    member: Member<'t>,
}

#[derive(Clone, Copy)]
enum Member<'t> {
    /// A method or constructor, by index in `Class::methods`.
    Method(usize),
    /// A field: the class that declares it, and its declaration.
    Field(usize, Node<'t>),
    /// One of `Object`'s `hashCode`, `equals` and `toString`, which the runtime's `Object` has
    /// (see `object`), looked up in `Object` itself: a proxy class passes them to its handler
    /// (`program/reach/proxies.rs`).
    Object(&'static str),
    /// One of `Class`'s own methods (`CLASS_METHODS`), looked up in `Class`.
    Class(&'static str),
    /// The static `valueOf(String)` of a boxed class (`BOX_PARSERS`), by its primitive: the boxed
    /// classes are the runtime's, with no methods of the program, and jopt-simple finds the
    /// converter of an option's values (`Integer.class`) so.
    BoxParse(Type),
}

/// The boxed classes whose `valueOf(String)` the table holds, with their primitives: all but
/// `Character`, which has none.
const BOX_PARSERS: [(&str, Type); 7] = [
    ("java.lang.Boolean", Type::Boolean),
    ("java.lang.Byte", Type::Byte),
    ("java.lang.Short", Type::Short),
    ("java.lang.Integer", Type::Int),
    ("java.lang.Long", Type::Long),
    ("java.lang.Float", Type::Float),
    ("java.lang.Double", Type::Double),
];

/// `Object`'s methods in the table: the name, the parameter types and the result, and the bits
/// of `Modifier` (`hashCode` is `native`).
const OBJECT_METHODS: [(&str, &[&str], &str, i32); 3] = [
    ("hashCode", &[], "int", 0x101),
    ("equals", &["java.lang.Object"], "boolean", 1),
    ("toString", &[], "java.lang.String", 1),
];

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
        // `c.asSubclass(of)`, which Java answers with `of.isAssignableFrom(c)`, evaluating `c`
        // and then `of`: `{ Class c = ..; Class o = ..; yield subclass(c, o.isAssignableFrom(c)); }`,
        // with no local for a class literal `of`.
        if name == "asSubclass" {
            let [of] = args[..] else { return unsupported(node, "this `asSubclass`") };
            let local = self.fresh_name("class");
            let var = || self.synth(ExprKind::Var(local.clone()), pos, Type::JavaClass);
            let mut body =
                vec![Stmt::Local { ty: Type::JavaClass, name: local.clone(), init: Some(value), pos }];
            let assignable = if super::classes::is_literal(of) {
                self.literal_assignable_from(node, of, var())?
            } else {
                let of = self.expr(of)?;
                if !matches!(of.ty, Type::JavaClass | Type::Null) {
                    return unsupported(node, "this `asSubclass`");
                }
                let of_local = self.fresh_name("of");
                body.push(Stmt::Local { ty: Type::JavaClass, name: of_local.clone(), init: Some(of), pos });
                let of = self.synth(ExprKind::Var(of_local), pos, Type::JavaClass);
                ExprKind::Builtin(Builtin::ClassMethod(ClassOp::IsAssignableFrom), vec![of, var()])
            };
            let assignable = self.synth(assignable, pos, Type::Boolean);
            let call = ExprKind::Call {
                name: method.to_string(),
                args: vec![var(), assignable],
                resolved: Some(resolved),
            };
            body.push(Stmt::Yield(self.synth(call, pos, Type::JavaClass)));
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
                let rest: Vec<Expr> = lowered.split_off(fixed);
                // A `Class[]` may be an `Object[]` (`covariant`), whose elements convert to `Object`.
                let packed = if rest.is_empty() {
                    ExprKind::NewArray(vec![self.synth(ExprKind::Int(0), pos, Type::Int)])
                } else {
                    ExprKind::ArrayLit(rest.into_iter().map(|e| self.convert(e, array.element())).collect())
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
    /// module's comment), `ServiceLoader` or `TranslatedTypes` (`generic_types`), with its
    /// arguments; `None` for any other call.
    pub(super) fn reflection_native(
        &self,
        node: Node<'t>,
        symbol: &Symbol,
        args: &mut Vec<Expr>,
    ) -> Result<Option<ExprKind>, Error> {
        if symbol.owner == SERVICE_LOADER && symbol.native {
            return Ok(Some(self.service_native(node, symbol, std::mem::take(args))?));
        }
        if symbol.owner == super::generic_types::TRANSLATED_TYPES && symbol.native {
            return Ok(Some(self.generic_types_native(node, symbol, std::mem::take(args))?));
        }
        if symbol.owner == RECORD_COMPONENT && symbol.native {
            return Ok(Some(self.record_components(node, std::mem::take(args))?));
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
            "tableClasses" | "tableInts" | "tableNames" | "invokePart" | "newInstancePart" | "getPart"
            | "setPart" | "genericTypePart" => {
                // The part, which `java/Reflection.java` spells out.
                let mut args = args.into_iter();
                let part = match args.next().map(|a| a.kind) {
                    Some(ExprKind::Int(part)) if (0..PARTS).contains(&part) => part as usize,
                    _ => {
                        return unsupported(
                            node,
                            &format!("`{}.{}` of a part not spelled out", symbol.owner, symbol.name),
                        );
                    }
                };
                if symbol.name.starts_with("table") {
                    return Ok(Some(self.table_data(node, symbol, &entries, part)?));
                }
                // The slot and the other arguments in locals, then a `switch` over the slots of
                // the part.
                let args: Vec<Expr> = args.collect();
                let names: Vec<String> =
                    (0..args.len()).map(|i| self.fresh_name(&format!("reflected_{i}"))).collect();
                let types: Vec<Type> = args.iter().map(|a| a.ty).collect();
                let var = |i: usize| self.synth(ExprKind::Var(names[i].clone()), pos, types[i]);
                let mut cases = Vec::new();
                for (slot, entry) in entries.iter().enumerate().skip(part).step_by(PARTS as usize) {
                    let Some(body) = self.slot_case(node, symbol.name, entry, &var)? else { continue };
                    cases.push((slot as i32, body));
                }
                // A part of a big table is thousands of cases: they go to methods of a few
                // hundred each (`chunked_switch`).
                let null = || vec![Stmt::Yield(self.synth(ExprKind::Null, pos, Type::Null))];
                let params: Vec<(String, Type)> = names.iter().cloned().zip(types.iter().copied()).collect();
                let at = super::added_methods::Chunked {
                    class: self.type_index(symbol.owner).expect("the class of a native is declared"),
                    base: &format!("{}{part}", symbol.name),
                    params: &params,
                    ty: Type::class(OBJECT),
                    pos,
                };
                let step = PARTS * super::added_methods::CASES as i32;
                let switch = self.chunked_int_switch(&at, cases, step, &null);
                let mut body: Vec<Stmt> = args
                    .into_iter()
                    .zip(&names)
                    .map(|(a, name)| Stmt::Local { ty: a.ty, name: name.clone(), init: Some(a), pos })
                    .collect();
                body.push(Stmt::Yield(switch));
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
                        ExprKind::ArrayLit(
                            providers.into_iter().map(|p| self.convert(literal(p), ret.element())).collect(),
                        )
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

    /// `RecordComponent.components(name)`: a `switch` over the binary names of the records whose
    /// components reflection reaches (`Reach::records`; in a file, all of its records), each
    /// case a new array of its components, the fields that are not static, in their order, each
    /// with the class of its declared type; `null` for any other name.
    fn record_components(&self, node: Node<'t>, args: Vec<Expr>) -> Result<ExprKind, Error> {
        let pos = node.pos();
        let [name] = <[Expr; 1]>::try_from(args).map_err(|_| Error {
            pos,
            message: "`RecordComponent.components` with other arguments".into(),
        })?;
        let Some(t) = self.type_index(RECORD_COMPONENT) else {
            return unsupported(node, "`RecordComponent` without its class");
        };
        let init = RECORD_MEMBERS.iter().find(|(n, _)| *n == "<init>").expect("its constructor").1;
        let Some(ctor) = self.template_method(RECORD_COMPONENT, "<init>", init) else {
            return unsupported(node, "`RecordComponent` without its constructor");
        };
        // The array may be an `Object[]` (`covariant`), as gson holds it.
        let element = self.self_type(t);
        let array = self.covariant_array(Type::array(element));
        let literal = |ty: Type| {
            self.synth(ExprKind::Builtin(Builtin::ClassLiteral(ty), Vec::new()), pos, Type::JavaClass)
        };
        let mut cases = Vec::new();
        for r in
            (0..self.types.len()).filter(|&r| self.types[r].node.kind() == "RECORD" && self.reflectable(r))
        {
            if self.reach.is_some_and(|reach| !reach.records.contains(&self.types[r].javac)) {
                continue;
            }
            let Some(binary) = self.binary_name(r) else { continue };
            let mut made = Vec::new();
            for member in
                self.types[r].node.list("members").filter(|m| m.kind() == "VARIABLE" && !m.has_flag("STATIC"))
            {
                let ty = match self.declared_type(member, member.symbol().map_or("", |s| s.ty), r, None)? {
                    Type::Class(name, _) => Type::class(name),
                    ty => ty,
                };
                let component = member.str("name").to_string();
                let args = vec![
                    literal(self.self_type(r)),
                    self.synth(ExprKind::Str(component), pos, Type::String),
                    literal(ty),
                ];
                let new = ExprKind::New { class: t, args, ctor: Some(ctor) };
                made.push(self.convert(self.synth(new, pos, element), array.element()));
            }
            let components = if made.is_empty() {
                ExprKind::NewArray(vec![self.synth(ExprKind::Int(0), pos, Type::Int)])
            } else {
                ExprKind::ArrayLit(made)
            };
            let label = self.synth(ExprKind::Str(binary), pos, Type::String);
            let body = vec![Stmt::Yield(self.synth(components, pos, array))];
            cases.push(Case { labels: vec![CaseLabel::Value(label)], body });
        }
        let null = self.synth(ExprKind::Null, pos, Type::Null);
        cases.push(Case { labels: vec![CaseLabel::Default], body: vec![Stmt::Yield(null)] });
        let local = self.fresh_name("record");
        let var = self.synth(ExprKind::Var(local.clone()), pos, name.ty);
        let switch = self.synth(ExprKind::Switch { selector: Box::new(var), cases }, pos, array);
        let body = vec![Stmt::Local { ty: name.ty, name: local, init: Some(name), pos }, Stmt::Yield(switch)];
        Ok(block(self, pos, body))
    }

    /// The body of the case of one slot of `invokePart`, `newInstancePart`, `getPart` or
    /// `setPart`, each of whose arguments after the part `var` reads by position; `None` when the
    /// member has none (a final field's `setPart`).
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
            (Member::Method(m), "invokePart" | "newInstancePart") => {
                let m = *m;
                let info = &self.methods[m];
                let creates = native == "newInstancePart";
                // A constructor of an abstract class has a negative slot (`table_data`): nothing
                // creates one.
                if creates && self.types[info.owner].node.has_flag("ABSTRACT") {
                    return Ok(None);
                }
                // The receiver (`invokePart` of an instance method) and the `Object[]` of arguments.
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
            (Member::Field(class, decl), "genericTypePart") => {
                let text = decl.symbol().map_or("", |s| s.ty);
                let made = self.field_generic_type(node, text, *class)?;
                Ok(made.map(|made| vec![Stmt::Yield(self.convert(made, object))]))
            }
            (Member::Field(class, decl), "getPart" | "setPart") => {
                let decl = *decl;
                let ty = self.reflected_field_type(decl)?;
                let name = decl.str("name").to_string();
                // An enum's constant, which no `set` changes.
                if super::enums::is_enum_constant(decl) {
                    if native == "setPart" {
                        return Ok(None);
                    }
                    let node = self.types[*class].node;
                    let ordinal = node
                        .list("members")
                        .filter(|m| super::enums::is_enum_constant(*m))
                        .position(|m| m.str("name") == name)
                        .unwrap_or_default();
                    let constant = ExprKind::EnumConstant { class: *class, ordinal };
                    let read = self.synth(constant, pos, self.self_type(*class));
                    return Ok(Some(vec![Stmt::Yield(self.convert(read, object))]));
                }
                let place = if decl.has_flag("STATIC") {
                    Place::Static { class: *class, name }
                } else {
                    let object = self.unboxed(node, var(1), self.self_type(*class))?;
                    Place::Field { object: Box::new(object), name }
                };
                if native == "getPart" {
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
            (Member::Object(name), "invokePart") => {
                let Some(at) = self.object else { return Ok(None) };
                // `Method.invoke` checked the receiver and the arguments before; they are tested
                // again, as for the program's methods.
                let null = || self.synth(ExprKind::Null, pos, Type::Null);
                let not_null = |e: Expr| {
                    self.synth(ExprKind::Binary(BinOp::Ne, Box::new(e), Box::new(null())), pos, Type::Boolean)
                };
                let receiver = Box::new(var(1));
                let (call, cond) = match *name {
                    "hashCode" => {
                        let call = ExprKind::MethodCall {
                            receiver,
                            name: name.to_string(),
                            args: Vec::new(),
                            resolved: Some(at.hash_code),
                            is_super: false,
                        };
                        (self.synth(call, pos, Type::Int), not_null(var(1)))
                    }
                    "equals" => {
                        let other = argument(2, 0, object)?;
                        let call = ExprKind::MethodCall {
                            receiver,
                            name: name.to_string(),
                            args: vec![other],
                            resolved: Some(at.equals),
                            is_super: false,
                        };
                        let both = ExprKind::Binary(
                            BinOp::And,
                            Box::new(not_null(var(1))),
                            Box::new(not_null(var(2))),
                        );
                        (self.synth(call, pos, Type::Boolean), self.synth(both, pos, Type::Boolean))
                    }
                    _ => {
                        let text = ExprKind::Builtin(Builtin::ObjectText, vec![*receiver]);
                        (self.synth(text, pos, Type::String), not_null(var(1)))
                    }
                };
                let then = Stmt::Block(vec![Stmt::Yield(self.convert(call, object))]);
                Ok(Some(vec![Stmt::If { cond, then: Box::new(then), otherwise: None }, Stmt::Yield(null())]))
            }
            (Member::BoxParse(primitive), "invokePart") => {
                // `Method.invoke` checked the argument before: a `String`, or null, which parses
                // as Java's `valueOf(null)` does.
                let text = argument(2, 0, Type::String)?;
                let parse = ExprKind::Builtin(Builtin::Boxed(BoxedOp::Parse, *primitive), vec![text]);
                let call = self.synth(parse, pos, Type::boxed(*primitive));
                Ok(Some(vec![Stmt::Yield(self.convert(call, object))]))
            }
            (Member::Class(name), "invokePart") => {
                let (_, ret, helper) =
                    CLASS_METHODS.iter().find(|(n, ..)| n == name).expect("one of `Class`'s");
                let Some(m) =
                    self.template_method(RECORD_COMPONENT, helper, &format!("(java.lang.Class<?>){ret}"))
                else {
                    return Ok(None);
                };
                let class = self.unboxed(node, var(1), Type::JavaClass)?;
                let call = ExprKind::Call { name: helper.to_string(), args: vec![class], resolved: Some(m) };
                let call = self.synth(call, pos, self.ty(node, ret)?);
                let null = || self.synth(ExprKind::Null, pos, Type::Null);
                let cond = ExprKind::Binary(BinOp::Ne, Box::new(var(1)), Box::new(null()));
                let cond = self.synth(cond, pos, Type::Boolean);
                let then = Stmt::Block(vec![Stmt::Yield(self.convert(call, object))]);
                Ok(Some(vec![Stmt::If { cond, then: Box::new(then), otherwise: None }, Stmt::Yield(null())]))
            }
            _ => Ok(None),
        }
    }

    /// `value`, an `Object`, as a `ty`: unboxed for a primitive, cast for any other type.
    pub(super) fn unboxed(&self, node: Node<'t>, value: Expr, ty: Type) -> Result<Expr, Error> {
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

    /// `tableClasses(part)`, `tableInts(part)` or `tableNames(part)` (the native `symbol`): an
    /// array of the classes, of the numbers or of the names of the members of the table's part
    /// `part`, which `java/Reflection.java`'s `part` makes the objects from. The table is data,
    /// not code that makes each object, so that a big program's part of thousands of members is
    /// a few constant arrays, not a function too big to compile.
    fn table_data(
        &self,
        node: Node<'t>,
        symbol: &Symbol,
        entries: &[Entry<'t>],
        part: usize,
    ) -> Result<ExprKind, Error> {
        let pos = node.pos();
        let (class, which) = (symbol.owner, symbol.name);
        let ret = self.ty(node, symbol.ty.rsplit(')').next().unwrap_or_default())?;
        let mut classes: Vec<Type> = Vec::new();
        let mut class_at: std::collections::HashMap<Type, i32> = Default::default();
        let mut names: Vec<String> = Vec::new();
        let mut name_at: std::collections::HashMap<String, i32> = Default::default();
        let mut index = |ty: Type| {
            *class_at.entry(ty).or_insert_with(|| {
                classes.push(ty);
                classes.len() as i32 - 1
            })
        };
        let mut name = |name: &str| {
            if let Some(&at) = name_at.get(name) {
                return at;
            }
            names.push(name.to_string());
            name_at.insert(name.to_string(), names.len() as i32 - 1);
            names.len() as i32 - 1
        };
        let mut ints = vec![0];
        let list = |ints: &mut Vec<i32>, index: &mut dyn FnMut(Type) -> i32, types: Vec<Type>| {
            ints.push(types.len() as i32);
            ints.extend(types.into_iter().map(index));
        };
        for (slot, entry) in entries.iter().enumerate().skip(part).step_by(PARTS as usize) {
            ints[0] += 1;
            let found_on = match entry.member {
                Member::Class(_) => Type::JavaClass,
                Member::BoxParse(primitive) => Type::boxed(primitive),
                _ => entry.found_on.map_or(Type::class(OBJECT), |t| self.self_type(t)),
            };
            // What `part` reads: for a method the classes that find it and declare it and its
            // result, its name, modifiers and slot, and the lists of its parameters' and
            // exceptions' classes; for a field the same with its type and no lists; for a
            // constructor its class, modifiers, slot and the two lists.
            match &entry.member {
                Member::Method(m) if class == CONSTRUCTOR => {
                    let owner = self.methods[*m].owner;
                    let slot = constructor_slot(self.types[owner].node.has_flag("ABSTRACT"), slot);
                    ints.extend([index(found_on), self.modifiers(*m), slot]);
                    list(&mut ints, &mut index, self.declared_param_classes(*m)?);
                    list(&mut ints, &mut index, self.thrown_types(*m)?);
                }
                Member::Method(m) => {
                    let info = &self.methods[*m];
                    let ret = info.sig.rsplit(')').next().unwrap_or_default();
                    let ret = self.declared_class(node, ret, info.owner, Some(*m))?;
                    let declaring = self.self_type(info.owner);
                    ints.extend([index(found_on), index(declaring), index(ret), name(info.name)]);
                    ints.extend([self.modifiers(*m), slot as i32]);
                    list(&mut ints, &mut index, self.declared_param_classes(*m)?);
                    list(&mut ints, &mut index, self.thrown_types(*m)?);
                }
                Member::Field(declaring, decl) => {
                    let decl = *decl;
                    let ty = decl.symbol().map_or("", |s| s.ty);
                    let ty = self.declared_class(decl, ty, *declaring, None)?;
                    let mut modifiers = flags(decl, self.types[*declaring].node.kind() == "INTERFACE", false);
                    // An enum's constant is public, static and final, and `ENUM`, which
                    // `isEnumConstant` reads.
                    if super::enums::is_enum_constant(decl) {
                        modifiers |= 1 | 8 | 0x10 | 0x4000;
                    }
                    let declaring = self.self_type(*declaring);
                    ints.extend([index(found_on), index(declaring), index(ty), name(decl.str("name"))]);
                    ints.extend([modifiers, slot as i32]);
                }
                Member::Object(method) => {
                    let (_, params, ret, modifiers) =
                        OBJECT_METHODS.iter().find(|(n, ..)| n == method).expect("one of `Object`'s methods");
                    let ty = |name: &str| self.ty(node, name);
                    let types = params.iter().map(|p| ty(p)).collect::<Result<Vec<_>, _>>()?;
                    let ret = ty(ret)?;
                    ints.extend([index(found_on), index(Type::class(OBJECT)), index(ret), name(method)]);
                    ints.extend([*modifiers, slot as i32]);
                    list(&mut ints, &mut index, types);
                    list(&mut ints, &mut index, Vec::new());
                }
                Member::BoxParse(primitive) => {
                    let boxed = index(Type::boxed(*primitive));
                    // Public and static.
                    ints.extend([boxed, boxed, boxed, name("valueOf"), 1 | 8, slot as i32]);
                    list(&mut ints, &mut index, vec![Type::String]);
                    list(&mut ints, &mut index, Vec::new());
                }
                Member::Class(method) => {
                    let (_, ret, _) =
                        CLASS_METHODS.iter().find(|(n, ..)| n == method).expect("one of `Class`'s");
                    // The class of the result as declared, not as the program holds it.
                    let ret = match ret.strip_suffix("[]") {
                        Some(element) => Type::array(Type::class(element)),
                        None => self.ty(node, ret)?,
                    };
                    let class = index(Type::JavaClass);
                    ints.extend([class, class, index(ret), name(method), 1, slot as i32]);
                    list(&mut ints, &mut index, Vec::new());
                    list(&mut ints, &mut index, Vec::new());
                }
            }
        }
        // A `String[]` or `Class[]` may be held as an `Object[]` (`covariant_array`).
        let ret = self.covariant_array(ret);
        let items: Vec<Expr> = match which {
            "tableClasses" => classes
                .into_iter()
                .map(|ty| {
                    let literal = ExprKind::Builtin(Builtin::ClassLiteral(ty), Vec::new());
                    self.convert(self.synth(literal, pos, Type::JavaClass), ret.element())
                })
                .collect(),
            "tableInts" => ints.into_iter().map(|i| self.synth(ExprKind::Int(i), pos, Type::Int)).collect(),
            _ => names
                .into_iter()
                .map(|n| self.convert(self.synth(ExprKind::Str(n), pos, Type::String), ret.element()))
                .collect(),
        };
        if items.is_empty() {
            return Ok(ExprKind::NewArray(vec![self.synth(ExprKind::Int(0), pos, Type::Int)]));
        }
        Ok(ExprKind::ArrayLit(items))
    }

    /// The members of the table of one kind (see the module's comment), each with the slot that
    /// its object holds, its declaration, and a name of the declaration that no other has: the
    /// declaring class's and the member's, with its signature for a method or constructor.
    pub(super) fn table_declarations(
        &self,
        node: Node<'t>,
        kind: LookupKind,
    ) -> Result<Vec<(i32, Node<'t>, String)>, Error> {
        let mut out = Vec::new();
        for (slot, entry) in self.table(node, kind)?.into_iter().enumerate() {
            out.push(match entry.member {
                Member::Method(m) => {
                    let info = &self.methods[m];
                    let owner = &self.types[info.owner];
                    let slot = match kind {
                        LookupKind::Constructor => constructor_slot(owner.node.has_flag("ABSTRACT"), slot),
                        _ => slot as i32,
                    };
                    (slot, info.node, format!("{}#{}{}", owner.javac, info.name, info.sig))
                }
                Member::Field(class, decl) => {
                    (slot as i32, decl, format!("{}#{}", self.types[class].javac, decl.str("name")))
                }
                // Without a declaration of the program, and so without annotations.
                Member::Object(_) | Member::Class(_) | Member::BoxParse(_) => continue,
            });
        }
        Ok(out)
    }

    /// The members of the table of one kind, in the order of their slots (see the module's
    /// comment). Each of the natives' many calls asks for the same table, which is made once per
    /// lowering: what it depends on that changes while lowering goes on is in its key
    /// (`TableKey`), and what making it records (covariant arrays, generics that fail, rejected
    /// classes) its first making has recorded.
    fn table(&self, node: Node<'t>, kind: LookupKind) -> Result<Vec<Entry<'t>>, Error> {
        let key = TableKey {
            kind,
            stubbing: self.stubbing.get(),
            joins: matches!(node.kind(), "CONDITIONAL_EXPRESSION" | "SWITCH_EXPRESSION" | "PARENTHESIZED"),
            // A field's type is read where the table is asked for (`reflected_field_type`), and so
            // are the type variables that it may name.
            type_variables: match kind {
                LookupKind::Field => self.type_variables_in_scope(),
                _ => None,
            },
        };
        if let Some(table) = self.reflection_tables.borrow().get(&key).cloned() {
            return Ok(table);
        }
        let table = self.make_table(node, kind)?;
        self.reflection_tables.borrow_mut().insert(key, table.clone());
        Ok(table)
    }

    /// Makes the table of one kind, for `table`.
    fn make_table(&self, node: Node<'t>, kind: LookupKind) -> Result<Vec<Entry<'t>>, Error> {
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
                // A record's components are listed by `record_components`, not by a table.
                LookupKind::Record => {}
                LookupKind::Constructor => {
                    // Without synthetic parameters: no enclosing instance and no captured locals,
                    // whose types are read where the class is declared.
                    let creatable = matches!(self.types[t].node.kind(), "CLASS" | "RECORD")
                        && self.nesting(t).is_none_or(|n| !n.outer && n.captures.is_empty());
                    if !creatable || !reflected(t, "<init>") {
                        continue;
                    }
                    for m in self.methods_of(t) {
                        if self.methods[m].name == "<init>" && self.callable(node, m)? {
                            out.push(Entry { found_on: Some(t), member: Member::Method(m) });
                        }
                    }
                }
                LookupKind::Method => {
                    let mut seen: HashSet<(&str, &str)> = HashSet::new();
                    for (depth, owner) in self.supertypes_of(t)?.into_iter().enumerate() {
                        for m in self.methods_of(owner) {
                            let info = &self.methods[m];
                            let public = info.node.has_flag("PUBLIC")
                                || self.types[owner].node.kind() == "INTERFACE"
                                    && !info.node.has_flag("PRIVATE");
                            if info.name.starts_with('<')
                                || FILTERED_METHODS.contains(&(self.types[owner].javac.as_str(), info.name))
                                || !reflected(t, info.name)
                                || depth > 0 && !public
                                || !seen.insert((info.name, info.sig))
                                || !self.callable(node, m)?
                            {
                                continue;
                            }
                            out.push(Entry { found_on: Some(t), member: Member::Method(m) });
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
                                || !self.keeps(decl, javac)
                                || !reflected(t, name)
                                || depth > 0 && !(interface || decl.has_flag("PUBLIC"))
                                || !seen.insert(name)
                                || !self.reflected_field_type(decl).is_ok_and(boxable)
                                // A field offset or another handle is no field of the
                                // translation (`handles::is_handle`).
                                || decl.has_flag("STATIC") && self.is_handle(owner, decl)
                            {
                                continue;
                            }
                            out.push(Entry { found_on: Some(t), member: Member::Field(owner, decl) });
                        }
                    }
                }
            }
        }
        // `Object`'s own, which only a whole program's lookups find (`Reach::reflected`).
        if kind == LookupKind::Method
            && self.object.is_some()
            && let Some(reach) = self.reach
        {
            for (name, ..) in OBJECT_METHODS {
                if reach.reflected.contains(&("java.lang.Object".to_string(), name.to_string())) {
                    out.push(Entry { found_on: None, member: Member::Object(name) });
                }
            }
        }
        // The boxed classes' `valueOf(String)`.
        if kind == LookupKind::Method
            && let Some(reach) = self.reach
        {
            for (class, primitive) in BOX_PARSERS {
                if reach.reflected.contains(&(class.to_string(), "valueOf".to_string())) {
                    out.push(Entry { found_on: None, member: Member::BoxParse(primitive) });
                }
            }
        }
        // And `Class`'s own, which the translator's `RecordComponent` answers.
        if kind == LookupKind::Method
            && let Some(reach) = self.reach
        {
            for (name, ret, helper) in CLASS_METHODS {
                let sig = format!("(java.lang.Class<?>){ret}");
                if reach.reflected.contains(&("java.lang.Class".to_string(), name.to_string()))
                    && self.template_method(RECORD_COMPONENT, helper, &sig).is_some()
                {
                    out.push(Entry { found_on: None, member: Member::Class(name) });
                }
            }
        }
        Ok(out)
    }

    /// Whether the class `t` can have members in the table: a class, interface, enum or record
    /// of the program, erased if generic (not a Rust generic, `real_generics`), nor one of the
    /// translator's standard classes.
    fn reflectable(&self, t: usize) -> bool {
        let info = &self.types[t];
        matches!(info.node.kind(), "CLASS" | "INTERFACE" | "ENUM" | "RECORD")
            && !self.is_real(real_generics::Owner::Class(t))
            && (!is_standard(&info.javac) || info.javac == RECORD_COMPONENT)
            && !self.functional.has(&info.javac)
            && self.plain_object != Some(t)
    }

    /// Whether the method or constructor `m` can be called through the table: erased if generic,
    /// with parameters and a result that are boxed to and from `Object`, and exceptions that are
    /// classes of the translation (its `getExceptionTypes`).
    fn callable(&self, node: Node<'t>, m: usize) -> Result<bool, Error> {
        let info = &self.methods[m];
        if self.is_real(real_generics::Owner::Method(m)) || self.thrown_types(m).is_err() {
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
        // The receiver of an instance method of `Constable` is unboxed from an `Object`, which
        // may be a class, to the interface, which the runtime's classes do not implement.
        let receiver = info.node.has_flag("STATIC")
            || info.name == "<init>"
            || !crate::ast::class_only_interface(&self.types[info.owner].javac);
        Ok(!main
            && receiver
            && params.into_iter().all(boxable)
            && ret.is_ok_and(|r| r == Type::Void || boxable(r)))
    }

    /// The type that method `m` returns, read where `m` is declared: the `E` of the anonymous
    /// iterator that `Collections.singletonIterator` returns is the method's type variable.
    pub(super) fn return_type(&self, node: Node<'t>, m: usize) -> Result<Type, Error> {
        let sig = self.methods[m].sig;
        self.declared_type(node, sig.rsplit(')').next().unwrap_or_default(), self.methods[m].owner, Some(m))
    }

    /// The type of the field that `decl` declares.
    fn reflected_field_type(&self, decl: Node<'t>) -> Result<Type, Error> {
        self.ty(decl, decl.symbol().map_or("", |s| s.ty))
    }

    /// The class that reflection gives for the type `name` declared by a member of class `t`
    /// (or by method `m`): an array's own class where the translation holds it as an `Object[]`
    /// (`covariant`), as log4j finds a converter's `newInstance(String[])` by the name of its
    /// parameter's class.
    fn declared_class(&self, node: Node<'t>, name: &str, t: usize, m: Option<usize>) -> Result<Type, Error> {
        let outer = self.scope.replace(Some((t, m)));
        let ty = self.exact_ty(node, name);
        self.scope.set(outer);
        ty
    }

    /// The classes of the parameters of method or constructor `m` as declared (`declared_class`).
    fn declared_param_classes(&self, m: usize) -> Result<Vec<Type>, Error> {
        let owner = self.methods[m].owner;
        let params = self.methods[m].node.list("parameters");
        params.map(|p| self.declared_class(p, p.symbol().map_or("", |s| s.ty), owner, Some(m))).collect()
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
    pub(super) fn supertypes_of(&self, t: usize) -> Result<Vec<usize>, Error> {
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
    pub(super) fn template_method(&self, class: &str, name: &str, sig: &str) -> Option<usize> {
        self.method_named(class, name, sig)
    }
}

/// `switch (0) { default -> { body } }`, a block as an expression.
fn block(lowering: &Lowering, pos: Pos, body: Vec<Stmt>) -> ExprKind {
    ExprKind::Switch {
        selector: Box::new(lowering.synth(ExprKind::Int(0), pos, Type::Int)),
        cases: vec![Case { labels: vec![CaseLabel::Default], body }],
    }
}

/// Whether a value of type `ty` goes to and from an `Object`: not as an interface that only
/// `Class` of the runtime's classes implements in Java (`Constable`), as the `Object` may be a
/// class.
fn boxable(ty: Type) -> bool {
    match ty {
        Type::Class(name, args) => args.is_empty() && !crate::ast::class_only_interface(name),
        Type::Array(element) => boxable(*element),
        Type::Enum(_)
        | Type::String
        | Type::Boxed(_)
        | Type::StringBuilder
        | Type::Boolean
        | Type::JavaClass => true,
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
