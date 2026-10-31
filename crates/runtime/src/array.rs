//! Java arrays. An array is a shared handle to its elements: copying the handle gives a second
//! name for the same array, as in Java, and every access is bounds-checked the way Java checks it.
//!
//! An array of a program without threads, or of a family that no two threads reach (`Array`),
//! keeps its elements in a `RefCell<Vec<T>>`. A bulk access (a sort, a copy, a runtime function
//! lent the elements) borrows it. A single element's read or write, and the length, go around the
//! borrow flag (`ArrayData::raw`): they hold nothing while other code runs, and a Java array's
//! length never changes, so the flag would only cost a store and a reload on every access, which
//! in a loop over an array chain one iteration to the next through memory. An array that threads
//! share (`SharedArray`) keeps each element in a cell of its own (`SharedElements`, a slice of
//! `java::SharedField`s), as a thread-shared object keeps its fields: a read or write of an
//! element is one atomic action, which takes no lock and checks no borrow flag. An element of
//! 1, 2, 4 or 8 bytes without drop glue (a primitive, a reference) is an atomic word, loaded with
//! acquire and stored with release (plain moves on x86-64); a wider one (a string without an
//! identity) is copied under a striped lock. Java makes each element access atomic and promises
//! nothing about the order or atomicity of a bulk operation, so `Arrays.fill`, a sort or
//! `System.arraycopy` reads and writes a shared array one element at a time (`Elements::reading`
//! and `writing`), and other threads may see it half done, as on the JVM.
//!
//! The functions here take either handle (`Elements`), and those that make an array have a
//! `shared_` twin. Runtime functions that read or write a range of an array lent by the generated
//! code take it as a `Source` or a `Target` (`lent.rs`).
//!
//! An array that the translation holds as an `Object[]` though the program made it as an array of
//! a narrower class (a covariant array, see the translator's `lower::covariant`) carries that
//! class (`ElementClass`): a store of an object of another class throws `ArrayStoreException`,
//! and `getClass()` is the array's own class, as in Java. Every other array carries none.

use crate::Arc;
use crate::lent::{Lent, LentMut};
use crate::{Class, Exception, SharedField, throw};
use std::cell::{RefCell, UnsafeCell};
use std::ops::{Deref, Range};

pub type Array<T> = Arc<ArrayData<RefCell<Vec<T>>, T>>;

/// An array of a program with threads, whose arrays of its family two threads may reach.
pub type SharedArray<T> = Arc<ArrayData<SharedElements<T>, T>>;

/// What an array handle points to: the elements, in a `RefCell` or as `SharedElements` (`C`),
/// which the handle reaches through `Deref` (the generated code lends them to a runtime function
/// as `&a.borrow()`), and the class of a covariant array.
pub struct ArrayData<C, T> {
    items: C,
    class: Option<Box<ElementClass<T>>>,
}

impl<C, T> Deref for ArrayData<C, T> {
    type Target = C;

    #[inline]
    fn deref(&self) -> &C {
        &self.items
    }
}

impl<C: std::fmt::Debug, T> std::fmt::Debug for ArrayData<C, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.items.fmt(f)
    }
}

/// The elements of a `SharedArray`: a cell per element, each read and written as one atomic action
/// (`java::SharedField`). The length never changes.
///
/// The array's monitor is made the first time a thread locks it (`synchronized (a)`), as few
/// arrays are ever locked.
pub struct SharedElements<T>(Box<[SharedField<T>]>, pub(crate) crate::monitor::LazyMonitor);

impl<T> SharedElements<T> {
    fn new(items: Vec<T>) -> SharedElements<T> {
        let items: Box<[T]> = items.into_boxed_slice();
        // A `SharedField<T>` is a transparent `UnsafeCell<T>`, which has the layout of a `T`.
        SharedElements(
            unsafe { Box::from_raw(Box::into_raw(items) as *mut [SharedField<T>]) },
            crate::monitor::LazyMonitor::new(),
        )
    }

    /// The elements lent to a runtime function that reads them (`lent.rs`): read from their cells
    /// as the function asks for them, never borrowed in place.
    #[inline]
    pub fn borrow(&self) -> Lent<'_, T> {
        Lent::new(&self.0)
    }

    /// The elements lent to a runtime function that writes them (`lent.rs`).
    #[inline]
    pub fn borrow_mut(&self) -> LentMut<'_, T>
    where
        T: Clone,
    {
        LentMut::new(&self.0)
    }
}

impl<T: Clone + std::fmt::Debug> std::fmt::Debug for SharedElements<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.0.iter()).finish()
    }
}

/// The class of a covariant array (`String[]` for one the translation holds as an `Object[]`),
/// as the translator writes it where the array is made.
pub struct ElementClass<T> {
    /// The array's class, which its `getClass()` gives.
    pub(crate) class: fn() -> Class,
    /// The binary name of the class of the elements: `java.lang.String`.
    pub(crate) element: &'static str,
    /// The module and class loader of the class of the elements as HotSpot names them, which a
    /// `ClassCastException` gives with the array's class (`java::BASE`, `java::APP`).
    module: &'static str,
    /// The binary names of the class of an element and of every class and interface above it,
    /// its own first (`None` for null), which the store of the element tests.
    names: fn(&T) -> Option<&'static [&'static str]>,
    /// The binary names of the array's class and of the classes and interfaces above it, its own
    /// first, which a store of the array into an array of arrays tests (`array_type_names`).
    own: &'static [&'static str],
}

impl<T> ElementClass<T> {
    pub fn new(
        class: fn() -> Class,
        element: &'static str,
        module: &'static str,
        names: fn(&T) -> Option<&'static [&'static str]>,
        own: &'static [&'static str],
    ) -> ElementClass<T> {
        ElementClass { class, element, module, names, own }
    }

    /// The binary name of the class of `v` where the array cannot hold it (`java.lang.Integer` in
    /// a `String[]`), which the `ArrayStoreException` gives. A null fits any array.
    pub(crate) fn refuses(&self, v: &T) -> Option<&'static str> {
        let names = (self.names)(v)?;
        (!crate::class::names_contain(names, self.element))
            .then(|| names.first().copied().unwrap_or("java.lang.Object"))
    }
}

impl<T> Clone for ElementClass<T> {
    fn clone(&self) -> ElementClass<T> {
        ElementClass {
            class: self.class,
            element: self.element,
            module: self.module,
            names: self.names,
            own: self.own,
        }
    }
}

/// A handle of an array: an `Array` or a `SharedArray`, or a reference to one. Each access of an
/// element is checked against the length the way Java checks it; the `try_` forms give the
/// `ArrayIndexOutOfBoundsException` back instead of throwing it, and the `_in_bounds` forms, for
/// an index the translator proved in bounds, keep only a check that stops the program
/// (`wrong_bound`), which LLVM drops where it sees the bound.
pub trait Elements<T> {
    /// `a.length`.
    fn length(&self) -> usize;
    /// `a[i]`.
    fn element(&self, i: i32) -> T
    where
        T: Clone;
    fn try_element(&self, i: i32) -> Result<T, Exception>
    where
        T: Clone;
    /// `a[i] = v`.
    fn put(&self, i: i32, v: T);
    fn try_put(&self, i: i32, v: T) -> Result<(), Exception>;
    /// `a[i]` for an `i` that is in bounds (`analysis::arrays::bounds`).
    fn element_in_bounds(&self, i: i32) -> T
    where
        T: Clone,
    {
        self.element(i)
    }
    /// `a[i] = v` for an `i` that is in bounds.
    fn put_in_bounds(&self, i: i32, v: T) {
        self.put(i, v)
    }
    /// The elements as a counting loop reads them (`view`).
    type View<'v>: View<T>
    where
        Self: 'v;
    fn view(&self) -> Self::View<'_>;
    /// `a[i]` as a volatile read (`VarHandle.getVolatile`, `Unsafe.getIntVolatile`), which is
    /// sequentially consistent with the other volatile accesses: a `SeqCst` load from a
    /// `SharedArray`; a plain read from an `Array`, which one thread holds.
    fn try_element_volatile(&self, i: i32) -> Result<T, Exception>
    where
        T: Clone,
    {
        self.try_element(i)
    }
    /// `a[i] = v` as a volatile write: a `SeqCst` store into a `SharedArray`.
    fn try_put_volatile(&self, i: i32, v: T) -> Result<(), Exception> {
        self.try_put(i, v)
    }
    /// `a[i] = v` in one atomic action with the read of the old element, which it returns.
    fn try_swap(&self, i: i32, v: T) -> Result<T, Exception>;
    /// The old `a[i]`, which is now `value` if it was `same` as `expected`, in one atomic action.
    fn try_compare_exchange(
        &self,
        i: i32,
        expected: T,
        value: T,
        same: impl Fn(&T, &T) -> bool,
    ) -> Result<T, Exception>
    where
        T: Clone;
    /// The old `a[i]`, which is now `f` of it, in one atomic action.
    fn try_get_and_update(&self, i: i32, f: impl Fn(&T) -> T) -> Result<T, Exception>
    where
        T: Clone;
    /// The elements where they are, for a search that reads them while no code of the program
    /// runs: an `Array`'s; `None` for a `SharedArray`, whose elements are atomic.
    #[inline(always)]
    fn in_place(&self) -> Option<&[T]> {
        None
    }
    /// Runs `f` on the elements in `range` (within the length): borrowed from an `Array`, read
    /// one at a time into a copy from a `SharedArray`. `f` runs no code of the program.
    fn reading<R>(&self, range: Range<usize>, f: impl FnOnce(&[T]) -> R) -> R
    where
        T: Clone;
    /// Runs `f` on the elements in `range` (within the length) to change them: borrowed from an
    /// `Array`; from a `SharedArray` copied out one at a time and stored back one at a time.
    /// `f` runs no code of the program.
    fn writing<R>(&self, range: Range<usize>, f: impl FnOnce(&mut [T]) -> R) -> R
    where
        T: Clone;
    /// Stores `values` from position `at` on, which the caller has checked they fit; into a
    /// `SharedArray` one at a time.
    fn store_from(&self, at: usize, values: &[T])
    where
        T: Clone;
    /// Stores `v` into every element in `range` (within the length): `Arrays.fill`. A
    /// `SharedArray` stores into each cell, rather than copying the range out and back.
    fn fill_with(&self, range: Range<usize>, v: T)
    where
        T: Clone,
    {
        self.writing(range, |items| items.fill(v));
    }
    /// Copies the `n` elements from position `from` to position `to`, which the caller has
    /// checked, as if through a temporary array where the two overlap (`System.arraycopy` within
    /// one array); within a `SharedArray`, one element at a time, in the order that keeps each
    /// element from being overwritten before it is copied.
    fn copy_within(&self, from: usize, to: usize, n: usize)
    where
        T: Clone;
    /// The array's address, which is its identity.
    fn address(&self) -> *const u8;
    /// The class of a covariant array.
    fn element_class(&self) -> Option<&ElementClass<T>>;
    /// The elements of an array that nothing else holds yet, taken out of it (or copied, from a
    /// `SharedArray`, whose cells cannot be emptied).
    fn take_items(&self) -> Vec<T>
    where
        T: Clone;
}

/// A handle that a new array can be made as: an `Array` or a `SharedArray`.
pub trait MadeElements<T>: Elements<T> {
    /// A new array of the same kind, of class `class` where it is a covariant array.
    fn made_as(items: Vec<T>, class: Option<ElementClass<T>>) -> Self;

    /// A new array of the same kind, such as a copy.
    fn made(items: Vec<T>) -> Self
    where
        Self: Sized,
    {
        Self::made_as(items, None)
    }

    /// A new array of the same kind and class as this one: a copy keeps the class.
    fn made_like(&self, items: Vec<T>) -> Self
    where
        Self: Sized,
    {
        Self::made_as(items, self.element_class().cloned())
    }
}

impl<T, A: Elements<T>> Elements<T> for &A {
    #[inline(always)]
    fn length(&self) -> usize {
        (**self).length()
    }

    #[inline(always)]
    fn in_place(&self) -> Option<&[T]> {
        (**self).in_place()
    }

    #[inline(always)]
    fn element(&self, i: i32) -> T
    where
        T: Clone,
    {
        (**self).element(i)
    }

    #[inline(always)]
    fn try_element(&self, i: i32) -> Result<T, Exception>
    where
        T: Clone,
    {
        (**self).try_element(i)
    }

    #[inline(always)]
    fn put(&self, i: i32, v: T) {
        (**self).put(i, v)
    }

    #[inline(always)]
    fn element_in_bounds(&self, i: i32) -> T
    where
        T: Clone,
    {
        (**self).element_in_bounds(i)
    }

    #[inline(always)]
    fn put_in_bounds(&self, i: i32, v: T) {
        (**self).put_in_bounds(i, v)
    }

    type View<'v>
        = A::View<'v>
    where
        Self: 'v;

    #[inline(always)]
    fn view(&self) -> A::View<'_> {
        (**self).view()
    }

    #[inline(always)]
    fn try_put(&self, i: i32, v: T) -> Result<(), Exception> {
        (**self).try_put(i, v)
    }

    #[inline]
    fn try_element_volatile(&self, i: i32) -> Result<T, Exception>
    where
        T: Clone,
    {
        (**self).try_element_volatile(i)
    }

    #[inline]
    fn try_put_volatile(&self, i: i32, v: T) -> Result<(), Exception> {
        (**self).try_put_volatile(i, v)
    }

    fn try_swap(&self, i: i32, v: T) -> Result<T, Exception> {
        (**self).try_swap(i, v)
    }

    fn try_compare_exchange(
        &self,
        i: i32,
        expected: T,
        value: T,
        same: impl Fn(&T, &T) -> bool,
    ) -> Result<T, Exception>
    where
        T: Clone,
    {
        (**self).try_compare_exchange(i, expected, value, same)
    }

    fn try_get_and_update(&self, i: i32, f: impl Fn(&T) -> T) -> Result<T, Exception>
    where
        T: Clone,
    {
        (**self).try_get_and_update(i, f)
    }

    #[inline]
    fn reading<R>(&self, range: Range<usize>, f: impl FnOnce(&[T]) -> R) -> R
    where
        T: Clone,
    {
        (**self).reading(range, f)
    }

    #[inline]
    fn writing<R>(&self, range: Range<usize>, f: impl FnOnce(&mut [T]) -> R) -> R
    where
        T: Clone,
    {
        (**self).writing(range, f)
    }

    fn store_from(&self, at: usize, values: &[T])
    where
        T: Clone,
    {
        (**self).store_from(at, values)
    }

    fn fill_with(&self, range: Range<usize>, v: T)
    where
        T: Clone,
    {
        (**self).fill_with(range, v)
    }

    fn copy_within(&self, from: usize, to: usize, n: usize)
    where
        T: Clone,
    {
        (**self).copy_within(from, to, n)
    }

    fn address(&self) -> *const u8 {
        (**self).address()
    }

    fn element_class(&self) -> Option<&ElementClass<T>> {
        (**self).element_class()
    }

    fn take_items(&self) -> Vec<T>
    where
        T: Clone,
    {
        (**self).take_items()
    }
}

impl<T> ArrayData<RefCell<Vec<T>>, T> {
    /// The start and the number of the elements, reached without the borrow flag, for an access
    /// of one element that holds nothing while other code runs (see the module's documentation).
    #[inline(always)]
    fn raw(&self) -> (*mut T, usize) {
        let items = self.items.as_ptr();
        unsafe { ((*items).as_mut_ptr(), (*items).len()) }
    }
}

impl<T> Elements<T> for Array<T> {
    #[inline(always)]
    fn length(&self) -> usize {
        self.raw().1
    }

    #[inline(always)]
    fn in_place(&self) -> Option<&[T]> {
        let (items, len) = self.raw();
        // SAFETY: the elements stay where they are while the caller reads them: it runs no code
        // of the program, which alone could store into the array or let it go.
        Some(unsafe { std::slice::from_raw_parts(items, len) })
    }

    #[inline(always)]
    fn element(&self, i: i32) -> T
    where
        T: Clone,
    {
        let (items, n) = self.raw();
        unsafe { (*items.add(position(n, i))).clone() }
    }

    #[inline(always)]
    fn try_element(&self, i: i32) -> Result<T, Exception>
    where
        T: Clone,
    {
        let (items, n) = self.raw();
        Ok(unsafe { (*items.add(try_position(n, i)?)).clone() })
    }

    #[inline(always)]
    fn put(&self, i: i32, v: T) {
        let (items, n) = self.raw();
        unsafe { *items.add(position(n, i)) = v }
    }

    #[inline(always)]
    fn element_in_bounds(&self, i: i32) -> T
    where
        T: Clone,
    {
        let (items, n) = self.raw();
        if (i as usize) >= n {
            wrong_bound(n, i)
        }
        unsafe { (*items.add(i as usize)).clone() }
    }

    #[inline(always)]
    fn put_in_bounds(&self, i: i32, v: T) {
        let (items, n) = self.raw();
        if (i as usize) >= n {
            wrong_bound(n, i)
        }
        unsafe { *items.add(i as usize) = v }
    }

    type View<'v>
        = ArrayView<'v, T>
    where
        Self: 'v;

    #[inline(always)]
    fn view(&self) -> ArrayView<'_, T> {
        // The start and the length stay right while the handle lives: the `Vec` of an `Array` is
        // never grown, shrunk or replaced, but by `take_items` of an array no other handle holds
        // yet, and the collected heap never moves a slot (`gc.rs`). Each access is still checked
        // against the length, so that a wrong proof stops the program and never reaches past it.
        let (items, len) = self.raw();
        ArrayView { items, len, array: std::marker::PhantomData }
    }

    #[inline(always)]
    fn try_put(&self, i: i32, v: T) -> Result<(), Exception> {
        let (items, n) = self.raw();
        unsafe { *items.add(try_position(n, i)?) = v }
        Ok(())
    }

    fn try_swap(&self, i: i32, v: T) -> Result<T, Exception> {
        let mut items = self.borrow_mut();
        let k = try_position(items.len(), i)?;
        Ok(std::mem::replace(&mut items[k], v))
    }

    // One borrow of the array: no other access comes between the read and the write.
    fn try_compare_exchange(
        &self,
        i: i32,
        expected: T,
        value: T,
        same: impl Fn(&T, &T) -> bool,
    ) -> Result<T, Exception>
    where
        T: Clone,
    {
        let mut items = self.borrow_mut();
        let k = try_position(items.len(), i)?;
        let old = items[k].clone();
        if same(&old, &expected) {
            items[k] = value;
        }
        Ok(old)
    }

    fn try_get_and_update(&self, i: i32, f: impl Fn(&T) -> T) -> Result<T, Exception>
    where
        T: Clone,
    {
        let mut items = self.borrow_mut();
        let k = try_position(items.len(), i)?;
        let new = f(&items[k]);
        Ok(std::mem::replace(&mut items[k], new))
    }

    #[inline]
    fn reading<R>(&self, range: Range<usize>, f: impl FnOnce(&[T]) -> R) -> R
    where
        T: Clone,
    {
        f(&self.borrow()[range])
    }

    #[inline]
    fn writing<R>(&self, range: Range<usize>, f: impl FnOnce(&mut [T]) -> R) -> R
    where
        T: Clone,
    {
        f(&mut self.borrow_mut()[range])
    }

    fn store_from(&self, at: usize, values: &[T])
    where
        T: Clone,
    {
        self.borrow_mut()[at..at + values.len()].clone_from_slice(values);
    }

    fn copy_within(&self, from: usize, to: usize, n: usize)
    where
        T: Clone,
    {
        let mut items = self.borrow_mut();
        if from < to {
            for k in (0..n).rev() {
                items[to + k] = items[from + k].clone();
            }
        } else {
            for k in 0..n {
                items[to + k] = items[from + k].clone();
            }
        }
    }

    fn address(&self) -> *const u8 {
        Arc::as_ptr(self) as *const u8
    }

    #[inline]
    fn element_class(&self) -> Option<&ElementClass<T>> {
        self.class.as_deref()
    }

    fn take_items(&self) -> Vec<T>
    where
        T: Clone,
    {
        std::mem::take(&mut *self.borrow_mut())
    }
}

impl<T> MadeElements<T> for Array<T> {
    fn made_as(items: Vec<T>, class: Option<ElementClass<T>>) -> Array<T> {
        Arc::new(ArrayData { items: RefCell::new(items), class: class.map(Box::new) })
    }
}

impl<T> Elements<T> for SharedArray<T> {
    #[inline(always)]
    fn length(&self) -> usize {
        self.items.0.len()
    }

    #[inline(always)]
    fn element(&self, i: i32) -> T
    where
        T: Clone,
    {
        let cells = &self.items.0;
        cells[position(cells.len(), i)].get()
    }

    #[inline(always)]
    fn try_element(&self, i: i32) -> Result<T, Exception>
    where
        T: Clone,
    {
        let cells = &self.items.0;
        Ok(cells[try_position(cells.len(), i)?].get())
    }

    #[inline(always)]
    fn put(&self, i: i32, v: T) {
        let cells = &self.items.0;
        cells[position(cells.len(), i)].set(v)
    }

    #[inline(always)]
    fn element_in_bounds(&self, i: i32) -> T
    where
        T: Clone,
    {
        let cells = &self.items.0;
        match cells.get(i as usize) {
            Some(cell) => cell.get(),
            None => wrong_bound(cells.len(), i),
        }
    }

    #[inline(always)]
    fn put_in_bounds(&self, i: i32, v: T) {
        let cells = &self.items.0;
        match cells.get(i as usize) {
            Some(cell) => cell.set(v),
            None => wrong_bound(cells.len(), i),
        }
    }

    type View<'v>
        = SharedView<'v, T>
    where
        Self: 'v;

    /// The cells, which are never replaced: each element is still read and written through its
    /// cell, as one atomic action.
    #[inline(always)]
    fn view(&self) -> SharedView<'_, T> {
        SharedView(&self.items.0)
    }

    #[inline(always)]
    fn try_put(&self, i: i32, v: T) -> Result<(), Exception> {
        let cells = &self.items.0;
        cells[try_position(cells.len(), i)?].set(v);
        Ok(())
    }

    #[inline]
    fn try_element_volatile(&self, i: i32) -> Result<T, Exception>
    where
        T: Clone,
    {
        let cells = &self.items.0;
        Ok(cells[try_position(cells.len(), i)?].get_volatile())
    }

    #[inline]
    fn try_put_volatile(&self, i: i32, v: T) -> Result<(), Exception> {
        let cells = &self.items.0;
        cells[try_position(cells.len(), i)?].set_volatile(v);
        Ok(())
    }

    fn try_swap(&self, i: i32, v: T) -> Result<T, Exception> {
        let cells = &self.items.0;
        Ok(cells[try_position(cells.len(), i)?].replace(v))
    }

    // The cell's compare-and-swap, or its striped lock for a wide element.
    fn try_compare_exchange(
        &self,
        i: i32,
        expected: T,
        value: T,
        same: impl Fn(&T, &T) -> bool,
    ) -> Result<T, Exception>
    where
        T: Clone,
    {
        let cells = &self.items.0;
        Ok(cells[try_position(cells.len(), i)?].compare_exchange_by(expected, value, same))
    }

    fn try_get_and_update(&self, i: i32, f: impl Fn(&T) -> T) -> Result<T, Exception>
    where
        T: Clone,
    {
        let cells = &self.items.0;
        Ok(cells[try_position(cells.len(), i)?].get_and_update(f))
    }

    fn reading<R>(&self, range: Range<usize>, f: impl FnOnce(&[T]) -> R) -> R
    where
        T: Clone,
    {
        let copy: Vec<T> = self.items.0[range].iter().map(SharedField::get).collect();
        f(&copy)
    }

    fn writing<R>(&self, range: Range<usize>, f: impl FnOnce(&mut [T]) -> R) -> R
    where
        T: Clone,
    {
        let cells = &self.items.0[range];
        let mut copy: Vec<T> = cells.iter().map(SharedField::get).collect();
        let result = f(&mut copy);
        for (cell, v) in cells.iter().zip(copy) {
            cell.set(v);
        }
        result
    }

    fn store_from(&self, at: usize, values: &[T])
    where
        T: Clone,
    {
        for (cell, v) in self.items.0[at..at + values.len()].iter().zip(values) {
            cell.set(v.clone());
        }
    }

    fn fill_with(&self, range: Range<usize>, v: T)
    where
        T: Clone,
    {
        for cell in &self.items.0[range] {
            cell.set(v.clone());
        }
    }

    fn copy_within(&self, from: usize, to: usize, n: usize)
    where
        T: Clone,
    {
        let (src, dest) = (&self.items.0[from..from + n], &self.items.0[to..to + n]);
        if from < to {
            for (d, s) in dest.iter().zip(src).rev() {
                d.set(s.get());
            }
        } else {
            for (d, s) in dest.iter().zip(src) {
                d.set(s.get());
            }
        }
    }

    fn address(&self) -> *const u8 {
        Arc::as_ptr(self) as *const u8
    }

    #[inline]
    fn element_class(&self) -> Option<&ElementClass<T>> {
        self.class.as_deref()
    }

    fn take_items(&self) -> Vec<T>
    where
        T: Clone,
    {
        self.items.0.iter().map(SharedField::get).collect()
    }
}

/// An array that is written only before any other thread can reach it, and then only read: a
/// `static final` array that its class's static initialization assigns a new array, and that
/// the program only indexes and measures (the translator's `frozen_statics`), or such a field
/// that only its object's constructor makes and writes, through `this` (`frozen_fields`), while
/// the object is a value the constructor holds. Its elements are read and written where they
/// are, with no atomic access and no borrow flag; the `java::Final` that holds a static one
/// publishes them with the array, and the object of a field one is published with the array in
/// it. `N`, when not 0, is the length of every array the static or field is given, a constant:
/// the compiler then drops the bounds check of an index it can bound (`h & 15` in an array of 16),
/// as the JIT does for a `static final` array, whose length it knows. A copy of the handle (the
/// constructor's hold of an array it stores into, `lower::argument_order`) shares the elements.
pub struct Frozen<T, const N: usize = 0>(std::sync::Arc<[UnsafeCell<T>]>);

// Its elements are written before another thread can reach them (see `Frozen`).
unsafe impl<T: Send + Sync, const N: usize> Sync for Frozen<T, N> {}
unsafe impl<T: Send + Sync, const N: usize> Send for Frozen<T, N> {}

impl<T, const N: usize> Clone for Frozen<T, N> {
    fn clone(&self) -> Self {
        Frozen(self.0.clone())
    }
}

/// The elements of the new array `a`, frozen (see `Frozen`).
pub fn freeze<T: Clone>(a: impl Elements<T>) -> Frozen<T> {
    Frozen(a.take_items().into_iter().map(UnsafeCell::new).collect())
}

/// The elements of the new array `a`, whose length is `N`, frozen (see `Frozen`).
pub fn freeze_fixed<T: Clone, const N: usize>(a: impl Elements<T>) -> Frozen<T, N> {
    let items = a.take_items();
    assert_eq!(items.len(), N, "an array frozen with another length");
    Frozen(items.into_iter().map(UnsafeCell::new).collect())
}

impl<T, const N: usize> Frozen<T, N> {
    #[inline(always)]
    fn len(&self) -> usize {
        if N == 0 { self.0.len() } else { N }
    }

    /// The cell of element `k`, which the caller has checked is below `len`.
    #[inline(always)]
    fn cell(&self, k: usize) -> &UnsafeCell<T> {
        // `len` is the slice's length: `freeze_fixed` checks that it is `N`.
        unsafe { self.0.get_unchecked(k) }
    }

    #[inline(always)]
    fn items(&self) -> &[T] {
        // `UnsafeCell<T>` has `T`'s layout, and no write runs while the slice is lent.
        unsafe { &*(&raw const *self.0 as *const [T]) }
    }

    /// Runs `f` on the elements to change them.
    #[inline(always)]
    fn changing<R>(&self, f: impl FnOnce(&mut [T]) -> R) -> R {
        // As `items`; the constructor that writes is the array's only user, and `f` runs no code
        // of the program.
        f(unsafe { &mut *(&raw const *self.0 as *mut [T]) })
    }

    #[inline(always)]
    fn replace(&self, k: usize, v: T) -> T {
        unsafe { std::mem::replace(&mut *self.cell(k).get(), v) }
    }
}

impl<T, const N: usize> Elements<T> for Frozen<T, N> {
    #[inline(always)]
    fn length(&self) -> usize {
        self.len()
    }

    #[inline(always)]
    fn in_place(&self) -> Option<&[T]> {
        Some(self.items())
    }

    #[inline(always)]
    fn element(&self, i: i32) -> T
    where
        T: Clone,
    {
        unsafe { (*self.cell(position(self.len(), i)).get()).clone() }
    }

    #[inline(always)]
    fn try_element(&self, i: i32) -> Result<T, Exception>
    where
        T: Clone,
    {
        Ok(unsafe { (*self.cell(try_position(self.len(), i)?).get()).clone() })
    }

    #[inline(always)]
    fn put(&self, i: i32, v: T) {
        self.replace(position(self.len(), i), v);
    }

    #[inline(always)]
    fn element_in_bounds(&self, i: i32) -> T
    where
        T: Clone,
    {
        if (i as usize) >= self.len() {
            wrong_bound(self.len(), i)
        }
        unsafe { (*self.cell(i as usize).get()).clone() }
    }

    #[inline(always)]
    fn put_in_bounds(&self, i: i32, v: T) {
        if (i as usize) >= self.len() {
            wrong_bound(self.len(), i)
        }
        self.replace(i as usize, v);
    }

    type View<'v>
        = FrozenView<'v, T>
    where
        Self: 'v;

    #[inline(always)]
    fn view(&self) -> FrozenView<'_, T> {
        FrozenView(&self.0)
    }

    #[inline(always)]
    fn try_put(&self, i: i32, v: T) -> Result<(), Exception> {
        self.replace(try_position(self.len(), i)?, v);
        Ok(())
    }

    fn try_swap(&self, i: i32, v: T) -> Result<T, Exception> {
        Ok(self.replace(try_position(self.len(), i)?, v))
    }

    fn try_compare_exchange(
        &self,
        i: i32,
        expected: T,
        value: T,
        same: impl Fn(&T, &T) -> bool,
    ) -> Result<T, Exception>
    where
        T: Clone,
    {
        let k = try_position(self.len(), i)?;
        let old = unsafe { (*self.cell(k).get()).clone() };
        if same(&old, &expected) {
            self.replace(k, value);
        }
        Ok(old)
    }

    fn try_get_and_update(&self, i: i32, f: impl Fn(&T) -> T) -> Result<T, Exception>
    where
        T: Clone,
    {
        let k = try_position(self.len(), i)?;
        let old = unsafe { (*self.cell(k).get()).clone() };
        self.replace(k, f(&old));
        Ok(old)
    }

    fn reading<R>(&self, range: Range<usize>, f: impl FnOnce(&[T]) -> R) -> R
    where
        T: Clone,
    {
        f(&self.items()[range])
    }

    fn writing<R>(&self, range: Range<usize>, f: impl FnOnce(&mut [T]) -> R) -> R
    where
        T: Clone,
    {
        self.changing(|items| f(&mut items[range]))
    }

    fn store_from(&self, at: usize, values: &[T])
    where
        T: Clone,
    {
        self.changing(|items| items[at..at + values.len()].clone_from_slice(values));
    }

    fn copy_within(&self, from: usize, to: usize, n: usize)
    where
        T: Clone,
    {
        self.changing(|items| {
            if from >= to {
                for k in 0..n {
                    items[to + k] = items[from + k].clone();
                }
            } else {
                for k in (0..n).rev() {
                    items[to + k] = items[from + k].clone();
                }
            }
        });
    }

    fn address(&self) -> *const u8 {
        self.0.as_ptr() as *const u8
    }

    fn element_class(&self) -> Option<&ElementClass<T>> {
        None
    }

    fn take_items(&self) -> Vec<T>
    where
        T: Clone,
    {
        self.items().to_vec()
    }
}

impl<T> MadeElements<T> for SharedArray<T> {
    fn made_as(items: Vec<T>, class: Option<ElementClass<T>>) -> SharedArray<T> {
        Arc::new(ArrayData { items: SharedElements::new(items), class: class.map(Box::new) })
    }
}

/// An array with the given elements, as in `{a, b, c}`.
pub fn from_vec<T>(items: Vec<T>) -> Array<T> {
    Array::made_as(items, None)
}

/// `from_vec` in a program with threads.
pub fn shared_from_vec<T>(items: Vec<T>) -> SharedArray<T> {
    SharedArray::made_as(items, None)
}

/// An array of a program with threads that has the elements of `a`, which the runtime's own
/// functions made (`String.getBytes`, `Character.toChars`).
pub fn to_shared<T: Clone>(a: Array<T>) -> SharedArray<T> {
    let class = a.element_class().cloned();
    // Nothing else holds an array the runtime just made, so its elements can be taken.
    SharedArray::made_as(a.take_items(), class)
}

/// The array `a` just made, as an array of class `class`: a covariant array (`new String[n]`
/// that the translation holds as an `Object[]`), or with `None` one that carries no class (a copy
/// into `Object[].class`).
pub fn tagged<T: Clone, A: MadeElements<T>>(a: A, class: Option<ElementClass<T>>) -> A {
    if class.is_none() && a.element_class().is_none() {
        return a;
    }
    A::made_as(a.take_items(), class)
}

/// The array `made` just made, as an array of the class of `like` (the class `None` of a null
/// `like` keeps): `Arrays.copyOf(x, n, a.getClass())` and `Array.newInstance(a.getClass()
/// .getComponentType(), n)`, the copy that the JDK's `toArray(a)` makes of `a`'s class.
pub fn tagged_like<T: Clone, A: MadeElements<T>, L: Elements<T>>(made: A, like: Option<&L>) -> A {
    tagged(made, like.and_then(|a| a.element_class().cloned()))
}

/// `a.getClass()` of an `Object[]`: the class of a covariant array, or `exact`, `Object[]`'s.
pub fn array_class<T>(a: &impl Elements<T>, exact: fn() -> Class) -> Class {
    a.element_class().map_or_else(exact, |c| (c.class)())
}

/// `Object.toString()` of `a`, an array of references that may carry its class: the binary
/// name of its own class, or `exact` (`[Ljava.lang.Object;`) where it carries none, `@` and
/// `hash` in hexadecimal.
pub fn array_default_text<T>(a: &impl Elements<T>, exact: &str, hash: i32) -> crate::JString {
    crate::default_text(a.element_class().map_or(exact, |c| (c.class)().binary_name()), hash)
}

/// The binary names of the class of `a`, an `Object[]` that may carry its class, and of the
/// classes above it (`ElementClass::own`); those of `Object[]`, `object_array`, where it carries
/// none.
pub fn array_type_names<T>(
    a: &impl Elements<T>,
    object_array: &'static [&'static str],
) -> &'static [&'static str] {
    a.element_class().map_or(object_array, |c| c.own)
}

/// `o instanceof C[]` of an `Object[]` `a`, given the array class `class` (`C[]`) where the
/// translation holds arrays of it as `Object[]`s: whether the class that `a` carries is `class`
/// or below it. An array that carries no class passes, as one of `class` would.
pub fn array_is<T>(a: &impl Elements<T>, class: Class) -> bool {
    a.element_class().is_none_or(|c| class.is_assignable_from((c.class)()))
}

/// `(C[]) o` of an `Object[]` `a` (as `array_is`): `a`, or the `ClassCastException` of a cast
/// of its class to `to`, the class `class` names.
pub fn try_cast_array<T, A: Elements<T>>(a: A, class: Class, to: crate::CastClass) -> Result<A, Exception> {
    match a.element_class() {
        Some(c) if !class.is_assignable_from((c.class)()) => {
            Err(crate::cast_exception(((c.class)().binary_name(), c.module), to))
        }
        _ => Ok(a),
    }
}

/// `try_cast_array`, which throws its `ClassCastException`.
pub fn cast_array<T, A: Elements<T>>(a: A, class: Class, to: crate::CastClass) -> A {
    try_cast_array(a, class, to).unwrap_or_else(|e| crate::throw_exception(e))
}

/// Java's check of `a[i] = v` in an `Object[]` that may be a covariant array: the index is
/// checked first, then whether the array can hold `v`.
#[cold]
#[inline(never)]
fn check_store<T>(class: &ElementClass<T>, len: usize, i: i32, v: &T) -> Result<(), Exception> {
    try_position(len, i)?;
    match class.refuses(v) {
        Some(name) => Err(Exception::new("java.lang.ArrayStoreException", name)),
        None => Ok(()),
    }
}

/// `a[i] = v` in an `Object[]` that may be a covariant array, which throws
/// `ArrayStoreException` for an element of another class.
#[inline]
pub fn checked_set<T>(a: &impl Elements<T>, i: i32, v: T) {
    if let Some(class) = a.element_class() {
        check_store(class, a.length(), i, &v).unwrap_or_else(|e| crate::throw_exception(e));
    }
    set(a, i, v)
}

/// `checked_set` where the program observes the exception.
pub fn try_checked_set<T>(a: &impl Elements<T>, i: i32, v: T) -> Result<(), Exception> {
    if let Some(class) = a.element_class() {
        check_store(class, a.length(), i, &v)?;
    }
    try_set(a, i, v)
}

/// `checked_set` as an expression: stores `v` and gives it back.
pub fn checked_store<T: Clone>(a: &impl Elements<T>, i: i32, v: T) -> T {
    checked_set(a, i, v.clone());
    v
}

/// `checked_store` where the program observes the exception.
pub fn try_checked_store<T: Clone>(a: &impl Elements<T>, i: i32, v: T) -> Result<T, Exception> {
    try_checked_set(a, i, v.clone())?;
    Ok(v)
}

/// `checked_store` where the elements may be null and `v` is not, as `store_some`.
pub fn checked_store_some<T: Clone>(a: &impl Elements<Option<T>>, i: i32, v: T) -> T {
    checked_set(a, i, Some(v.clone()));
    v
}

/// `checked_store_some` where the program observes the exception.
pub fn try_checked_store_some<T: Clone>(a: &impl Elements<Option<T>>, i: i32, v: T) -> Result<T, Exception> {
    try_checked_set(a, i, Some(v.clone()))?;
    Ok(v)
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
#[inline(always)]
pub(crate) fn position(len: usize, i: i32) -> usize {
    if (i as usize) < len { i as usize } else { out_of_bounds(len, i) }
}

/// The elements of an array taken once before a counting loop whose accesses of it are proven
/// in bounds (`java::view`), so that the loop does not load the start and the length again at
/// each access, which a call in the loop could have changed as far as the compiler knows. An
/// access still compares the index with the length, now a value the compiler can compare with the
/// loop's bound, and stops the program on a wrong proof (`wrong_bound`).
pub trait View<T> {
    /// `a[i]`.
    fn at(&self, i: i32) -> T
    where
        T: Clone;
    /// `a[i] = v`.
    fn put(&self, i: i32, v: T);
}

/// The view of an `Array`: the start and the length of its `Vec`, which hold while the handle
/// lives (`Elements::view`). The elements are reached through the pointer, not a borrowed slice,
/// as other handles may write them while the loop runs.
pub struct ArrayView<'v, T> {
    items: *mut T,
    len: usize,
    array: std::marker::PhantomData<&'v T>,
}

impl<T> View<T> for ArrayView<'_, T> {
    #[inline(always)]
    fn at(&self, i: i32) -> T
    where
        T: Clone,
    {
        if (i as usize) >= self.len {
            wrong_bound(self.len, i)
        }
        unsafe { (*self.items.add(i as usize)).clone() }
    }

    #[inline(always)]
    fn put(&self, i: i32, v: T) {
        if (i as usize) >= self.len {
            wrong_bound(self.len, i)
        }
        unsafe { *self.items.add(i as usize) = v }
    }
}

/// The view of a `SharedArray`: its cells.
pub struct SharedView<'v, T>(&'v [SharedField<T>]);

impl<T> View<T> for SharedView<'_, T> {
    #[inline(always)]
    fn at(&self, i: i32) -> T
    where
        T: Clone,
    {
        match self.0.get(i as usize) {
            Some(cell) => cell.get(),
            None => wrong_bound(self.0.len(), i),
        }
    }

    #[inline(always)]
    fn put(&self, i: i32, v: T) {
        match self.0.get(i as usize) {
            Some(cell) => cell.set(v),
            None => wrong_bound(self.0.len(), i),
        }
    }
}

/// The view of a `Frozen` array: its cells, read and written where they are, as `Frozen` does.
pub struct FrozenView<'v, T>(&'v [UnsafeCell<T>]);

impl<T> View<T> for FrozenView<'_, T> {
    #[inline(always)]
    fn at(&self, i: i32) -> T
    where
        T: Clone,
    {
        match self.0.get(i as usize) {
            Some(cell) => unsafe { (*cell.get()).clone() },
            None => wrong_bound(self.0.len(), i),
        }
    }

    #[inline(always)]
    fn put(&self, i: i32, v: T) {
        match self.0.get(i as usize) {
            Some(cell) => unsafe { *cell.get() = v },
            None => wrong_bound(self.0.len(), i),
        }
    }
}

/// The elements of `a` for a counting loop that reads or writes them by indexes proven in bounds
/// (`View`).
#[inline(always)]
pub fn view<T, A: Elements<T>>(a: &A) -> A::View<'_> {
    a.view()
}

/// `a[i]` through a view.
#[inline(always)]
pub fn view_get<T: Clone>(v: &impl View<T>, i: i32) -> T {
    v.at(i)
}

/// `a[i] = x` through a view.
#[inline(always)]
pub fn view_set<T>(v: &impl View<T>, i: i32, x: T) {
    v.put(i, x)
}

/// Stops the program at an index that the translator proved in bounds and is not: a mistake of the
/// translator, which no `catch` of the program should see, as a plain array's `a[i as usize]`
/// panics on one.
#[cold]
#[inline(never)]
fn wrong_bound(len: usize, i: i32) -> ! {
    panic!("index {i} proven in bounds is out of bounds for length {len}")
}

/// Throws Java's `ArrayIndexOutOfBoundsException` for index `i` of an array of `len` elements.
#[cold]
#[inline(never)]
fn out_of_bounds(len: usize, i: i32) -> ! {
    throw("java.lang.ArrayIndexOutOfBoundsException", &format!("Index {i} out of bounds for length {len}"))
}

/// `position` where the program observes the `ArrayIndexOutOfBoundsException`.
#[inline(always)]
pub(crate) fn try_position(len: usize, i: i32) -> Result<usize, Exception> {
    if (i as usize) < len {
        return Ok(i as usize);
    }
    Err(index_out_of_bounds(len, i))
}

/// The `ArrayIndexOutOfBoundsException` of index `i` in an array of `len` elements.
#[cold]
#[inline(never)]
fn index_out_of_bounds(len: usize, i: i32) -> Exception {
    let message = format!("Index {i} out of bounds for length {len}");
    Exception::new("java.lang.ArrayIndexOutOfBoundsException", message)
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
#[inline(always)]
pub fn len<T>(a: &impl Elements<T>) -> i32 {
    let n = a.length();
    if n > i32::MAX as usize {
        too_long(n)
    }
    n as i32
}

#[cold]
#[inline(never)]
fn too_long(n: usize) -> ! {
    panic!("an array of {n} elements, more than Java's arrays hold")
}

/// `a[i]`.
#[inline(always)]
pub fn get<T: Clone>(a: &impl Elements<T>, i: i32) -> T {
    a.element(i)
}

/// `for (i = from; i < end; i++) if (a[i] == key) ..`, with `end` the length when it is `None`:
/// the first such `i`, for values that `==` compares and that cannot fail to compare (integers,
/// characters, booleans, enum constants). The elements of an `Array` are compared in place, four
/// to a turn: LLVM does not unroll a loop that it can leave in two places, its end and the element
/// found, so as Java writes it each element also paid the loop's compare and branch; the JIT
/// unrolls such a loop. An `end` past the length throws Java's `ArrayIndexOutOfBoundsException`
/// once the elements before it are compared, a negative `from` at once.
#[inline(always)]
pub fn search_eq<T: PartialEq + Clone>(
    a: &impl Elements<T>,
    from: i32,
    end: Option<i32>,
    key: T,
) -> Option<i32> {
    let (start, stop, past) = search_range(a.length(), from, end, |i| {
        let _ = a.element(i);
    })?;
    match a.in_place() {
        Some(items) => first_equal(&items[start..stop], &key).map(|k| (start + k) as i32),
        None => (start..stop).find(|&i| a.element(i as i32) == key).map(|i| i as i32),
    }
    .or_else(|| {
        search_past(past, |i| {
            let _ = a.element(i);
        })
    })
}

/// `search_eq` for `a[i] == key` by identity, of references (`Option<E>`): the elements found
/// `same` as the key. Where the key is the same object as a value exactly when their bits are
/// equal (`by_bits`: a handle, or a value of an enum of classes whose class is compared by
/// identity) and an element is one word, the elements are compared as words, four to a turn,
/// which the compiler does with vector compares; otherwise one at a time with `same`.
#[inline(always)]
pub fn search_same<E: Clone>(
    a: &impl Elements<Option<E>>,
    from: i32,
    end: Option<i32>,
    key: Option<&E>,
    by_bits: bool,
    mut same: impl FnMut(Option<E>) -> bool,
) -> Option<i32> {
    let (start, stop, past) = search_range(a.length(), from, end, |i| {
        let _ = a.element(i);
    })?;
    let words = std::mem::size_of::<Option<E>>() == 8 && std::mem::align_of::<Option<E>>() == 8;
    match a.in_place() {
        Some(items) if by_bits && words => {
            let key: Option<E> = key.cloned();
            // SAFETY: a one-word `Option` of a handle or a value held in a word has every bit
            // initialized, and the slice is aligned and as long as `items`.
            let (key, items) = unsafe {
                let key = std::mem::transmute_copy::<Option<E>, u64>(&key);
                (key, std::slice::from_raw_parts(items.as_ptr() as *const u64, items.len()))
            };
            first_equal(&items[start..stop], &key).map(|k| (start + k) as i32)
        }
        _ => (start..stop).find(|&i| same(a.element(i as i32))).map(|i| i as i32),
    }
    .or_else(|| {
        search_past(past, |i| {
            let _ = a.element(i);
        })
    })
}

/// The elements a search compares, `start..stop`, and the index past the length that throws
/// once they are compared, where its end is past the length (Java's next `a[i]`); `None` where it
/// compares nothing. A negative `from` below the end throws at once, through `fail`, as Java's
/// first `a[from]` does.
#[inline(always)]
pub(crate) fn search_range(
    len: usize,
    from: i32,
    end: Option<i32>,
    fail: impl Fn(i32),
) -> Option<(usize, usize, Option<i32>)> {
    let end = end.unwrap_or(len as i32);
    if from >= end {
        return None;
    }
    if from < 0 {
        fail(from);
    }
    let start = (from as usize).min(len);
    let stop = (end as usize).min(len).max(start);
    let past = ((end as usize) > len).then(|| from.max(len as i32));
    Some((start, stop, past))
}

/// After a search found nothing: throws at `past`, where it is the index past the length that
/// Java's loop reads next.
#[inline(always)]
pub(crate) fn search_past(past: Option<i32>, fail: impl Fn(i32)) -> Option<i32> {
    if let Some(i) = past {
        fail(i);
    }
    None
}

/// The first position of `key` in `items`, four to a turn: the four compares are made whatever
/// they find, then one branch, so the loop has one exit per turn and the compiler can make the
/// compares one vector compare.
#[inline(always)]
pub(crate) fn first_equal<T: PartialEq>(items: &[T], key: &T) -> Option<usize> {
    let mut i = 0;
    while i + 4 <= items.len() {
        let turn = &items[i..i + 4];
        if (turn[0] == *key) | (turn[1] == *key) | (turn[2] == *key) | (turn[3] == *key) {
            break;
        }
        i += 4;
    }
    items[i..].iter().position(|item| item == key).map(|k| i + k)
}

/// `a[i]` where the program observes the `ArrayIndexOutOfBoundsException`.
#[inline(always)]
pub fn try_get<T: Clone>(a: &impl Elements<T>, i: i32) -> Result<T, Exception> {
    a.try_element(i)
}

/// `a[i] = v` where the program observes the `ArrayIndexOutOfBoundsException`.
#[inline(always)]
pub fn try_set<T>(a: &impl Elements<T>, i: i32, v: T) -> Result<(), Exception> {
    a.try_put(i, v)
}

/// `a[i] = v`. Java evaluates `a`, `i` and `v` before the check, and so do the arguments.
#[inline(always)]
pub fn set<T>(a: &impl Elements<T>, i: i32, v: T) {
    a.put(i, v)
}

/// `a[i]` for an `i` that the translator proved in bounds: a wrong proof stops the program.
#[inline(always)]
pub fn get_in_bounds<T: Clone>(a: &impl Elements<T>, i: i32) -> T {
    a.element_in_bounds(i)
}

/// `a[i] = v` for an `i` that the translator proved in bounds.
#[inline(always)]
pub fn set_in_bounds<T>(a: &impl Elements<T>, i: i32, v: T) {
    a.put_in_bounds(i, v)
}

/// `a[i]` as a volatile read (`Elements::try_element_volatile`), which a volatile access through
/// a `VarHandle` or `Unsafe` makes.
#[inline]
pub fn get_volatile<T: Clone>(a: &impl Elements<T>, i: i32) -> T {
    a.try_element_volatile(i).unwrap_or_else(|e| crate::throw_exception(e))
}

/// `get_volatile` where the program observes the `ArrayIndexOutOfBoundsException`.
#[inline]
pub fn try_get_volatile<T: Clone>(a: &impl Elements<T>, i: i32) -> Result<T, Exception> {
    a.try_element_volatile(i)
}

/// `a[i] = v` as a volatile write (`Elements::try_put_volatile`).
#[inline]
pub fn set_volatile<T>(a: &impl Elements<T>, i: i32, v: T) {
    a.try_put_volatile(i, v).unwrap_or_else(|e| crate::throw_exception(e))
}

/// `set_volatile` where the program observes the `ArrayIndexOutOfBoundsException`.
#[inline]
pub fn try_set_volatile<T>(a: &impl Elements<T>, i: i32, v: T) -> Result<(), Exception> {
    a.try_put_volatile(i, v)
}

/// `checked_set` as a volatile write.
pub fn checked_set_volatile<T>(a: &impl Elements<T>, i: i32, v: T) {
    if let Some(class) = a.element_class() {
        check_store(class, a.length(), i, &v).unwrap_or_else(|e| crate::throw_exception(e));
    }
    set_volatile(a, i, v)
}

/// `checked_set_volatile` where the program observes the exception.
pub fn try_checked_set_volatile<T>(a: &impl Elements<T>, i: i32, v: T) -> Result<(), Exception> {
    if let Some(class) = a.element_class() {
        check_store(class, a.length(), i, &v)?;
    }
    try_set_volatile(a, i, v)
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

// The atomic updates of an element that `Unsafe` makes at an element offset. Each reads, compares
// and writes the element in one atomic action: under one borrow of an `Array`, and as one
// compare-and-swap of a `SharedArray`'s cell (or under its striped lock, for a wide element), so
// no other access of the element, from any thread, comes between the read and the write, as with
// Java's atomic instructions. `same` is Java's `==` on the elements.

/// `Unsafe.compareAndSet<Kind>`: whether `a[i]` was `expected`, in which case it is now `value`.
pub fn compare_and_set_element<T: Clone>(
    a: &impl Elements<T>,
    i: i32,
    expected: T,
    value: T,
    same: impl Fn(&T, &T) -> bool,
) -> bool {
    try_compare_and_set_element(a, i, expected, value, same).unwrap_or_else(|e| crate::throw_exception(e))
}

/// `compare_and_set_element` where the program observes the `ArrayIndexOutOfBoundsException`.
pub fn try_compare_and_set_element<T: Clone>(
    a: &impl Elements<T>,
    i: i32,
    expected: T,
    value: T,
    same: impl Fn(&T, &T) -> bool,
) -> Result<bool, Exception> {
    let old = a.try_compare_exchange(i, expected.clone(), value, &same)?;
    Ok(same(&old, &expected))
}

/// `Unsafe.compareAndExchange<Kind>`: the old `a[i]`, which is now `value` if it was `expected`.
pub fn compare_and_exchange_element<T: Clone>(
    a: &impl Elements<T>,
    i: i32,
    expected: T,
    value: T,
    same: impl Fn(&T, &T) -> bool,
) -> T {
    try_compare_and_exchange_element(a, i, expected, value, same)
        .unwrap_or_else(|e| crate::throw_exception(e))
}

/// `compare_and_exchange_element` where the program observes the `ArrayIndexOutOfBoundsException`.
pub fn try_compare_and_exchange_element<T: Clone>(
    a: &impl Elements<T>,
    i: i32,
    expected: T,
    value: T,
    same: impl Fn(&T, &T) -> bool,
) -> Result<T, Exception> {
    a.try_compare_exchange(i, expected, value, same)
}

/// `Unsafe.getAndSet<Kind>`: the old `a[i]`, which is now `value`.
pub fn get_and_set_element<T>(a: &impl Elements<T>, i: i32, value: T) -> T {
    try_get_and_set_element(a, i, value).unwrap_or_else(|e| crate::throw_exception(e))
}

/// `get_and_set_element` where the program observes the `ArrayIndexOutOfBoundsException`.
pub fn try_get_and_set_element<T>(a: &impl Elements<T>, i: i32, value: T) -> Result<T, Exception> {
    a.try_swap(i, value)
}

/// `Unsafe.getAndAdd<Kind>` (and the bitwise forms): the old `a[i]`, which is now
/// `add(old, delta)` (Java's wrapping addition, or the bitwise operation).
pub fn get_and_add_element<T: Copy>(a: &impl Elements<T>, i: i32, delta: T, add: fn(T, T) -> T) -> T {
    try_get_and_add_element(a, i, delta, add).unwrap_or_else(|e| crate::throw_exception(e))
}

/// `get_and_add_element` where the program observes the `ArrayIndexOutOfBoundsException`.
pub fn try_get_and_add_element<T: Copy>(
    a: &impl Elements<T>,
    i: i32,
    delta: T,
    add: fn(T, T) -> T,
) -> Result<T, Exception> {
    a.try_get_and_update(i, |old| add(*old, delta))
}

/// A value that `MethodHandles.byteArrayViewVarHandle` reads from and writes to the bytes of a
/// `byte[]`: a `short`, `char`, `int`, `long`, `float` or `double`, as its bytes in either order (a
/// `float` or `double` as its raw bits, as Java's view does).
pub trait ByteView: Sized {
    /// The number of bytes.
    const SIZE: usize;
    fn from_bytes(bytes: &[i8], big_endian: bool) -> Self;
    fn write_bytes(self, bytes: &mut [i8], big_endian: bool);
}

macro_rules! byte_view {
    ($($t:ty),*) => {$(
        impl ByteView for $t {
            const SIZE: usize = std::mem::size_of::<$t>();
            fn from_bytes(bytes: &[i8], big_endian: bool) -> Self {
                let mut raw = [0u8; std::mem::size_of::<$t>()];
                for (r, b) in raw.iter_mut().zip(bytes) {
                    *r = *b as u8;
                }
                if big_endian { <$t>::from_be_bytes(raw) } else { <$t>::from_le_bytes(raw) }
            }
            fn write_bytes(self, bytes: &mut [i8], big_endian: bool) {
                let raw = if big_endian { self.to_be_bytes() } else { self.to_le_bytes() };
                for (b, r) in bytes.iter_mut().zip(raw) {
                    *b = r as i8;
                }
            }
        }
    )*};
}

byte_view!(i16, u16, i32, i64, f32, f64);

/// The first of the bytes of a view of `size` bytes at index `i` of an array of `len` bytes:
/// Java checks `i` against `len - (size - 1)`, and names that length in its message.
fn view_position(len: usize, size: usize, i: i32) -> Result<usize, Exception> {
    let fits = len as i64 - (size as i64 - 1);
    if i >= 0 && (i as i64) < fits {
        return Ok(i as usize);
    }
    let message = format!("Index {i} out of bounds for length {fits}");
    Err(Exception::new("java.lang.ArrayIndexOutOfBoundsException", message))
}

/// `VIEW.get(a, i)` of a `byteArrayViewVarHandle`: the value whose bytes are `a[i..]`.
pub fn byte_view_get<V: ByteView>(a: &impl Elements<i8>, i: i32, big_endian: bool) -> V {
    try_byte_view_get(a, i, big_endian).unwrap_or_else(|e| crate::throw_exception(e))
}

/// `byte_view_get` where the program observes the `ArrayIndexOutOfBoundsException`.
pub fn try_byte_view_get<V: ByteView>(
    a: &impl Elements<i8>,
    i: i32,
    big_endian: bool,
) -> Result<V, Exception> {
    let k = view_position(a.length(), V::SIZE, i)?;
    Ok(a.reading(k..k + V::SIZE, |bytes| V::from_bytes(bytes, big_endian)))
}

/// `VIEW.set(a, i, v)` of a `byteArrayViewVarHandle`: `v`'s bytes written to `a[i..]`, all of them
/// or, when they do not fit, none.
pub fn byte_view_set<V: ByteView>(a: &impl Elements<i8>, i: i32, v: V, big_endian: bool) {
    try_byte_view_set(a, i, v, big_endian).unwrap_or_else(|e| crate::throw_exception(e))
}

/// `byte_view_set` where the program observes the `ArrayIndexOutOfBoundsException`.
pub fn try_byte_view_set<V: ByteView>(
    a: &impl Elements<i8>,
    i: i32,
    v: V,
    big_endian: bool,
) -> Result<(), Exception> {
    let k = view_position(a.length(), V::SIZE, i)?;
    a.writing(k..k + V::SIZE, |bytes| v.write_bytes(bytes, big_endian));
    Ok(())
}

/// `a[i] op= ...` where the program observes the exceptions of the index or of `f` (a division
/// by zero).
pub fn try_update<T: Clone>(
    a: &impl Elements<T>,
    i: i32,
    f: impl FnOnce(T) -> Result<T, Exception>,
) -> Result<T, Exception> {
    let new = f(try_get(a, i)?)?;
    try_set(a, i, new.clone())?;
    Ok(new)
}

/// `a[i]++` and the like where the program observes the exceptions: returns the old element.
pub fn try_post_update<T: Clone>(
    a: &impl Elements<T>,
    i: i32,
    f: impl FnOnce(T) -> Result<T, Exception>,
) -> Result<T, Exception> {
    let old = try_get(a, i)?;
    try_set(a, i, f(old.clone())?)?;
    Ok(old)
}

/// `Arrays.toString(a)`, with `show` formatting each element as Java does.
pub fn to_string<T: Clone, S: Into<crate::JString>>(
    a: &impl Elements<T>,
    show: impl Fn(T) -> S,
) -> crate::JString {
    // The elements are copied out first: `show` may run code of the program, which must not run
    // while the array is borrowed.
    let items: Vec<T> = a.reading(0..a.length(), <[T]>::to_vec);
    let mut out = crate::JString::from("[");
    for (i, item) in items.into_iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_wtf8(&show(item).into());
    }
    out.push_str("]");
    out
}

/// `Arrays.fill(a, v)`.
pub fn fill<T: Clone>(a: &impl Elements<T>, v: T) {
    a.fill_with(0..a.length(), v);
}

/// `Arrays.sort(a)` for integers (`int`, `long`, `short`, `byte`, `char`). Equal values cannot be
/// told apart, so the order among them is not seen: a long array is sorted by its bytes
/// (`radix_sort`), as Java's `DualPivotQuicksort` sorts a large `int[]` by its digits, and a short
/// one with Rust's unstable sort, which moves less than the stable one.
pub fn sort<T: RadixKey>(a: &impl Elements<T>) {
    a.writing(0..a.length(), |items| {
        if items.len() >= RADIX_SORT_MIN { radix_sort(items) } else { items.sort_unstable() }
    });
}

/// The fewest elements that `sort` sorts by their bytes: below, Rust's sort is as fast, and needs
/// no second array.
const RADIX_SORT_MIN: usize = 4096;

/// An integer that `radix_sort` sorts by its bytes: `key` maps it to an unsigned number in the
/// same order (the sign bit flipped), of which the low `BYTES` bytes count.
pub trait RadixKey: Ord + Copy {
    const BYTES: usize;
    fn key(self) -> u64;
}

macro_rules! radix_key {
    ($($t:ty => $flip:expr),*) => {$(
        impl RadixKey for $t {
            const BYTES: usize = size_of::<$t>();
            #[inline(always)]
            fn key(self) -> u64 {
                (self as u64 ^ $flip) & (u64::MAX >> (64 - 8 * size_of::<$t>()))
            }
        }
    )*};
}

radix_key!(i8 => 1 << 7, i16 => 1 << 15, i32 => 1 << 31, i64 => 1 << 63, u16 => 0);

/// Sorts `items` by their keys a byte at a time, lowest first, each pass a stable scatter into the
/// other of two arrays: one count of every byte's values first, then a pass for each byte that
/// does not hold the same value in every element. A million random `int`s take 2.6 ms, against
/// 9 ms for Rust's unstable sort (SortMillion's library sort).
fn radix_sort<T: RadixKey>(items: &mut [T]) {
    let n = items.len();
    let mut counts = vec![[0usize; 256]; T::BYTES];
    for &v in items.iter() {
        let k = v.key();
        for (b, c) in counts.iter_mut().enumerate() {
            c[((k >> (8 * b)) & 0xff) as usize] += 1;
        }
    }
    let mut other: Vec<T> = items.to_vec();
    let (mut from, mut to): (&mut [T], &mut [T]) = (items, &mut other);
    let mut in_other = false;
    for (b, c) in counts.iter_mut().enumerate() {
        if c.contains(&n) {
            continue;
        }
        let mut sum = 0;
        for x in c.iter_mut() {
            sum += std::mem::replace(x, sum);
        }
        for &v in from.iter() {
            let d = ((v.key() >> (8 * b)) & 0xff) as usize;
            to[c[d]] = v;
            c[d] += 1;
        }
        std::mem::swap(&mut from, &mut to);
        in_other = !in_other;
    }
    if in_other {
        to.copy_from_slice(from);
    }
}

/// `Arrays.sort(a)` for doubles, in Java's order (`Double.compare`): -0.0 before 0.0, and every
/// NaN last, whatever its sign bit (`total_cmp` puts a negative NaN, which x86 makes for `0.0 / 0.0`,
/// first).
pub fn sort_doubles(a: &impl Elements<f64>) {
    let key = |v: f64| if v.is_nan() { i64::MAX } else { v.to_bits() as i64 };
    a.writing(0..a.length(), |items| {
        items.sort_by(|x, y| x.partial_cmp(y).filter(|o| o.is_ne()).unwrap_or_else(|| key(*x).cmp(&key(*y))))
    });
}

/// `Arrays.sort(a)` for floats, in Java's order (`Float.compare`), as for doubles.
pub fn sort_floats(a: &impl Elements<f32>) {
    let key = |v: f32| if v.is_nan() { i32::MAX } else { v.to_bits() as i32 };
    a.writing(0..a.length(), |items| {
        items.sort_by(|x, y| x.partial_cmp(y).filter(|o| o.is_ne()).unwrap_or_else(|| key(*x).cmp(&key(*y))))
    });
}

/// `Arrays.copyOf(a, len)`: the first `len` elements, padded with `zero`, of `a`'s class.
pub fn copy_of<T: Clone, A: MadeElements<T>>(a: &A, len: i32, zero: T) -> A {
    let len = size(len);
    let mut items = a.reading(0..len.min(a.length()), <[T]>::to_vec);
    items.resize(len, zero);
    a.made_like(items)
}

/// `copy_of` where the program observes the `NegativeArraySizeException`.
pub fn try_copy_of<T: Clone, A: MadeElements<T>>(a: &A, len: i32, zero: T) -> Result<A, Exception> {
    let len = try_size(len)?;
    let mut items = a.reading(0..len.min(a.length()), <[T]>::to_vec);
    items.resize(len, zero);
    Ok(a.made_like(items))
}

/// `a[i] = v` as an expression: stores `v` and gives it back.
#[inline]
pub fn store<T: Clone>(a: &impl Elements<T>, i: i32, v: T) -> T {
    set(a, i, v.clone());
    v
}

/// `a[i] = v` as an expression, where the program observes the exception.
#[inline]
pub fn try_store<T: Clone>(a: &impl Elements<T>, i: i32, v: T) -> Result<T, Exception> {
    try_set(a, i, v.clone())?;
    Ok(v)
}

/// `a[i] = v` as an expression, where the elements may be null and `v` is not: the value of the
/// assignment is `v` itself, not an `Option`.
#[inline]
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

/// Checks the sizes of a multi-dimensional `new`, before any array is allocated. Inside
/// `catching`, a negative size unwinds to it.
pub fn check_sizes(dims: &[i32]) {
    try_check_sizes(dims).unwrap_or_else(|e| crate::throw_exception(e))
}

/// `check_sizes` where the program observes the `NegativeArraySizeException`.
pub fn try_check_sizes(dims: &[i32]) -> Result<(), Exception> {
    for &d in dims {
        try_size(d)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::JString;

    #[test]
    fn a_frozen_array_is_written_while_it_is_made_and_then_read_where_it_is() {
        let squares: Frozen<i32, 4> = freeze_fixed(shared_filled(4, 0));
        let held = squares.clone();
        for i in 0..4 {
            set(&held, i, i * i);
        }
        update(&squares, 3, |old| old + 100);
        assert_eq!((0..4).map(|i| get(&squares, i)).collect::<Vec<_>>(), [0, 1, 4, 109]);
        assert_eq!((len(&squares), squares.in_place()), (4, Some(&[0, 1, 4, 109][..])));
        let error = try_get(&squares, 4).unwrap_err();
        assert_eq!(error.message.as_deref(), Some("Index 4 out of bounds for length 4"));
        let names: Frozen<Option<JString>> = freeze(from_vec(vec![None, Some(JString::from("b"))]));
        assert_eq!((len(&names), get(&names, 1)), (2, Some(JString::from("b"))));
    }

    #[test]
    #[should_panic(expected = "an array frozen with another length")]
    fn a_frozen_array_of_a_constant_length_has_that_length() {
        let _: Frozen<i32, 4> = freeze_fixed(shared_filled(3, 0));
    }

    #[test]
    fn a_shared_array_is_read_and_written_one_element_at_a_time() {
        let longs = shared_filled(4, 0i64);
        let names = shared_from_vec(vec![Some(JString::from("left")); 4]);
        let (a, b) = (0x0123_4567_89ab_cdefi64, -0x0123_4567_89ab_cdf0i64);
        let writers: Vec<_> = (0..2)
            .map(|w| {
                let (longs, names) = (longs.clone(), names.clone());
                std::thread::spawn(move || {
                    for i in 0..2000 {
                        set(&longs, i & 3, if (i + w) & 1 == 0 { a } else { b });
                        let name = if (i + w) & 1 == 0 { "left" } else { "right" };
                        set(&names, i & 3, Some(JString::from(name)));
                    }
                })
            })
            .collect();
        for i in 0..2000 {
            // A reader sees only values that a writer stored, never half of one.
            assert!([0, a, b].contains(&get(&longs, i & 3)));
            let name = get(&names, i & 3).expect("never null");
            assert!(name == "left" || name == "right");
        }
        for w in writers {
            w.join().unwrap();
        }
        assert!(try_get(&longs, 4).is_err());
    }

    #[test]
    fn a_long_array_of_integers_sorts_by_its_bytes_as_rust_sorts_it() {
        let mut x = 7i64;
        let mut next = || {
            x ^= x << 13;
            x ^= ((x as u64) >> 7) as i64;
            x ^= x << 17;
            x
        };
        let longs: Vec<i64> = (0..10000).map(|_| next()).chain([i64::MIN, i64::MAX, 0, -1]).collect();
        let ints: Vec<i32> = longs.iter().map(|&v| v as i32).collect();
        let shorts: Vec<i16> = longs.iter().map(|&v| (v >> 20) as i16).collect();
        let bytes: Vec<i8> = longs.iter().map(|&v| v as i8).collect();
        let chars: Vec<u16> = longs.iter().map(|&v| v as u16).collect();
        // Values that differ only in their high bytes, whose low passes are skipped.
        let high: Vec<i32> = longs.iter().map(|&v| (v as i32) & !0xffff).collect();
        fn check<T: RadixKey + std::fmt::Debug>(v: Vec<T>) {
            let mut expected = v.clone();
            expected.sort();
            let a = from_vec(v.clone());
            sort(&a);
            assert_eq!(a.take_items(), expected);
            let shared = shared_from_vec(v);
            sort(&shared);
            assert_eq!(shared.take_items(), expected);
        }
        check(longs);
        check(ints);
        check(shorts);
        check(bytes);
        check(chars);
        check(high);
    }

    #[test]
    fn bulk_operations_on_a_shared_array_match_those_on_an_array() {
        let shared = shared_from_vec(vec![5, 3, 9, 1, 7]);
        let local = from_vec(vec![5, 3, 9, 1, 7]);
        crate::arraycopy(&shared, 0, &shared, 1, 4);
        crate::arraycopy(&local, 0, &local, 1, 4);
        assert_eq!(shared.take_items(), local.take_items());
        let shared = shared_from_vec(vec![5, 3, 9, 1, 7]);
        sort(&shared);
        assert_eq!(shared.take_items(), [1, 3, 5, 7, 9]);
        let filled = shared_from_vec(vec![0; 5]);
        crate::fill_range(&filled, 1, 4, 6);
        assert_eq!(filled.take_items(), [0, 6, 6, 6, 0]);
        fill(&filled, 2);
        assert_eq!(filled.take_items(), [2; 5]);
        assert_eq!(compare_and_exchange_element(&shared, 2, 5, 6, |x, y| x == y), 5);
        assert!(!compare_and_set_element(&shared, 2, 5, 8, |x, y| x == y));
        assert_eq!(get_and_add_element(&shared, 2, 10, i32::wrapping_add), 6);
        assert_eq!(copy_of(&shared, 3, 0).take_items(), [1, 3, 16]);
        let mut chars = vec![0u16; 3];
        crate::copy_slice(&shared_from_vec(vec![7u16, 8, 9]).borrow(), 1, &mut chars, 0, 2);
        assert_eq!(chars, [8, 9, 0]);
        let target = shared_filled(4, 0u16);
        crate::copy_slice(&chars, 0, &mut target.borrow_mut(), 2, 2);
        assert_eq!(target.take_items(), [0, 0, 8, 9]);
    }

    #[test]
    fn volatile_accesses_reach_the_element_of_either_kind_of_array() {
        let shared = shared_filled(2, 0i64);
        let local = filled(2, 0i64);
        set_volatile(&shared, 1, 7);
        set_volatile(&local, 1, 7);
        assert_eq!((get_volatile(&shared, 1), get_volatile(&local, 1)), (7, 7));
        assert_eq!((get(&shared, 1), get(&local, 1)), (7, 7));
        assert!(try_get_volatile(&shared, 2).is_err() && try_set_volatile(&local, -1, 1).is_err());
        let names = shared_filled(1, None);
        set_volatile(&names, 0, Some(JString::from("set")));
        assert!(get_volatile(&names, 0).is_some_and(|name| name == "set"));
    }
}
