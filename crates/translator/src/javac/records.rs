//! The members of records that javac adds after attribution, written into the tree as javac's
//! own declarations would be, so that lowering and reachability see ordinary methods. javac's
//! attributed tree has the canonical constructor (implicit or compact) without the assignments
//! of the fields, and no accessors, `toString`, `equals` or `hashCode`: their symbols exist, and
//! calls name them, but `Lower` and `ObjectMethods` write their code later. This module writes
//! them as Java would behave:
//!
//! - the canonical constructor that does not assign the fields itself (the implicit one or a
//!   compact one) ends with `this.c = c;` for each component, after its own statements;
//! - `toString()` is `"Point[x=" + x + ", y=" + y + "]"`, with the record's simple name;
//! - `hashCode()` is `31 * h + hash(c)` over the components, starting at 0, with each component's
//!   own hash (`Double.hashCode(d)`, `Objects.hashCode(o)` for a reference);
//! - `equals(Object o)` is `o instanceof Point that && x == that.x && ..`, comparing floating
//!   components with `Double.compare(a, b) == 0` and references with `Objects.equals`;
//! - an accessor `c()` returns the field.
//!
//! A member the record declares itself is kept. An accessor is written only when something calls
//! it, an interface of the record declares it, or the program reads records' components
//! (`Class.getRecordComponents`, called or found by its name as gson finds it), whose accessors
//! reflection calls, `hashCode` only when it is called (directly or
//! through a record holding this one) or `Object`'s may run, and `equals` only when `Object`'s
//! may run (every call of it is one): whatever else would be dead code. `Object`'s methods may
//! run when the program uses `Object` and calls a method of the name, or `java.util`. Written
//! members carry `"synthetic": true`, and the calls of `java.util.Objects` they make
//! `"implicit": true` (they do not make the program use `Object`).

use crate::value::{Map, Value, json};
use std::collections::{BTreeSet, HashMap, HashSet};

/// Completes the records of these compilation units.
pub(super) fn complete(units: &mut [&mut Value]) {
    let survey = survey(&units.iter().map(|u| &**u).collect::<Vec<_>>());
    for unit in units.iter_mut() {
        complete_unit(unit, &survey);
    }
}

/// What `complete_unit` needs to know of all the compilation units: what they call, and their
/// interfaces, records and enums. Where a local type is declared does not change it.
pub(super) fn survey(units: &[&Value]) -> Survey {
    let mut survey = Survey::default();
    survey.add(units);
    survey
}

/// Completes the records of one compilation unit, with the survey of all of them, after moving
/// its local types and its enum constants' bodies' static fields (`enum_bodies`) where they belong.
pub(super) fn complete_unit(unit: &mut Value, survey: &Survey) {
    hoist_local_types(unit);
    super::enum_bodies::hoist_body_statics(unit);
    visit_records(unit, &mut |record| complete_record(record, survey));
}

/// The record declarations of a compilation unit, inner ones first, as `visit_records` finds them.
pub(super) fn records(unit: &Value) -> Vec<&Value> {
    fn find<'v>(v: &'v Value, out: &mut Vec<&'v Value>) {
        match v {
            Value::Object(fields) => {
                fields.values().for_each(|x| find(x, out));
                if fields.get("kind").and_then(Value::as_str) == Some("RECORD") {
                    out.push(v);
                }
            }
            Value::Array(items) => items.iter().for_each(|x| find(x, out)),
            _ => {}
        }
    }
    let mut records = Vec::new();
    find(unit, &mut records);
    records
}

/// What `complete_record` writes into these records (`records`) that this survey decides: all of
/// the unit that the survey decides, as moving local types out of the code does not depend on it.
/// When it is the same with two surveys, so is what `complete_unit` makes of the unit.
pub(super) fn completion(records: &[&Value], survey: &Survey) -> Completion {
    let decide = |record: &Value| {
        let name = qualified(record);
        let components = components(record);
        let interface_methods = survey.interface_methods(record);
        let accessors =
            components.iter().map(|c| accessor_overrides(record, &name, c, &interface_methods, survey));
        let hash_code = writes_hash_code(record, &name, survey);
        let equals = writes_equals(record, survey);
        let objects = components.iter().filter(|c| hashed_by_objects(&c.ty));
        let plain = objects.filter(|_| hash_code || equals).map(|c| survey.plain_value(&c.ty)).collect();
        RecordCompletion { accessors: accessors.collect(), hash_code, equals, plain }
    };
    Completion(records.iter().map(|record| decide(record)).collect())
}

/// What `completion` gives: for each record, what the survey decides of the members written.
#[derive(PartialEq)]
pub struct Completion(Vec<RecordCompletion>);

impl Completion {
    /// Whether what `complete_record` writes with this completion only adds to what it wrote
    /// with `before`, of the same records: every member written before is written still, each
    /// accessor overriding at least what it did, so the new unit has the members of the old one
    /// and maybe more. Which components are plain values changes only the `implicit` marks of the
    /// calls of `java.util.Objects` in `hashCode` and `equals`. As the units only add to the
    /// survey, a later completion always adds to an earlier one.
    pub fn only_adds_to(&self, before: &Completion) -> bool {
        self.0.len() == before.0.len()
            && self.0.iter().zip(&before.0).all(|(now, was)| {
                now.accessors.len() == was.accessors.len()
                    && now.accessors.iter().zip(&was.accessors).all(|(now, was)| match (now, was) {
                        (_, None) => true,
                        (Some(now), Some(was)) => was.iter().all(|o| now.contains(o)),
                        (None, Some(_)) => false,
                    })
                    && (now.hash_code || !was.hash_code)
                    && (now.equals || !was.equals)
            })
    }
}

#[derive(PartialEq)]
struct RecordCompletion {
    /// For each component, the interfaces and types of the methods that its accessor overrides,
    /// when one is written (`accessor_overrides`).
    accessors: Vec<Option<Vec<(String, String)>>>,
    /// Whether `hashCode` and `equals` are written.
    hash_code: bool,
    equals: bool,
    /// When either is, whether each component that `java.util.Objects` hashes and compares
    /// (`hashed_by_objects`) is a plain value (`Survey::plain_value`), which makes its calls
    /// implicit.
    plain: Vec<bool>,
}

/// Whether `complete_unit` may change the compilation unit: it has no package, it declares a
/// record, or a statement list inside a method or a block declares an enum or interface
/// (`take_local_types`), which is hoisted, or an enum constant's body declares a static field
/// (`enum_bodies`). Anything else that is an enum or interface, such as the symbol of a name, is
/// not one.
pub(super) fn changes(unit: &Value) -> bool {
    fn has_record_or_local(v: &Value, in_body: bool) -> bool {
        match v {
            Value::Object(fields) => {
                let kind = fields.get("kind").and_then(Value::as_str);
                let in_body = in_body || matches!(kind, Some("METHOD" | "BLOCK"));
                let local = |s: &Value| matches!(text(s, "kind"), "ENUM" | "INTERFACE");
                kind == Some("RECORD")
                    || in_body
                        && fields
                            .get("statements")
                            .and_then(Value::as_array)
                            .is_some_and(|s| s.iter().any(local))
                    || fields.values().any(|x| has_record_or_local(x, in_body))
            }
            Value::Array(items) => items.iter().any(|x| has_record_or_local(x, in_body)),
            _ => false,
        }
    }
    unit["packageName"].is_null()
        || has_record_or_local(unit, false)
        || super::enum_bodies::has_body_statics(unit)
}

/// An interface: the names and types of its methods without parameters, and the interfaces it
/// extends.
type Interface = (Vec<(String, String)>, Vec<String>);

/// What the units use, which decides the members written.
#[derive(Default)]
pub(super) struct Survey {
    /// Methods that a call or a method reference names, as (owner, name).
    called: HashSet<(String, String)>,
    /// Whether the program uses `java.lang.Object` (see `lower::object::uses_object`).
    uses_object: bool,
    /// The program's interfaces by qualified name.
    interfaces: HashMap<String, Interface>,
    /// The records by qualified name, with the qualified names of the records among their
    /// components' types.
    records: HashMap<String, Vec<String>>,
    /// The program's enums by qualified name.
    enums: HashSet<String>,
    /// The records that get a `hashCode`.
    hashed: BTreeSet<String>,
    /// Whether the units read records' components: they call `Class.getRecordComponents`, or
    /// spell its name, to find it by reflection.
    reads_components: bool,
}

impl Survey {
    /// Adds these compilation units to those surveyed.
    pub(super) fn add(&mut self, units: &[&Value]) {
        for unit in units {
            self.scan(unit);
        }
        self.hashed_closure();
    }

    fn scan(&mut self, v: &Value) {
        match v {
            Value::Object(fields) => {
                let kind = text(v, "kind");
                if kind == "TYPE_PARAMETER" {
                    self.uses_object = true;
                }
                let symbol = &v["symbol"];
                if matches!(kind, "METHOD_INVOCATION" | "MEMBER_REFERENCE") && symbol.is_object() {
                    self.called.insert((text(symbol, "owner").to_string(), text(symbol, "name").to_string()));
                    self.reads_components |= (text(symbol, "owner"), text(symbol, "name"))
                        == ("java.lang.Class", "getRecordComponents");
                }
                if kind == "STRING_LITERAL" && v["value"].as_str() == Some("getRecordComponents") {
                    self.reads_components = true;
                }
                if kind == "INTERFACE" {
                    let methods = members(v)
                        .filter(|m| text(m, "kind") == "METHOD" && params(m).is_empty())
                        .map(|m| (text(m, "name").to_string(), text(&m["symbol"], "type").to_string()))
                        .collect();
                    let extends = supertypes(v);
                    self.interfaces.insert(qualified(v), (methods, extends));
                }
                if kind == "ENUM" {
                    self.enums.insert(qualified(v));
                }
                if kind == "RECORD" {
                    let held = components(v).iter().map(|c| raw(&c.ty).to_string()).collect();
                    self.records.insert(qualified(v), held);
                }
                let object_ctor = text(v, "kind") == "CONSTRUCTOR" && text(v, "owner") == "java.lang.Object";
                if !object_ctor
                    && ["javaType", "type", "owner"].iter().any(|f| {
                        fields.get(f).and_then(Value::as_str).is_some_and(|t| t.contains("java.lang.Object"))
                    })
                {
                    self.uses_object = true;
                }
                fields.values().for_each(|f| self.scan(f));
            }
            Value::Array(items) => items.iter().for_each(|i| self.scan(i)),
            _ => {}
        }
    }

    /// Whether `Object`'s method `name` may run on a record: the program uses `Object`, and calls a
    /// method of that name or the library of `java.util`, which calls it on the objects it holds.
    fn runs(&self, name: &str) -> bool {
        self.uses_object
            && self.called.iter().any(|(owner, called)| called == name || owner.starts_with("java.util."))
    }

    /// The records whose `hashCode` runs: those it is called on, every record when `Object`'s may
    /// run, and the records held by those.
    fn hashed_closure(&mut self) {
        // A record that is hashed hashes the records among its components, of which more may
        // have been surveyed since the last closure.
        self.hashed.clear();
        let mut work: Vec<String> = self
            .records
            .keys()
            .filter(|r| {
                self.runs("hashCode") || self.called.contains(&((*r).clone(), "hashCode".to_string()))
            })
            .cloned()
            .collect();
        while let Some(record) = work.pop() {
            if self.hashed.insert(record.clone()) {
                work.extend(self.records[&record].iter().filter(|r| self.records.contains_key(*r)).cloned());
            }
        }
    }

    /// Whether a value of javac's type `ty` hashes and compares without `Object`: a string, a box,
    /// an enum or a record (whose own methods are written here).
    fn plain_value(&self, ty: &str) -> bool {
        let ty = raw(ty);
        ty == "java.lang.String"
            || ["Integer", "Long", "Double", "Float", "Short", "Byte", "Character", "Boolean"]
                .iter()
                .any(|b| ty.strip_prefix("java.lang.") == Some(b))
            || self.enums.contains(ty)
            || self.records.contains_key(ty)
    }

    /// The methods without parameters of the interfaces that `record` implements, directly or
    /// through the interfaces they extend, as (interface, name, type).
    fn interface_methods(&self, record: &Value) -> Vec<(String, String, String)> {
        let mut out = Vec::new();
        let mut work = supertypes(record);
        let mut seen = HashSet::new();
        while let Some(i) = work.pop() {
            if !seen.insert(i.clone()) {
                continue;
            }
            if let Some((methods, extends)) = self.interfaces.get(&i) {
                out.extend(methods.iter().map(|(n, t)| (i.clone(), n.clone(), t.clone())));
                work.extend(extends.iter().cloned());
            }
        }
        out
    }
}

/// A component of a record: its field.
struct Component {
    name: String,
    /// javac's type of the field, such as `int` or `java.lang.String`.
    ty: String,
    symbol: Value,
    line: Value,
    column: Value,
}

fn text<'v>(v: &'v Value, field: &str) -> &'v str {
    v[field].as_str().unwrap_or("")
}

fn members(decl: &Value) -> impl Iterator<Item = &Value> {
    decl["members"].as_array().into_iter().flatten()
}

fn params(method: &Value) -> &[Value] {
    method["parameters"].as_array().map_or(&[], Vec::as_slice)
}

/// A type without its type arguments: `R.Named` for `R.Named<T>`.
fn raw(ty: &str) -> &str {
    ty.split('<').next().unwrap_or(ty)
}

/// The qualified name of a declaration, as symbols name their owner.
fn qualified(decl: &Value) -> String {
    raw(text(&decl["symbol"], "type")).to_string()
}

/// The qualified names of the interfaces that a declaration implements or extends.
fn supertypes(decl: &Value) -> Vec<String> {
    decl["implementsClause"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|c| raw(text(c, "javaType")).to_string())
        .collect()
}

/// The components of a record: its instance fields, in order.
fn components(record: &Value) -> Vec<Component> {
    members(record)
        .filter(|m| text(m, "kind") == "VARIABLE" && m["symbol"]["static"].as_bool() == Some(false))
        .map(|m| Component {
            name: text(m, "name").to_string(),
            ty: text(&m["symbol"], "type").to_string(),
            symbol: m["symbol"].clone(),
            line: m["line"].clone(),
            column: m["column"].clone(),
        })
        .collect()
}

/// Moves the records, enums and interfaces declared in method bodies and initializer blocks to the
/// top level of their unit. They are static, so they see nothing of the method, and they capture
/// nothing. Lowering names them after their package and simple name, and after the method where
/// another top-level type has that name (`lower::inner`'s `name_hoisted`).
fn hoist_local_types(unit: &mut Value) {
    let mut local = Vec::new();
    for decl in unit["typeDecls"].as_array_mut().into_iter().flatten() {
        take_local_types(&mut decl["members"], false, &mut local);
    }
    if let Some(decls) = unit["typeDecls"].as_array_mut() {
        decls.extend(local);
    }
}

/// Takes the local records, enums and interfaces out of the statement lists in `v` (`in_body`
/// once inside a method or a block: outside a method, a block is an initializer), with the ones
/// declared inside them.
fn take_local_types(v: &mut Value, in_body: bool, out: &mut Vec<Value>) {
    match v {
        Value::Object(fields) => {
            let in_body =
                in_body || matches!(fields.get("kind").and_then(Value::as_str), Some("METHOD" | "BLOCK"));
            if in_body && let Some(Value::Array(statements)) = fields.get_mut("statements") {
                let local = |s: &Value| matches!(text(s, "kind"), "RECORD" | "ENUM" | "INTERFACE");
                let (taken, rest): (Vec<Value>, Vec<Value>) = statements.drain(..).partition(local);
                *statements = rest;
                for mut decl in taken {
                    take_local_types(&mut decl, true, out);
                    out.push(decl);
                }
            }
            fields.values_mut().for_each(|x| take_local_types(x, in_body, out));
        }
        Value::Array(items) => items.iter_mut().for_each(|x| take_local_types(x, in_body, out)),
        _ => {}
    }
}

/// Calls `f` on every record declaration in `v`, inner ones first.
fn visit_records(v: &mut Value, f: &mut dyn FnMut(&mut Value)) {
    match v {
        Value::Object(fields) => {
            fields.values_mut().for_each(|x| visit_records(x, f));
            if fields.get("kind").and_then(Value::as_str) == Some("RECORD") {
                f(v);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|x| visit_records(x, f)),
        _ => {}
    }
}

fn complete_record(record: &mut Value, survey: &Survey) {
    let name = qualified(record);
    let this_type = text(&record["symbol"], "type").to_string();
    let simple = text(record, "simpleName").to_string();
    let components = components(record);
    let at = Pos { line: record["line"].clone(), column: record["column"].clone() };
    let interface_methods = survey.interface_methods(record);
    let mut added = Vec::new();
    for c in &components {
        let Some(overrides) = accessor_overrides(record, &name, c, &interface_methods, survey) else {
            continue;
        };
        let overrides: Vec<Value> = overrides.iter().map(|(i, t)| json!({"owner": i, "type": t})).collect();
        let body = vec![at.node("RETURN", json!({"expression": field(c)}))];
        added.push(at.method(&name, &c.name, &c.ty, Vec::new(), body, overrides));
    }
    if !declares(record, "toString", 0) {
        let body = vec![at.node("RETURN", json!({"expression": at.to_string_text(&simple, &components)}))];
        added.push(at.method(&name, "toString", "java.lang.String", Vec::new(), body, Vec::new()));
    }
    if writes_hash_code(record, &name, survey) {
        let body = vec![at.node("RETURN", json!({"expression": at.hash(&components, survey)}))];
        added.push(at.method(&name, "hashCode", "int", Vec::new(), body, Vec::new()));
    }
    if writes_equals(record, survey) {
        let owner = "equals(java.lang.Object)";
        let param = json!({
            "kind": "VARIABLE", "line": at.line, "column": at.column, "name": "other",
            "symbol": {"kind": "PARAMETER", "name": "other", "owner": owner, "static": false, "type": "java.lang.Object"},
            "modifiers": {"kind": "MODIFIERS", "annotations": [], "flags": []},
        });
        let test = at.equals_test(&this_type, &components, survey);
        let body = vec![at.node("RETURN", json!({"expression": test}))];
        added.push(at.method(&name, "equals", "boolean", vec![param], body, Vec::new()));
    }
    assign_fields(record, &name, &components);
    if let Some(list) = record["members"].as_array_mut() {
        list.extend(added);
    }
}

/// Whether the record declares a method of this name with this many parameters.
fn declares(record: &Value, method: &str, arity: usize) -> bool {
    members(record)
        .any(|m| text(m, "kind") == "METHOD" && text(m, "name") == method && params(m).len() == arity)
}

/// The interfaces and types of the methods that the accessor of the component `c` of the record
/// `name` overrides, when one is written: when the record does not declare it, and something
/// calls it or an interface of the record declares it (`interface_methods`, of the record).
fn accessor_overrides(
    record: &Value,
    name: &str,
    c: &Component,
    interface_methods: &[(String, String, String)],
    survey: &Survey,
) -> Option<Vec<(String, String)>> {
    let implements: Vec<&(String, String, String)> =
        interface_methods.iter().filter(|(_, n, _)| *n == c.name).collect();
    if declares(record, &c.name, 0)
        || (implements.is_empty()
            && !survey.reads_components
            && !survey.called.contains(&(name.to_string(), c.name.clone())))
    {
        return None;
    }
    Some(implements.iter().map(|(i, _, t)| (i.clone(), t.clone())).collect())
}

/// Whether `hashCode` is written into the record `name`.
fn writes_hash_code(record: &Value, name: &str, survey: &Survey) -> bool {
    !declares(record, "hashCode", 0) && survey.hashed.contains(name)
}

/// Whether `equals` is written into the record.
fn writes_equals(record: &Value, survey: &Survey) -> bool {
    let declares_equals = members(record).any(|m| {
        text(m, "kind") == "METHOD"
            && text(m, "name") == "equals"
            && matches!(params(m), [p] if text(&p["symbol"], "type") == "java.lang.Object")
    });
    !declares_equals && survey.runs("equals")
}

/// Whether the written `hashCode` and `equals` hash and compare a component of javac's type `ty`
/// with `java.util.Objects` (`Pos::objects_call`): one that is no primitive and no array.
fn hashed_by_objects(ty: &str) -> bool {
    !matches!(ty, "int" | "long" | "double" | "float" | "boolean" | "char" | "short" | "byte")
        && !ty.ends_with("[]")
}

/// Ends the canonical constructor with the assignments of the fields, unless it assigns them
/// itself (an explicit canonical constructor must assign every one).
fn assign_fields(record: &mut Value, owner: &str, components: &[Component]) {
    let types: Vec<&str> = components.iter().map(|c| c.ty.as_str()).collect();
    let Some(members) = record["members"].as_array_mut() else { return };
    let Some(ctor) = members.iter_mut().find(|m| {
        text(m, "kind") == "METHOD"
            && text(m, "name") == "<init>"
            && params(m).iter().map(|p| text(&p["symbol"], "type")).eq(types.iter().copied())
    }) else {
        return;
    };
    if assigns_field(&ctor["body"], owner) {
        return;
    }
    let assignments: Vec<Value> = params(ctor)
        .iter()
        .zip(components)
        .map(|(param, c)| {
            let at = Pos { line: c.line.clone(), column: c.column.clone() };
            let this = at.node("IDENTIFIER", json!({"name": "this", "javaType": owner}));
            let target = at.node(
                "MEMBER_SELECT",
                json!({"javaType": c.ty, "symbol": c.symbol, "expression": this, "identifier": c.name}),
            );
            let value = at.node(
                "IDENTIFIER",
                json!({"name": text(param, "name"), "javaType": c.ty, "symbol": param["symbol"]}),
            );
            let assign =
                at.node("ASSIGNMENT", json!({"javaType": c.ty, "variable": target, "expression": value}));
            at.node("EXPRESSION_STATEMENT", json!({"expression": assign}))
        })
        .collect();
    if let Some(statements) = ctor["body"]["statements"].as_array_mut() {
        statements.extend(assignments);
    }
}

/// Whether the code assigns a field of the class `owner`.
fn assigns_field(v: &Value, owner: &str) -> bool {
    match v {
        Value::Object(fields) => {
            let symbol = &v["variable"]["symbol"];
            (text(v, "kind").ends_with("ASSIGNMENT")
                && text(symbol, "kind") == "FIELD"
                && text(symbol, "owner") == owner)
                || fields.values().any(|f| assigns_field(f, owner))
        }
        Value::Array(items) => items.iter().any(|i| assigns_field(i, owner)),
        _ => false,
    }
}

/// The field of a component, read in a method of the record.
fn field(c: &Component) -> Value {
    json!({"kind": "IDENTIFIER", "line": c.line, "column": c.column, "javaType": c.ty, "symbol": c.symbol, "name": c.name})
}

/// The position that the written nodes take: the record's, or a component's.
struct Pos {
    line: Value,
    column: Value,
}

impl Pos {
    /// A node of `kind` with these fields, at this position.
    fn node(&self, kind: &str, fields: Value) -> Value {
        let mut node = Map::new();
        node.insert("kind".to_string(), json!(kind));
        node.insert("line".to_string(), self.line.clone());
        node.insert("column".to_string(), self.column.clone());
        if let Value::Object(fields) = fields {
            node.extend(fields);
        }
        Value::Object(node)
    }

    /// A public instance method of `owner` that returns `ret`.
    fn method(
        &self,
        owner: &str,
        name: &str,
        ret: &str,
        parameters: Vec<Value>,
        statements: Vec<Value>,
        overrides: Vec<Value>,
    ) -> Value {
        let param_types: Vec<&str> = parameters.iter().map(|p| text(&p["symbol"], "type")).collect();
        let mut symbol = json!({
            "kind": "METHOD", "name": name, "owner": owner, "static": false,
            "type": format!("({}){ret}", param_types.join(",")),
        });
        if !overrides.is_empty() {
            symbol["overrides"] = json!(overrides);
        }
        let body = self.node("BLOCK", json!({"statements": statements, "static": false}));
        let mut method = self.node(
            "METHOD",
            json!({
                "symbol": symbol,
                "body": body,
                "modifiers": {"kind": "MODIFIERS", "annotations": [], "flags": ["PUBLIC"]},
                "name": name,
                "parameters": parameters,
                "returnType": self.node("IDENTIFIER", json!({"javaType": ret, "name": ret})),
                "throws": [],
                "typeParameters": [],
            }),
        );
        method["synthetic"] = json!(true);
        method
    }

    fn string(&self, s: &str) -> Value {
        self.node("STRING_LITERAL", json!({"javaType": "java.lang.String", "value": s}))
    }

    fn int(&self, i: i32) -> Value {
        self.node("INT_LITERAL", json!({"javaType": "int", "value": i.to_string()}))
    }

    fn binary(&self, kind: &str, ty: &str, left: Value, right: Value) -> Value {
        self.node(kind, json!({"javaType": ty, "leftOperand": left, "rightOperand": right}))
    }

    /// A call of the static method `owner.name` of the library, of type `sig`.
    fn library_call(&self, owner: &str, name: &str, sig: &str, ret: &str, args: Vec<Value>) -> Value {
        self.call(owner, name, sig, ret, args, false)
    }

    /// A call of `java.util.Objects.name` on a value of `ty`, which does not make the program use
    /// `Object` when the value is a string, a box, an enum or a record.
    fn objects_call(
        &self,
        name: &str,
        sig: &str,
        ret: &str,
        args: Vec<Value>,
        ty: &str,
        survey: &Survey,
    ) -> Value {
        self.call("java.util.Objects", name, sig, ret, args, survey.plain_value(ty))
    }

    fn call(&self, owner: &str, name: &str, sig: &str, ret: &str, args: Vec<Value>, implicit: bool) -> Value {
        let symbol = json!({"kind": "METHOD", "name": name, "owner": owner, "static": true, "type": sig});
        let class = self.node(
            "IDENTIFIER",
            json!({"javaType": owner, "name": owner.rsplit('.').next().unwrap_or(owner),
                   "symbol": {"kind": "CLASS", "name": owner.rsplit('.').next().unwrap_or(owner), "owner": "", "static": false, "type": owner}}),
        );
        let select = self.node(
            "MEMBER_SELECT",
            json!({"javaType": sig, "symbol": symbol, "expression": class, "identifier": name}),
        );
        let mut call = self.node(
            "METHOD_INVOCATION",
            json!({"javaType": ret, "symbol": symbol, "arguments": args, "methodSelect": select, "typeArguments": []}),
        );
        if implicit {
            call["implicit"] = json!(true);
        }
        call
    }

    /// `"Point[x=" + x + ", y=" + y + "]"`.
    fn to_string_text(&self, simple: &str, components: &[Component]) -> Value {
        let concat = |l, r| self.binary("PLUS", "java.lang.String", l, r);
        let mut text = None;
        for (i, c) in components.iter().enumerate() {
            let label =
                format!("{}{}=", if i == 0 { format!("{simple}[") } else { ", ".to_string() }, c.name);
            let head = match text.take() {
                None => self.string(&label),
                Some(t) => concat(t, self.string(&label)),
            };
            text = Some(concat(head, field(c)));
        }
        match text {
            None => self.string(&format!("{simple}[]")),
            Some(t) => concat(t, self.string("]")),
        }
    }

    /// The hash of the components: `31 * (31 * hash(a) + hash(b)) + hash(c)`.
    fn hash(&self, components: &[Component], survey: &Survey) -> Value {
        let mut acc: Option<Value> = None;
        for c in components {
            let h = match c.ty.as_str() {
                "int" => field(c),
                "long" => self.library_call("java.lang.Long", "hashCode", "(long)int", "int", vec![field(c)]),
                "double" => {
                    self.library_call("java.lang.Double", "hashCode", "(double)int", "int", vec![field(c)])
                }
                "float" => {
                    self.library_call("java.lang.Float", "hashCode", "(float)int", "int", vec![field(c)])
                }
                "boolean" => {
                    self.library_call("java.lang.Boolean", "hashCode", "(boolean)int", "int", vec![field(c)])
                }
                "char" => {
                    self.library_call("java.lang.Character", "hashCode", "(char)int", "int", vec![field(c)])
                }
                "short" => {
                    self.library_call("java.lang.Short", "hashCode", "(short)int", "int", vec![field(c)])
                }
                "byte" => self.library_call("java.lang.Byte", "hashCode", "(byte)int", "int", vec![field(c)]),
                // An array hashes by its identity, which no program can depend on: here it adds
                // nothing, which keeps equal records (with the same arrays) hashing alike.
                ty if ty.ends_with("[]") => self.int(0),
                _ => self.objects_call(
                    "hashCode",
                    "(java.lang.Object)int",
                    "int",
                    vec![field(c)],
                    &c.ty,
                    survey,
                ),
            };
            acc = Some(match acc {
                None => h,
                Some(a) => self.binary("PLUS", "int", self.binary("MULTIPLY", "int", self.int(31), a), h),
            });
        }
        acc.unwrap_or_else(|| self.int(0))
    }

    /// `other instanceof Point that && x == that.x && ..`.
    fn equals_test(&self, this_type: &str, components: &[Component], survey: &Survey) -> Value {
        let owner = "equals(java.lang.Object)";
        let other = self.node(
            "IDENTIFIER",
            json!({"name": "other", "javaType": "java.lang.Object",
                   "symbol": {"kind": "PARAMETER", "name": "other", "owner": owner, "static": false, "type": "java.lang.Object"}}),
        );
        let that_symbol = json!({"kind": "BINDING_VARIABLE", "name": "that", "owner": owner, "static": false, "type": this_type});
        let variable = self.node(
            "VARIABLE",
            json!({"name": "that", "symbol": that_symbol,
                   "modifiers": {"kind": "MODIFIERS", "annotations": [], "flags": []},
                   "type": self.node("IDENTIFIER", json!({"javaType": this_type, "name": this_type}))}),
        );
        let pattern = self.node("BINDING_PATTERN", json!({"variable": variable}));
        let mut test = self.node(
            "INSTANCE_OF",
            json!({"javaType": "boolean", "expression": other, "pattern": pattern,
                   "type": self.node("IDENTIFIER", json!({"javaType": this_type, "name": this_type}))}),
        );
        for c in components {
            let that = self
                .node("IDENTIFIER", json!({"name": "that", "javaType": this_type, "symbol": that_symbol}));
            let theirs = self.node(
                "MEMBER_SELECT",
                json!({"javaType": c.ty, "symbol": c.symbol, "expression": that, "identifier": c.name}),
            );
            let same = match c.ty.as_str() {
                "double" | "float" => {
                    let owner = if c.ty == "double" { "java.lang.Double" } else { "java.lang.Float" };
                    let sig = format!("({0},{0})int", c.ty);
                    let compare = self.library_call(owner, "compare", &sig, "int", vec![field(c), theirs]);
                    self.binary("EQUAL_TO", "boolean", compare, self.int(0))
                }
                "int" | "long" | "short" | "byte" | "char" | "boolean" => {
                    self.binary("EQUAL_TO", "boolean", field(c), theirs)
                }
                // An array is equal only to itself.
                ty if ty.ends_with("[]") => self.binary("EQUAL_TO", "boolean", field(c), theirs),
                _ => self.objects_call(
                    "equals",
                    "(java.lang.Object,java.lang.Object)boolean",
                    "boolean",
                    vec![field(c), theirs],
                    &c.ty,
                    survey,
                ),
            };
            test = self.binary("CONDITIONAL_AND", "boolean", test, same);
        }
        test
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn completion(accessors: Vec<Option<Vec<&str>>>, hash_code: bool, equals: bool) -> Completion {
        let accessors = accessors
            .into_iter()
            .map(|a| a.map(|o| o.into_iter().map(|i| (i.to_string(), "()int".to_string())).collect()))
            .collect();
        Completion(vec![RecordCompletion { accessors, hash_code, equals, plain: Vec::new() }])
    }

    #[test]
    fn a_completion_adds_to_another_when_it_keeps_every_member_written() {
        let before = completion(vec![None, Some(vec!["a.I"])], false, true);
        assert!(before.only_adds_to(&before));
        // An accessor and `hashCode` written, an accessor that overrides more.
        assert!(completion(vec![Some(vec![]), Some(vec!["a.I", "a.J"])], true, true).only_adds_to(&before));
        // An accessor no longer written, one that overrides less, `equals` no longer written.
        assert!(!completion(vec![None, None], false, true).only_adds_to(&before));
        assert!(!completion(vec![None, Some(vec![])], false, true).only_adds_to(&before));
        assert!(!completion(vec![None, Some(vec!["a.I"])], false, false).only_adds_to(&before));
    }
}
