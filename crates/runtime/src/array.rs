//! Java arrays. An array is a shared handle to its elements: copying the handle gives a second
//! name for the same array, as in Java, and every access is bounds-checked the way Java checks it.

use crate::{Exception, throw, uncaught};
use std::cell::RefCell;
use std::rc::Rc;

pub type Array<T> = Rc<RefCell<Vec<T>>>;

/// An array with the given elements, as in `{a, b, c}`.
pub fn from_vec<T>(items: Vec<T>) -> Array<T> {
    Rc::new(RefCell::new(items))
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

/// `a.length`.
#[inline]
pub fn len<T>(a: &Array<T>) -> i32 {
    a.borrow().len() as i32
}

/// `a[i]`.
#[inline]
pub fn get<T: Clone>(a: &Array<T>, i: i32) -> T {
    let items = a.borrow();
    items[position(items.len(), i)].clone()
}

/// `a[i]` where the program observes the `ArrayIndexOutOfBoundsException`.
pub fn try_get<T: Clone>(a: &Array<T>, i: i32) -> Result<T, Exception> {
    let items = a.borrow();
    Ok(items[try_position(items.len(), i)?].clone())
}

/// `a[i] = v` where the program observes the `ArrayIndexOutOfBoundsException`.
pub fn try_set<T>(a: &Array<T>, i: i32, v: T) -> Result<(), Exception> {
    let mut items = a.borrow_mut();
    let i = try_position(items.len(), i)?;
    items[i] = v;
    Ok(())
}

/// `a[i] = v`. Java evaluates `a`, `i` and `v` before the check, and so do the arguments.
#[inline]
pub fn set<T>(a: &Array<T>, i: i32, v: T) {
    let mut items = a.borrow_mut();
    let i = position(items.len(), i);
    items[i] = v;
}

/// `a[i] op= ...`: `f` computes the new element from the old one. The old element is read, and
/// the borrow released, before `f` runs, so `f` may read the array. Returns the new element.
pub fn update<T: Clone>(a: &Array<T>, i: i32, f: impl FnOnce(T) -> T) -> T {
    let new = f(get(a, i));
    set(a, i, new.clone());
    new
}

/// `a[i]++` and the like: as `update`, but returns the old element.
pub fn post_update<T: Clone>(a: &Array<T>, i: i32, f: impl FnOnce(T) -> T) -> T {
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
pub fn to_string<T: Clone>(a: &Array<T>, show: impl Fn(T) -> String) -> String {
    let items: Vec<String> = a.borrow().iter().cloned().map(show).collect();
    format!("[{}]", items.join(", "))
}

/// `Arrays.fill(a, v)`.
pub fn fill<T: Clone>(a: &Array<T>, v: T) {
    a.borrow_mut().fill(v);
}

/// `Arrays.sort(a)` for integers and booleans.
pub fn sort<T: Ord>(a: &Array<T>) {
    a.borrow_mut().sort();
}

/// `Arrays.sort(a)` for doubles, in Java's order (`Double.compare`): -0.0 before 0.0, and every
/// NaN last, whatever its sign bit (`total_cmp` puts a negative NaN, which x86 makes for `0.0 / 0.0`,
/// first).
pub fn sort_doubles(a: &Array<f64>) {
    let key = |v: f64| if v.is_nan() { i64::MAX } else { v.to_bits() as i64 };
    a.borrow_mut()
        .sort_by(|x, y| x.partial_cmp(y).filter(|o| o.is_ne()).unwrap_or_else(|| key(*x).cmp(&key(*y))));
}

/// `Arrays.sort(a)` for floats, in Java's order (`Float.compare`), as for doubles.
pub fn sort_floats(a: &Array<f32>) {
    let key = |v: f32| if v.is_nan() { i32::MAX } else { v.to_bits() as i32 };
    a.borrow_mut()
        .sort_by(|x, y| x.partial_cmp(y).filter(|o| o.is_ne()).unwrap_or_else(|| key(*x).cmp(&key(*y))));
}

/// `Arrays.copyOf(a, len)`: the first `len` elements, padded with `zero`.
pub fn copy_of<T: Clone>(a: &Array<T>, len: i32, zero: T) -> Array<T> {
    let len = size(len);
    let mut items = a.borrow().clone();
    items.resize(len, zero);
    from_vec(items)
}

/// `a[i] = v` as an expression: stores `v` and gives it back.
pub fn store<T: Clone>(a: &Array<T>, i: i32, v: T) -> T {
    set(a, i, v.clone());
    v
}

/// `a[i] = v` as an expression, where the program observes the exception.
pub fn try_store<T: Clone>(a: &Array<T>, i: i32, v: T) -> Result<T, Exception> {
    try_set(a, i, v.clone())?;
    Ok(v)
}

/// `a[i] = v` as an expression, where the elements may be null and `v` is not: the value of the
/// assignment is `v` itself, not an `Option`.
pub fn store_some<T: Clone>(a: &Array<Option<T>>, i: i32, v: T) -> T {
    set(a, i, Some(v.clone()));
    v
}

/// `store_some`, where the program observes the exception.
pub fn try_store_some<T: Clone>(a: &Array<Option<T>>, i: i32, v: T) -> Result<T, Exception> {
    try_set(a, i, Some(v.clone()))?;
    Ok(v)
}

/// The elements of `a` in order, read one at a time as a for-each loop reads them. The length
/// is fixed when the loop starts, and the loop holds its own handle, as Java's does.
pub fn each<T: Clone>(a: Array<T>) -> impl Iterator<Item = T> {
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
