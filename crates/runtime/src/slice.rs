//! Plain arrays: an array that no other name can see is a `Vec`, not a shared `Array`, and the
//! generated code reads and writes it through these helpers. They check bounds and sizes the
//! same way as the shared arrays, so an error exits with Java's code. An array of arrays is a
//! `Vec` of `Vec`s, whose rows are reached with `row` and `row_mut`.

use crate::array::{Array, from_vec, position, size};

/// `new T[len]` as a `Vec`, filled with the default value `zero`.
#[inline]
pub fn new_vec<T: Clone>(len: i32, zero: T) -> Vec<T> {
    vec![zero; size(len)]
}

/// `a[i]`.
#[inline]
pub fn at<T: Clone>(a: &[T], i: i32) -> T {
    a[position(a.len(), i)].clone()
}

/// The row `a[i]` of an array of arrays, to be read.
#[inline]
pub fn row<T>(a: &[T], i: i32) -> &T {
    &a[position(a.len(), i)]
}

/// The row `a[i]` of an array of arrays, to be written.
#[inline]
pub fn row_mut<T>(a: &mut [T], i: i32) -> &mut T {
    let k = position(a.len(), i);
    &mut a[k]
}

/// The checked position of `a[i]`, for an access whose other parts are computed in between.
#[inline]
pub fn position_of<T>(a: &[T], i: i32) -> usize {
    position(a.len(), i)
}

/// `a[i] = v`, which yields `v` as the assignment does.
#[inline]
pub fn put<T: Clone>(a: &mut [T], i: i32, v: T) -> T {
    let k = position(a.len(), i);
    a[k] = v.clone();
    v
}

/// `a[i] = v` as an expression, where the elements may be null and `v` is not: yields `v`.
#[inline]
pub fn put_some<T: Clone>(a: &mut [Option<T>], i: i32, v: T) -> T {
    let k = position(a.len(), i);
    a[k] = Some(v.clone());
    v
}

/// `a[i] = v` as a statement, which moves `v` into the array: a row of an array of arrays.
#[inline]
pub fn assign<T>(a: &mut [T], i: i32, v: T) {
    let k = position(a.len(), i);
    a[k] = v;
}

/// `a[i] op= v`, `a[i]++` and the like, as statements: the index is checked and the old element
/// read before `f` computes the new one from it, as Java does.
#[inline]
pub fn modify<T: Clone>(a: &mut [T], i: i32, f: impl FnOnce(T) -> T) {
    let k = position(a.len(), i);
    a[k] = f(a[k].clone());
}

/// A plain array made shared, where it meets code that needs a `java::Array`: the `Vec` of a
/// one-dimensional array becomes the shared array without a copy, and each row of an array of
/// arrays becomes a shared array of its own.
pub trait Share {
    type Shared;
    fn share(self) -> Self::Shared;
}

/// `v` as a shared array.
#[inline]
pub fn share<T: Share>(v: T) -> T::Shared {
    v.share()
}

impl<T: Share> Share for Vec<T> {
    type Shared = Array<T::Shared>;
    fn share(self) -> Array<T::Shared> {
        from_vec(self.into_iter().map(Share::share).collect())
    }
}

macro_rules! shared_as_is {
    ($($t:ty),*) => {
        $(impl Share for $t {
            type Shared = $t;
            #[inline]
            fn share(self) -> $t {
                self
            }
        })*
    };
}

shared_as_is!(i8, i16, i32, i64, u16, f32, f64, bool, String, Option<String>);
