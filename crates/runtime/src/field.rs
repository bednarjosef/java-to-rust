//! `java::Field<T>`: a field of an object that the program changes after building it, read and
//! written by value through the object's address, as Java does (docs/object-model.md, 1.2).
//!
//! An object whose fields change is shared by every reference to it, so a field is a cell: a
//! read copies its value out (`get`), a write puts a new one in (`set`). No reference into the
//! cell is ever handed out, so two accesses cannot conflict the way two borrows of a `RefCell`
//! can, and there is no borrow flag to keep. The values are handles, primitives and strings,
//! whose clones touch nothing but their own bits or bytes, so a read never runs code that could
//! write the field it reads.
//!
//! Only the objects of one thread hold `Field`s (it is not `Sync`). The objects that threads share
//! hold `SharedField`s, which are read and written by value too, each access one atomic action.

use std::cell::UnsafeCell;
use std::fmt;
use std::mem::{align_of, needs_drop, size_of, transmute_copy};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU16, AtomicU32, AtomicU64, Ordering};

#[repr(transparent)]
pub struct Field<T>(UnsafeCell<T>);

impl<T> Field<T> {
    #[inline(always)]
    pub const fn new(value: T) -> Field<T> {
        Field(UnsafeCell::new(value))
    }

    /// A copy of the value.
    #[inline(always)]
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        // No reference into the cell outlives a call of `get` or `set`, and cloning the value
        // does not reach the cell (see the module comment).
        unsafe { (*self.0.get()).clone() }
    }

    /// Replaces the value.
    #[inline(always)]
    pub fn set(&self, value: T) {
        // As in `get`: nothing else reads or writes the cell meanwhile.
        let old = unsafe { std::ptr::replace(self.0.get(), value) };
        drop(old);
    }

    /// Replaces the value, returning the old one.
    #[inline(always)]
    pub fn replace(&self, value: T) -> T {
        // As in `get`.
        unsafe { std::ptr::replace(self.0.get(), value) }
    }

    /// The value of a cell that nothing else reaches.
    #[inline(always)]
    pub fn get_mut(&mut self) -> &mut T {
        self.0.get_mut()
    }

    pub fn into_inner(self) -> T {
        self.0.into_inner()
    }
}

impl<T: Default> Default for Field<T> {
    fn default() -> Field<T> {
        Field::new(T::default())
    }
}

/// A copy of the object copies each field's value (`Object.clone()`).
impl<T: Clone> Clone for Field<T> {
    fn clone(&self) -> Field<T> {
        Field::new(self.get())
    }
}

impl<T: Clone + fmt::Debug> fmt::Debug for Field<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get().fmt(f)
    }
}

/// `java::SharedField<T>`: a field of an object that threads share (docs/multi-threading.md, 1.1).
///
/// A value of 1, 2, 4 or 8 bytes without drop glue (a primitive, a reference, a string with an
/// identity) is its bits, which a clone copies: it is loaded with acquire and stored with release,
/// which on x86-64 are plain moves. A `volatile` field's accesses are sequentially consistent, as
/// Java's are. Any other value is read and written under one of a few striped spin locks, picked
/// by the field's address and held only to copy or replace the value; while one thread runs alone
/// no lock is taken. So every access is one atomic action, as Java's are, and none is ever torn.
#[repr(transparent)]
pub struct SharedField<T>(UnsafeCell<T>);

// Every access is atomic: the value moves between threads as an `AtomicU64` or a `Mutex<T>`'s.
unsafe impl<T: Send> Sync for SharedField<T> {}

/// The number of striped locks of the fields that are not atomic words.
const STRIPES: usize = 64;

/// A spin lock on its own cache line.
#[repr(align(64))]
struct Stripe(AtomicBool);

static STRIPED: [Stripe; STRIPES] = [const { Stripe(AtomicBool::new(false)) }; STRIPES];

/// Runs `f` holding the striped lock of `address`. `f` copies or replaces one value, and the copy
/// of a field's value reaches no other field, so no lock is taken while one is held.
#[inline]
fn striped<R>(address: usize, f: impl FnOnce() -> R) -> R {
    if crate::locked::alone() {
        return f();
    }
    let stripe = &STRIPED[(address >> 3) % STRIPES].0;
    let mut spins = 0u32;
    while stripe.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
        spins += 1;
        if spins < 64 {
            std::hint::spin_loop();
        } else {
            std::thread::yield_now();
        }
    }
    let result = f();
    stripe.store(false, Ordering::Release);
    result
}

impl<T> SharedField<T> {
    /// Whether the value is an atomic word: its bits, which a clone copies.
    const WORD: bool =
        !needs_drop::<T>() && matches!(size_of::<T>(), 1 | 2 | 4 | 8) && align_of::<T>() >= size_of::<T>();

    #[inline(always)]
    pub const fn new(value: T) -> SharedField<T> {
        SharedField(UnsafeCell::new(value))
    }

    #[inline(always)]
    fn load(&self, order: Ordering) -> T {
        let p = self.0.get();
        // `WORD`: the value is a word of its size, aligned for it, whose bits are a copy.
        unsafe {
            match size_of::<T>() {
                1 => transmute_copy(&(*(p as *const AtomicU8)).load(order)),
                2 => transmute_copy(&(*(p as *const AtomicU16)).load(order)),
                4 => transmute_copy(&(*(p as *const AtomicU32)).load(order)),
                _ => transmute_copy(&(*(p as *const AtomicU64)).load(order)),
            }
        }
    }

    #[inline(always)]
    fn store(&self, value: T, order: Ordering) {
        let p = self.0.get();
        // As in `load`; the old value has no drop glue.
        unsafe {
            match size_of::<T>() {
                1 => (*(p as *const AtomicU8)).store(transmute_copy(&value), order),
                2 => (*(p as *const AtomicU16)).store(transmute_copy(&value), order),
                4 => (*(p as *const AtomicU32)).store(transmute_copy(&value), order),
                _ => (*(p as *const AtomicU64)).store(transmute_copy(&value), order),
            }
        }
        std::mem::forget(value);
    }

    /// A copy of the value.
    #[inline(always)]
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        if Self::WORD {
            return self.load(Ordering::Acquire);
        }
        // Under the stripe, nothing writes the value while it is copied.
        striped(self.0.get() as usize, || unsafe { (*self.0.get()).clone() })
    }

    /// Replaces the value.
    #[inline(always)]
    pub fn set(&self, value: T) {
        if Self::WORD {
            return self.store(value, Ordering::Release);
        }
        drop(self.replace(value));
    }

    /// Replaces the value, returning the old one.
    pub fn replace(&self, value: T) -> T {
        if Self::WORD {
            let p = self.0.get();
            // As in `load`.
            let old = unsafe {
                match size_of::<T>() {
                    1 => transmute_copy(
                        &(*(p as *const AtomicU8)).swap(transmute_copy(&value), Ordering::AcqRel),
                    ),
                    2 => transmute_copy(
                        &(*(p as *const AtomicU16)).swap(transmute_copy(&value), Ordering::AcqRel),
                    ),
                    4 => transmute_copy(
                        &(*(p as *const AtomicU32)).swap(transmute_copy(&value), Ordering::AcqRel),
                    ),
                    _ => transmute_copy(
                        &(*(p as *const AtomicU64)).swap(transmute_copy(&value), Ordering::AcqRel),
                    ),
                }
            };
            std::mem::forget(value);
            return old;
        }
        // Under the stripe, nothing else reads or writes the value.
        striped(self.0.get() as usize, || unsafe { std::ptr::replace(self.0.get(), value) })
    }

    /// The bits of a word value.
    #[inline(always)]
    fn bits(value: &T) -> u64 {
        // `WORD`: the value is its bits, of its size.
        unsafe {
            match size_of::<T>() {
                1 => transmute_copy::<T, u8>(value) as u64,
                2 => transmute_copy::<T, u16>(value) as u64,
                4 => transmute_copy::<T, u32>(value) as u64,
                _ => transmute_copy::<T, u64>(value),
            }
        }
    }

    /// Writes `value` if the field still holds the word `old`, in one atomic action.
    #[inline(always)]
    fn swap_if(&self, old: &T, value: &T) -> bool {
        let p = self.0.get();
        let (old, new) = (Self::bits(old), Self::bits(value));
        let order = (Ordering::SeqCst, Ordering::Acquire);
        // As in `load`.
        unsafe {
            match size_of::<T>() {
                1 => {
                    (*(p as *const AtomicU8)).compare_exchange(old as u8, new as u8, order.0, order.1).is_ok()
                }
                2 => (*(p as *const AtomicU16))
                    .compare_exchange(old as u16, new as u16, order.0, order.1)
                    .is_ok(),
                4 => (*(p as *const AtomicU32))
                    .compare_exchange(old as u32, new as u32, order.0, order.1)
                    .is_ok(),
                _ => (*(p as *const AtomicU64)).compare_exchange(old, new, order.0, order.1).is_ok(),
            }
        }
    }

    /// `compareAndExchange` through `Unsafe`: the old value, and the field set to `value` if the
    /// old value is `same` as `expected` (identity for references), in one atomic action.
    pub fn compare_exchange_by(&self, expected: T, value: T, same: impl Fn(&T, &T) -> bool) -> T
    where
        T: Clone,
    {
        if Self::WORD {
            loop {
                let old = self.load(Ordering::Acquire);
                if !same(&old, &expected) || self.swap_if(&old, &value) {
                    std::mem::forget(value);
                    return old;
                }
            }
        }
        // Under the stripe, nothing else reads or writes the value.
        striped(self.0.get() as usize, || unsafe {
            let old = (*self.0.get()).clone();
            if same(&old, &expected) {
                drop(std::ptr::replace(self.0.get(), value));
            }
            old
        })
    }

    /// `compareAndSet` through `Unsafe`: whether the old value was `same` as `expected`, and the
    /// field then set to `value`, in one atomic action.
    pub fn compare_and_set_by(&self, expected: T, value: T, same: impl Fn(&T, &T) -> bool) -> bool
    where
        T: Clone,
    {
        let old = self.compare_exchange_by(expected.clone(), value, &same);
        same(&old, &expected)
    }

    /// `getAndAdd` and the bitwise forms through `Unsafe`: the old value, and the field set to
    /// `f` of it, in one atomic action.
    pub fn get_and_update(&self, f: impl Fn(&T) -> T) -> T
    where
        T: Clone,
    {
        if Self::WORD {
            loop {
                let old = self.load(Ordering::Acquire);
                let new = f(&old);
                if self.swap_if(&old, &new) {
                    std::mem::forget(new);
                    return old;
                }
            }
        }
        // As in `compare_exchange_by`.
        striped(self.0.get() as usize, || unsafe {
            let old = (*self.0.get()).clone();
            drop(std::ptr::replace(self.0.get(), f(&old)));
            old
        })
    }

    /// A copy of the value of a `volatile` field.
    #[inline(always)]
    pub fn get_volatile(&self) -> T
    where
        T: Clone,
    {
        if Self::WORD {
            return self.load(Ordering::SeqCst);
        }
        self.get()
    }

    /// Replaces the value of a `volatile` field.
    #[inline(always)]
    pub fn set_volatile(&self, value: T) {
        if Self::WORD {
            return self.store(value, Ordering::SeqCst);
        }
        self.set(value)
    }

    /// The value of a cell that nothing else reaches.
    #[inline(always)]
    pub fn get_mut(&mut self) -> &mut T {
        self.0.get_mut()
    }

    pub fn into_inner(self) -> T {
        self.0.into_inner()
    }
}

impl<T: Default> Default for SharedField<T> {
    fn default() -> SharedField<T> {
        SharedField::new(T::default())
    }
}

/// A copy of the object copies each field's value (`Object.clone()`).
impl<T: Clone> Clone for SharedField<T> {
    fn clone(&self) -> SharedField<T> {
        SharedField::new(self.get())
    }
}

impl<T: Clone + fmt::Debug> fmt::Debug for SharedField<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get().fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_field_is_read_and_written_by_value() {
        let f = Field::new(3);
        f.set(f.get() + 4);
        assert_eq!(f.get(), 7);
        assert_eq!(f.replace(1), 7);
        let s = Field::new(String::from("a"));
        s.set(s.get() + "b");
        assert_eq!(s.get(), "ab");
        assert_eq!(s.clone().into_inner(), "ab");
    }

    #[test]
    fn a_shared_field_is_one_atomic_action_per_access() {
        let n = std::sync::Arc::new(SharedField::new(0i64));
        let s = std::sync::Arc::new(SharedField::new(String::from("a")));
        let threads: Vec<_> = (0..4)
            .map(|_| {
                let (n, s) = (n.clone(), s.clone());
                std::thread::spawn(move || {
                    for _ in 0..1000 {
                        n.set(n.get() + 1);
                        let old = s.get();
                        s.set(if old.len() > 8 { String::from("a") } else { old + "b" });
                    }
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        // Updates may be lost between a read and a write, as Java's may, but none tears.
        assert!((1..=4000).contains(&n.get()));
        assert!(s.get().starts_with('a') && s.get()[1..].bytes().all(|b| b == b'b'));
        let c = std::sync::Arc::new(SharedField::new(0i32));
        let threads: Vec<_> = (0..4)
            .map(|_| {
                let c = c.clone();
                std::thread::spawn(move || {
                    for _ in 0..1000 {
                        c.get_and_update(|a| a + 1);
                        let mut old = c.get();
                        while !c.compare_and_set_by(old, old + 1, |a, b| a == b) {
                            old = c.get();
                        }
                    }
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        // No update is lost.
        assert_eq!(c.get(), 8000);
        assert_eq!(c.compare_exchange_by(1, 2, |a, b| a == b), 8000);
        let v = SharedField::new(7u8);
        v.set_volatile(v.get_volatile() + 1);
        assert_eq!(v.get(), 8);
    }
}
