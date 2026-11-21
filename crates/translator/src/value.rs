//! javac's trees as the translator holds them: JSON values like `serde_json::Value`, with the same
//! methods, in a fraction of its memory. A `serde_json` object is a `BTreeMap` whose smallest
//! node has room for eleven entries, each key a `String` of its own, so a node of a few fields
//! takes over half a kilobyte, about ten times its JSON text. Here an object is a list of its
//! values beside its fields' names, and every string, key or value, is interned: the few hundred
//! keys and the names and types that repeat across the units are each held once, for the whole
//! run, in tables that the threads reading a round of javac's trees share (`javac::parse_units`).
//!
//! A value is 16 bytes: an interned string is a pointer to the one copy of its text, and the
//! elements of an array are behind one pointer to their length and items (`ThinVec`). An object
//! is one allocation of a pointer to the names of its fields, held once for all objects with the
//! same names, and its values, 16 bytes each (`Map`); one whose values are all strings, numbers,
//! booleans or nulls is held once for all objects equal to it (`Map::leaf`). With a `&str`,
//! `Vec`s and (name, value) pairs, a value took 32 bytes and a field 48, and the Minecraft
//! server's trees about 9 GB.

use std::alloc::Layout;
use std::cell::Cell;
use std::fmt;
use std::ops::{Deref, Index, IndexMut};
use std::ptr::NonNull;
use std::sync::{LazyLock, Mutex};
use thin_vec::ThinVec;

/// An interned string: held once for the whole run, so a copy is a pointer, to the place that
/// holds the text's address and length.
#[derive(Clone, Copy)]
pub struct Str(&'static &'static str);

/// The number of sets that the interned strings are spread over, by their hash, each behind a
/// lock of its own: the threads that read a round of javac's trees at once
/// (`javac::parse_units`) intern their strings without waiting for each other.
const SHARDS: usize = 64;

/// The strings interned so far.
static STRINGS: LazyLock<[Mutex<crate::analysis::FastSet<Str>>; SHARDS]> =
    LazyLock::new(|| std::array::from_fn(|_| Mutex::default()));

/// How many of the strings it interned last each thread keeps (`RECENT`).
const RECENT_STRINGS: usize = 4096;

thread_local! {
    /// Strings this thread interned, each at the place its hash gives: the names of fields and
    /// kinds of node that nearly every node of javac's trees has are found here, without a lock.
    /// Each was taken from `STRINGS`, so it is the one copy of its text.
    static RECENT: [Cell<Option<Str>>; RECENT_STRINGS] =
        const { [const { Cell::new(None) }; RECENT_STRINGS] };
}

impl Str {
    pub fn new(s: &str) -> Str {
        use std::hash::BuildHasher;
        let hash = std::hash::BuildHasherDefault::<crate::analysis::IndexHasher>::default().hash_one(s);
        let slot = hash as usize % RECENT_STRINGS;
        if let Some(held) = RECENT.with(|recent| recent[slot].get())
            && held.as_str() == s
        {
            return held;
        }
        // The bits of the hash that choose the set are none of those its table uses.
        let mut strings = STRINGS[(hash >> 40) as usize % SHARDS].lock().unwrap_or_else(|e| e.into_inner());
        let held = match strings.get(s) {
            Some(&held) => held,
            None => {
                let held = Str(Box::leak(Box::new(&*Box::leak(s.to_owned().into_boxed_str()))));
                strings.insert(held);
                held
            }
        };
        drop(strings);
        RECENT.with(|recent| recent[slot].set(Some(held)));
        held
    }

    pub fn as_str(&self) -> &'static str {
        self.0
    }
}

// Strings are interned, so two are the same string exactly when they are at the same place; their
// hash and order are those of their text, as a `Str` is looked up by a `&str` (`Borrow`).
impl PartialEq for Str {
    fn eq(&self, other: &Str) -> bool {
        std::ptr::eq(self.0, other.0)
    }
}

impl Eq for Str {}

impl std::hash::Hash for Str {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl PartialOrd for Str {
    fn partial_cmp(&self, other: &Str) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Str {
    fn cmp(&self, other: &Str) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl Deref for Str {
    type Target = str;
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for Str {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::borrow::Borrow<str> for Str {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Debug for Str {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl fmt::Display for Str {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_str())
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
        s.as_str().to_string()
    }
}

impl PartialEq<str> for Str {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for Str {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<String> for Str {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<Str> for str {
    fn eq(&self, other: &Str) -> bool {
        self == other.as_str()
    }
}

impl PartialEq<Str> for &str {
    fn eq(&self, other: &Str) -> bool {
        *self == other.as_str()
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
///
/// The names of the fields are held once for every object with the same names (`keys`): an
/// object is one allocation of a pointer to those names and its values, 16 bytes and 16 for each
/// field, where a list of (name, value) pairs took 16 and 24 for each.
pub struct Map(NonNull<Header>);

/// The start of an object's allocation, whose values follow it.
#[repr(C)]
struct Header {
    keys: &'static [Str],
}

/// The header of every object without fields, which is never freed.
static EMPTY: Header = Header { keys: &[] };

/// The lists of field names held so far, each once (`Map::keys`).
static KEYS: LazyLock<Mutex<crate::analysis::FastSet<&'static [Str]>>> = LazyLock::new(Default::default);

/// How many of the lists of field names it held last each thread keeps (`RECENT_KEYS`).
const RECENT_KEY_LISTS: usize = 1024;

thread_local! {
    /// Lists of field names this thread held, each at the place that the places of its names
    /// give: javac's trees have a few hundred, which every object takes one of. Each was taken
    /// from `KEYS`, so it is the one copy of its names.
    static RECENT_KEYS: [Cell<&'static [Str]>; RECENT_KEY_LISTS] =
        const { [const { Cell::new(&[]) }; RECENT_KEY_LISTS] };
}

/// The one copy of the names `keys`.
fn held_keys(keys: &[Str]) -> &'static [Str] {
    use std::hash::Hasher;
    if keys.is_empty() {
        return &[];
    }
    // Interned names are the same exactly when they are at the same place.
    let mut hasher = crate::analysis::IndexHasher::default();
    for key in keys {
        hasher.write_usize(std::ptr::from_ref(key.0).addr());
    }
    let slot = hasher.finish() as usize % RECENT_KEY_LISTS;
    let recent = RECENT_KEYS.with(|recent| recent[slot].get());
    if recent == keys {
        return recent;
    }
    let mut held = KEYS.lock().unwrap_or_else(|e| e.into_inner());
    let found = match held.get(keys) {
        Some(&found) => found,
        None => {
            let found: &'static [Str] = Box::leak(keys.to_vec().into_boxed_slice());
            held.insert(found);
            found
        }
    };
    drop(held);
    RECENT_KEYS.with(|recent| recent[slot].set(found));
    found
}

// A `Map` owns its values as a `Box<[Value]>` would.
unsafe impl Send for Map {}
unsafe impl Sync for Map {}

/// The most fields of an object that `Map::find` scans rather than searches.
const SCANNED: usize = 12;

impl Map {
    pub fn new() -> Map {
        Map(NonNull::from(&EMPTY))
    }

    /// The layout of an object of `n` fields, and where its values start.
    fn layout(n: usize) -> (Layout, usize) {
        Layout::new::<Header>()
            .extend(Layout::array::<Value>(n).expect("an object too large"))
            .expect("an object too large")
    }

    /// The object with fields `keys`, in the order of their names and each once, and `values`,
    /// one for each.
    fn from_parts(keys: &[Str], values: impl ExactSizeIterator<Item = Value>) -> Map {
        assert_eq!(keys.len(), values.len());
        if keys.is_empty() {
            return Map::new();
        }
        let keys = held_keys(keys);
        let (layout, offset) = Map::layout(keys.len());
        // SAFETY: the layout has the header's size, which is not zero; the header and each value
        // are written before the map is used, at the offsets of the layout.
        unsafe {
            let start = std::alloc::alloc(layout);
            let Some(header) = NonNull::new(start.cast::<Header>()) else {
                std::alloc::handle_alloc_error(layout)
            };
            header.as_ptr().write(Header { keys });
            let first = start.add(offset).cast::<Value>();
            let mut written = 0;
            for value in values.take(keys.len()) {
                first.add(written).write(value);
                written += 1;
            }
            assert_eq!(written, keys.len(), "an iterator gave fewer values than its length");
            Map(header)
        }
    }

    /// The object of fields `keys`, as `from_parts`, whose `values` are all strings, numbers,
    /// booleans or nulls: the one copy of it (`LEAVES`).
    fn leaf(keys: &[Str], values: Vec<Value>) -> Map {
        let keys = held_keys(keys);
        if keys.is_empty() {
            return Map::new();
        }
        assert_eq!(keys.len(), values.len());
        let hash = leaf_hash(keys, &values);
        let mut leaves = LEAVES.lock().unwrap_or_else(|e| e.into_inner());
        let bucket = leaves.entry(hash).or_default();
        for held in bucket.iter() {
            let map = std::mem::ManuallyDrop::new(Map(held.0.map_addr(|a| a | SHARED)));
            if std::ptr::eq(map.keys_slice(), keys)
                && map.values_slice().iter().zip(&values).all(|(a, b)| same(a, b))
            {
                // SAFETY: the count of a shared object changes only under the lock of `LEAVES`.
                unsafe { *count(held.0.as_ptr()) += 1 };
                return Map(map.0);
            }
        }
        let (layout, offset) = shared_layout(keys.len());
        // SAFETY: as in `from_parts`, after the count, which the layout puts first.
        unsafe {
            let start = std::alloc::alloc(layout);
            if start.is_null() {
                std::alloc::handle_alloc_error(layout)
            }
            start.cast::<usize>().write(1);
            let header = NonNull::new_unchecked(start.add(offset).cast::<Header>());
            header.as_ptr().write(Header { keys });
            let first = start.add(offset + Map::layout(keys.len()).1).cast::<Value>();
            for (i, value) in values.into_iter().enumerate() {
                first.add(i).write(value);
            }
            bucket.push(Leaf(header));
            Map(header.map_addr(|a| a | SHARED))
        }
    }

    /// Whether the object is the one copy of objects equal to it (`leaf`).
    fn is_shared(&self) -> bool {
        self.0.addr().get() & SHARED != 0
    }

    /// Where the header is.
    fn header(&self) -> *mut Header {
        self.0.as_ptr().map_addr(|a| a & !SHARED)
    }

    /// The names of the fields, in order.
    fn keys_slice(&self) -> &'static [Str] {
        // SAFETY: the header is `EMPTY` or one that `from_parts` or `leaf` wrote, alive as long
        // as `self`.
        unsafe { (*self.header()).keys }
    }

    /// Where the values start.
    fn first(&self) -> *mut Value {
        let n = self.keys_slice().len();
        if n == 0 {
            return NonNull::dangling().as_ptr();
        }
        // SAFETY: the offset is within the allocation of `n` values (`layout`).
        unsafe { self.header().cast::<u8>().add(Map::layout(n).1).cast::<Value>() }
    }

    fn values_slice(&self) -> &[Value] {
        // SAFETY: the values are initialized and alive as long as `self`.
        unsafe { std::slice::from_raw_parts(self.first(), self.keys_slice().len()) }
    }

    /// The values, to change: a shared object (`leaf`) is copied first.
    fn values_slice_mut(&mut self) -> &mut [Value] {
        if self.is_shared() {
            *self = Map::from_parts(self.keys_slice(), self.values_slice().iter().cloned());
        }
        // SAFETY: as `values_slice`, owned by `self` alone and borrowed mutably with it.
        unsafe { std::slice::from_raw_parts_mut(self.first(), self.keys_slice().len()) }
    }

    /// The values, moved out, with the allocation freed.
    fn into_values(self) -> Vec<Value> {
        if self.is_shared() {
            return self.values_slice().to_vec();
        }
        let this = std::mem::ManuallyDrop::new(self);
        let n = this.keys_slice().len();
        let mut values = Vec::with_capacity(n);
        if n > 0 {
            // SAFETY: each value is read once, then the allocation is freed without dropping
            // them (`this` is not dropped).
            unsafe {
                values.extend((0..n).map(|i| this.first().add(i).read()));
                std::alloc::dealloc(this.header().cast::<u8>(), Map::layout(n).0);
            }
        }
        values
    }

    /// Where field `key` is, or would be.
    fn search(&self, key: &str) -> Result<usize, usize> {
        // The first bytes decide most comparisons, without comparing the strings in full.
        let first = key.as_bytes().first();
        self.keys_slice()
            .binary_search_by(|k| k.as_str().as_bytes().first().cmp(&first).then_with(|| k.as_str().cmp(key)))
    }

    /// Where field `key` is, if the object has it. A tree's node has a few fields, which a scan
    /// that compares lengths first finds sooner than a binary search of the names.
    fn find(&self, key: &str) -> Option<usize> {
        let keys = self.keys_slice();
        match keys.len() {
            0..=SCANNED => keys.iter().position(|k| k.as_str().len() == key.len() && k.as_str() == key),
            _ => self.search(key).ok(),
        }
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.find(key).map(|i| &self.values_slice()[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.find(key).map(|i| &mut self.values_slice_mut()[i])
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.find(key).is_some()
    }

    /// Sets field `key`, returning its value before.
    pub fn insert(&mut self, key: impl Into<Str>, value: Value) -> Option<Value> {
        let key = key.into();
        match self.search(&key) {
            Ok(i) => Some(std::mem::replace(&mut self.values_slice_mut()[i], value)),
            Err(i) => {
                let mut keys = self.keys_slice().to_vec();
                keys.insert(i, key);
                let mut values = std::mem::take(self).into_values();
                values.insert(i, value);
                *self = Map::from_parts(&keys, values.into_iter());
                None
            }
        }
    }

    pub fn remove(&mut self, key: &str) -> Option<Value> {
        let i = self.search(key).ok()?;
        let mut keys = self.keys_slice().to_vec();
        keys.remove(i);
        let mut values = std::mem::take(self).into_values();
        let removed = values.remove(i);
        *self = Map::from_parts(&keys, values.into_iter());
        Some(removed)
    }

    pub fn len(&self) -> usize {
        self.keys_slice().len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys_slice().is_empty()
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (&Str, &Value)> + ExactSizeIterator {
        self.into_iter()
    }

    pub fn iter_mut(&mut self) -> impl DoubleEndedIterator<Item = (&Str, &mut Value)> + ExactSizeIterator {
        self.into_iter()
    }

    pub fn keys(&self) -> impl Iterator<Item = &Str> {
        self.keys_slice().iter()
    }

    pub fn values(&self) -> impl Iterator<Item = &Value> {
        self.values_slice().iter()
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut Value> {
        self.values_slice_mut().iter_mut()
    }

    /// Sets the fields of `other`, as `insert` sets each.
    pub fn extend(&mut self, other: impl IntoIterator<Item = (Str, Value)>) {
        for (k, v) in other {
            self.insert(k, v);
        }
    }

    /// The object of `fields`, which may be in any order: one named twice keeps its last value.
    fn of_fields(mut fields: Vec<(Str, Value)>) -> Map {
        fields.sort_by(|(a, _), (b, _)| a.as_str().cmp(b.as_str()));
        let mut kept: Vec<(Str, Value)> = Vec::with_capacity(fields.len());
        for (key, value) in fields {
            match kept.last_mut() {
                Some(last) if last.0 == key => last.1 = value,
                _ => kept.push((key, value)),
            }
        }
        let keys: Vec<Str> = kept.iter().map(|(k, _)| *k).collect();
        Map::from_parts(&keys, kept.into_iter().map(|(_, v)| v))
    }
}

impl Drop for Map {
    fn drop(&mut self) {
        let n = self.keys_slice().len();
        if n == 0 {
            return;
        }
        if self.is_shared() {
            let mut leaves = LEAVES.lock().unwrap_or_else(|e| e.into_inner());
            let header = self.header();
            // SAFETY: the count changes only under the lock; the last object to go frees the
            // allocation that `leaf` made, after taking it out of `LEAVES`.
            unsafe {
                *count(header) -= 1;
                if *count(header) > 0 {
                    return;
                }
                let hash = leaf_hash(self.keys_slice(), self.values_slice());
                if let Some(bucket) = leaves.get_mut(&hash) {
                    bucket.retain(|l| l.0.as_ptr() != header);
                    if bucket.is_empty() {
                        leaves.remove(&hash);
                    }
                }
                std::ptr::drop_in_place(std::ptr::slice_from_raw_parts_mut(self.first(), n));
                let (layout, offset) = shared_layout(n);
                std::alloc::dealloc(header.cast::<u8>().sub(offset), layout);
            }
            return;
        }
        // SAFETY: the values are initialized and owned, and the allocation is the one that
        // `from_parts` made for `n` values.
        unsafe {
            std::ptr::drop_in_place(std::ptr::slice_from_raw_parts_mut(self.first(), n));
            std::alloc::dealloc(self.header().cast::<u8>(), Map::layout(n).0);
        }
    }
}

impl Clone for Map {
    fn clone(&self) -> Map {
        if self.is_shared() {
            let _leaves = LEAVES.lock().unwrap_or_else(|e| e.into_inner());
            // SAFETY: the count changes only under the lock.
            unsafe { *count(self.header()) += 1 };
            return Map(self.0);
        }
        Map::from_parts(self.keys_slice(), self.values_slice().iter().cloned())
    }
}

/// The bit of a `Map`'s pointer that marks an object shared by all those equal to it
/// (`Map::leaf`); headers are aligned to eight bytes.
const SHARED: usize = 1;

/// An object held by `LEAVES`.
struct Leaf(NonNull<Header>);

// The objects of `LEAVES` change only under its lock.
unsafe impl Send for Leaf {}

/// The objects of javac's trees whose values are all strings, numbers, booleans or nulls (the
/// nodes of most identifiers, literals and primitive types), each held once for all equal to it,
/// by `leaf_hash`, with a count of the copies of it before its header (`count`). Most such nodes
/// repeat: on javac's census, 1.57 million are 402 thousand different ones.
static LEAVES: LazyLock<Mutex<crate::analysis::FastMap<u64, Vec<Leaf>>>> = LazyLock::new(Default::default);

/// The layout of a shared object of `n` fields (`Map::leaf`): its count, then the object as
/// `Map::layout`; and where the header starts.
fn shared_layout(n: usize) -> (Layout, usize) {
    Layout::new::<usize>().extend(Map::layout(n).0).expect("an object too large")
}

/// The count of copies of a shared object, whose header is `header`.
///
/// # Safety
/// The object is shared (`Map::leaf`) and alive, and the lock of `LEAVES` is held.
unsafe fn count(header: *mut Header) -> *mut usize {
    unsafe { header.cast::<u8>().sub(shared_layout(0).1).cast::<usize>() }
}

/// The hash of an object of `LEAVES`.
fn leaf_hash(keys: &'static [Str], values: &[Value]) -> u64 {
    use std::hash::Hasher;
    let mut hasher = crate::analysis::IndexHasher::default();
    hasher.write_usize(keys.as_ptr().addr());
    for value in values {
        let (kind, bits) = match value {
            Value::Null => (0, 0),
            Value::Bool(b) => (1, *b as u64),
            Value::Number(Number::Int(n)) => (2, *n as u64),
            Value::Number(Number::Float(x)) => (3, x.to_bits()),
            Value::String(s) => (4, s.as_str().as_ptr().addr() as u64),
            Value::Array(_) | Value::Object(_) => (5, 0),
        };
        hasher.write_usize(kind);
        hasher.write_u64(bits);
    }
    hasher.finish()
}

/// Whether two values of objects of `LEAVES` are the same: a number of the same bits.
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(Number::Float(x)), Value::Number(Number::Float(y))) => x.to_bits() == y.to_bits(),
        (Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_), _) => a == b,
        _ => false,
    }
}

impl PartialEq for Map {
    fn eq(&self, other: &Map) -> bool {
        self.keys_slice() == other.keys_slice() && self.values_slice() == other.values_slice()
    }
}

impl Default for Map {
    fn default() -> Map {
        Map::new()
    }
}

impl<'a> IntoIterator for &'a Map {
    type Item = (&'a Str, &'a Value);
    type IntoIter = std::iter::Zip<std::slice::Iter<'a, Str>, std::slice::Iter<'a, Value>>;
    fn into_iter(self) -> Self::IntoIter {
        self.keys_slice().iter().zip(self.values_slice())
    }
}

impl<'a> IntoIterator for &'a mut Map {
    type Item = (&'a Str, &'a mut Value);
    type IntoIter = std::iter::Zip<std::slice::Iter<'a, Str>, std::slice::IterMut<'a, Value>>;
    fn into_iter(self) -> Self::IntoIter {
        let keys = self.keys_slice();
        keys.iter().zip(self.values_slice_mut())
    }
}

impl IntoIterator for Map {
    type Item = (Str, Value);
    type IntoIter =
        std::iter::Zip<std::iter::Copied<std::slice::Iter<'static, Str>>, std::vec::IntoIter<Value>>;
    fn into_iter(self) -> Self::IntoIter {
        let keys = self.keys_slice();
        keys.iter().copied().zip(self.into_values())
    }
}

impl<K: Into<Str>> FromIterator<(K, Value)> for Map {
    fn from_iter<I: IntoIterator<Item = (K, Value)>>(iter: I) -> Map {
        Map::of_fields(iter.into_iter().map(|(k, v)| (k.into(), v)).collect())
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
    Array(ThinVec<Value>),
    Object(Map),
}

static NULL: Value = Value::Null;

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s.as_str()),
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

    pub fn as_array(&self) -> Option<&ThinVec<Value>> {
        match self {
            Value::Array(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_array_mut(&mut self) -> Option<&mut ThinVec<Value>> {
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
        if fields.find(self).is_none() {
            fields.insert(self, Value::Null);
        }
        fields.get_mut(self).expect("a field just set")
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
        Value::Array(items.into())
    }
}

impl From<ThinVec<Value>> for Value {
    fn from(items: ThinVec<Value>) -> Value {
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
            Value::String(s) => serde_json::Value::String(s.as_str().to_string()),
            Value::Array(items) => {
                serde_json::Value::Array(items.iter().map(serde_json::Value::from).collect())
            }
            Value::Object(fields) => serde_json::Value::Object(
                fields.iter().map(|(k, v)| (k.as_str().to_string(), serde_json::Value::from(v))).collect(),
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
            Value::String(s) => serializer.serialize_str(s.as_str()),
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
            map.serialize_entry(k.as_str(), v)?;
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
        // The elements go on a stack that every array being read shares, and are moved from there
        // into an array of their exact length: one allocation per array, not one per doubling.
        let start = ITEMS.with_borrow(Vec::len);
        while let Some(item) = seq.next_element::<Value>()? {
            ITEMS.with_borrow_mut(|items| items.push(item));
        }
        Ok(Value::Array(ITEMS.with_borrow_mut(|items| exact(items.drain(start..)))))
    }

    fn visit_map<A: serde::de::MapAccess<'de>>(self, mut access: A) -> Result<Value, A::Error> {
        let start = FIELDS.with_borrow(Vec::len);
        while let Some(key) = access.next_key::<KeyStr>()? {
            let value = access.next_value::<Value>()?;
            FIELDS.with_borrow_mut(|fields| fields.push((key.0, value)));
        }
        Ok(Value::Object(FIELDS.with_borrow_mut(|fields| {
            let read = &mut fields[start..];
            // In the order of their names; a key that javac writes twice keeps the last value, as
            // `serde_json` does.
            read.sort_by(|(a, _), (b, _)| a.as_str().cmp(b.as_str()));
            let mut keys: Vec<Str> = Vec::with_capacity(read.len());
            let mut values: Vec<Value> = Vec::with_capacity(read.len());
            for (key, value) in fields.drain(start..) {
                match keys.last() {
                    Some(&last) if last == key => *values.last_mut().expect("a value for each key") = value,
                    _ => {
                        keys.push(key);
                        values.push(value);
                    }
                }
            }
            if values
                .iter()
                .all(|v| matches!(v, Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_)))
            {
                Map::leaf(&keys, values)
            } else {
                Map::from_parts(&keys, values.into_iter())
            }
        })))
    }
}

thread_local! {
    /// The elements of the arrays being read, innermost last (`Visitor::visit_seq`).
    static ITEMS: std::cell::RefCell<Vec<Value>> = const { std::cell::RefCell::new(Vec::new()) };
    /// The fields of the objects being read, innermost last (`Visitor::visit_map`).
    static FIELDS: std::cell::RefCell<Vec<(Str, Value)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The items, in a list of their exact length.
fn exact<T>(items: impl ExactSizeIterator<Item = T>) -> ThinVec<T> {
    let mut out = ThinVec::with_capacity(items.len());
    out.extend(items);
    out
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn objects_read_set_and_remove_fields() {
        let mut v: Value =
            serde_json::from_str(r#"{"b": [1, {"x": null}], "a": "s", "b": 2, "c": {}}"#).unwrap();
        assert_eq!(v.to_string(), r#"{"a":"s","b":2,"c":{}}"#);
        v["d"] = Value::from(true);
        v["a"] = Value::from(3i64);
        let fields = v.as_object_mut().unwrap();
        assert_eq!(fields.remove("b"), Some(Value::from(2i64)));
        assert_eq!(fields.remove("b"), None);
        assert_eq!(fields.len(), 3);
        let copy = v.clone();
        assert_eq!(copy, v);
        assert_eq!(v.to_string(), r#"{"a":3,"c":{},"d":true}"#);
        let Value::Object(fields) = v else { unreachable!() };
        let pairs: Vec<(String, Value)> = fields.into_iter().map(|(k, v)| (k.into(), v)).collect();
        assert_eq!(pairs.len(), 3);
        assert_eq!(pairs[2], ("d".to_string(), Value::from(true)));
        let mut emptied: Map = [("k", Value::from("v"))].into_iter().collect();
        emptied.remove("k");
        assert!(emptied.is_empty());
        assert_eq!(emptied, Map::new());
    }

    #[test]
    fn strings_and_names_interned_on_several_threads_are_one_copy_each() {
        // Each text five times, interned by each thread in an order of its own.
        let texts: Vec<String> = (0..25_000).map(|i| format!("interned on threads {}", i % 5_000)).collect();
        let names: Vec<Vec<String>> =
            (0..2_000).map(|i| (0..1 + i % 4).map(|k| format!("field {} {k}", i % 400)).collect()).collect();
        let found: Vec<(Vec<usize>, Vec<usize>)> = std::thread::scope(|scope| {
            let threads: Vec<_> = (0..8)
                .map(|t| {
                    let (texts, names) = (&texts, &names);
                    scope.spawn(move || {
                        let order = |n: usize| (0..n).map(move |i| (i + t * 997) % n);
                        let mut strings = vec![0; texts.len()];
                        for i in order(texts.len()) {
                            strings[i] = Str::new(&texts[i]).as_str().as_ptr().addr();
                        }
                        let mut lists = vec![0; names.len()];
                        for i in order(names.len()) {
                            let keys: Vec<Str> = names[i].iter().map(|n| Str::new(n)).collect();
                            lists[i] = held_keys(&keys).as_ptr().addr();
                        }
                        (strings, lists)
                    })
                })
                .collect();
            threads.into_iter().map(|t| t.join().unwrap()).collect()
        });
        for other in &found[1..] {
            assert_eq!(other, &found[0]);
        }
        let (strings, lists) = &found[0];
        assert!((0..texts.len()).all(|i| strings[i] == strings[i % 5_000]));
        // The same names are the same list, as the list of the first index with them.
        let mut first = std::collections::HashMap::new();
        for (i, list) in names.iter().enumerate() {
            assert_eq!(lists[i], lists[*first.entry(list).or_insert(i)]);
        }
        assert_eq!(first.len(), 400);
    }

    #[test]
    fn equal_leaves_are_shared_and_copied_to_change() {
        let text =
            r#"[{"k": "a", "n": 1.5}, {"k": "a", "n": 1.5}, {"k": "a", "n": -0.0}, {"k": "a", "n": 0.0}]"#;
        let mut v: Value = serde_json::from_str(text).unwrap();
        let items = v.as_array().unwrap();
        let shared = |i: usize| items[i].as_object().unwrap().0;
        assert_eq!(shared(0), shared(1));
        assert_ne!(shared(2), shared(3));
        let copy = v.clone();
        v[1]["n"] = Value::from(2i64);
        v[0].as_object_mut().unwrap().insert("m", Value::Null);
        assert_eq!(
            v.to_string(),
            r#"[{"k":"a","m":null,"n":1.5},{"k":"a","n":2},{"k":"a","n":-0.0},{"k":"a","n":0.0}]"#
        );
        assert_eq!(
            copy.to_string(),
            r#"[{"k":"a","n":1.5},{"k":"a","n":1.5},{"k":"a","n":-0.0},{"k":"a","n":0.0}]"#
        );
        drop((v, copy));
        let again: Value = serde_json::from_str(text).unwrap();
        assert_eq!(again[1]["n"].as_f64(), Some(1.5));
    }
}
