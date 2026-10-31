//! Plain arrays: an array that no other name can see is a `Vec`, not a shared `Array`, and the
//! generated code reads and writes it through these helpers. They check bounds and sizes the
//! same way as the shared arrays, so an error exits with Java's code. An array of arrays is a
//! `Vec` of `Vec`s, whose rows are reached with `row` and `row_mut`.

use crate::Exception;
use crate::array::{Array, SharedArray, from_vec, position, shared_from_vec, size, try_position, try_size};

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

/// `new T[len]` as a `Vec`, where the program observes the `NegativeArraySizeException`.
pub fn try_new_vec<T: Clone>(len: i32, zero: T) -> Result<Vec<T>, Exception> {
    Ok(vec![zero; try_size(len)?])
}

/// `a[i]` where the program observes the `ArrayIndexOutOfBoundsException`.
pub fn try_at<T: Clone>(a: &[T], i: i32) -> Result<T, Exception> {
    Ok(a[try_position(a.len(), i)?].clone())
}

/// `a[i] = v` where the program observes the `ArrayIndexOutOfBoundsException`.
pub fn try_put<T: Clone>(a: &mut [T], i: i32, v: T) -> Result<T, Exception> {
    let k = try_position(a.len(), i)?;
    a[k] = v.clone();
    Ok(v)
}

/// `put_some` where the program observes the `ArrayIndexOutOfBoundsException`.
pub fn try_put_some<T: Clone>(a: &mut [Option<T>], i: i32, v: T) -> Result<T, Exception> {
    let k = try_position(a.len(), i)?;
    a[k] = Some(v.clone());
    Ok(v)
}

/// `a[i] op= v` and the like where the program observes the exceptions of the index or of `f`.
pub fn try_modify<T: Clone>(
    a: &mut [T],
    i: i32,
    f: impl FnOnce(T) -> Result<T, Exception>,
) -> Result<(), Exception> {
    let k = try_position(a.len(), i)?;
    a[k] = f(a[k].clone())?;
    Ok(())
}

/// The row `a[i]` of an array of arrays, to be read, where the program observes the
/// `ArrayIndexOutOfBoundsException`.
pub fn try_row<T>(a: &[T], i: i32) -> Result<&T, Exception> {
    Ok(&a[try_position(a.len(), i)?])
}

/// The row `a[i]` of an array of arrays, to be written, where the program observes the
/// `ArrayIndexOutOfBoundsException`.
pub fn try_row_mut<T>(a: &mut [T], i: i32) -> Result<&mut T, Exception> {
    let k = try_position(a.len(), i)?;
    Ok(&mut a[k])
}

/// `position_of` where the program observes the `ArrayIndexOutOfBoundsException`.
pub fn try_position_of<T>(a: &[T], i: i32) -> Result<usize, Exception> {
    try_position(a.len(), i)
}

/// `assign` where the program observes the `ArrayIndexOutOfBoundsException`.
pub fn try_assign<T>(a: &mut [T], i: i32, v: T) -> Result<(), Exception> {
    let k = try_position(a.len(), i)?;
    a[k] = v;
    Ok(())
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

shared_as_is!(
    i8,
    i16,
    i32,
    i64,
    u16,
    f32,
    f64,
    bool,
    crate::JString,
    Option<crate::JString>,
    crate::JStr,
    Option<crate::JStr>
);

/// `Share` in a program with threads: a plain array made a `SharedArray`.
pub trait ShareAcross {
    type Shared;
    fn share_across(self) -> Self::Shared;
}

/// `v` as an array of a program with threads.
#[inline]
pub fn share_across<T: ShareAcross>(v: T) -> T::Shared {
    v.share_across()
}

impl<T: ShareAcross> ShareAcross for Vec<T> {
    type Shared = SharedArray<T::Shared>;
    fn share_across(self) -> SharedArray<T::Shared> {
        shared_from_vec(self.into_iter().map(ShareAcross::share_across).collect())
    }
}

macro_rules! shared_across_as_is {
    ($($t:ty),*) => {
        $(impl ShareAcross for $t {
            type Shared = $t;
            #[inline]
            fn share_across(self) -> $t {
                self
            }
        })*
    };
}

shared_across_as_is!(
    i8,
    i16,
    i32,
    i64,
    u16,
    f32,
    f64,
    bool,
    crate::JString,
    Option<crate::JString>,
    crate::JStr,
    Option<crate::JStr>
);

/// `search_eq` (array.rs) over a plain array.
#[inline(always)]
pub fn slice_search_eq<T: PartialEq + Clone>(a: &[T], from: i32, end: Option<i32>, key: T) -> Option<i32> {
    let (start, stop, past) = crate::array::search_range(a.len(), from, end, |i| {
        let _ = at(a, i);
    })?;
    crate::array::first_equal(&a[start..stop], &key).map(|k| (start + k) as i32).or_else(|| {
        crate::array::search_past(past, |i| {
            let _ = at(a, i);
        })
    })
}
