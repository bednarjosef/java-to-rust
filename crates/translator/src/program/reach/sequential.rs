//! Whether every stream of a program stays sequential, so that the parallel paths of
//! `java.util.stream` (`ForkJoinPool`, the `ConcurrentHashMap` of a parallel `distinct()`, the
//! parallel evaluation of every terminal operation) cannot run and are not reached.
//!
//! A pipeline is parallel when its source stage's `AbstractPipeline.parallel` is true, which
//! `AbstractPipeline.isParallel()` reads. Reachability first assumes that it is never true and
//! skips what `isParallel()` guards (`never_true`); this module then checks the assumption on the
//! code so reached. Every assignment of the field must assign a value that cannot be true, given
//! that the field is not: `false`, `isParallel()` itself, `||` of two such values or `&&` with
//! one, or a parameter that cannot be true. A parameter cannot be true when its method can only
//! be called directly (a constructor, a static or a private method that no method reference
//! names), never assigns it, and every reachable call passes it such a value; that is a greatest
//! fixed point over the `boolean` parameters, so a parameter that only passes itself on counts.
//!
//! That follows the source stages, which take `parallel` from `StreamSupport.stream(spliterator,
//! parallel)` through three constructors, and it fails for `BaseStream.parallel()`, which assigns
//! `true`, and for `Collection.parallelStream()`, which passes it. By induction over the run the
//! field then stays false, so the code skipped never runs. When the check fails, reachability
//! runs again without the assumption.
//!
//! The assumption also covers the nodes that hold a pipeline's elements: only parallel
//! evaluation concatenates two nodes (`Nodes.conc`, a `ConcNode`), so in a sequential program no
//! node has children, and `Nodes.flatten`'s `node.getChildCount() > 0`, which copies a tree of
//! nodes into an array with a `ForkJoinTask`, is never true either. That holds when no class
//! that overrides `Node.getChildCount()`, whose default is 0, has objects.

use super::{CLASS_KINDS, FieldKey, MethodKey, Walk, class_name, has_flag, members, never_true, text};
use crate::value::Value;
use std::collections::{HashMap, HashSet};

/// The classes of a walk, with their declarations and outer classes.
type Classes<'t> = HashMap<String, (&'t Value, Option<String>)>;

/// The field that makes a pipeline parallel, and the method that reads it.
const PARALLEL_FIELD: (&str, &str) = ("java.util.stream.AbstractPipeline", "parallel");
const PIPELINE: &str = "java.util.stream.AbstractPipeline";

/// The stream interfaces through which `isParallel()` may be called.
const STREAMS: &[&str] = &[
    "java.util.stream.BaseStream",
    "java.util.stream.Stream",
    "java.util.stream.IntStream",
    "java.util.stream.LongStream",
    "java.util.stream.DoubleStream",
];

/// The interface of the nodes that hold a pipeline's elements, whose `getChildCount()` is 0
/// unless a class overrides it.
const NODE: &str = "java.util.stream.Node";

/// Whether a condition is `node.getChildCount() > 0` on a pipeline's node.
pub(super) fn has_children_test(condition: &Value) -> bool {
    let call = &condition["leftOperand"];
    let symbol = &call["symbol"];
    text(condition, "kind") == "GREATER_THAN"
        && text(call, "kind") == "METHOD_INVOCATION"
        && text(symbol, "owner") == NODE
        && text(symbol, "name") == "getChildCount"
        && text(&condition["rightOperand"], "kind") == "INT_LITERAL"
        && text(&condition["rightOperand"], "value") == "0"
}

/// Whether a condition is the call `isParallel()` of a pipeline, `AbstractPipeline`'s own.
pub(super) fn is_parallel_test(condition: &Value) -> bool {
    let symbol = &condition["symbol"];
    text(condition, "kind") == "METHOD_INVOCATION"
        && text(symbol, "kind") == "METHOD"
        && text(symbol, "owner") == PIPELINE
        && text(symbol, "name") == "isParallel"
}

impl<'t> Index<'t> {
    /// Whether, in the code that a walk which assumed sequential streams reached, the assumption
    /// holds. The index keeps the code it indexed, so that a walk that went on since (`Rounds`)
    /// has only what it reached since indexed.
    pub(super) fn streams_stay_sequential(&mut self, walk: &Walk<'t>) -> bool {
        // No node can have children: no class that overrides `getChildCount()` has objects.
        let child_count = (NODE.to_string(), "getChildCount".to_string(), "()int".to_string());
        let overriders = walk.overriders.get(&child_count).map(Vec::as_slice).unwrap_or_default();
        if overriders.iter().any(|(class, _, _)| walk.reach.live.contains(class)) {
            return false;
        }
        for key in &walk.reach.methods {
            if let Some(node) = walk.methods.get(key)
                && self.methods.insert(key.clone())
            {
                self.contexts.push(key.clone());
                self.scan(&walk.classes, node, Some(self.contexts.len() - 1));
            }
        }
        for key in &walk.reach.fields {
            if let Some(node) = walk.fields.get(key)
                && self.fields.insert(key.clone())
            {
                self.scan(&walk.classes, &node["initializer"], None);
            }
        }
        for (class, (decl, _)) in &walk.classes {
            for is_static in [true, false] {
                let runs = if is_static { &walk.reach.initialized } else { &walk.reach.instantiated };
                if !runs.contains(class) || !self.blocks.insert((class.clone(), is_static)) {
                    continue;
                }
                for member in members(decl).filter(|m| text(m, "kind") == "BLOCK") {
                    if (member["static"].as_bool() == Some(true)) == is_static {
                        self.scan(&walk.classes, member, None);
                    }
                }
            }
        }
        // A call of `isParallel()` through a stream interface runs a reachable implementation, and
        // every one with a body must be the pipeline's.
        let only_pipelines = walk.reach.methods.iter().all(|key| {
            key.1 != "isParallel"
                || key.0 == PIPELINE
                || walk.methods.get(key).is_none_or(|m| m["body"].is_null())
        });
        let mut check = Check { index: self, only_pipelines, never_true: HashSet::new() };
        check.solve(walk);
        let contexts = &check.index.contexts;
        self.writes.iter().all(|(context, value)| value.is_some_and(|v| check.value(v, context.map(|c| &contexts[c]))))
    }
}

/// What the check needs from the reached code: the calls of each method with their arguments,
/// the methods that a method reference names, the parameters that are assigned, and every write
/// of the field with the value written (`None` for a compound assignment); and what of the code
/// it has indexed: the methods, the fields' initializers, and the initializer blocks of classes,
/// static (`true`) or not. A call and a write are made in the code of the method whose parameters
/// its names refer to, by its position in `contexts`.
#[derive(Default)]
pub(super) struct Index<'t> {
    calls: HashMap<Member<'t>, Vec<(Option<usize>, &'t Value)>>,
    referenced: HashSet<Member<'t>>,
    assigned: HashSet<(MethodKey, String)>,
    writes: Vec<(Option<usize>, Option<&'t Value>)>,
    contexts: Vec<MethodKey>,
    methods: HashSet<MethodKey>,
    fields: HashSet<FieldKey>,
    blocks: HashSet<(String, bool)>,
}

impl<'t> Index<'t> {
    /// Indexes a piece of code. `context` is the method whose parameters its names refer to;
    /// inside a lambda or a local or anonymous class it is unknown, which only makes the check
    /// fail more often. What the assumption skips is skipped, as reachability does.
    /// A local or anonymous class among the program's `classes` is indexed with its own reached
    /// members, not with the code that declares it, as reachability does.
    fn scan(&mut self, classes: &Classes<'t>, node: &'t Value, context: Option<usize>) {
        match node {
            Value::Object(fields) => {
                let kind = text(node, "kind");
                if kind == "ANNOTATION" || kind == "TYPE_ANNOTATION" {
                    return;
                }
                if CLASS_KINDS.contains(&kind) && classes.contains_key(&class_name(node)) {
                    return;
                }
                if matches!(kind, "IF" | "CONDITIONAL_EXPRESSION") && never_true(&node["condition"], true) {
                    let branch = &node[if kind == "IF" { "elseStatement" } else { "falseExpression" }];
                    self.scan(classes, branch, context);
                    return;
                }
                let context =
                    if CLASS_KINDS.contains(&kind) || kind == "LAMBDA_EXPRESSION" { None } else { context };
                self.node(node, kind, context);
                for (field, child) in fields {
                    if field != "symbol" {
                        self.scan(classes, child, context);
                    }
                }
            }
            Value::Array(items) => items.iter().for_each(|item| self.scan(classes, item, context)),
            _ => {}
        }
    }

    /// Records a call, a method reference or an assignment.
    fn node(&mut self, node: &'t Value, kind: &str, context: Option<usize>) {
        let target = match kind {
            "METHOD_INVOCATION" | "NEW_CLASS" => {
                let calls = self.calls.entry(key(&node["symbol"])).or_default();
                calls.push((context, &node["arguments"]));
                return;
            }
            "MEMBER_REFERENCE" => {
                self.referenced.insert(key(&node["symbol"]));
                return;
            }
            "ASSIGNMENT" => &node["variable"],
            _ if kind.ends_with("_ASSIGNMENT") => &node["variable"],
            _ if kind.ends_with("_INCREMENT") || kind.ends_with("_DECREMENT") => &node["expression"],
            _ => return,
        };
        let symbol = &target["symbol"];
        match text(symbol, "kind") {
            "FIELD" if (text(symbol, "owner"), text(symbol, "name")) == PARALLEL_FIELD => {
                let value = (kind == "ASSIGNMENT").then(|| &node["expression"]);
                self.writes.push((context, value));
            }
            "PARAMETER" => {
                if let Some(method) = context {
                    self.assigned.insert((self.contexts[method].clone(), text(symbol, "name").to_string()));
                }
            }
            _ => {}
        }
    }
}

/// A method or constructor, by class, name and type, as a symbol names it.
type Member<'t> = (&'t str, &'t str, &'t str);

/// The method or constructor that a symbol names.
fn key(symbol: &Value) -> Member<'_> {
    (text(symbol, "owner"), text(symbol, "name"), text(symbol, "type"))
}

/// A method's key as a `Member`, to look it up by.
fn member(key: &MethodKey) -> Member<'_> {
    (&key.0, &key.1, &key.2)
}

/// A parameter: its method and its name.
type Parameter = (MethodKey, String);

struct Check<'i, 't> {
    index: &'i Index<'t>,
    /// Whether `isParallel()` through a stream interface can only be the pipeline's.
    only_pipelines: bool,
    /// The `boolean` parameters that cannot be true.
    never_true: HashSet<Parameter>,
}

impl Check<'_, '_> {
    /// The greatest set of `boolean` parameters that cannot be true: start from every one whose
    /// method can only be called directly and that is never assigned, and drop those that a call
    /// passes a value that may be true, until none is dropped.
    fn solve(&mut self, walk: &Walk) {
        let mut candidates: Vec<(Parameter, usize)> = Vec::new();
        for method in &walk.reach.methods {
            let Some(&node) = walk.methods.get(method) else { continue };
            let direct = method.1 == "<init>"
                || node["symbol"]["static"].as_bool() == Some(true)
                || has_flag(node, "PRIVATE");
            if !direct || self.index.referenced.contains(&member(method)) {
                continue;
            }
            let parameters: Vec<&Value> = node["parameters"].as_array().into_iter().flatten().collect();
            let varargs = node["symbol"]["varargs"].as_bool() == Some(true);
            for (position, parameter) in parameters.iter().enumerate() {
                let name = text(parameter, "name").to_string();
                let last_varargs = varargs && position + 1 == parameters.len();
                if text(&parameter["symbol"], "type") == "boolean"
                    && !last_varargs
                    && !self.index.assigned.contains(&(method.clone(), name.clone()))
                {
                    candidates.push(((method.clone(), name), position));
                }
            }
        }
        self.never_true = candidates.iter().map(|(p, _)| p.clone()).collect();
        loop {
            let dropped: Vec<Parameter> = candidates
                .iter()
                .filter(|(p, _)| self.never_true.contains(p))
                .filter(|((method, _), position)| {
                    let calls = self.index.calls.get(&member(method)).map(Vec::as_slice).unwrap_or_default();
                    !calls.iter().all(|(caller, args)| {
                        let caller = caller.map(|c| &self.index.contexts[c]);
                        args.get(*position).is_some_and(|arg| self.value(arg, caller))
                    })
                })
                .map(|(p, _)| p.clone())
                .collect();
            if dropped.is_empty() {
                return;
            }
            for p in dropped {
                self.never_true.remove(&p);
            }
        }
    }

    /// Whether a `boolean` value cannot be true, given that the field is not and that the
    /// parameters in `never_true` are not, in code whose names refer to the parameters of
    /// `context`.
    fn value(&self, value: &Value, context: Option<&MethodKey>) -> bool {
        let both = |l: &str, r: &str| (self.value(&value[l], context), self.value(&value[r], context));
        match text(value, "kind") {
            "BOOLEAN_LITERAL" => value["value"] == false,
            "PARENTHESIZED" => self.value(&value["expression"], context),
            "CONDITIONAL_OR" => {
                let (l, r) = both("leftOperand", "rightOperand");
                l && r
            }
            "CONDITIONAL_AND" => {
                let (l, r) = both("leftOperand", "rightOperand");
                l || r
            }
            "METHOD_INVOCATION" if is_parallel_test(value) => true,
            "METHOD_INVOCATION" => {
                let symbol = &value["symbol"];
                self.only_pipelines
                    && text(symbol, "name") == "isParallel"
                    && text(symbol, "type") == "()boolean"
                    && STREAMS.contains(&text(symbol, "owner"))
            }
            "IDENTIFIER" if text(&value["symbol"], "kind") == "PARAMETER" => context.is_some_and(|method| {
                self.never_true.contains(&(method.clone(), text(value, "name").to_string()))
            }),
            _ => false,
        }
    }
}
