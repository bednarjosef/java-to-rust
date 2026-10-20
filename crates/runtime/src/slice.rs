//! Plain arrays: an array local that no other name can see is a `Vec`, not a shared `Array`, and
//! the generated code reads and writes it through these helpers. They check bounds and sizes the
//! same way as the shared arrays, so an error exits with Java's code.

use crate::array::{position, size};

/// `new T[len]` as a `Vec`, filled with the default value `zero`.
pub fn new_vec<T: Clone>(len: i32, zero: T) -> Vec<T> {
    vec![zero; size(len)]
}

/// `a[i]`.
pub fn at<T: Clone>(a: &[T], i: i32) -> T {
    a[position(a.len(), i)].clone()
}

/// `a[i] = v`, which yields `v` as the assignment does.
pub fn put<T: Clone>(a: &mut [T], i: i32, v: T) -> T {
    let k = position(a.len(), i);
    a[k] = v.clone();
    v
}

/// `a[i] op= v`, `a[i]++` and the like, as statements: the index is checked and the old element
/// read before `f` computes the new one from it, as Java does.
pub fn modify<T: Clone>(a: &mut [T], i: i32, f: impl FnOnce(T) -> T) {
    let k = position(a.len(), i);
    a[k] = f(a[k].clone());
}
