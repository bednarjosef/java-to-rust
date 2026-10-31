//! javac's trees as the translator holds them: JSON values like `serde_json::Value`, with the same
//! methods, in a fraction of its memory. A `serde_json` object is a `BTreeMap` whose smallest
//! node has room for eleven entries, each key a `String` of its own, so a node of a few fields
//! takes over half a kilobyte, about ten times its JSON text. Here an object is a list of its
//! fields, and every string, key or value, is interned: the few hundred keys and the names and
//! types that repeat across the units are each held once, for the whole run.

use std::fmt;
use std::ops::{Deref, Index, IndexMut};
use std::sync::{LazyLock, Mutex};

/// An interned string: held once for the whole run, so a copy is a pointer.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Str(&'static str);

/// The strings interned so far.
static STRINGS: LazyLock<Mutex<crate::analysis::FastSet<&'static str>>> = LazyLock::new(Default::default);

impl Str {
    pub fn new(s: &str) -> Str {
        let mut strings = STRINGS.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(&held) = strings.get(s) {
            return Str(held);
        }
        let held: &'static str = Box::leak(s.to_owned().into_boxed_str());
        strings.insert(held);
        Str(held)
    }

    pub fn as_str(&self) -> &'static str {
        self.0
    }
}

impl Deref for Str {
    type Target = str;
    fn deref(&self) -> &str {
        self.0
    }
}

impl AsRef<str> for Str {
    fn as_ref(&self) -> &str {
        self.0
    }
}

impl std::borrow::Borrow<str> for Str {
    fn borrow(&self) -> &str {
        self.0
    }
}

impl fmt::Debug for Str {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self.0, f)
    }
}

impl fmt::Display for Str {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl From<&str> for Str {
    fn from(s: &str) -> Str {
        Str::new(s)
    }
}

impl From<String> for Str {
    fn from(s: String) -> Str {
        Str::new(&s)
    }
}

impl From<&String> for Str {
    fn from(s: &String) -> Str {
        Str::new(s)
    }
}

impl From<Str> for String {
    fn from(s: Str) -> String {
        s.0.to_string()
    }
}

impl PartialEq<str> for Str {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for Str {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<String> for Str {
    fn eq(&self, other: &String) -> bool {
        self.0 == other
    }
}

impl PartialEq<Str> for str {
    fn eq(&self, other: &Str) -> bool {
        self == other.0
    }
}

impl PartialEq<Str> for &str {
    fn eq(&self, other: &Str) -> bool {
        *self == other.0
    }
}

/// A number, as javac writes them: an integer, or a floating-point constant.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Number {
    Int(i64),
    Float(f64),
}

impl Number {
    pub fn as_u64(&self) -> Option<u64> {
        match *self {
            Number::Int(n) => u64::try_from(n).ok(),
            Number::Float(_) => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match *self {
            Number::Int(n) => Some(n),
            Number::Float(_) => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match *self {
            Number::Int(n) => Some(n as f64),
            Number::Float(x) => Some(x),
        }
    }

    pub fn is_i64(&self) -> bool {
        matches!(self, Number::Int(_))
    }

    pub fn is_u64(&self) -> bool {
        matches!(self, Number::Int(n) if *n >= 0)
    }

    pub fn is_f64(&self) -> bool {
        matches!(self, Number::Float(_))
    }
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Number::Int(n) => write!(f, "{n}"),
            Number::Float(x) => write!(
                f,
                "{}",
                serde_json::Number::from_f64(*x).map_or("null".to_string(), |n| n.to_string())
            ),
        }
    }
}

/// A JSON object: its fields in the order of their names, as `serde_json`'s `BTreeMap` keeps
/// them (the walks of the trees see them in that order).
#[derive(Clone, PartialEq, Default)]
pub struct Map(Vec<(Str, Value)>);

/// The most fields of an object that `Map::find` scans rather than searches.
const SCANNED: usize = 12;

impl Map {
    pub fn new() -> Map {
        Map(Vec::new())
    }

    /// Where field `key` is, or would be.
    fn search(&self, key: &str) -> Result<usize, usize> {
        // The first bytes decide most comparisons, without comparing the strings in full.
        let first = key.as_bytes().first();
        self.0.binary_search_by(|(k, _)| k.0.as_bytes().first().cmp(&first).then_with(|| k.0.cmp(key)))
    }

    /// Where field `key` is, if the object has it. A tree's node has a few fields, which a scan
    /// that compares lengths first finds sooner than a binary search of the names.
    fn find(&self, key: &str) -> Option<usize> {
        match self.0.len() {
            0..=SCANNED => self.0.iter().position(|(k, _)| k.0.len() == key.len() && k.0 == key),
            _ => self.search(key).ok(),
        }
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.find(key).map(|i| &self.0[i].1)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.find(key).map(|i| &mut self.0[i].1)
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    /// Sets field `key`, returning its value before.
    pub fn insert(&mut self, key: impl Into<Str>, value: Value) -> Option<Value> {
        let key = key.into();
        match self.search(&key) {
            Ok(i) => Some(std::mem::replace(&mut self.0[i].1, value)),
            Err(i) => {
                self.0.insert(i, (key, value));
                None
            }
        }
    }

    pub fn remove(&mut self, key: &str) -> Option<Value> {
        let i = self.search(key).ok()?;
        Some(self.0.remove(i).1)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (&Str, &Value)> + ExactSizeIterator {
        self.0.iter().map(|(k, v)| (k, v))
    }

    pub fn iter_mut(&mut self) -> impl DoubleEndedIterator<Item = (&Str, &mut Value)> + ExactSizeIterator {
        self.0.iter_mut().map(|(k, v)| (&*k, v))
    }

    pub fn keys(&self) -> impl Iterator<Item = &Str> {
        self.0.iter().map(|(k, _)| k)
    }

    pub fn values(&self) -> impl Iterator<Item = &Value> {
        self.0.iter().map(|(_, v)| v)
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut Value> {
        self.0.iter_mut().map(|(_, v)| v)
    }

    /// Sets the fields of `other`, as `insert` sets each.
    pub fn extend(&mut self, other: impl IntoIterator<Item = (Str, Value)>) {
        for (k, v) in other {
            self.insert(k, v);
        }
    }

    fn shrink(&mut self) {
        self.0.shrink_to_fit();
    }
}

impl<'a> IntoIterator for &'a Map {
    type Item = (&'a Str, &'a Value);
    type IntoIter =
        std::iter::Map<std::slice::Iter<'a, (Str, Value)>, fn(&'a (Str, Value)) -> (&'a Str, &'a Value)>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter().map(|(k, v)| (k, v))
    }
}

impl<'a> IntoIterator for &'a mut Map {
    type Item = (&'a Str, &'a mut Value);
    type IntoIter = std::iter::Map<
        std::slice::IterMut<'a, (Str, Value)>,
        fn(&'a mut (Str, Value)) -> (&'a Str, &'a mut Value),
    >;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut().map(|(k, v)| (&*k, v))
    }
}

impl IntoIterator for Map {
    type Item = (Str, Value);
    type IntoIter = std::vec::IntoIter<(Str, Value)>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<K: Into<Str>> FromIterator<(K, Value)> for Map {
    fn from_iter<I: IntoIterator<Item = (K, Value)>>(iter: I) -> Map {
        let mut map = Map::new();
        for (k, v) in iter {
            map.insert(k, v);
        }
        map
    }
}

impl fmt::Debug for Map {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

/// A JSON value, as `serde_json::Value`.
#[derive(Clone, PartialEq, Default)]
pub enum Value {
    #[default]
    Null,
    Bool(bool),
    Number(Number),
    String(Str),
    Array(Vec<Value>),
    Object(Map),
}

static NULL: Value = Value::Null;

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s.0),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match *self {
            Value::Bool(b) => Some(b),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Value::Number(n) => n.as_u64(),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Number(n) => n.as_i64(),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Number(n) => n.as_f64(),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Vec<Value>> {
        match self {
            Value::Array(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_array_mut(&mut self) -> Option<&mut Vec<Value>> {
        match self {
            Value::Array(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&Map> {
        match self {
            Value::Object(fields) => Some(fields),
            _ => None,
        }
    }

    pub fn as_object_mut(&mut self) -> Option<&mut Map> {
        match self {
            Value::Object(fields) => Some(fields),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    pub fn is_object(&self) -> bool {
        matches!(self, Value::Object(_))
    }

    pub fn is_array(&self) -> bool {
        matches!(self, Value::Array(_))
    }

    pub fn is_string(&self) -> bool {
        matches!(self, Value::String(_))
    }

    pub fn is_boolean(&self) -> bool {
        matches!(self, Value::Bool(_))
    }

    pub fn is_number(&self) -> bool {
        matches!(self, Value::Number(_))
    }

    pub fn is_u64(&self) -> bool {
        matches!(self, Value::Number(n) if n.is_u64())
    }

    pub fn is_i64(&self) -> bool {
        matches!(self, Value::Number(n) if n.is_i64())
    }

    /// The field `index` of an object, or the element of an array.
    pub fn get<I: Key>(&self, index: I) -> Option<&Value> {
        index.find(self)
    }

    pub fn get_mut<I: Key>(&mut self, index: I) -> Option<&mut Value> {
        index.find_mut(self)
    }

    /// The value at a JSON pointer (`/a/0/b`), as `serde_json::Value::pointer`.
    pub fn pointer(&self, pointer: &str) -> Option<&Value> {
        if pointer.is_empty() {
            return Some(self);
        }
        let rest = pointer.strip_prefix('/')?;
        rest.split('/').map(|t| t.replace("~1", "/").replace("~0", "~")).try_fold(self, |v, token| match v {
            Value::Object(fields) => fields.get(&token),
            Value::Array(items) => token.parse::<usize>().ok().and_then(|i| items.get(i)),
            _ => None,
        })
    }

    pub fn pointer_mut(&mut self, pointer: &str) -> Option<&mut Value> {
        if pointer.is_empty() {
            return Some(self);
        }
        let rest = pointer.strip_prefix('/')?;
        rest.split('/').map(|t| t.replace("~1", "/").replace("~0", "~")).try_fold(self, |v, token| match v {
            Value::Object(fields) => fields.get_mut(&token),
            Value::Array(items) => token.parse::<usize>().ok().and_then(|i| items.get_mut(i)),
            _ => None,
        })
    }

    /// The value, leaving null in its place.
    pub fn take(&mut self) -> Value {
        std::mem::take(self)
    }
}

/// What indexes a value: a field's name or an element's index.
pub trait Key {
    fn find<'v>(&self, v: &'v Value) -> Option<&'v Value>;
    fn find_mut<'v>(&self, v: &'v mut Value) -> Option<&'v mut Value>;
    /// The place of the field in an object, made when missing (and the object made from null).
    fn place<'v>(&self, v: &'v mut Value) -> &'v mut Value;
}

impl Key for str {
    fn find<'v>(&self, v: &'v Value) -> Option<&'v Value> {
        v.as_object()?.get(self)
    }
    fn find_mut<'v>(&self, v: &'v mut Value) -> Option<&'v mut Value> {
        v.as_object_mut()?.get_mut(self)
    }
    fn place<'v>(&self, v: &'v mut Value) -> &'v mut Value {
        if v.is_null() {
            *v = Value::Object(Map::new());
        }
        let Value::Object(fields) = v else { panic!("cannot index a non-object JSON value with {self:?}") };
        let i = match fields.search(self) {
            Ok(i) => i,
            Err(i) => {
                fields.0.insert(i, (Str::new(self), Value::Null));
                i
            }
        };
        &mut fields.0[i].1
    }
}

impl Key for usize {
    fn find<'v>(&self, v: &'v Value) -> Option<&'v Value> {
        v.as_array()?.get(*self)
    }
    fn find_mut<'v>(&self, v: &'v mut Value) -> Option<&'v mut Value> {
        v.as_array_mut()?.get_mut(*self)
    }
    fn place<'v>(&self, v: &'v mut Value) -> &'v mut Value {
        match v {
            Value::Array(items) => {
                let len = items.len();
                items
                    .get_mut(*self)
                    .unwrap_or_else(|| panic!("index {self} out of range for a JSON array of {len}"))
            }
            _ => panic!("cannot index a non-array JSON value with {self}"),
        }
    }
}

impl Key for String {
    fn find<'v>(&self, v: &'v Value) -> Option<&'v Value> {
        Key::find(self.as_str(), v)
    }
    fn find_mut<'v>(&self, v: &'v mut Value) -> Option<&'v mut Value> {
        Key::find_mut(self.as_str(), v)
    }
    fn place<'v>(&self, v: &'v mut Value) -> &'v mut Value {
        Key::place(self.as_str(), v)
    }
}

impl<T: Key + ?Sized> Key for &T {
    fn find<'v>(&self, v: &'v Value) -> Option<&'v Value> {
        (**self).find(v)
    }
    fn find_mut<'v>(&self, v: &'v mut Value) -> Option<&'v mut Value> {
        (**self).find_mut(v)
    }
    fn place<'v>(&self, v: &'v mut Value) -> &'v mut Value {
        (**self).place(v)
    }
}

impl<I: Key> Index<I> for Value {
    type Output = Value;
    /// The field or element, or null where there is none, as `serde_json` gives.
    fn index(&self, index: I) -> &Value {
        index.find(self).unwrap_or(&NULL)
    }
}

impl<I: Key> IndexMut<I> for Value {
    fn index_mut(&mut self, index: I) -> &mut Value {
        index.place(self)
    }
}

impl PartialEq<str> for Value {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == Some(other)
    }
}

impl PartialEq<&str> for Value {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == Some(*other)
    }
}

impl PartialEq<String> for Value {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == Some(other.as_str())
    }
}

impl PartialEq<bool> for Value {
    fn eq(&self, other: &bool) -> bool {
        self.as_bool() == Some(*other)
    }
}

impl PartialEq<i64> for Value {
    fn eq(&self, other: &i64) -> bool {
        self.as_i64() == Some(*other)
    }
}

impl PartialEq<u64> for Value {
    fn eq(&self, other: &u64) -> bool {
        self.as_u64() == Some(*other)
    }
}

impl PartialEq<Value> for str {
    fn eq(&self, other: &Value) -> bool {
        other == self
    }
}

impl PartialEq<Value> for &str {
    fn eq(&self, other: &Value) -> bool {
        other == *self
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Value {
        Value::String(Str::new(s))
    }
}

impl From<String> for Value {
    fn from(s: String) -> Value {
        Value::String(Str::new(&s))
    }
}

impl From<Str> for Value {
    fn from(s: Str) -> Value {
        Value::String(s)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Value {
        Value::Bool(b)
    }
}

impl From<i64> for Value {
    fn from(n: i64) -> Value {
        Value::Number(Number::Int(n))
    }
}

impl From<u64> for Value {
    fn from(n: u64) -> Value {
        Value::Number(Number::Int(n as i64))
    }
}

impl From<usize> for Value {
    fn from(n: usize) -> Value {
        Value::Number(Number::Int(n as i64))
    }
}

impl From<i32> for Value {
    fn from(n: i32) -> Value {
        Value::Number(Number::Int(n.into()))
    }
}

impl From<Vec<Value>> for Value {
    fn from(items: Vec<Value>) -> Value {
        Value::Array(items)
    }
}

impl From<Map> for Value {
    fn from(fields: Map) -> Value {
        Value::Object(fields)
    }
}

impl From<serde_json::Value> for Value {
    fn from(v: serde_json::Value) -> Value {
        match v {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(b) => Value::Bool(b),
            serde_json::Value::Number(n) => Value::Number(match n.as_i64() {
                Some(i) => Number::Int(i),
                None => Number::Float(n.as_f64().unwrap_or_default()),
            }),
            serde_json::Value::String(s) => Value::String(Str::new(&s)),
            serde_json::Value::Array(items) => Value::Array(items.into_iter().map(Value::from).collect()),
            serde_json::Value::Object(fields) => {
                Value::Object(fields.into_iter().map(|(k, v)| (k, Value::from(v))).collect())
            }
        }
    }
}

impl From<&Value> for serde_json::Value {
    fn from(v: &Value) -> serde_json::Value {
        match v {
            Value::Null => serde_json::Value::Null,
            Value::Bool(b) => serde_json::Value::Bool(*b),
            Value::Number(Number::Int(n)) => serde_json::Value::from(*n),
            Value::Number(Number::Float(x)) => serde_json::Value::from(*x),
            Value::String(s) => serde_json::Value::String(s.0.to_string()),
            Value::Array(items) => {
                serde_json::Value::Array(items.iter().map(serde_json::Value::from).collect())
            }
            Value::Object(fields) => serde_json::Value::Object(
                fields.iter().map(|(k, v)| (k.0.to_string(), serde_json::Value::from(v))).collect(),
            ),
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Null => f.write_str("Null"),
            Value::Bool(b) => write!(f, "Bool({b})"),
            Value::Number(n) => write!(f, "Number({n})"),
            Value::String(s) => write!(f, "String({s:?})"),
            Value::Array(items) => {
                f.write_str("Array ")?;
                f.debug_list().entries(items).finish()
            }
            Value::Object(fields) => {
                f.write_str("Object ")?;
                fmt::Debug::fmt(fields, f)
            }
        }
    }
}

/// The value as JSON text, as `serde_json` writes it.
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let text = serde_json::to_string(self).map_err(|_| fmt::Error)?;
        f.write_str(&text)
    }
}

impl serde::Serialize for Value {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        match self {
            Value::Null => serializer.serialize_unit(),
            Value::Bool(b) => serializer.serialize_bool(*b),
            Value::Number(Number::Int(n)) => serializer.serialize_i64(*n),
            Value::Number(Number::Float(x)) => serializer.serialize_f64(*x),
            Value::String(s) => serializer.serialize_str(s.0),
            Value::Array(items) => {
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            Value::Object(fields) => fields.serialize(serializer),
        }
    }
}

impl serde::Serialize for Map {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(self.len()))?;
        for (k, v) in self {
            map.serialize_entry(k.0, v)?;
        }
        map.end()
    }
}

impl<'de> serde::Deserialize<'de> for Value {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(Visitor)
    }
}

struct Visitor;

impl<'de> serde::de::Visitor<'de> for Visitor {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a JSON value")
    }

    fn visit_unit<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_none<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_bool<E>(self, b: bool) -> Result<Value, E> {
        Ok(Value::Bool(b))
    }

    fn visit_i64<E>(self, n: i64) -> Result<Value, E> {
        Ok(Value::Number(Number::Int(n)))
    }

    fn visit_u64<E>(self, n: u64) -> Result<Value, E> {
        Ok(match i64::try_from(n) {
            Ok(n) => Value::Number(Number::Int(n)),
            Err(_) => Value::Number(Number::Float(n as f64)),
        })
    }

    fn visit_f64<E>(self, x: f64) -> Result<Value, E> {
        Ok(Value::Number(Number::Float(x)))
    }

    fn visit_str<E>(self, s: &str) -> Result<Value, E> {
        Ok(Value::String(Str::new(s)))
    }

    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut items = Vec::with_capacity(seq.size_hint().unwrap_or(0));
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        items.shrink_to_fit();
        Ok(Value::Array(items))
    }

    fn visit_map<A: serde::de::MapAccess<'de>>(self, mut access: A) -> Result<Value, A::Error> {
        let mut fields = Map(Vec::with_capacity(access.size_hint().unwrap_or(0)));
        while let Some(key) = access.next_key::<KeyStr>()? {
            let value = access.next_value()?;
            // A key that javac writes twice keeps the last value, as `serde_json` does.
            fields.insert(key.0, value);
        }
        fields.shrink();
        Ok(Value::Object(fields))
    }
}

/// A key of an object, interned as it is read.
struct KeyStr(Str);

impl<'de> serde::Deserialize<'de> for KeyStr {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<KeyStr, D::Error> {
        struct KeyVisitor;
        impl serde::de::Visitor<'_> for KeyVisitor {
            type Value = KeyStr;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a key")
            }
            fn visit_str<E>(self, s: &str) -> Result<KeyStr, E> {
                Ok(KeyStr(Str::new(s)))
            }
        }
        deserializer.deserialize_str(KeyVisitor)
    }
}

/// `serde_json::json!`, made into a `Value` of the translator.
macro_rules! json {
    ($($json:tt)+) => {
        $crate::value::Value::from(serde_json::json!($($json)+))
    };
}
pub(crate) use json;
