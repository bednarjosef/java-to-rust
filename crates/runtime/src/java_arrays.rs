//! `java.util.Arrays` on arrays of primitives and strings, and `clone()` of arrays. Each element
//! type says how Java compares, hashes and tests it equal, so one function serves every type.

use crate::{
    Elements, MadeElements, bool_hash, double_compare, double_hash, float_hash, len, long_hash, string_hash,
    throw,
};
use std::cmp::Ordering;

/// What `java.util.Arrays` does with an element: Java's `compareTo`, `equals` and `hashCode`.
pub trait JavaElement: Clone {
    fn java_cmp(&self, other: &Self) -> Ordering;

    fn java_eq(&self, other: &Self) -> bool {
        self.java_cmp(other).is_eq()
    }

    fn java_hash(&self) -> i32;
}

macro_rules! integers {
    ($($t:ty),*) => {$(
        impl JavaElement for $t {
            fn java_cmp(&self, other: &Self) -> Ordering {
                self.cmp(other)
            }

            fn java_hash(&self) -> i32 {
                *self as i32
            }
        }
    )*};
}

integers!(i8, i16, i32, u16);

impl JavaElement for i64 {
    fn java_cmp(&self, other: &Self) -> Ordering {
        self.cmp(other)
    }

    fn java_hash(&self) -> i32 {
        long_hash(*self)
    }
}

impl JavaElement for bool {
    fn java_cmp(&self, other: &Self) -> Ordering {
        self.cmp(other)
    }

    fn java_hash(&self) -> i32 {
        bool_hash(*self)
    }
}

impl JavaElement for f64 {
    fn java_cmp(&self, other: &Self) -> Ordering {
        double_compare(*self, *other).cmp(&0)
    }

    fn java_hash(&self) -> i32 {
        double_hash(*self)
    }
}

impl JavaElement for f32 {
    fn java_cmp(&self, other: &Self) -> Ordering {
        double_compare(*self as f64, *other as f64).cmp(&0)
    }

    fn java_hash(&self) -> i32 {
        float_hash(*self)
    }
}

impl JavaElement for String {
    fn java_cmp(&self, other: &Self) -> Ordering {
        compare_strings(self, other)
    }

    fn java_hash(&self) -> i32 {
        string_hash(self)
    }
}

/// A string that may be null: comparing a null throws `NullPointerException`, as Java's
/// `compareTo` does, while `equals` and `hashCode` accept it.
impl<T: JavaElement> JavaElement for Option<T> {
    fn java_cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Some(a), Some(b)) => a.java_cmp(b),
            _ => throw("java.lang.NullPointerException", ""),
        }
    }

    fn java_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Some(a), Some(b)) => a.java_eq(b),
            (None, None) => true,
            _ => false,
        }
    }

    fn java_hash(&self) -> i32 {
        self.as_ref().map_or(0, JavaElement::java_hash)
    }
}

/// `String.compareTo`: the UTF-16 code units, in order.
pub fn compare_strings(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// Java's `rangeCheck` for the methods that take a range `from..to` of an array of `len` elements.
fn range(len: usize, from: i32, to: i32) -> std::ops::Range<usize> {
    if from > to {
        throw("java.lang.IllegalArgumentException", &format!("fromIndex({from}) > toIndex({to})"));
    }
    if from < 0 {
        throw("java.lang.ArrayIndexOutOfBoundsException", &format!("Array index out of range: {from}"));
    }
    if to as usize > len {
        throw("java.lang.ArrayIndexOutOfBoundsException", &format!("Array index out of range: {to}"));
    }
    from as usize..to as usize
}

/// `Arrays.equals(a, b)`: the same length, and equal elements in order. A null array equals only
/// another null array.
pub fn equals<T: JavaElement, A: Elements<T>>(a: Option<&A>, b: Option<&A>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => {
            let (a, b) = (a.items(), b.items());
            a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.java_eq(y))
        }
        (None, None) => true,
        _ => false,
    }
}

/// `Arrays.hashCode(a)`: `31 * h + hash(e)` over the elements, from 1; 0 for a null array.
pub fn hash_code<T: JavaElement>(a: Option<&impl Elements<T>>) -> i32 {
    a.map_or(0, |a| a.items().iter().fold(1i32, |h, e| h.wrapping_mul(31).wrapping_add(e.java_hash())))
}

/// `Arrays.sort(a)` for elements that are not doubles or floats, in Java's order.
pub fn sort_all<T: JavaElement>(a: &impl Elements<T>) {
    a.items_mut().sort_by(JavaElement::java_cmp);
}

/// `Arrays.sort(a, from, to)`.
pub fn sort_range<T: JavaElement>(a: &impl Elements<T>, from: i32, to: i32) {
    let r = range(a.items().len(), from, to);
    a.items_mut()[r].sort_by(JavaElement::java_cmp);
}

/// `Arrays.fill(a, from, to, v)`.
pub fn fill_range<T: Clone>(a: &impl Elements<T>, from: i32, to: i32, v: T) {
    let r = range(a.items().len(), from, to);
    a.items_mut()[r].fill(v);
}

/// `Arrays.copyOfRange(a, from, to)`: the elements from `from` up to `to`, padded with `zero` past
/// the end of `a`.
pub fn copy_of_range<T: Clone, A: MadeElements<T>>(a: &A, from: i32, to: i32, zero: T) -> A {
    if from > to {
        throw("java.lang.IllegalArgumentException", &format!("{from} > {to}"));
    }
    let items = a.items();
    if from < 0 || from as usize > items.len() {
        throw("java.lang.ArrayIndexOutOfBoundsException", &format!("Array index out of range: {from}"));
    }
    let mut copy: Vec<T> = items[from as usize..].to_vec();
    copy.resize(to as usize - from as usize, zero);
    A::made(copy)
}

/// `Arrays.copyOfRange(a, from, to)` of an array that may be null, where a catch sees the
/// `NullPointerException` (with `message`): the JDK's code asks a null array of `objects` for its
/// class first, and checks the range of one of primitives first unless `from` is 0.
pub fn try_copy_of_range<T: Clone, A: MadeElements<T>>(
    a: Option<&A>,
    from: i32,
    to: i32,
    zero: T,
    message: Option<&'static str>,
    objects: bool,
) -> Result<A, crate::Exception> {
    match a {
        Some(a) => Ok(copy_of_range(a, from, to, zero)),
        None => {
            if !objects && from != 0 && from > to {
                throw("java.lang.IllegalArgumentException", &format!("{from} > {to}"));
            }
            Err(match message {
                Some(message) => crate::Exception::new("java.lang.NullPointerException", message),
                None => crate::Exception::bare("java.lang.NullPointerException"),
            })
        }
    }
}

/// `Arrays.binarySearch(a, key)`: the index of `key`, or `-(insertion point) - 1`.
pub fn binary_search_all<T: JavaElement>(a: &impl Elements<T>, key: T) -> i32 {
    binary_search_range(a, 0, len(a), key)
}

/// `Arrays.binarySearch(a, key)` of a key that may be null: `None` where Java compares a null
/// key with an element, which fails in the element's `compareTo` (the caller throws that
/// `NullPointerException`); an empty array has nothing to compare it with.
pub fn binary_search_all_nullable<T: JavaElement>(a: &impl Elements<T>, key: Option<T>) -> Option<i32> {
    binary_search_range_nullable(a, 0, len(a), key)
}

/// `Arrays.binarySearch(a, from, to, key)` of a key that may be null, as
/// `binary_search_all_nullable`: the range is checked first.
pub fn binary_search_range_nullable<T: JavaElement>(
    a: &impl Elements<T>,
    from: i32,
    to: i32,
    key: Option<T>,
) -> Option<i32> {
    match key {
        Some(key) => Some(binary_search_range(a, from, to, key)),
        None => {
            let r = range(a.items().len(), from, to);
            r.is_empty().then(|| -(r.start as i32) - 1)
        }
    }
}

/// `Arrays.binarySearch(a, key)`, or of `range` (`from`, `to`), over elements that may be null,
/// where a catch sees the `NullPointerException` of comparing a null: Java's, with `null_element`
/// as the message where the element compared is null (the JDK calls its `compareTo`), and
/// `null_key` where the key is (the element's `compareTo` reads it). The range is checked first.
pub fn try_binary_search<T: JavaElement>(
    a: &impl Elements<Option<T>>,
    range: Option<(i32, i32)>,
    key: Option<T>,
    null_element: &'static str,
    null_key: &'static str,
) -> Result<i32, crate::Exception> {
    let items = a.items();
    let r = match range {
        Some((from, to)) => self::range(items.len(), from, to),
        None => 0..items.len(),
    };
    let (mut low, mut high) = (r.start as i64, r.end as i64 - 1);
    let npe = |message| Err(crate::Exception::new("java.lang.NullPointerException", message));
    while low <= high {
        let mid = (low + high) >> 1;
        let (Some(element), Some(key)) = (&items[mid as usize], &key) else {
            return npe(if items[mid as usize].is_none() { null_element } else { null_key });
        };
        match element.java_cmp(key) {
            Ordering::Less => low = mid + 1,
            Ordering::Greater => high = mid - 1,
            Ordering::Equal => return Ok(mid as i32),
        }
    }
    Ok(-(low as i32) - 1)
}

/// `Arrays.binarySearch(a, from, to, key)`, with Java's algorithm, so that the same index is found
/// among equal elements.
pub fn binary_search_range<T: JavaElement>(a: &impl Elements<T>, from: i32, to: i32, key: T) -> i32 {
    let items = a.items();
    let r = range(items.len(), from, to);
    let (mut low, mut high) = (r.start as i64, r.end as i64 - 1);
    while low <= high {
        let mid = (low + high) >> 1;
        match items[mid as usize].java_cmp(&key) {
            Ordering::Less => low = mid + 1,
            Ordering::Greater => high = mid - 1,
            Ordering::Equal => return mid as i32,
        }
    }
    -(low as i32) - 1
}

/// `Arrays.deepToString(a)`: `show` formats each element, a row being formatted the same way; a
/// null array is `null`.
pub fn deep_to_string<T: Clone>(a: Option<&impl Elements<T>>, show: impl Fn(T) -> String) -> String {
    match a {
        Some(a) => {
            let items: Vec<T> = a.items().clone();
            let items: Vec<String> = items.into_iter().map(show).collect();
            format!("[{}]", items.join(", "))
        }
        None => String::from("null"),
    }
}

/// `a.clone()`: a new array of the same elements. The elements are handles, so rows of an array
/// of arrays are shared with the copy, as in Java.
pub fn clone_array<T: Clone, A: MadeElements<T>>(a: &A) -> A {
    A::made(a.items().clone())
}
