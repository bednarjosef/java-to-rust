//! The static fields that the bodies of an enum's constants declare (`BYTE { private static final
//! ByteBuffer EMPTY_BUFFER = ..; }`), moved into the enum: lowering moves a body's methods into
//! the enum (`lower::constant_bodies`), and its static fields become the enum's own, declared right
//! after the constants, with the enum as their owner in their declaration and in the body's uses.
//! Java initializes such a field with the body's class, as its constant is built; here it is
//! initialized after every constant, which only an initializer with effects can tell apart. A
//! field whose name the enum or another body declares too stays in its body, where lowering
//! rejects it.

use crate::value::Value;
use std::collections::HashMap;

/// Moves the static fields of the constants' bodies of every enum in `v` into their enums.
pub(super) fn hoist_body_statics(v: &mut Value) {
    match v {
        Value::Object(fields) => {
            fields.values_mut().for_each(hoist_body_statics);
            if fields.get("kind").and_then(Value::as_str) == Some("ENUM") {
                hoist_in_enum(v);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(hoist_body_statics),
        _ => {}
    }
}

/// Whether some enum in `v` has a constant whose body declares a static field.
pub(super) fn has_body_statics(v: &Value) -> bool {
    match v {
        Value::Object(fields) => {
            (fields.get("kind").and_then(Value::as_str) == Some("ENUM")
                && members(v).filter_map(body).any(|b| members(b).any(is_static_field)))
                || fields.values().any(has_body_statics)
        }
        Value::Array(items) => items.iter().any(has_body_statics),
        _ => false,
    }
}

fn hoist_in_enum(decl: &mut Value) {
    let owner = decl["symbol"]["type"].as_str().unwrap_or("").to_string();
    // How many declarations of the enum and of its constants' bodies have each name.
    let mut declared: HashMap<String, usize> = HashMap::new();
    for member in members(decl) {
        let names = std::iter::once(member).chain(body(member).into_iter().flat_map(members));
        for name in names.filter(|m| m["kind"] == "VARIABLE").filter_map(|m| m["name"].as_str()) {
            *declared.entry(name.to_string()).or_default() += 1;
        }
    }
    let Some(list) = decl["members"].as_array_mut() else { return };
    let mut hoisted = Vec::new();
    for member in list.iter_mut() {
        // Indexing a value for writing makes the fields it names, so a body is looked for first.
        if body(member).is_none() {
            continue;
        }
        let Some(body) = member["initializer"]["classBody"]["members"].as_array_mut() else { continue };
        let (moved, kept): (Vec<Value>, Vec<Value>) = body.drain(..).partition(|m| {
            is_static_field(m) && m["name"].as_str().is_some_and(|n| declared.get(n) == Some(&1))
        });
        *body = kept.into();
        for mut field in moved {
            let name = field["name"].as_str().unwrap_or("").to_string();
            for m in body.iter_mut() {
                name_owner(m, &name, &owner);
            }
            name_owner(&mut field, &name, &owner);
            hoisted.push(field);
        }
    }
    let after = list.iter().rposition(is_constant).map_or(0, |i| i + 1);
    list.splice(after..after, hoisted);
}

/// Gives the uses and the declaration in `v` of the body's static field `name` the owner `owner`.
fn name_owner(v: &mut Value, name: &str, owner: &str) {
    match v {
        Value::Object(fields) => {
            if let Some(symbol) = fields.get_mut("symbol")
                && symbol["kind"] == "FIELD"
                && symbol["name"] == name
                && symbol["owner"] == ""
                && symbol["static"] == true
            {
                symbol["owner"] = Value::from(owner);
            }
            fields.values_mut().for_each(|x| name_owner(x, name, owner));
        }
        Value::Array(items) => items.iter_mut().for_each(|x| name_owner(x, name, owner)),
        _ => {}
    }
}

fn members(decl: &Value) -> impl Iterator<Item = &Value> {
    decl["members"].as_array().into_iter().flatten()
}

fn is_constant(member: &Value) -> bool {
    member["symbol"]["kind"] == "ENUM_CONSTANT"
}

/// The body of an enum constant, if it has one.
fn body(member: &Value) -> Option<&Value> {
    let body = &member["initializer"]["classBody"];
    (is_constant(member) && !body.is_null()).then_some(body)
}

fn is_static_field(member: &Value) -> bool {
    member["kind"] == "VARIABLE"
        && member["modifiers"]["flags"].as_array().is_some_and(|f| f.iter().any(|x| x == "STATIC"))
}
