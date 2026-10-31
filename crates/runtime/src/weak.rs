//! The referent of a `java.lang.ref.Reference` (docs/collector.md, "References"): a cell that
//! the collector does not trace, so that a soft, weak or phantom reference does not keep its
//! referent alive, and that the collector clears once nothing else does.
//!
//! The cell holds the referent's bits inverted, a word that is no address in the heap, so that
//! marking the reference's object does not reach the referent (`!0` stands for no bits at all,
//! 0). A thread stopped between loading that word and inverting it back holds the inverted word
//! in a register or on its stack, which the collector also reads inverted, so the referent stays.
//! The cell's second word is the collector's clock when the referent was last read
//! (`gc::references`), which decides when a soft reference's referent is cleared.

use std::marker::PhantomData;
use std::mem::{size_of, transmute_copy};
use std::sync::atomic::{AtomicU64, Ordering};

/// The cell of `Reference.referent`, of a value of one word: a handle of an object, or null.
#[repr(C)]
pub struct WeakField<T> {
    word: AtomicU64,
    read: AtomicU64,
    value: PhantomData<T>,
}

// Every access is one atomic action on the word, as `java::SharedField`'s.
unsafe impl<T: Send> Sync for WeakField<T> {}

/// The word a value is held as.
#[inline(always)]
fn encode<T>(value: &T) -> u64 {
    const { assert!(size_of::<T>() == 8, "a referent is one word") };
    let bits = unsafe { transmute_copy::<T, u64>(value) };
    if bits == 0 { 0 } else { !bits }
}

/// The value held as `word`.
#[inline(always)]
fn decode<T>(word: u64) -> T {
    let bits = if word == 0 { 0 } else { !word };
    unsafe { transmute_copy::<u64, T>(&bits) }
}

impl<T: Clone> WeakField<T> {
    #[inline(always)]
    pub fn new(value: T) -> WeakField<T> {
        let word = encode(&value);
        std::mem::forget(value);
        WeakField { word: AtomicU64::new(word), read: AtomicU64::new(0), value: PhantomData }
    }

    /// A copy of the value, noting when it was read.
    #[inline(always)]
    pub fn get(&self) -> T {
        let clock = crate::gc::references::CLOCK.load(Ordering::Relaxed);
        if self.read.load(Ordering::Relaxed) != clock {
            self.read.store(clock, Ordering::Relaxed);
        }
        decode(self.word.load(Ordering::Acquire))
    }

    #[inline(always)]
    pub fn get_volatile(&self) -> T {
        self.get()
    }

    /// Replaces the value.
    #[inline(always)]
    pub fn set(&self, value: T) {
        self.word.store(encode(&value), Ordering::Release);
        std::mem::forget(value);
    }

    #[inline(always)]
    pub fn set_volatile(&self, value: T) {
        self.set(value)
    }

    /// Replaces the value, returning the old one.
    pub fn replace(&self, value: T) -> T {
        let old = self.word.swap(encode(&value), Ordering::AcqRel);
        std::mem::forget(value);
        decode(old)
    }

    /// `compareAndExchange`: the old value, and the cell set to `value` if the old value is `same`
    /// as `expected`, in one atomic action.
    pub fn compare_exchange_by(&self, expected: T, value: T, same: impl Fn(&T, &T) -> bool) -> T {
        let new = encode(&value);
        std::mem::forget(value);
        loop {
            let word = self.word.load(Ordering::Acquire);
            let old: T = decode(word);
            if !same(&old, &expected)
                || self.word.compare_exchange(word, new, Ordering::SeqCst, Ordering::Acquire).is_ok()
            {
                return old;
            }
        }
    }

    /// `compareAndSet`: whether the old value was `same` as `expected`, and the cell then set to
    /// `value`, in one atomic action.
    pub fn compare_and_set_by(&self, expected: T, value: T, same: impl Fn(&T, &T) -> bool) -> bool {
        let old = self.compare_exchange_by(expected.clone(), value, &same);
        same(&old, &expected)
    }

    pub fn into_inner(self) -> T {
        decode(self.word.load(Ordering::Acquire))
    }

    /// The address of the word, and the word that holds no value (`Default`'s), for the
    /// collector's registry.
    pub(crate) fn parts(&self) -> (usize, u64)
    where
        T: Default,
    {
        (&self.word as *const AtomicU64 as usize, encode(&T::default()))
    }
}

/// A copy of the reference copies its referent (`Object.clone()`, which `Reference` refuses).
impl<T: Clone> Clone for WeakField<T> {
    fn clone(&self) -> WeakField<T> {
        WeakField::new(self.get())
    }
}

impl<T: Clone + std::fmt::Debug> std::fmt::Debug for WeakField<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.get().fmt(f)
    }
}

/// Registers the reference `reference` (a handle of an object of a subclass of `Reference`),
/// whose referent is in `cell`, with the collector, which clears the referent once nothing else
/// keeps it (`kind`: 1 soft, 2 weak, 3 phantom; any other is not registered, and its referent
/// stays as long as the reference), and hands the reference over once it did if it has a queue
/// (`queued`, `take_pending_reference`). A reference outside the heap is not registered.
pub fn register_reference<R, T: Clone + Default>(
    reference: &R,
    cell: &WeakField<T>,
    kind: i32,
    queued: bool,
) {
    const { assert!(size_of::<R>() == 8, "a reference is one word") };
    let word = unsafe { transmute_copy::<R, u64>(reference) };
    let (address, null) = cell.parts();
    cell.read.store(crate::gc::references::CLOCK.load(Ordering::Relaxed), Ordering::Relaxed);
    crate::gc::references::register(word, address, null, kind, queued);
}

/// A reference with a queue whose referent the collector cleared, which the program has not taken
/// yet (`TranslatedReferences.next()`): `R` is the type of the handles that
/// `register_reference` was given.
pub fn take_pending_reference<R>() -> Option<R> {
    const { assert!(size_of::<R>() == 8, "a reference is one word") };
    let word = crate::gc::references::take_pending()?;
    // SAFETY: the word is a handle of type `R` that `register_reference` was given, and the
    // collector kept its object while it was pending.
    Some(unsafe { transmute_copy::<u64, R>(&word) })
}
