//! Java arrays. An array is a shared handle to its elements: copying the handle gives a second
//! name for the same array, as in Java, and every access is bounds-checked the way Java checks it.
//!
//! A program that starts threads has its arrays behind an `Arc` and a `java::Locked`
//! (`SharedArray`), which one thread at a time may borrow; the functions here take either handle
//! (`Elements`), and those that make an array have a `shared_` twin.

use crate::{Exception, Locked, throw, uncaught};
use std::cell::RefCell;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::sync::Arc;

pub type Array<T> = Rc<RefCell<Vec<T>>>;

/// An array of a program with threads.
pub type SharedArray<T> = Arc<Locked<Vec<T>>>;

/// A handle of an array: an `Array`, or a `SharedArray`, whose elements are borrowed the same way.
pub trait Elements<T> {
    fn items(&self) -> impl Deref<Target = Vec<T>> + '_;
    fn items_mut(&self) -> impl DerefMut<Target = Vec<T>> + '_;
    /// A new array of the same kind, such as a copy.
    fn made(items: Vec<T>) -> Self;
    /// The array's address, which is its identity.
    fn address(&self) -> *const u8;
}

impl<T> Elements<T> for Array<T> {
    #[inline]
    fn items(&self) -> impl Deref<Target = Vec<T>> + '_ {
        self.borrow()
    }

    #[inline]
    fn items_mut(&self) -> impl DerefMut<Target = Vec<T>> + '_ {
        self.borrow_mut()
    }

    fn made(items: Vec<T>) -> Array<T> {
        from_vec(items)
    }

    fn address(&self) -> *const u8 {
        Rc::as_ptr(self) as *const u8
    }
}

impl<T> Elements<T> for SharedArray<T> {
    #[inline]
    fn items(&self) -> impl Deref<Target = Vec<T>> + '_ {
        self.borrow()
    }

    #[inline]
    fn items_mut(&self) -> impl DerefMut<Target = Vec<T>> + '_ {
        self.borrow_mut()
    }

    fn made(items: Vec<T>) -> SharedArray<T> {
        shared_from_vec(items)
    }

    fn address(&self) -> *const u8 {
        Arc::as_ptr(self) as *const u8
    }
}

/// An array with the given elements, as in `{a, b, c}`.
pub fn from_vec<T>(items: Vec<T>) -> Array<T> {
    Rc::new(RefCell::new(items))
}

/// `from_vec` in a program with threads.
pub fn shared_from_vec<T>(items: Vec<T>) -> SharedArray<T> {
    Arc::new(Locked::new(items))
}

/// An array of a program with threads that has the elements of `a`, which the runtime's own
/// functions made (`String.getBytes`, `Character.toChars`).
pub fn to_shared<T: Clone>(a: Array<T>) -> SharedArray<T> {
    shared_from_vec(Rc::try_unwrap(a).map_or_else(|a| a.borrow().clone(), RefCell::into_inner))
}

/// `new T[len]` with every element `zero`, the default value of `T`.
pub fn filled<T: Clone>(len: i32, zero: T) -> Array<T> {
    let len = size(len);
    from_vec(vec![zero; len])
}

/// `new T[len]` whose element `i` is `make(i)`, so that each row of `new T[n][m]` is a new array.
pub fn tabulate<T>(len: i32, make: impl FnMut(usize) -> T) -> Array<T> {
    let len = size(len);
    from_vec((0..len).map(make).collect())
}

/// `filled` in a program with threads.
pub fn shared_filled<T: Clone>(len: i32, zero: T) -> SharedArray<T> {
    let len = size(len);
    shared_from_vec(vec![zero; len])
}

/// `tabulate` in a program with threads.
pub fn shared_tabulate<T>(len: i32, make: impl FnMut(usize) -> T) -> SharedArray<T> {
    let len = size(len);
    shared_from_vec((0..len).map(make).collect())
}

/// The length of an array being created; Java throws for a negative length.
#[inline]
pub(crate) fn size(len: i32) -> usize {
    if len < 0 {
        negative_size(len);
    }
    len as usize
}

#[cold]
#[inline(never)]
fn negative_size(len: i32) -> ! {
    throw("java.lang.NegativeArraySizeException", &len.to_string())
}

/// `size` where the program observes the `NegativeArraySizeException`.
pub(crate) fn try_size(len: i32) -> Result<usize, Exception> {
    if len < 0 {
        return Err(Exception::new("java.lang.NegativeArraySizeException", len.to_string()));
    }
    Ok(len as usize)
}

/// The position of element `i` in an array of `len` elements, checked the way Java checks it.
/// A negative `i` becomes a huge `usize`, so one comparison checks both ends. The check is
/// inlined into every access and the throw is kept out of line, so an access that stays in
/// bounds costs a compare and a branch that is never taken.
#[inline]
pub(crate) fn position(len: usize, i: i32) -> usize {
    if (i as usize) < len { i as usize } else { out_of_bounds(len, i) }
}

/// Throws Java's `ArrayIndexOutOfBoundsException` for index `i` of an array of `len` elements.
#[cold]
#[inline(never)]
fn out_of_bounds(len: usize, i: i32) -> ! {
    throw("java.lang.ArrayIndexOutOfBoundsException", &format!("Index {i} out of bounds for length {len}"))
}

/// `position` where the program observes the `ArrayIndexOutOfBoundsException`.
pub(crate) fn try_position(len: usize, i: i32) -> Result<usize, Exception> {
    if (i as usize) < len {
        return Ok(i as usize);
    }
    let message = format!("Index {i} out of bounds for length {len}");
    Err(Exception::new("java.lang.ArrayIndexOutOfBoundsException", message))
}

/// `new T[len]` where the program observes the `NegativeArraySizeException` of a negative length.
pub fn try_filled<T: Clone>(len: i32, zero: T) -> Result<Array<T>, Exception> {
    Ok(from_vec(vec![zero; try_size(len)?]))
}

/// `new T[len]` of arrays where the program observes the `NegativeArraySizeException`.
pub fn try_tabulate<T>(len: i32, make: impl FnMut(usize) -> T) -> Result<Array<T>, Exception> {
    Ok(from_vec((0..try_size(len)?).map(make).collect()))
}

/// `try_filled` in a program with threads.
pub fn try_shared_filled<T: Clone>(len: i32, zero: T) -> Result<SharedArray<T>, Exception> {
    Ok(shared_from_vec(vec![zero; try_size(len)?]))
}

/// `try_tabulate` in a program with threads.
pub fn try_shared_tabulate<T>(len: i32, make: impl FnMut(usize) -> T) -> Result<SharedArray<T>, Exception> {
    Ok(shared_from_vec((0..try_size(len)?).map(make).collect()))
}

/// `a.length`.
#[inline]
pub fn len<T>(a: &impl Elements<T>) -> i32 {
    a.items().len() as i32
}

/// `a[i]`.
#[inline]
pub fn get<T: Clone>(a: &impl Elements<T>, i: i32) -> T {
    let items = a.items();
    items[position(items.len(), i)].clone()
}

/// `a[i]` where the program observes the `ArrayIndexOutOfBoundsException`.
pub fn try_get<T: Clone>(a: &impl Elements<T>, i: i32) -> Result<T, Exception> {
    let items = a.items();
    Ok(items[try_position(items.len(), i)?].clone())
}

/// `a[i] = v` where the program observes the `ArrayIndexOutOfBoundsException`.
pub fn try_set<T>(a: &impl Elements<T>, i: i32, v: T) -> Result<(), Exception> {
    let mut items = a.items_mut();
    let i = try_position(items.len(), i)?;
    items[i] = v;
    Ok(())
}

/// `a[i] = v`. Java evaluates `a`, `i` and `v` before the check, and so do the arguments.
#[inline]
pub fn set<T>(a: &impl Elements<T>, i: i32, v: T) {
    let mut items = a.items_mut();
    let i = position(items.len(), i);
    items[i] = v;
}

/// `a[i] op= ...`: `f` computes the new element from the old one. The old element is read, and
/// the borrow released, before `f` runs, so `f` may read the array. Returns the new element.
pub fn update<T: Clone>(a: &impl Elements<T>, i: i32, f: impl FnOnce(T) -> T) -> T {
    let new = f(get(a, i));
    set(a, i, new.clone());
    new
}

/// `a[i]++` and the like: as `update`, but returns the old element.
pub fn post_update<T: Clone>(a: &impl Elements<T>, i: i32, f: impl FnOnce(T) -> T) -> T {
    let old = get(a, i);
    set(a, i, f(old.clone()));
    old
}

/// `a[i] op= ...` where the program observes the exceptions of the index or of `f` (a division
/// by zero).
pub fn try_update<T: Clone>(
    a: &Array<T>,
    i: i32,
    f: impl FnOnce(T) -> Result<T, Exception>,
) -> Result<T, Exception> {
    let new = f(try_get(a, i)?)?;
    try_set(a, i, new.clone())?;
    Ok(new)
}

/// `a[i]++` and the like where the program observes the exceptions: returns the old element.
pub fn try_post_update<T: Clone>(
    a: &Array<T>,
    i: i32,
    f: impl FnOnce(T) -> Result<T, Exception>,
) -> Result<T, Exception> {
    let old = try_get(a, i)?;
    try_set(a, i, f(old.clone())?)?;
    Ok(old)
}

/// `Arrays.toString(a)`, with `show` formatting each element as Java does.
pub fn to_string<T: Clone>(a: &impl Elements<T>, show: impl Fn(T) -> String) -> String {
    // The elements are copied out first: `show` may run code of the program, which must not run
    // while the array is borrowed (with threads, locked).
    let items: Vec<T> = a.items().clone();
    let items: Vec<String> = items.into_iter().map(show).collect();
    format!("[{}]", items.join(", "))
}

/// `Arrays.fill(a, v)`.
pub fn fill<T: Clone>(a: &impl Elements<T>, v: T) {
    a.items_mut().fill(v);
}

/// `Arrays.sort(a)` for integers and booleans.
pub fn sort<T: Ord>(a: &impl Elements<T>) {
    a.items_mut().sort();
}

/// `Arrays.sort(a)` for doubles, in Java's order (`Double.compare`): -0.0 before 0.0, and every
/// NaN last, whatever its sign bit (`total_cmp` puts a negative NaN, which x86 makes for `0.0 / 0.0`,
/// first).
pub fn sort_doubles(a: &impl Elements<f64>) {
    let key = |v: f64| if v.is_nan() { i64::MAX } else { v.to_bits() as i64 };
    a.items_mut()
        .sort_by(|x, y| x.partial_cmp(y).filter(|o| o.is_ne()).unwrap_or_else(|| key(*x).cmp(&key(*y))));
}

/// `Arrays.sort(a)` for floats, in Java's order (`Float.compare`), as for doubles.
pub fn sort_floats(a: &impl Elements<f32>) {
    let key = |v: f32| if v.is_nan() { i32::MAX } else { v.to_bits() as i32 };
    a.items_mut()
        .sort_by(|x, y| x.partial_cmp(y).filter(|o| o.is_ne()).unwrap_or_else(|| key(*x).cmp(&key(*y))));
}

/// `Arrays.copyOf(a, len)`: the first `len` elements, padded with `zero`.
pub fn copy_of<T: Clone, A: Elements<T>>(a: &A, len: i32, zero: T) -> A {
    let len = size(len);
    let mut items = a.items().clone();
    items.resize(len, zero);
    A::made(items)
}

/// `a[i] = v` as an expression: stores `v` and gives it back.
pub fn store<T: Clone>(a: &impl Elements<T>, i: i32, v: T) -> T {
    set(a, i, v.clone());
    v
}

/// `a[i] = v` as an expression, where the program observes the exception.
pub fn try_store<T: Clone>(a: &impl Elements<T>, i: i32, v: T) -> Result<T, Exception> {
    try_set(a, i, v.clone())?;
    Ok(v)
}

/// `a[i] = v` as an expression, where the elements may be null and `v` is not: the value of the
/// assignment is `v` itself, not an `Option`.
pub fn store_some<T: Clone>(a: &impl Elements<Option<T>>, i: i32, v: T) -> T {
    set(a, i, Some(v.clone()));
    v
}

/// `store_some`, where the program observes the exception.
pub fn try_store_some<T: Clone>(a: &impl Elements<Option<T>>, i: i32, v: T) -> Result<T, Exception> {
    try_set(a, i, Some(v.clone()))?;
    Ok(v)
}

/// The elements of `a` in order, read one at a time as a for-each loop reads them. The length
/// is fixed when the loop starts, and the loop holds its own handle, as Java's does.
pub fn each<T: Clone>(a: impl Elements<T>) -> impl Iterator<Item = T> {
    let len = len(&a);
    (0..len).map(move |k| get(&a, k))
}

/// Checks the sizes of a multi-dimensional `new`, before any array is allocated.
pub fn check_sizes(dims: &[i32]) {
    try_check_sizes(dims).unwrap_or_else(uncaught)
}

/// `check_sizes` where the program observes the `NegativeArraySizeException`.
pub fn try_check_sizes(dims: &[i32]) -> Result<(), Exception> {
    for &d in dims {
        try_size(d)?;
    }
    Ok(())
}
