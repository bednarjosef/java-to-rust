//! Handles that the code holds as values: a `VarHandle`, `MethodHandle`, `MethodHandles.Lookup`
//! or `MethodType` that is not a constant (see `handles`), but is kept in a local or a field,
//! passed to a method or returned from one, and may be null, as netty finds its handles:
//!
//! ```java
//! static {
//!     MethodHandle find = null;
//!     try {
//!         find = MethodHandles.lookup().findVirtual(MethodHandles.Lookup.class, "findVarHandle",
//!                 MethodType.methodType(VarHandle.class, Class.class, String.class, Class.class));
//!     } catch (Throwable e) {
//!         find = null;
//!     } finally {
//!         FIND_VAR_HANDLE = find;
//!     }
//! }
//! static VarHandle findIntField(MethodHandles.Lookup lookup, Class<?> c, String name) {
//!     return enabled ? (VarHandle) FIND_VAR_HANDLE.invokeExact(privateLookupIn(c, lookup), c, name, int.class) : null;
//! }
//! ... VH = findIntField(MethodHandles.lookup(), Counter.class, "value"); ... VH.getAndAdd(this, 1) ...
//! ```
//!
//! At run time such a value is a token that only tells whether it is null: a value of type
//! `Class` (`Type::JavaClass`), non-null for a handle. Which handle it is, lowering finds when
//! translating (`known`): what each expression can hold, following the assignments of a local in
//! its method, the values of a static final field, the `return`s of a static, private or final
//! method with its parameters bound to the arguments of the call, and a parameter of such a method
//! to the arguments of every call of it in the program (a method that a method reference names is
//! not followed). The calls that make handles are followed when their arguments are known: the
//! lookups (`MethodHandles.lookup()`, `publicLookup()`, `privateLookupIn`), the `VarHandle`s of a
//! field, a static field, the elements of an array and the views of a `byte[]` or a `ByteBuffer`,
//! `MethodType.methodType` of class literals (or `Void.TYPE` and its kin), `findVirtual` and
//! `findStatic` of these methods themselves, and `invokeExact` or `invoke` of such a
//! `MethodHandle`, with the call's own types exactly the handle's (as `invokeExact` requires, and
//! `invoke` without a conversion). `findStatic` also finds a static method of the program that
//! reachability keeps, by its name and exact type, and `VarHandle`'s fences; `invokeExact` of
//! such a handle is a call of the method (`static_handle_call`). A static method's handle has
//! its class as its token, so that a value that may be one of several methods, each of its own
//! class or a fence, calls the one it holds (netty's skip list takes `VarHandle.acquireFence` or
//! a fallback of its own). A method whose `return`s are followed may also return a `Class`
//! (netty's `cls`), and `Class.forName` of a constant name is that class, or no value where
//! it throws.
//!
//! A call that makes a handle lowers to the token, after its receiver and arguments are evaluated,
//! when every handle that its known arguments can make is made without an exception (the field
//! exists and has the type asked for, and the lookup may access it); a call of a `MethodHandle`
//! that may be null throws Java's `NullPointerException` first (`Builtin::NullChecked` with the
//! method's name, whose message names the call as the class file does). An access through a
//! `VarHandle` value that can hold one handle, or null, is the access of that handle
//! (`handles::var_handle_access`), after the same null check. Java makes that check after it
//! evaluates the arguments, so through a value that may be null the arguments must be ones whose
//! evaluation does nothing (`quiet`: literals, locals, and casts and arithmetic of them that cannot
//! throw), and a `MethodHandle` read before arguments that do something must be one that can be read
//! again after them.
//!
//! A call of a lookup's, a `MethodHandle`'s or a `MethodType`'s method that lowering does not
//! follow (a handle of any other method, a call such as `unreflect`, `invokeWithArguments` or
//! `insertParameterTypes`, or arguments that it cannot tell or that may fail to make a handle) evaluates its receiver and arguments and then throws an
//! `UnsupportedOperationException` that says so (`unfollowed`). The code sees a JDK whose lookup
//! fails, which netty expects of some and catches, going without that handle. A census lists
//! each such call (`program::census`, by `UNFOLLOWED_MESSAGE`). Anything else is
//! rejected: a handle that may be one of several, and any use of a handle value but these
//! (`slots`): an assignment, a `return`, an argument of a method's handle parameter, a comparison
//! with `null`, and the receiver of a handle's method.

use super::handles::{VarHandleOf, byte_order, plain};
use super::*;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

pub(super) const VAR_HANDLE: &str = "java.lang.invoke.VarHandle";
pub(super) const METHOD_HANDLE: &str = "java.lang.invoke.MethodHandle";
pub(super) const LOOKUP: &str = "java.lang.invoke.MethodHandles.Lookup";
pub(super) const METHOD_TYPE: &str = "java.lang.invoke.MethodType";
const METHOD_HANDLES: &str = "java.lang.invoke.MethodHandles";

/// The exception that a call of a handle's method that lowering does not follow throws
/// (`unfollowed`), whose constructor of a message the code that calls one uses
/// (`exceptions::Uses`).
pub const UNFOLLOWED: &str = "java.lang.UnsupportedOperationException";

/// What the message of that exception says after the call's name, which a census finds it by
/// (`program::census`).
pub const UNFOLLOWED_MESSAGE: &str = "of a handle or arguments that the translation does not follow";

/// The types of handles, whose values are tokens.
pub(super) const HANDLE_TYPES: [&str; 4] = [VAR_HANDLE, METHOD_HANDLE, LOOKUP, METHOD_TYPE];

/// How many calls and values deep `known` follows a value.
const DEPTH: usize = 12;

/// What a value can be, as lowering follows it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) enum Known<'t> {
    Null,
    /// A class literal's class, as javac names the type (`int[]`, `java.lang.String`).
    Class(&'t str),
    /// A string literal.
    Text(&'t str),
    /// `ByteOrder.BIG_ENDIAN` (`true`) or `LITTLE_ENDIAN`.
    Order(bool),
    Var(VarHandleOf<'t>),
    /// A lookup with every access of its lookup class, by javac's name of the class.
    Lookup(&'t str),
    /// `MethodHandles.publicLookup()`.
    PublicLookup,
    /// A `MethodHandle` of one of the methods that make handles (`FACTORIES`).
    Method(Factory),
    /// A `MethodHandle` of a static method of the program, by its index in `Lowering::methods`.
    Static(usize),
    /// A `MethodHandle` of one of `VarHandle`'s static fences (`FENCES`), each a full fence.
    Fence,
    /// A `MethodType`: the return type and then the parameter types, as javac names them.
    Type(Vec<&'t str>),
}

/// A method of the JDK that makes a handle, which a `MethodHandle` can be of.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct Factory {
    owner: &'static str,
    name: &'static str,
    is_static: bool,
    /// The method's type: the return type, then the parameter types.
    ty: &'static [&'static str],
}

/// The methods that make handles that a `MethodHandle` value can be of (`findVirtual` and
/// `findStatic` of them).
const FACTORIES: &[Factory] = &[
    Factory {
        owner: LOOKUP,
        name: "findVarHandle",
        is_static: false,
        ty: &[VAR_HANDLE, "java.lang.Class", "java.lang.String", "java.lang.Class"],
    },
    Factory {
        owner: LOOKUP,
        name: "findStaticVarHandle",
        is_static: false,
        ty: &[VAR_HANDLE, "java.lang.Class", "java.lang.String", "java.lang.Class"],
    },
    Factory {
        owner: METHOD_HANDLES,
        name: "privateLookupIn",
        is_static: true,
        ty: &[LOOKUP, "java.lang.Class", LOOKUP],
    },
    Factory {
        owner: METHOD_HANDLES,
        name: "byteArrayViewVarHandle",
        is_static: true,
        ty: &[VAR_HANDLE, "java.lang.Class", "java.nio.ByteOrder"],
    },
    Factory {
        owner: METHOD_HANDLES,
        name: "byteBufferViewVarHandle",
        is_static: true,
        ty: &[VAR_HANDLE, "java.lang.Class", "java.nio.ByteOrder"],
    },
    Factory {
        owner: METHOD_HANDLES,
        name: "arrayElementVarHandle",
        is_static: true,
        ty: &[VAR_HANDLE, "java.lang.Class"],
    },
];

/// `VarHandle`'s static fences, which a `MethodHandle` value can be of (`findStatic` of them).
const FENCES: [&str; 5] = ["fullFence", "acquireFence", "releaseFence", "loadLoadFence", "storeStoreFence"];

/// The primitive type whose class the static field `TYPE` of the wrapper class `owner` holds.
fn wrapped_primitive(owner: &str) -> Option<&'static str> {
    Some(match owner {
        "java.lang.Void" => "void",
        "java.lang.Boolean" => "boolean",
        "java.lang.Byte" => "byte",
        "java.lang.Character" => "char",
        "java.lang.Short" => "short",
        "java.lang.Integer" => "int",
        "java.lang.Long" => "long",
        "java.lang.Float" => "float",
        "java.lang.Double" => "double",
        _ => return None,
    })
}

/// Where an expression is: the code it is in (a method, an initializer block or a field's
/// declaration), the class whose code that is, and the call whose arguments the method's
/// parameters are, if known.
#[derive(Clone)]
struct At<'t> {
    code: Node<'t>,
    class: usize,
    bound: Option<Rc<(Node<'t>, At<'t>)>>,
}

/// A method: its class (javac's name), name and type.
type MethodKey<'t> = (&'t str, &'t str, &'t str);

/// The calls of the program's methods, each with the code it is in and that code's class, and the
/// methods that a method reference names, whose parameters are not followed.
#[derive(Default)]
pub(super) struct Calls<'t> {
    calls: HashMap<MethodKey<'t>, Vec<(Node<'t>, Node<'t>, usize)>>,
    referenced: HashSet<MethodKey<'t>>,
}

/// javac's type `ty` without type arguments.
/// Whether javac's type `ty` is a primitive type or `void`.
fn primitive(ty: &str) -> bool {
    matches!(ty, "boolean" | "byte" | "char" | "short" | "int" | "long" | "float" | "double" | "void")
}

fn erased(ty: &str) -> &str {
    ty.split_once('<').map_or(ty, |(raw, _)| raw)
}

/// The parameter types and the return type of javac's method type `ty`, without the type
/// parameters of a generic method (`<T>(java.lang.Class<?>)java.lang.Class<T>`).
fn method_signature(ty: &str) -> (Vec<&str>, &str) {
    let mut depth = 0;
    let start = match ty.starts_with('<') {
        true => ty
            .char_indices()
            .find(|&(_, c)| {
                depth += match c {
                    '<' => 1,
                    '>' => -1,
                    _ => 0,
                };
                depth == 0
            })
            .map_or(0, |(i, _)| i + 1),
        false => 0,
    };
    lambdas::signature(&ty[start..])
}

/// Whether `ty` (as javac names it) is a handle's type.
pub(super) fn is_handle_type(ty: &str) -> bool {
    HANDLE_TYPES.contains(&ty)
}

/// Calls `f` on every node of the code under `node`, but not in the classes in it, which are code
/// of their own, nor, unless `lambdas`, in its lambdas.
fn walk_code<'t>(node: Node<'t>, lambdas: bool, f: &mut impl FnMut(Node<'t>)) {
    fn go<'t>(v: &'t Value, top: bool, lambdas: bool, f: &mut impl FnMut(Node<'t>)) {
        match v {
            Value::Object(map) => {
                let node = Node(v);
                let kind = node.kind();
                let other =
                    (kind == "LAMBDA_EXPRESSION" && !lambdas) || crate::program::CLASS_KINDS.contains(&kind);
                if !top && other {
                    return;
                }
                f(node);
                map.values().for_each(|c| go(c, false, lambdas, f));
            }
            Value::Array(items) => items.iter().for_each(|i| go(i, false, lambdas, f)),
            _ => {}
        }
    }
    go(node.0, true, lambdas, f)
}

/// Whether `inner` is `outer` or a node under it.
fn contains(outer: &Value, inner: &Value) -> bool {
    std::ptr::eq(outer, inner)
        || match outer {
            Value::Object(map) => map.values().any(|c| contains(c, inner)),
            Value::Array(items) => items.iter().any(|c| contains(c, inner)),
            _ => false,
        }
}

/// Whether evaluating `n` again gives the same value and does nothing: a literal, a class
/// literal, a local or parameter, perhaps negated, cast to a primitive type or in parentheses.
pub(super) fn quiet(n: Node) -> bool {
    let n = plain(n);
    let primitive = |n: Node| handles::PRIMITIVES.contains(&n.java_type()) && quiet(n);
    match n.kind() {
        "UNARY_MINUS" | "UNARY_PLUS" | "BITWISE_COMPLEMENT" | "LOGICAL_COMPLEMENT" => {
            n.get("expression").is_some_and(primitive)
        }
        "TYPE_CAST" => {
            handles::PRIMITIVES.contains(&n.java_type()) && n.get("expression").is_some_and(primitive)
        }
        // An integer's division by zero throws.
        "DIVIDE" | "REMAINDER" if !matches!(n.java_type(), "float" | "double") => false,
        "PLUS"
        | "MINUS"
        | "MULTIPLY"
        | "DIVIDE"
        | "REMAINDER"
        | "LEFT_SHIFT"
        | "RIGHT_SHIFT"
        | "UNSIGNED_RIGHT_SHIFT"
        | "AND"
        | "OR"
        | "XOR"
        | "CONDITIONAL_AND"
        | "CONDITIONAL_OR"
        | "LESS_THAN"
        | "LESS_THAN_EQUAL"
        | "GREATER_THAN"
        | "GREATER_THAN_EQUAL"
        | "EQUAL_TO"
        | "NOT_EQUAL_TO" => {
            n.get("leftOperand").is_some_and(primitive) && n.get("rightOperand").is_some_and(primitive)
        }
        "MEMBER_SELECT" => n.str("identifier") == "class",
        "IDENTIFIER" => {
            n.str("name") == "this"
                || n.symbol().is_some_and(|s| {
                    matches!(
                        s.kind,
                        "LOCAL_VARIABLE" | "PARAMETER" | "EXCEPTION_PARAMETER" | "RESOURCE_VARIABLE"
                    )
                })
        }
        kind => kind.ends_with("_LITERAL"),
    }
}

/// Adds to `out`, by address, the slots of the code under `v` where a handle may be a value (see
/// the module's comment); a node is one when `out` holds it as its parent's walk reaches it.
fn slots(v: &Value, out: &mut HashSet<*const Value>) {
    let items = match v {
        Value::Array(items) => {
            items.iter().for_each(|i| slots(i, out));
            return;
        }
        Value::Object(map) => map,
        _ => return,
    };
    let node = Node(v);
    let approved = out.contains(&(v as *const Value));
    let handle = |n: Node| is_handle_type(n.java_type());
    let mut allow = |n: Option<Node>| {
        if let Some(n) = n {
            out.insert(n.0 as *const Value);
        }
    };
    match node.kind() {
        "VARIABLE" if node.symbol().is_some_and(|s| is_handle_type(s.ty)) => allow(node.get("initializer")),
        "ASSIGNMENT" if node.get("variable").is_some_and(handle) => allow(node.get("expression")),
        "RETURN" | "EXPRESSION_STATEMENT" => allow(node.get("expression")),
        "PARENTHESIZED" if approved => allow(node.get("expression")),
        "TYPE_CAST" if approved && handle(node) => allow(node.get("expression")),
        "CONDITIONAL_EXPRESSION" if approved => {
            allow(node.get("trueExpression"));
            allow(node.get("falseExpression"));
        }
        "EQUAL_TO" | "NOT_EQUAL_TO" => {
            let null = |f: &str| node.get(f).is_some_and(|n| plain(n).kind() == "NULL_LITERAL");
            if null("rightOperand") {
                allow(node.get("leftOperand"));
            }
            if null("leftOperand") {
                allow(node.get("rightOperand"));
            }
        }
        "METHOD_INVOCATION" | "NEW_CLASS" => {
            let symbol = node.symbol();
            let owner = symbol.map_or("", |s| s.owner);
            // The receiver of a handle's method, which lowering resolves or rejects.
            if is_handle_type(owner) || owner == METHOD_HANDLES {
                allow(node.get("methodSelect").and_then(|s| s.get("expression")));
            }
            let args: Vec<Node> = node.list("arguments").collect();
            if owner == METHOD_HANDLE {
                args.iter().for_each(|a| allow(Some(*a)));
            } else if let Some(symbol) = symbol {
                let (params, _) = method_signature(symbol.ty);
                // The arguments of a variable arity parameter, such as `filterArguments`'s
                // `MethodHandle...`, each have its element type.
                let element = params.last().filter(|_| symbol.varargs).and_then(|p| p.strip_suffix("[]"));
                let fixed = if element.is_some() { params.len() - 1 } else { params.len() };
                for (i, arg) in args.iter().enumerate() {
                    let param = if i < fixed { params.get(i).copied() } else { element };
                    if param.is_some_and(is_handle_type) {
                        allow(Some(*arg));
                    }
                }
            }
        }
        _ => {}
    }
    items.values().for_each(|child| slots(child, out));
}

impl<'t> Lowering<'t> {
    /// Rejects the expression `node` when it is a handle that the code uses as a value other than
    /// in one of the slots that lowering resolves (see the module's comment).
    pub(super) fn check_handle_value(&self, node: Node<'t>) -> Result<(), Error> {
        let ty = node.java_type();
        if !is_handle_type(ty) {
            return Ok(());
        }
        let class = self.current.get();
        let approved = {
            let mut slots_by_class = self.handle_slots.borrow_mut();
            let set = slots_by_class.entry(class).or_insert_with(|| {
                let mut out = HashSet::new();
                slots(self.types[class].node.0, &mut out);
                out
            });
            set.contains(&(node.0 as *const Value))
        };
        if approved {
            return Ok(());
        }
        let short = ty.rsplit('.').next().unwrap_or(ty);
        unsupported(node, &format!("a `{short}` used as a value"))
    }

    /// Where the expression `node` of the class being lowered is (see `At`).
    fn at(&self, node: Node<'t>) -> Option<At<'t>> {
        let class = self.current.get();
        let code = self.types[class].node.list("members").find(|m| contains(m.0, node.0))?;
        Some(At { code, class, bound: None })
    }

    /// What the expression `node` of the class being lowered can hold (see `known`).
    pub(super) fn held(&self, node: Node<'t>) -> Option<Vec<Known<'t>>> {
        let at = self.at(node)?;
        self.known(node, &at, 0)
    }

    /// What the expression `node`, at `at`, can be, each value once; `None` when lowering does
    /// not follow it, or when it can throw (a call that makes a handle that may fail).
    fn known(&self, node: Node<'t>, at: &At<'t>, depth: usize) -> Option<Vec<Known<'t>>> {
        if depth > DEPTH {
            return None;
        }
        let node = plain(node);
        let union = |sets: Vec<Option<Vec<Known<'t>>>>| -> Option<Vec<Known<'t>>> {
            let mut out = Vec::new();
            for set in sets {
                for k in set? {
                    if !out.contains(&k) {
                        out.push(k);
                    }
                }
            }
            Some(out)
        };
        match node.kind() {
            "NULL_LITERAL" => return Some(vec![Known::Null]),
            "STRING_LITERAL" => return node.0["value"].as_str().map(|s| vec![Known::Text(s)]),
            "TYPE_CAST" => return self.known(node.get("expression")?, at, depth),
            "CONDITIONAL_EXPRESSION" => {
                let branches = ["trueExpression", "falseExpression"];
                return union(branches.iter().map(|b| self.known(node.get(b)?, at, depth)).collect());
            }
            "MEMBER_SELECT" if node.str("identifier") == "class" => {
                return Some(vec![Known::Class(node.get("expression")?.java_type())]);
            }
            "METHOD_INVOCATION" => return self.call_known(node, at, depth),
            "IDENTIFIER" | "MEMBER_SELECT" => {}
            _ => return None,
        }
        let symbol = node.symbol()?;
        match symbol.kind {
            "LOCAL_VARIABLE" if node.kind() == "IDENTIFIER" => {
                // The local's declaration and assignments, in its own method's code (not that of
                // a lambda in it).
                let (mut values, mut declared) = (Vec::new(), false);
                walk_code(at.code, false, &mut |n| {
                    let named = |v: Node| v.kind() == "IDENTIFIER" && v.str("name") == symbol.name;
                    if n.kind() == "VARIABLE" && n.str("name") == symbol.name {
                        declared = true;
                        values.extend(n.get("initializer"));
                    } else if n.kind() == "ASSIGNMENT" && n.get("variable").is_some_and(|v| named(plain(v))) {
                        values.extend(n.get("expression"));
                    } else if n.kind().ends_with("_ASSIGNMENT")
                        && n.get("variable").is_some_and(|v| named(plain(v)))
                    {
                        values.push(n);
                    }
                });
                if !declared {
                    return None;
                }
                union(values.into_iter().map(|v| self.known(v, at, depth + 1)).collect())
            }
            "PARAMETER" if node.kind() == "IDENTIFIER" => self.parameter_known(symbol.name, at, depth),
            "ENUM_CONSTANT" => byte_order(node).map(|order| vec![Known::Order(order)]),
            "FIELD" if symbol.is_static => {
                if let Some(h) = self.var_handle(node) {
                    return Some(vec![Known::Var(h)]);
                }
                if let Some(order) = byte_order(node) {
                    return Some(vec![Known::Order(order)]);
                }
                // `Void.TYPE`, `Integer.TYPE`: the class of a primitive type.
                if symbol.name == "TYPE"
                    && let Some(primitive) = wrapped_primitive(symbol.owner)
                {
                    return Some(vec![Known::Class(primitive)]);
                }
                let (owner, member) = self.static_field(&symbol)?;
                if !member.has_flag("FINAL") {
                    return None;
                }
                let mut values: Vec<(Node<'t>, Node<'t>)> = Vec::new();
                if let Some(init) = member.get("initializer") {
                    values.push((init, member));
                }
                let javac = self.types[owner].javac.as_str();
                for block in self.types[owner].node.list("members").filter(|m| Self::is_static_block(*m)) {
                    walk_code(block, false, &mut |n| {
                        let is_field = |v: Node| {
                            v.symbol().is_some_and(|s| {
                                s.kind == "FIELD" && s.name == symbol.name && s.owner == javac && s.is_static
                            })
                        };
                        if n.kind() == "ASSIGNMENT" && n.get("variable").is_some_and(|v| is_field(plain(v))) {
                            values.extend(n.get("expression").map(|e| (e, block)));
                        }
                    });
                }
                let mut sets = Vec::new();
                for (value, code) in values {
                    let at = At { code, class: owner, bound: None };
                    sets.push(self.known(value, &at, depth + 1));
                }
                union(sets)
            }
            _ => None,
        }
    }

    /// What the parameter `name` of the method at `at` can be: the argument of the call that `at`
    /// binds, or else the argument of every call of the method in the program.
    fn parameter_known(&self, name: &str, at: &At<'t>, depth: usize) -> Option<Vec<Known<'t>>> {
        let method = at.code;
        if method.kind() != "METHOD" {
            return None;
        }
        let position = method.list("parameters").position(|p| p.str("name") == name)?;
        let mut written = false;
        walk_code(method, false, &mut |n| {
            let target =
                n.get("variable").or_else(|| n.get("expression").filter(|_| n.kind().ends_with("CREMENT")));
            let assigning = n.kind() == "ASSIGNMENT"
                || n.kind().ends_with("_ASSIGNMENT")
                || n.kind().ends_with("CREMENT");
            written |= assigning
                && target.is_some_and(|v| plain(v).kind() == "IDENTIFIER" && plain(v).str("name") == name);
        });
        let declared = method.symbol()?;
        if written || declared.varargs {
            return None;
        }
        if let Some(bound) = &at.bound {
            let (call, caller) = &**bound;
            let arg = call.list("arguments").nth(position)?;
            return self.known(arg, caller, depth + 1);
        }
        // A method that no override or other path can reach but a call of it: static, private,
        // final or of a final class, or a constructor.
        let owner = self.type_index(declared.owner)?;
        let exact = declared.is_static
            || method.has_flag("PRIVATE")
            || method.has_flag("FINAL")
            || method.str("name") == "<init>"
            || self.types[owner].node.has_flag("FINAL");
        if !exact {
            return None;
        }
        let key = (declared.owner, method.str("name"), declared.ty);
        let sites = {
            let mut calls = self.handle_calls.borrow_mut();
            let calls = calls.get_or_insert_with(|| self.all_calls());
            if calls.referenced.contains(&key) {
                return None;
            }
            calls.calls.get(&key).cloned().unwrap_or_default()
        };
        let mut out: Vec<Known<'t>> = Vec::new();
        for (call, code, class) in sites {
            let arg = call.list("arguments").nth(position)?;
            for k in self.known(arg, &At { code, class, bound: None }, depth + 1)? {
                if !out.contains(&k) {
                    out.push(k);
                }
            }
        }
        Some(out)
    }

    /// Every call of a method of the program, and every method that a method reference names.
    fn all_calls(&self) -> Calls<'t> {
        let mut calls = Calls::default();
        for (class, info) in self.types.iter().enumerate() {
            let members =
                info.node.list("members").filter(|m| !crate::program::CLASS_KINDS.contains(&m.kind()));
            for member in members {
                walk_code(member, true, &mut |n| {
                    let kind = n.kind();
                    if !matches!(kind, "METHOD_INVOCATION" | "NEW_CLASS" | "MEMBER_REFERENCE") {
                        return;
                    }
                    let Some(symbol) = n.symbol().filter(|s| s.kind == "METHOD" || s.kind == "CONSTRUCTOR")
                    else {
                        return;
                    };
                    let name = if symbol.kind == "CONSTRUCTOR" { "<init>" } else { symbol.name };
                    let key = (symbol.owner, name, symbol.ty);
                    if kind == "MEMBER_REFERENCE" {
                        calls.referenced.insert(key);
                    } else {
                        calls.calls.entry(key).or_default().push((n, member, class));
                    }
                });
            }
        }
        calls
    }

    /// What the call `node`, at `at`, returns (see `known`).
    fn call_known(&self, node: Node<'t>, at: &At<'t>, depth: usize) -> Option<Vec<Known<'t>>> {
        let symbol = node.symbol()?;
        let receiver = node.get("methodSelect").and_then(|s| s.get("expression"));
        let args: Vec<Node<'t>> = node.list("arguments").collect();
        let known_all = |nodes: &[Node<'t>]| -> Option<Vec<Vec<Known<'t>>>> {
            nodes.iter().map(|n| self.known(*n, at, depth + 1)).collect()
        };
        match (symbol.owner, symbol.name) {
            // The lookup class is the class whose code calls `lookup()`.
            (METHOD_HANDLES, "lookup") if args.is_empty() => {
                return Some(vec![Known::Lookup(self.javac_of(at.class))]);
            }
            (METHOD_HANDLES, "publicLookup") if args.is_empty() => return Some(vec![Known::PublicLookup]),
            (METHOD_TYPE, "methodType") => {
                if !matches!(
                    symbol.ty,
                    "(java.lang.Class<?>)java.lang.invoke.MethodType"
                        | "(java.lang.Class<?>,java.lang.Class<?>)java.lang.invoke.MethodType"
                        | "(java.lang.Class<?>,java.lang.Class<?>,java.lang.Class<?>[])java.lang.invoke.MethodType"
                ) || (symbol.ty.ends_with("[])java.lang.invoke.MethodType") && !symbol.varargs)
                {
                    return None;
                }
                let mut ty = Vec::new();
                for (i, arg) in args.iter().enumerate() {
                    let [Known::Class(class)] = &self.known(*arg, at, depth + 1)?[..] else { return None };
                    // A `void` parameter type is an `IllegalArgumentException`.
                    if i > 0 && *class == "void" {
                        return None;
                    }
                    ty.push(*class);
                }
                return Some(vec![Known::Type(ty)]);
            }
            // A class that the program has; one that it does not have, the call throws
            // (`by_name`), so it gives no value.
            ("java.lang.Class", "forName") => {
                let (name, _) = crate::program::class_for_name(node.0)?;
                return Some(match self.type_index(&name) {
                    Some(class) => vec![Known::Class(self.javac_of(class))],
                    None => Vec::new(),
                });
            }
            (VAR_HANDLE, "withInvokeExactBehavior" | "withInvokeBehavior") => {
                let held = self.known(receiver?, at, depth + 1)?;
                return (!held.contains(&Known::Null)).then_some(held);
            }
            (METHOD_HANDLE, "invokeExact" | "invoke") => {
                let mut out = Vec::new();
                let site = self.site_type(node)?;
                for target in self.known(receiver?, at, depth + 1)? {
                    let Known::Method(factory) = target else {
                        if target == Known::Null {
                            continue;
                        }
                        return None;
                    };
                    // The handle of an instance method takes its receiver first.
                    let mut ty = factory.ty.to_vec();
                    if !factory.is_static {
                        ty.insert(1, factory.owner);
                    }
                    if ty[..] != site[..] {
                        return None;
                    }
                    let args = known_all(&args)?;
                    let (lookup, args) = match factory.is_static {
                        true => (None, &args[..]),
                        false => (Some(args.first()?.clone()), &args[1..]),
                    };
                    for k in self.make(factory.name, lookup, args)? {
                        if !out.contains(&k) {
                            out.push(k);
                        }
                    }
                }
                return Some(out);
            }
            (METHOD_HANDLES | LOOKUP, name)
                if FACTORIES.iter().any(|f| f.name == name) || name.starts_with("find") =>
            {
                let factory = FACTORIES.iter().find(|f| f.owner == symbol.owner && f.name == name);
                let lookup = match symbol.is_static {
                    true => None,
                    false => Some(self.known(receiver?, at, depth + 1)?),
                };
                let args = known_all(&args)?;
                if let Some(factory) = factory {
                    let params: Vec<&str> = lambdas::signature(symbol.ty).0.into_iter().map(erased).collect();
                    if params[..] != factory.ty[1..] {
                        return None;
                    }
                    return self.make(factory.name, lookup, &args);
                }
                return self.find_method(name, lookup?, &args);
            }
            _ => {}
        }
        if let Some(h) = self.var_handle_value(node, None, &[], at.class) {
            return Some(vec![Known::Var(h)]);
        }
        // A method of the program whose `return`s are followed with its parameters bound: one
        // that returns a handle, or a class (netty's `cls`, which casts one to `Class<T>`).
        let returns = erased(method_signature(symbol.ty).1);
        if !(is_handle_type(returns) || returns == "java.lang.Class") || symbol.kind != "METHOD" {
            return None;
        }
        let owner = self.type_index(symbol.owner)?;
        let method = self.types[owner].node.list("members").find(|m| {
            m.kind() == "METHOD"
                && m.str("name") == symbol.name
                && m.symbol().is_some_and(|s| s.ty == symbol.ty)
        })?;
        let exact = symbol.is_static
            || method.has_flag("PRIVATE")
            || method.has_flag("FINAL")
            || self.types[owner].node.has_flag("FINAL");
        if !exact {
            return None;
        }
        let body = method.get("body")?;
        let mut returned = Vec::new();
        walk_code(body, false, &mut |n| {
            if n.kind() == "RETURN" {
                returned.push(n.get("expression"));
            }
        });
        let callee = At { code: method, class: owner, bound: Some(Rc::new((node, at.clone()))) };
        let mut out = Vec::new();
        for value in returned {
            for k in self.known(value?, &callee, depth + 1)? {
                if !out.contains(&k) {
                    out.push(k);
                }
            }
        }
        Some(out)
    }

    /// javac's name of class `class`.
    fn javac_of(&self, class: usize) -> &'t str {
        interned_name(&self.types[class].javac)
    }

    /// The type of the call `node` of a `MethodHandle`: what the call is cast to (javac's type of
    /// the call), then the static types of its arguments, without type arguments.
    fn site_type(&self, node: Node<'t>) -> Option<Vec<&'t str>> {
        let mut ty = vec![erased(node.java_type())];
        for arg in node.list("arguments") {
            let arg = erased(arg.java_type());
            if arg.is_empty() || arg == "<nulltype>" {
                return None;
            }
            ty.push(arg);
        }
        Some(ty)
    }

    /// The `MethodHandle` that `findVirtual` or `findStatic` (`name`) of the lookups `lookup`
    /// finds for every combination of its known arguments `args` (class, name and type), each a
    /// method of `FACTORIES` of that kind and exactly that type, or for `findStatic` a static
    /// method that `static_target` finds; `None` when one is not.
    fn find_method(
        &self,
        name: &str,
        lookup: Vec<Known<'t>>,
        args: &[Vec<Known<'t>>],
    ) -> Option<Vec<Known<'t>>> {
        let [classes, names, types] = args else { return None };
        if lookup.iter().any(|l| !matches!(l, Known::Lookup(_) | Known::PublicLookup)) {
            return None;
        }
        let mut out = Vec::new();
        for class in classes {
            for method in names {
                for ty in types {
                    let (Known::Class(class), Known::Text(method), Known::Type(ty)) = (class, method, ty)
                    else {
                        return None;
                    };
                    let factory = FACTORIES.iter().find(|f| {
                        f.owner == *class
                            && f.name == *method
                            && f.is_static == (name == "findStatic")
                            && f.ty[..] == ty[..]
                    });
                    let found = match factory {
                        Some(factory) if matches!(name, "findStatic" | "findVirtual") => {
                            vec![Known::Method(*factory)]
                        }
                        None if name == "findStatic" => lookup
                            .iter()
                            .map(|l| self.static_target(l, class, method, ty))
                            .collect::<Option<_>>()?,
                        _ => return None,
                    };
                    for found in found {
                        if !out.contains(&found) {
                            out.push(found);
                        }
                    }
                }
            }
        }
        Some(out)
    }

    /// The handle that `findStatic` of the lookup `lookup` finds in class `class` by the
    /// method's name and its type `ty` (the return type, then the parameter types, as javac
    /// names them): one of `VarHandle`'s fences, or a static method that the class declares with
    /// exactly that type, that reachability keeps and that the lookup may access (`may_access`).
    /// `None` for any other.
    fn static_target(&self, lookup: &Known<'t>, class: &str, name: &str, ty: &[&str]) -> Option<Known<'t>> {
        if class == VAR_HANDLE && FENCES.contains(&name) && ty == ["void"] {
            return Some(Known::Fence);
        }
        // A package of the JDK's that is not `java.*` is not open to the program's lookups.
        let internal = Self::in_jdk(class) && !class.starts_with("java.");
        if internal && !matches!(lookup, Known::Lookup(from) if Self::in_jdk(from)) {
            return None;
        }
        let index = self.type_index(class)?;
        let method = self.methods_of(index).into_iter().find(|&m| {
            let info = &self.methods[m];
            let (params, returns) = method_signature(info.sig);
            info.name == name
                && info.node.has_flag("STATIC")
                && std::iter::once(returns).chain(params).map(erased).eq(ty.iter().copied())
        })?;
        let info = &self.methods[method];
        let javac = &self.types[index].javac;
        if self.reach.is_some_and(|reach| {
            !reach.methods.contains(&(javac.clone(), name.to_string(), info.sig.to_string()))
        }) {
            return None;
        }
        self.may_access(lookup, class, index, info.node).then_some(Known::Static(method))
    }

    /// The handles that the factory `name` makes of every combination of its known receiver
    /// `lookup` and arguments `args`; `None` when one of them can fail.
    fn make(
        &self,
        name: &str,
        lookup: Option<Vec<Known<'t>>>,
        args: &[Vec<Known<'t>>],
    ) -> Option<Vec<Known<'t>>> {
        let mut out = Vec::new();
        let mut add = |k: Known<'t>| {
            if !out.contains(&k) {
                out.push(k);
            }
        };
        let none = vec![Known::Null];
        let lookups = lookup.as_ref().unwrap_or(&none);
        match (name, args) {
            ("privateLookupIn", [classes, callers]) => {
                for class in classes {
                    for caller in callers {
                        let (Known::Class(class), Known::Lookup(caller)) = (class, caller) else {
                            return None;
                        };
                        // The target class must be one of the translation's, in the caller's
                        // module: both outside the JDK, which the translation runs unnamed.
                        let target = self.type_index(class)?;
                        if Self::in_jdk(class) || Self::in_jdk(caller) {
                            return None;
                        }
                        add(Known::Lookup(self.javac_of(target)));
                    }
                }
            }
            ("findVarHandle" | "findStaticVarHandle", [classes, names, types]) => {
                for lookup in lookups {
                    for class in classes {
                        for field in names {
                            for ty in types {
                                let (Known::Class(class), Known::Text(field), Known::Class(ty)) =
                                    (class, field, ty)
                                else {
                                    return None;
                                };
                                let is_static = name == "findStaticVarHandle";
                                let index = self.accessible_field(lookup, class, field, ty, is_static)?;
                                let field = interned_name(field);
                                add(Known::Var(match is_static {
                                    true => VarHandleOf::Static(index, field),
                                    false => VarHandleOf::Field(index, field),
                                }));
                            }
                        }
                    }
                }
            }
            ("byteArrayViewVarHandle" | "byteBufferViewVarHandle", [classes, orders]) => {
                for class in classes {
                    for order in orders {
                        let (Known::Class(class), Known::Order(big_endian)) = (class, order) else {
                            return None;
                        };
                        let kind = literal_kind_of(class)?;
                        if !matches!(kind, "short" | "char" | "int" | "long" | "float" | "double") {
                            return None;
                        }
                        add(Known::Var(match name {
                            "byteArrayViewVarHandle" => VarHandleOf::ByteView(kind, *big_endian),
                            _ => VarHandleOf::BufferView(kind, *big_endian),
                        }));
                    }
                }
            }
            ("arrayElementVarHandle", [classes]) => {
                for class in classes {
                    let Known::Class(class) = class else { return None };
                    add(Known::Var(VarHandleOf::Element(literal_kind_of(class)?)));
                }
            }
            _ => return None,
        }
        Some(out)
    }

    /// The index of class `class`, when the lookup `lookup` finds its field `field` of type `ty`
    /// (static or not) with `findVarHandle` (or `findStaticVarHandle`) and may access it: a field
    /// that the class declares, of exactly that type, that is not final (whose handle could not
    /// write it), that reachability keeps, and that the lookup may access (`may_access`).
    fn accessible_field(
        &self,
        lookup: &Known<'t>,
        class: &str,
        field: &str,
        ty: &str,
        is_static: bool,
    ) -> Option<usize> {
        let index = self.type_index(class)?;
        let member = self.types[index]
            .node
            .list("members")
            .find(|m| m.kind() == "VARIABLE" && m.str("name") == field)?;
        let declared = member.symbol()?;
        if member.has_flag("STATIC") != is_static || member.has_flag("FINAL") || erased(declared.ty) != ty {
            return None;
        }
        if self.reach.is_some_and(|reach| {
            !reach.fields.contains(&(self.types[index].javac.clone(), field.to_string()))
        }) {
            return None;
        }
        self.may_access(lookup, class, index, member).then_some(index)
    }

    /// Whether the lookup `lookup` may access the member `member` of class `class` (javac's name,
    /// whose index is `index`): a lookup with every access of a class of the same outermost class
    /// does, of one of the same package unless it is private, and any lookup if it and its class
    /// are public.
    fn may_access(&self, lookup: &Known<'t>, class: &str, index: usize, member: Node<'t>) -> bool {
        let outermost = |name: &str| -> Option<usize> {
            let mut t = self.type_index(name)?;
            while let Some(outer) = self.types[t].node.symbol().and_then(|s| self.type_index(s.owner)) {
                t = outer;
            }
            Some(t)
        };
        let package = |t: usize| {
            let javac = &self.types[t].javac;
            javac.rsplit_once('.').map_or(String::new(), |(p, _)| p.to_string())
        };
        let public = member.has_flag("PUBLIC") && self.types[index].node.has_flag("PUBLIC");
        match lookup {
            Known::Lookup(from) => {
                let (Some(from), Some(to)) = (outermost(from), outermost(class)) else { return false };
                from == to || (!member.has_flag("PRIVATE") && package(from) == package(to)) || public
            }
            Known::PublicLookup => public,
            _ => false,
        }
    }

    /// Whether the class with javac's name `name` is one of the JDK's.
    fn in_jdk(name: &str) -> bool {
        ["java", "javax", "jdk", "sun", "com.sun"]
            .iter()
            .any(|root| name.strip_prefix(root).is_some_and(|rest| rest.starts_with('.')))
    }

    /// The handle that the value `node` of a `VarHandle` (which is no constant) holds when it is
    /// not null, and whether it may be null; `None` when it can hold more than one, or lowering
    /// does not follow it.
    pub(super) fn held_var_handle(&self, node: Node<'t>) -> Option<(VarHandleOf<'t>, bool)> {
        let held = self.held(node)?;
        let nullable = held.contains(&Known::Null);
        let handles: Vec<VarHandleOf<'t>> = held
            .iter()
            .filter_map(|k| match k {
                Known::Var(h) => Some(*h),
                _ => None,
            })
            .collect();
        match handles[..] {
            [h] if handles.len() + usize::from(nullable) == held.len() => Some((h, nullable)),
            _ => None,
        }
    }

    /// A call that makes a handle (a lookup, a `VarHandle`, a `MethodType`, `findVirtual` and
    /// `findStatic` of a factory, `invokeExact` or `invoke` of a `MethodHandle` value): its
    /// receiver and arguments evaluated, the receiver of a `MethodHandle` checked for null, then
    /// a token (see the module's comment). `None` for any other call.
    pub(super) fn handle_value_call(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
    ) -> Result<Option<ExprKind>, Error> {
        let followed = match symbol.owner {
            METHOD_HANDLES => {
                matches!(symbol.name, "lookup" | "publicLookup")
                    || FACTORIES.iter().any(|f| f.owner == METHOD_HANDLES && f.name == symbol.name)
            }
            LOOKUP => symbol.name.starts_with("find"),
            METHOD_TYPE => symbol.name == "methodType",
            METHOD_HANDLE => matches!(symbol.name, "invokeExact" | "invoke"),
            VAR_HANDLE => matches!(symbol.name, "withInvokeExactBehavior" | "withInvokeBehavior"),
            _ => false,
        };
        // The other methods of the lookups and of `MethodHandle` that make or call a method's
        // handle (`unreflect`, `asType`, `invokeWithArguments`), which lowering does not follow,
        // and every other method of `MethodType`, whose value is a token
        // (`insertParameterTypes`, `parameterCount`, as `HBShaper` of `java.desktop` builds the
        // type of an upcall).
        let returns = erased(method_signature(symbol.ty).1);
        let makes = followed
            || symbol.owner == METHOD_TYPE
            || matches!(symbol.owner, METHOD_HANDLES | LOOKUP | METHOD_HANDLE)
                && (is_handle_type(returns) || symbol.name.starts_with("invoke"));
        if !makes {
            return Ok(None);
        }
        let short = symbol.owner.rsplit('.').next().unwrap_or_default();
        let what = format!("`{short}.{}`", symbol.name);
        let receiver =
            node.get("methodSelect").and_then(|s| s.get("expression")).filter(|_| !symbol.is_static);
        if symbol.owner == METHOD_HANDLE
            && matches!(symbol.name, "invokeExact" | "invoke")
            && let Some(r) = receiver
            && let Some(held) = self.held(r)
            && held.iter().any(|k| matches!(k, Known::Static(_) | Known::Fence))
        {
            return self.static_handle_call(node, symbol.name, r, &held).map(Some);
        }
        let held = if followed { self.held(node) } else { None };
        let Some(held) = held else {
            return self.unfollowed(node, symbol, &what, followed).map(Some);
        };
        // No value at all: a call whose arguments always throw, or code that nothing calls.
        if held.contains(&Known::Null) {
            return unsupported(node, &format!("{what} that may give null"));
        }
        let pos = node.pos();
        let args: Vec<Node<'t>> = node.list("arguments").collect();
        let mut block = Vec::new();
        // A `MethodHandle` that may be null throws after its arguments are evaluated, so its
        // receiver is read again after them, which only a local or a static field of this class
        // allows.
        let checked = match receiver {
            Some(r) if symbol.owner == METHOD_HANDLE => {
                let nullable = self.held(r).is_none_or(|h| h.contains(&Known::Null));
                if nullable && !self.rereadable(r) && !args.iter().all(|a| quiet(*a)) {
                    return unsupported(
                        node,
                        &format!("{what} of a handle that may be null, read where it cannot be read again"),
                    );
                }
                nullable.then_some(r)
            }
            _ => None,
        };
        for part in receiver.filter(|_| checked.is_none()).into_iter().chain(args.iter().copied()) {
            if quiet(part) {
                continue;
            }
            // A part that makes a handle and does nothing else is its token alone.
            let part = self.expr(part)?;
            if !matches!(&part.kind, ExprKind::Builtin(Builtin::ClassLiteral(Type::JavaClass), a) if a.is_empty())
            {
                block.push(Stmt::Expr(part));
            }
        }
        if let Some(r) = checked {
            let receiver = self.expr(r)?;
            let check =
                ExprKind::Builtin(Builtin::NullChecked(Some(interned_name(symbol.name))), vec![receiver]);
            block.push(Stmt::Expr(self.synth(check, pos, Type::JavaClass)));
        }
        // A handle that a call of type `Object` gives (`invoke` of a handle that makes handles)
        // is its token as an `Object`.
        let ty = self.ty(node, node.java_type())?;
        let token = self.convert(self.static_token(node, &held)?, ty);
        if block.is_empty() {
            return Ok(Some(token.kind));
        }
        block.push(Stmt::Yield(token));
        Ok(Some(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body: block }],
        }))
    }

    /// The reference `value` as a `to`, which `invoke` converts it to: as it is, an upcast, or
    /// `Class.cast`'s check (`Cannot cast A to B`) and the cast, as `MethodHandle.asType` does.
    ///
    /// ```text
    /// switch (0) { default -> { T cast = value; check(to.class, cast); yield (to) cast; } }
    /// ```
    fn fitted(&self, value: Expr, to: Type) -> Expr {
        if value.ty == to || self.upcast_to(value.ty, to) {
            return self.convert(value, to);
        }
        let (pos, ty, name) = (value.pos, value.ty, self.fresh_name("cast"));
        let read = || self.synth(ExprKind::Var(name.clone()), pos, ty);
        let class =
            self.synth(ExprKind::Builtin(Builtin::ClassLiteral(to), Vec::new()), pos, Type::JavaClass);
        let check = self.synth(ExprKind::Builtin(Builtin::ClassCheck, vec![class, read()]), pos, Type::Void);
        let body = vec![
            Stmt::Local { ty, name: name.clone(), init: Some(value), pos },
            Stmt::Expr(check),
            Stmt::Yield(self.class_result(read, to)),
        ];
        let selector = Box::new(self.synth(ExprKind::Int(0), pos, Type::Int));
        let cases = vec![Case { labels: vec![CaseLabel::Default], body }];
        self.synth(ExprKind::Switch { selector, cases }, pos, to)
    }

    /// Whether reading the handle `node` again gives the same value and does nothing: a local,
    /// or a static field of the class being lowered.
    fn rereadable(&self, node: Node<'t>) -> bool {
        quiet(node)
            || plain(node).symbol().is_some_and(|s| {
                s.kind == "FIELD" && s.is_static && s.owner == self.javac_of(self.current.get())
            })
    }

    /// `invokeExact` or `invoke` (`name`) of the `MethodHandle` `receiver`, which holds the
    /// handle of a static method, or null (`held`): the call of that method, or a full fence for
    /// one of `VarHandle`'s, with the call's arguments, which must have exactly the method's types
    /// (as `invokeExact` requires), or for `invoke` differ only in reference types, which are
    /// cast. As in Java, the receiver is evaluated, then the arguments, then the receiver checked
    /// for null. A handle that may be one of several methods, as netty's fence is `VarHandle`'s
    /// or its own, calls the one whose token the receiver holds (`static_token`).
    fn static_handle_call(
        &self,
        node: Node<'t>,
        name: &str,
        receiver: Node<'t>,
        held: &[Known<'t>],
    ) -> Result<ExprKind, Error> {
        let what = format!("`MethodHandle.{name}`");
        let mut targets: Vec<(Option<usize>, Vec<&'t str>)> = Vec::new();
        for target in held {
            targets.push(match target {
                Known::Null => continue,
                Known::Fence => (None, vec!["void"]),
                Known::Static(m) => {
                    let (params, returns) = method_signature(self.methods[*m].sig);
                    (Some(*m), std::iter::once(returns).chain(params).map(erased).collect())
                }
                _ => {
                    return unsupported(
                        node,
                        &format!("{what} of a handle that may be a static method's or another"),
                    );
                }
            });
        }
        // `invoke` converts the arguments to the method's types and its result to the call's
        // (`MethodHandle.asType`): a reference type may differ, as it is cast; a primitive may not.
        let site = self.site_type(node);
        let fits = |ty: &Vec<&str>| match &site {
            Some(site) if name == "invoke" && site.len() == ty.len() => {
                site.iter().zip(ty).all(|(a, b)| a == b || (!primitive(a) && !primitive(b)))
            }
            site => site.as_ref() == Some(ty),
        };
        if targets.iter().any(|(_, ty)| !fits(ty)) {
            return unsupported(
                node,
                &format!("{what} of a static method's handle with other types than the method's"),
            );
        }
        // The fence's token is no class of the program, so it is the one left when the others
        // are not; two methods of one class have the same token.
        targets.sort_by_key(|(m, _)| m.is_none());
        let owners: HashSet<usize> =
            targets.iter().filter_map(|(m, _)| Some(self.methods[(*m)?].owner)).collect();
        if owners.len() + targets.iter().filter(|(m, _)| m.is_none()).count().min(1) != targets.len()
            || targets.iter().filter(|(m, _)| m.is_none()).count() > 1
        {
            return unsupported(
                node,
                &format!("{what} of a handle that may be one of several methods of a class"),
            );
        }
        let nullable = held.contains(&Known::Null);
        let several = targets.len() > 1;
        let pos = node.pos();
        let mut block = Vec::new();
        // A value that is read again: a quiet node, lowered at each read, or a local that holds
        // a value that does something.
        enum Read<'t> {
            Node(Node<'t>, Option<Type>),
            Held(String, Type),
        }
        let hold = |value: Expr, prefix: &str, block: &mut Vec<Stmt>| {
            let name = format!("{prefix}u{}", value.id);
            let ty = value.ty;
            block.push(Stmt::Local { ty, name: name.clone(), init: Some(value), pos });
            Read::Held(name, ty)
        };
        let read = |r: &Read<'t>| -> Result<Expr, Error> {
            match r {
                Read::Node(n, None) => self.expr(*n),
                Read::Node(n, Some(ty)) => Ok(self.convert(self.expr(*n)?, *ty)),
                Read::Held(name, ty) => Ok(self.synth(ExprKind::Var(name.clone()), pos, *ty)),
            }
        };
        // The receiver's value, when the call reads it; one that a second read gives again
        // (`rereadable`) is not held, and not read when the call does not need it.
        let token = match (self.rereadable(receiver), nullable || several) {
            (true, true) => Some(Read::Node(receiver, None)),
            (true, false) => None,
            (false, false) => {
                block.push(Stmt::Expr(self.expr(receiver)?));
                None
            }
            (false, true) => Some(hold(self.expr(receiver)?, RECEIVER, &mut block)),
        };
        // Through a handle that may be null, the arguments that do something are held in locals
        // before the check, and for several methods, before the test.
        let args: Vec<Node<'t>> = node.list("arguments").collect();
        let mut values = Vec::new();
        for (arg, param) in args.iter().zip(site.iter().flatten().skip(1)) {
            let param = self.ty(*arg, param)?;
            values.push(match (nullable || several) && !quiet(*arg) {
                true => hold(self.convert(self.expr(*arg)?, param), ARGUMENT, &mut block),
                false => Read::Node(*arg, Some(param)),
            });
        }
        if nullable && let Some(token) = &token {
            let check =
                ExprKind::Builtin(Builtin::NullChecked(Some(interned_name(name))), vec![read(token)?]);
            block.push(Stmt::Expr(self.synth(check, pos, Type::JavaClass)));
        }
        let ty = self.ty(node, node.java_type())?;
        let call = |method: Option<usize>| -> Result<Expr, Error> {
            let kind = match method {
                None => ExprKind::Builtin(Builtin::Native(Native::FullFence), Vec::new()),
                Some(m) => {
                    let (params, returns) = method_signature(self.methods[m].sig);
                    let mut args = Vec::new();
                    for (value, param) in values.iter().zip(params) {
                        args.push(self.fitted(read(value)?, self.ty(node, erased(param))?));
                    }
                    let name = self.methods[m].name.to_string();
                    let call = ExprKind::Call { name, args, resolved: Some(m) };
                    return Ok(self.fitted(self.synth(call, pos, self.ty(node, erased(returns))?), ty));
                }
            };
            Ok(self.synth(kind, pos, ty))
        };
        // `token == C.class ? C.m(..) : ..`, the last target without a test.
        let (last, tested) = targets.split_last().expect("a static method's handle");
        let mut result = call(last.0)?;
        for (method, _) in tested.iter().rev() {
            let m = method.expect("the fence is last");
            let class = self.ty(node, &self.types[self.methods[m].owner].javac)?;
            let literal =
                self.synth(ExprKind::Builtin(Builtin::ClassLiteral(class), Vec::new()), pos, Type::JavaClass);
            let token = read(token.as_ref().expect("a token read for several targets"))?;
            let test = self.synth(
                ExprKind::Binary(BinOp::Eq, Box::new(token), Box::new(literal)),
                pos,
                Type::Boolean,
            );
            result = match ty {
                Type::Void => {
                    let branch = Stmt::If {
                        cond: test,
                        then: Box::new(Stmt::Expr(call(Some(m))?)),
                        otherwise: Some(Box::new(Stmt::Expr(result))),
                    };
                    self.synth(
                        ExprKind::Switch {
                            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
                            cases: vec![Case { labels: vec![CaseLabel::Default], body: vec![branch] }],
                        },
                        pos,
                        ty,
                    )
                }
                _ => self.synth(
                    ExprKind::Conditional(Box::new(test), Box::new(call(Some(m))?), Box::new(result)),
                    pos,
                    ty,
                ),
            };
        }
        if block.is_empty() {
            return Ok(result.kind);
        }
        block.push(if ty == Type::Void { Stmt::Expr(result) } else { Stmt::Yield(result) });
        Ok(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body: block }],
        })
    }

    /// The token of a handle made with the handles `held` that the call `node` can make: the
    /// class of a static method's class, which `static_handle_call` tells from the others, and
    /// for any other handle the same token (`handle_token`).
    fn static_token(&self, node: Node<'t>, held: &[Known<'t>]) -> Result<Expr, Error> {
        let pos = node.pos();
        let statics: Vec<usize> = held
            .iter()
            .filter_map(|k| match k {
                Known::Static(m) => Some(*m),
                _ => None,
            })
            .collect();
        match statics[..] {
            [] => Ok(self.handle_token(pos)),
            [m] if held.len() == 1 => {
                let class = self.ty(node, &self.types[self.methods[m].owner].javac)?;
                Ok(self.synth(
                    ExprKind::Builtin(Builtin::ClassLiteral(class), Vec::new()),
                    pos,
                    Type::JavaClass,
                ))
            }
            _ => unsupported(node, "`Lookup.findStatic` that may find one of several handles"),
        }
    }

    /// A call of the method `what` of a lookup or a `MethodHandle` that lowering does not follow
    /// (a handle of a method other than one that makes handles, or, for a method that lowering
    /// `follows`, arguments that it cannot tell or that may fail to make a handle): its receiver
    /// and arguments evaluated, then an `UnsupportedOperationException` that says so, which the
    /// code can catch, as netty catches a lookup that fails and goes without the handle.
    fn unfollowed(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
        what: &str,
        follows: bool,
    ) -> Result<ExprKind, Error> {
        let pos = node.pos();
        let receiver =
            node.get("methodSelect").and_then(|s| s.get("expression")).filter(|_| !symbol.is_static);
        let mut before = Vec::new();
        for part in receiver.into_iter().chain(node.list("arguments")) {
            if !quiet(part) {
                // A local of the value's type, which an expression statement of `new Object[] { null }`
                // would leave to inference.
                let value = self.expr(part)?;
                let name = self.fresh_name("unfollowed");
                before.push(Stmt::Local { ty: value.ty, name, init: Some(value), pos });
            }
        }
        let may_fail = if follows { ", or that may fail" } else { "" };
        let message = format!("{what} {UNFOLLOWED_MESSAGE}{may_fail}");
        let ty = self.ty(node, node.java_type())?;
        match self.standard_thrown(UNFOLLOWED, Some(&message), before, ty, pos) {
            Some(thrown) => Ok(thrown.kind),
            None => unsupported(node, &message),
        }
    }

    /// The token of a handle that is not null.
    fn handle_token(&self, pos: Pos) -> Expr {
        self.synth(
            ExprKind::Builtin(Builtin::ClassLiteral(Type::JavaClass), Vec::new()),
            pos,
            Type::JavaClass,
        )
    }

    /// The access `access` of the handle that the `VarHandle` value `receiver` holds, by the
    /// method `name`: after the receiver is evaluated, and checked for null when `nullable`. Java
    /// checks it after it evaluates the arguments, so a handle that may be null takes only
    /// arguments whose evaluation does nothing (`args`).
    pub(super) fn through_value(
        &self,
        node: Node<'t>,
        name: &'static str,
        receiver: Node<'t>,
        nullable: bool,
        access: ExprKind,
    ) -> Result<ExprKind, Error> {
        if nullable && !node.list("arguments").all(quiet) {
            return unsupported(
                node,
                &format!("`VarHandle.{name}` of a handle that may be null, with arguments that do something"),
            );
        }
        if !nullable && quiet(receiver) {
            return Ok(access);
        }
        let pos = node.pos();
        let value = self.expr(receiver)?;
        let first = match nullable {
            true => self.synth(
                ExprKind::Builtin(Builtin::NullChecked(Some(name)), vec![value]),
                pos,
                Type::JavaClass,
            ),
            false => value,
        };
        let ty = self.ty(node, node.java_type())?;
        let access = self.synth(access, pos, ty);
        let last = if ty == Type::Void { Stmt::Expr(access) } else { Stmt::Yield(access) };
        Ok(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body: vec![Stmt::Expr(first), last] }],
        })
    }
}

/// The kind of the elements of the array type `class` (as javac names it).
fn literal_kind_of(class: &str) -> Option<&'static str> {
    let element = class.strip_suffix("[]")?;
    Some(handles::PRIMITIVES.into_iter().find(|p| *p == element).unwrap_or("reference"))
}
