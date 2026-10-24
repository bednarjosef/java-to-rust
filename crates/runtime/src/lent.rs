//! The elements of an array as a runtime function that reads or writes a range of them takes
//! them (`new String(chars, offset, count)`, `System.arraycopy` with a plain array, a file read
//! into a buffer): a `Source` to read and a `Target` to store into.
//!
//! The generated code lends an array to such a function as it lends it to any function that takes
//! a slice: a plain array as `&v` or `&mut v`, an `Array` as `&a.borrow()` or `&mut
//! a.borrow_mut()`, which lend its `Vec` for the call. A `SharedArray`'s elements are cells that
//! other threads read and write while the call runs, which no slice may point into: its
//! `borrow()` and `borrow_mut()` give a `Lent` and a `LentMut` instead, which read and write the
//! cells one element at a time, each element one atomic action (`java::SharedField`). A function
//! that takes a `Source` copies out only the range it reads, and one that takes a `Target` stores
//! only the elements it writes, as Java's natives do. A function that still takes a slice gets
//! one through `Deref`: a copy of the whole array, made when the function first looks at it,
//! and for a `LentMut` stored back, the elements that changed, when the call's statement ends.

use crate::SharedField;
use std::borrow::Cow;
use std::cell::OnceCell;
use std::ops::{Deref, DerefMut, Range};

/// Elements that a runtime function reads: a slice, a plain array, an `Array`'s borrow, or a
/// `SharedArray`'s `Lent`.
pub trait Source<T: Clone> {
    /// The number of elements: the array's length.
    fn length(&self) -> usize;

    /// The elements in `range`, which is within the length: borrowed where they are, copied out
    /// of a shared array's cells.
    fn range(&self, range: Range<usize>) -> Cow<'_, [T]>;

    /// Every element.
    fn whole(&self) -> Cow<'_, [T]> {
        self.range(0..self.length())
    }
}

/// Elements that a runtime function stores into: a slice, a plain array, an `Array`'s mutable
/// borrow, or a `SharedArray`'s `LentMut`.
pub trait Target<T: Clone>: Source<T> {
    /// Stores `values` from position `at` on; the caller has checked that they fit.
    fn store(&mut self, at: usize, values: &[T]);
}

impl<T: Clone> Source<T> for [T] {
    #[inline]
    fn length(&self) -> usize {
        self.len()
    }

    #[inline]
    fn range(&self, range: Range<usize>) -> Cow<'_, [T]> {
        Cow::Borrowed(&self[range])
    }
}

impl<T: Clone> Target<T> for [T] {
    #[inline]
    fn store(&mut self, at: usize, values: &[T]) {
        self[at..at + values.len()].clone_from_slice(values);
    }
}

impl<T: Clone, const N: usize> Source<T> for [T; N] {
    #[inline]
    fn length(&self) -> usize {
        N
    }

    #[inline]
    fn range(&self, range: Range<usize>) -> Cow<'_, [T]> {
        Cow::Borrowed(&self[range])
    }
}

impl<T: Clone, const N: usize> Target<T> for [T; N] {
    #[inline]
    fn store(&mut self, at: usize, values: &[T]) {
        self.as_mut_slice().store(at, values)
    }
}

impl<T: Clone> Source<T> for Vec<T> {
    #[inline]
    fn length(&self) -> usize {
        self.len()
    }

    #[inline]
    fn range(&self, range: Range<usize>) -> Cow<'_, [T]> {
        Cow::Borrowed(&self[range])
    }
}

impl<T: Clone> Target<T> for Vec<T> {
    #[inline]
    fn store(&mut self, at: usize, values: &[T]) {
        self.as_mut_slice().store(at, values)
    }
}

impl<T: Clone> Source<T> for std::cell::Ref<'_, Vec<T>> {
    #[inline]
    fn length(&self) -> usize {
        self.len()
    }

    #[inline]
    fn range(&self, range: Range<usize>) -> Cow<'_, [T]> {
        Cow::Borrowed(&self[range])
    }
}

impl<T: Clone> Source<T> for std::cell::RefMut<'_, Vec<T>> {
    #[inline]
    fn length(&self) -> usize {
        self.len()
    }

    #[inline]
    fn range(&self, range: Range<usize>) -> Cow<'_, [T]> {
        Cow::Borrowed(&self[range])
    }
}

impl<T: Clone> Target<T> for std::cell::RefMut<'_, Vec<T>> {
    #[inline]
    fn store(&mut self, at: usize, values: &[T]) {
        self.as_mut_slice().store(at, values)
    }
}

impl<T: Clone, S: Source<T> + ?Sized> Source<T> for &S {
    #[inline]
    fn length(&self) -> usize {
        (**self).length()
    }

    #[inline]
    fn range(&self, range: Range<usize>) -> Cow<'_, [T]> {
        (**self).range(range)
    }
}

impl<T: Clone, S: Source<T> + ?Sized> Source<T> for &mut S {
    #[inline]
    fn length(&self) -> usize {
        (**self).length()
    }

    #[inline]
    fn range(&self, range: Range<usize>) -> Cow<'_, [T]> {
        (**self).range(range)
    }
}

impl<T: Clone, S: Target<T> + ?Sized> Target<T> for &mut S {
    #[inline]
    fn store(&mut self, at: usize, values: &[T]) {
        (**self).store(at, values)
    }
}

/// The elements `offset..offset + len` of an array, which the caller has checked are within it,
/// as a new `Vec`: only those are read.
pub fn copy_range<T: Clone>(source: &(impl Source<T> + ?Sized), offset: i32, len: i32) -> Vec<T> {
    source.range(offset as usize..(offset + len) as usize).into_owned()
}

/// An array's handle as a `Source` and a `Target`, for a runtime function that is given the
/// handle itself (`getChars` into a `char[]` of a `CharSequence`): what it reads is copied out,
/// what it stores is stored one element at a time.
pub(crate) struct Handled<'a, A>(pub(crate) &'a A);

impl<T: Clone, A: crate::Elements<T>> Source<T> for Handled<'_, A> {
    fn length(&self) -> usize {
        self.0.length()
    }

    fn range(&self, range: Range<usize>) -> Cow<'_, [T]> {
        Cow::Owned(self.0.reading(range, <[T]>::to_vec))
    }
}

impl<T: Clone, A: crate::Elements<T>> Target<T> for Handled<'_, A> {
    fn store(&mut self, at: usize, values: &[T]) {
        self.0.store_from(at, values)
    }
}

/// A copy of the cells' values, each read as one atomic action.
fn copied<T: Clone>(cells: &[SharedField<T>]) -> Vec<T> {
    cells.iter().map(SharedField::get).collect()
}

/// The elements of a `SharedArray` lent for reading (`a.borrow()`): see the module comment.
pub struct Lent<'a, T> {
    cells: &'a [SharedField<T>],
    /// The copy of every element that `Deref` makes, once.
    copy: OnceCell<Vec<T>>,
}

impl<'a, T> Lent<'a, T> {
    pub(crate) fn new(cells: &'a [SharedField<T>]) -> Lent<'a, T> {
        Lent { cells, copy: OnceCell::new() }
    }
}

impl<T: Clone> Source<T> for Lent<'_, T> {
    #[inline]
    fn length(&self) -> usize {
        self.cells.len()
    }

    fn range(&self, range: Range<usize>) -> Cow<'_, [T]> {
        match self.copy.get() {
            Some(copy) => Cow::Borrowed(&copy[range]),
            None => Cow::Owned(copied(&self.cells[range])),
        }
    }
}

impl<T: Clone> Deref for Lent<'_, T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        self.copy.get_or_init(|| copied(self.cells))
    }
}

/// The elements of a `SharedArray` lent for writing (`a.borrow_mut()`): see the module comment.
pub struct LentMut<'a, T: Clone> {
    cells: &'a [SharedField<T>],
    /// The copy of every element that `Deref` makes, once: the elements as they were, and as the
    /// function leaves them, which are stored back where they differ.
    copy: OnceCell<(Vec<T>, Vec<T>)>,
}

impl<'a, T: Clone> LentMut<'a, T> {
    pub(crate) fn new(cells: &'a [SharedField<T>]) -> LentMut<'a, T> {
        LentMut { cells, copy: OnceCell::new() }
    }

    fn copy(&self) -> &(Vec<T>, Vec<T>) {
        self.copy.get_or_init(|| {
            let was = copied(self.cells);
            (was.clone(), was)
        })
    }
}

impl<T: Clone> Source<T> for LentMut<'_, T> {
    #[inline]
    fn length(&self) -> usize {
        self.cells.len()
    }

    fn range(&self, range: Range<usize>) -> Cow<'_, [T]> {
        match self.copy.get() {
            Some((_, now)) => Cow::Borrowed(&now[range]),
            None => Cow::Owned(copied(&self.cells[range])),
        }
    }
}

impl<T: Clone> Target<T> for LentMut<'_, T> {
    fn store(&mut self, at: usize, values: &[T]) {
        for (cell, v) in self.cells[at..at + values.len()].iter().zip(values) {
            cell.set(v.clone());
        }
        // A copy made for `Deref` holds them too, as stored already.
        if let Some((was, now)) = self.copy.get_mut() {
            was[at..at + values.len()].clone_from_slice(values);
            now[at..at + values.len()].clone_from_slice(values);
        }
    }
}

impl<T: Clone> Deref for LentMut<'_, T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        &self.copy().1
    }
}

impl<T: Clone> DerefMut for LentMut<'_, T> {
    fn deref_mut(&mut self) -> &mut [T] {
        self.copy();
        &mut self.copy.get_mut().expect("made above").1
    }
}

impl<T: Clone> Drop for LentMut<'_, T> {
    /// Stores back the elements of the copy that changed: a word whose bits differ, and any
    /// wider element (a string), which cannot be compared without its `PartialEq`.
    fn drop(&mut self) {
        let Some((was, now)) = self.copy.take() else { return };
        for ((cell, was), now) in self.cells.iter().zip(&was).zip(now) {
            if !SharedField::<T>::WORD || SharedField::bits(was) != SharedField::bits(&now) {
                cell.set(now);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cells(values: &[i32]) -> Vec<SharedField<i32>> {
        values.iter().map(|&v| SharedField::new(v)).collect()
    }

    fn values(cells: &[SharedField<i32>]) -> Vec<i32> {
        copied(cells)
    }

    #[test]
    fn a_lent_array_copies_only_the_range_it_is_asked_for() {
        let cells = cells(&[1, 2, 3, 4]);
        let lent = Lent::new(&cells);
        assert_eq!(lent.length(), 4);
        assert_eq!(&*lent.range(1..3), &[2, 3]);
        assert!(lent.copy.get().is_none());
        assert_eq!(&*lent, &[1, 2, 3, 4]);
    }

    #[test]
    fn a_mutably_lent_array_stores_what_changed() {
        let cells = cells(&[1, 2, 3, 4]);
        let mut lent = LentMut::new(&cells);
        lent.store(1, &[7, 8]);
        assert_eq!(values(&cells), [1, 7, 8, 4]);
        lent[3] = 9;
        // Another thread's write of an element the function leaves alone is kept.
        cells[0].set(5);
        drop(lent);
        assert_eq!(values(&cells), [5, 7, 8, 9]);
    }
}
