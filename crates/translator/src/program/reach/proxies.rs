//! The proxy classes that `Proxy.newProxyInstance` and `Proxy.getProxyClass` of reachable code
//! can make, each a class of the program that the translator writes as Java source, as the JDK's
//! `ProxyGenerator` writes a class file.
//!
//! The JVM defines a proxy class at run time for a list of interfaces. The translation has one
//! for each list that a creation of reachable code can pass, chosen as the providers that
//! `ServiceLoader` loads are (`reflection.rs`): the creation's array of interfaces is an array
//! creation, and each element a class literal, a `Class` parameter of a method that only its
//! reachable calls give values (followed to what they pass), or, alone in the array, any other
//! `Class` value, which can be any public interface whose class literal reachable code has or
//! whose name a string literal spells out. A list made any other way has no proxy class, and its
//! proxy fails at run time, saying so (`java/overlay/java/lang/reflect/TranslatedProxies.java`).
//!
//! The class is `TranslatedProxy` and a hash of the list, in the package of its interfaces that
//! are not public, as in Java (a list whose non-public interfaces are in two packages has none),
//! or else in that of the first interface. javac is already running when reachability finds the
//! list, so `program::load` writes the source to a directory of its own and has javac add it by
//! its file. Its run-time name (`jdk.proxy1.$Proxy0`) is the overlay's, given when the class is
//! first asked for; a class in a dynamic module has no `Module`, and `getModule` throws. It is what `ProxyGenerator` makes: it extends `Proxy`, has a public
//! constructor that takes the `InvocationHandler`, and for `hashCode`, `equals` and `toString`
//! and then each method of the interfaces in their order (with those of their superinterfaces,
//! but not static or private ones), a final method that calls the handler with the proxy, the
//! `Method` and the arguments boxed into an `Object[]` (`null` for none). Its result is cast to
//! the method's return type, or unboxed, which throws the `ClassCastException` and the
//! `NullPointerException` of Java's checks, and an exception that the method may not throw is
//! wrapped in an `UndeclaredThrowableException`. A method that two interfaces declare with the
//! same parameter types is one, the first's; with other return types or exceptions, the list
//! has no class. The `Method`s are found in the class's static initialization, by name, so they
//! are in the reflection table.

use std::collections::{BTreeSet, HashSet};

use super::{Item, Walk, has_flag, members, outside, strip_arguments, symbol_type, text};
use crate::value::Value;

/// The start of the simple name of every proxy class, which a hash of its list ends.
const NAME: &str = "TranslatedProxy";

/// The most lists that a creation's candidates make.
const MOST_LISTS: usize = 256;

/// A proxy class of the program: its qualified name, its source, and the package of its
/// interfaces that are not public, if one is not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProxyClass {
    pub class: String,
    pub source: String,
    pub hidden: Option<String>,
}

/// The array of interfaces that the call `node` makes proxies of, if it is
/// `Proxy.newProxyInstance` or `Proxy.getProxyClass`: its elements when it is an array creation
/// with them, or `None`.
fn creation(node: &Value) -> Option<Option<&Vec<Value>>> {
    let symbol = &node["symbol"];
    let creates = text(node, "kind") == "METHOD_INVOCATION"
        && text(symbol, "owner") == "java.lang.reflect.Proxy"
        && matches!(text(symbol, "name"), "newProxyInstance" | "getProxyClass");
    if !creates {
        return None;
    }
    let array = node["arguments"].as_array()?.get(1)?;
    if text(array, "kind") != "NEW_ARRAY" {
        return Some(None);
    }
    Some(array["initializers"].as_array())
}

/// A method of a proxy class: its name, erased parameter and return types, the exceptions it
/// declares, and the interface whose `Method` the handler gets.
#[derive(Clone)]
struct Proxied {
    name: String,
    params: Vec<String>,
    ret: String,
    throws: Vec<String>,
    declaring: String,
}

impl<'t> Walk<'t> {
    /// Notes the list of interfaces of a creation of proxies in reachable code (`node`).
    pub(super) fn proxy_scan(&mut self, node: &Value) {
        let Some(elements) = creation(node) else { return };
        let values = elements.map(|e| e.iter().map(|e| self.class_value(e)).collect());
        self.proxy_creations.insert(values);
    }

    /// Makes reachable the proxy classes of the lists that the creations of reachable code can
    /// pass (see the module's comment), and whether that is anything new.
    pub(super) fn make_proxies(&mut self) -> bool {
        let mut new = false;
        for creation in self.proxy_creations.clone() {
            let Some(values) = creation else { continue };
            let mut choices: Vec<Vec<String>> = Vec::new();
            for value in &values {
                let mut classes = BTreeSet::new();
                if !self.classes_of(value, &mut HashSet::new(), &mut classes) {
                    if values.len() > 1 {
                        choices.clear();
                        break;
                    }
                    let mut any: BTreeSet<&String> = self.class_literals.iter().collect();
                    any.extend(self.named.iter());
                    any.extend(self.reach.found_by_name.iter());
                    classes.extend(any.into_iter().filter(|c| self.is_interface(c)).cloned());
                }
                choices.push(classes.into_iter().collect());
            }
            if choices.is_empty() && !values.is_empty() {
                continue;
            }
            let mut lists: Vec<Vec<String>> = vec![Vec::new()];
            for choice in &choices {
                lists = lists
                    .iter()
                    .flat_map(|list| {
                        choice.iter().map(move |c| list.iter().cloned().chain([c.clone()]).collect())
                    })
                    .take(MOST_LISTS)
                    .collect();
            }
            for list in lists {
                new |= self.proxy(list);
            }
        }
        new
    }

    /// Makes the proxy class of `list` reachable, once its interfaces are declared; whether that
    /// is anything new.
    fn proxy(&mut self, list: Vec<String>) -> bool {
        if let Some(made) = self.reach.proxies.get(&list) {
            let class = made.class.clone();
            if !self.unmade_proxies.contains(&class) || !self.classes.contains_key(&class) {
                return false;
            }
            self.unmade_proxies.remove(&class);
            let ctor = (class, "<init>".to_string(), "(java.lang.reflect.InvocationHandler)void".to_string());
            self.work.push(Item::Method(ctor));
            return true;
        }
        if self.proxy_failed.contains(&list) {
            return false;
        }
        match self.proxied(&list) {
            Err(waiting) => {
                // The classes it needs are declared by units still to come, or never.
                let mut new = false;
                for class in waiting {
                    if self.proxy_waiting.insert(class.clone()) {
                        self.work.push(Item::Class(class));
                        new = true;
                    }
                }
                new
            }
            Ok(None) => {
                self.proxy_failed.insert(list);
                false
            }
            Ok(Some((package, methods))) => {
                let subclass = |a: &str, b: &str| {
                    self.is_subtype(a, b)
                        || self.supertypes_of(a).iter().any(|s| crate::lower::standard_subclass(s, b))
                };
                let public = list.iter().all(|i| self.is_public(i));
                let made = proxy_class(&list, &package, public, &methods, &subclass);
                self.work.push(Item::Class(made.class.clone()));
                self.unmade_proxies.insert(made.class.clone());
                self.reach.proxies.insert(list, made);
                true
            }
        }
    }

    /// The package of the proxy class of `list` and its methods after those of `Object`, or
    /// `None` when it has none (see the module's comment); or the classes not declared yet that
    /// it needs.
    fn proxied(&self, list: &[String]) -> Result<Option<(String, Vec<Proxied>)>, Vec<String>> {
        let mut waiting = Vec::new();
        let mut seen = HashSet::new();
        for interface in list {
            if !self.classes.contains_key(interface) {
                waiting.push(interface.clone());
            }
            if !seen.insert(interface.clone()) {
                // A repeated interface, which Java rejects before it would make a class.
                return Ok(None);
            }
        }
        if !waiting.is_empty() {
            return Err(waiting);
        }
        // In the package of its interfaces that are not public, which must be one, as Java has
        // it; else in any, that of the first interface.
        let hidden: BTreeSet<String> =
            list.iter().filter(|i| !self.is_public(i)).map(|i| self.package_of(i)).collect();
        let package = match (hidden.len(), list.first()) {
            (0, Some(first)) => self.package_of(first),
            (0, None) => "java.lang.reflect".to_string(),
            (1, _) => hidden.into_iter().next().unwrap_or_default(),
            _ => return Ok(None),
        };
        let mut methods: Vec<Proxied> = Vec::new();
        for interface in list {
            for owner in self.interface_and_supers(interface, &mut waiting) {
                let Some(&(decl, _)) = self.classes.get(&owner) else { continue };
                let interface = text(decl, "kind") == "INTERFACE";
                if !interface || has_flag(decl, "SEALED") || !self.accessible(&owner, &package) {
                    return Ok(None);
                }
                for member in members(decl) {
                    if text(member, "kind") != "METHOD"
                        || has_flag(member, "STATIC")
                        || has_flag(member, "PRIVATE")
                    {
                        continue;
                    }
                    let Some(proxied) = proxied_method(member, &owner) else { return Ok(None) };
                    for ty in proxied.params.iter().chain([&proxied.ret]).chain(&proxied.throws) {
                        let class = ty.trim_end_matches("[]");
                        if class.contains('.') && !outside(class) {
                            match self.classes.get(class) {
                                None => waiting.push(class.to_string()),
                                Some(_) if !self.accessible(class, &package) => return Ok(None),
                                Some(_) => {}
                            }
                        }
                    }
                    let object = OBJECT_METHODS
                        .iter()
                        .any(|(n, p, _)| *n == proxied.name && p.iter().eq(proxied.params.iter()));
                    match methods.iter().find(|m| m.name == proxied.name && m.params == proxied.params) {
                        _ if object => {}
                        // A superinterface's method that an interface overrides, or the same
                        // method of another interface.
                        Some(m) if self.is_subtype(&m.declaring, &owner) => {}
                        Some(m) if m.ret == proxied.ret && m.throws == proxied.throws => {}
                        Some(_) => return Ok(None),
                        None => methods.push(proxied),
                    }
                }
            }
        }
        if waiting.is_empty() { Ok(Some((package, methods))) } else { Err(waiting) }
    }

    /// The interface `interface` and its superinterfaces, directly or not, in the order of their
    /// declarations; those not declared yet are added to `waiting`.
    fn interface_and_supers(&self, interface: &str, waiting: &mut Vec<String>) -> Vec<String> {
        let mut out = vec![interface.to_string()];
        let mut at = 0;
        while at < out.len() {
            if !self.classes.contains_key(&out[at]) {
                waiting.push(out[at].clone());
            }
            for s in self.supertypes.get(&out[at]).into_iter().flatten() {
                if !out.contains(s) && s != "java.lang.Object" {
                    out.push(s.clone());
                }
            }
            at += 1;
        }
        out
    }

    /// Whether the class `class` is declared public, and so are the classes it is nested in.
    fn is_public(&self, class: &str) -> bool {
        self.nest(class).iter().all(|(decl, _)| has_flag(decl, "PUBLIC"))
    }

    /// Whether code of the package `package` can name the class `class`: it and the classes it is
    /// nested in are public, or not private and in that package.
    fn accessible(&self, class: &str, package: &str) -> bool {
        let here = self.package_of(class) == package;
        let nest = self.nest(class);
        !nest.is_empty()
            && nest.iter().all(|(decl, _)| has_flag(decl, "PUBLIC") || here && !has_flag(decl, "PRIVATE"))
    }

    /// The declarations of the class `class` and of the classes it is nested in, with their
    /// names; empty when one is not declared.
    fn nest(&self, class: &str) -> Vec<(&'t Value, String)> {
        let mut out = Vec::new();
        let mut at = Some(class.to_string());
        while let Some(c) = at {
            let Some((decl, outer)) = self.classes.get(&c) else { return Vec::new() };
            out.push((*decl, c));
            at = outer.clone();
        }
        out
    }

    /// The package of the class `class`: the name of its top-level class without the last part.
    fn package_of(&self, class: &str) -> String {
        let top = self.nest(class).last().map_or(class.to_string(), |(_, name)| name.clone());
        top.rsplit_once('.').map_or(String::new(), |(package, _)| package.to_string())
    }

    /// Whether `class` is an interface that a proxy class can implement (not an annotation type,
    /// which is not translated).
    fn is_interface(&self, class: &str) -> bool {
        self.classes.get(class).is_some_and(|(decl, _)| text(decl, "kind") == "INTERFACE")
    }
}

/// `Object`'s methods that a proxy class passes to its handler, in `ProxyGenerator`'s order:
/// the name, the parameter types and the return type.
pub(super) const OBJECT_METHODS: &[(&str, &[&str], &str)] = &[
    ("hashCode", &[], "int"),
    ("equals", &["java.lang.Object"], "boolean"),
    ("toString", &[], "java.lang.String"),
];

/// The method of a proxy class for the interface method `member` of `owner`, from its erased
/// signature; `None` when it throws a type variable.
fn proxied_method(member: &Value, owner: &str) -> Option<Proxied> {
    let erased = member["symbol"]["erased"].as_str().unwrap_or(symbol_type(member));
    let (params, ret) = split_signature(erased)?;
    let mut throws = Vec::new();
    for thrown in member["throws"].as_array().into_iter().flatten() {
        let ty = strip_arguments(text(thrown, "javaType"));
        if !ty.contains('.') {
            return None;
        }
        throws.push(ty);
    }
    Some(Proxied {
        name: text(member, "name").to_string(),
        params,
        ret,
        throws,
        declaring: owner.to_string(),
    })
}

/// The parameter types and the return type of an erased signature, `(int,java.lang.String)void`.
fn split_signature(signature: &str) -> Option<(Vec<String>, String)> {
    let rest = signature.strip_prefix('(')?;
    let (params, ret) = rest.split_once(')')?;
    let params = if params.is_empty() { Vec::new() } else { params.split(',').map(str::to_string).collect() };
    Some((params, ret.to_string()))
}

/// The proxy class of `list` in `package`, public when its interfaces are, with `methods` after
/// `Object`'s.
fn proxy_class(
    list: &[String],
    package: &str,
    public: bool,
    methods: &[Proxied],
    subclass: &dyn Fn(&str, &str) -> bool,
) -> ProxyClass {
    let mut fingerprint = crate::check_cache::Fingerprint::new();
    for interface in list {
        fingerprint.field(interface.as_bytes());
    }
    let name = format!("{NAME}{}", &fingerprint.key()[..8]);
    let object: Vec<Proxied> = OBJECT_METHODS
        .iter()
        .map(|(name, params, ret)| Proxied {
            name: name.to_string(),
            params: params.iter().map(|p| p.to_string()).collect(),
            ret: ret.to_string(),
            throws: Vec::new(),
            declaring: "java.lang.Object".to_string(),
        })
        .collect();
    let all: Vec<&Proxied> = object.iter().chain(methods).collect();
    let mut out = String::new();
    if !package.is_empty() {
        out.push_str(&format!("package {package};\n\n"));
    }
    out.push_str(
        "// A proxy class that the translator wrote for a list of interfaces, as the JDK's\n\
         // `ProxyGenerator` would make it (see `program/reach/proxies.rs`).\n",
    );
    let implements = if list.is_empty() { String::new() } else { format!(" implements {}", list.join(", ")) };
    let access = if public { "public " } else { "" };
    out.push_str(&format!("{access}final class {name} extends java.lang.reflect.Proxy{implements} {{\n"));
    for i in 0..all.len() {
        out.push_str(&format!("    private static final java.lang.reflect.Method m{i};\n"));
    }
    out.push_str(&format!(
        "\n    public {name}(java.lang.reflect.InvocationHandler h) {{\n        super(h);\n    }}\n"
    ));
    for (i, m) in all.iter().enumerate() {
        out.push('\n');
        out.push_str(&method_source(i, m, subclass));
    }
    out.push_str("\n    static {\n        try {\n");
    for (i, m) in all.iter().enumerate() {
        let types: String = m.params.iter().map(|p| format!(", {p}.class")).collect();
        out.push_str(&format!(
            "            m{i} = {}.class.getMethod(\"{}\"{types});\n",
            m.declaring, m.name
        ));
    }
    out.push_str(
        "        } catch (java.lang.NoSuchMethodException e) {\n            \
         throw new java.lang.NoSuchMethodError(e.getMessage());\n        }\n    }\n}\n",
    );
    let class = if package.is_empty() { name } else { format!("{package}.{name}") };
    ProxyClass { class, source: out, hidden: (!public).then(|| package.to_string()) }
}

/// The source of the proxy class's method for `m`, whose `Method` is `m{index}`.
fn method_source(index: usize, m: &Proxied, subclass: &dyn Fn(&str, &str) -> bool) -> String {
    let params: Vec<String> = m.params.iter().enumerate().map(|(i, p)| format!("{p} a{i}")).collect();
    let throws = if m.throws.is_empty() { String::new() } else { format!(" throws {}", m.throws.join(", ")) };
    let args = if m.params.is_empty() {
        "null".to_string()
    } else {
        let boxed: Vec<String> = (0..m.params.len()).map(|i| format!("a{i}")).collect();
        format!("new java.lang.Object[] {{ {} }}", boxed.join(", "))
    };
    let call = format!("h.invoke(this, m{index}, {args})");
    let body = match m.ret.as_str() {
        "void" => format!("{call};"),
        ret => format!("return ({}) {call};", boxed_type(ret)),
    };
    let mut out = format!("    public final {} {}({}){throws} {{\n", m.ret, m.name, params.join(", "));
    match catch_list(&m.throws, subclass) {
        // A method that may throw any `Throwable` passes on what the handler throws.
        None => out.push_str(&format!("        {body}\n")),
        Some(caught) => out.push_str(&format!(
            "        try {{\n            {body}\n        }} catch ({} e) {{\n            throw e;\n        }} \
             catch (java.lang.Throwable t) {{\n            \
             throw new java.lang.reflect.UndeclaredThrowableException(t);\n        }}\n",
            caught.join(" | ")
        )),
    }
    out.push_str("    }\n");
    out
}

/// The exceptions that a proxy method passes on as they are: `Error`, `RuntimeException` and those
/// it declares, without one that is a `subclass` of another; `None` when that is every
/// `Throwable`, as `ProxyGenerator.computeUniqueCatchList` does.
fn catch_list(throws: &[String], subclass: &dyn Fn(&str, &str) -> bool) -> Option<Vec<String>> {
    let mut all: Vec<String> = ["java.lang.Error", "java.lang.RuntimeException"].map(String::from).to_vec();
    all.extend(throws.iter().cloned());
    if all.iter().any(|t| t == "java.lang.Throwable") {
        return None;
    }
    let mut out: Vec<String> = Vec::new();
    for t in &all {
        let covered = all.iter().any(|other| other != t && subclass(t, other));
        if !covered && !out.contains(t) {
            out.push(t.clone());
        }
    }
    Some(out)
}

/// The type that the handler's result is cast to for a method that returns `ty`: its box for a
/// primitive type, which the cast unboxes.
fn boxed_type(ty: &str) -> &str {
    match ty {
        "boolean" => "java.lang.Boolean",
        "byte" => "java.lang.Byte",
        "char" => "java.lang.Character",
        "short" => "java.lang.Short",
        "int" => "java.lang.Integer",
        "long" => "java.lang.Long",
        "float" => "java.lang.Float",
        "double" => "java.lang.Double",
        ty => ty,
    }
}
