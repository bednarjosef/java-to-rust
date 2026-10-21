//! The memory allocator of translated programs: the system's, with a cache of freed small blocks
//! per thread. Java programs make and drop many small objects, which a JVM allocates by bumping
//! a pointer and frees in bulk; every Rust object is a `malloc` and a `free` instead. Freed blocks
//! of up to 136 bytes are kept per size, and the next allocation of that size on the same thread
//! takes one back, which is a few instructions.
//!
//! Every block is a block of the system allocator, of the largest size that fits the same glibc
//! chunk as the request (24, 40, 56, ... bytes: a chunk is a multiple of 16 bytes and 8 of them
//! are its header), so a block takes no more memory than `malloc` would give, and any thread may
//! free or keep any block. A thread keeps at most `THREAD_BYTES` in all sizes together, and all
//! threads together at most `PROCESS_BYTES`, counting the arrays that list the blocks; past
//! either budget a freed block goes back to the system allocator. The main thread's cache lasts
//! as long as the process; a thread started by `Thread.start` gives its cache, and its share of
//! the process's budget, back when its body ends (`release_thread_cache`). Other threads, which
//! the runtime does not start, keep what they cached until the process ends.
//!
//! The freed blocks of a size are kept in an array, not in a list threaded through the blocks,
//! so that keeping or taking one does not touch its memory. When the thread goes from freeing
//! many blocks to allocating, the array is sorted by address (see `Stack::take`).
//!
//! A generated program installs it at its crate root:
//! `#[global_allocator] static JAVA_RUNTIME_ALLOCATOR: java_runtime::Allocator = java_runtime::Allocator;`.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The sizes that are cached: `SMALLEST`, `SMALLEST + STEP`, ..., 136 bytes.
const CLASSES: usize = 8;
const STEP: usize = 16;
const SMALLEST: usize = 24;

/// How many bytes one thread keeps, in freed blocks of all sizes and the arrays that list them.
const THREAD_BYTES: usize = 64 << 20;

/// How many bytes all threads together keep.
const PROCESS_BYTES: usize = 256 << 20;

/// A thread takes its share of `PROCESS_BYTES` in grants of this size, so that keeping a block
/// rarely touches the shared count.
const GRANT: usize = 1 << 20;

/// The bytes of `PROCESS_BYTES` that threads hold as grants.
static GRANTED: AtomicUsize = AtomicUsize::new(0);

/// The fewest blocks freed since the last sort that make the next allocation sort the array:
/// a few megabytes, more than a processor's second-level cache, below which the order of the
/// blocks hardly matters. With 4096, binary trees ran 3% slower and NodeList no faster.
const SORT_FROM: usize = 65536;

/// The freed blocks of one size, in an array allocated from the system.
struct Stack {
    blocks: Cell<*mut *mut u8>,
    len: Cell<usize>,
    capacity: Cell<usize>,
    /// How many blocks were added since the array was last sorted.
    unsorted: Cell<usize>,
}

/// The bytes a thread keeps, against its budget and its grants from the process's.
struct Budget {
    /// The bytes of the kept blocks and of the arrays' capacity.
    kept: Cell<usize>,
    /// The bytes of `PROCESS_BYTES` this thread holds; `kept` stays at most this.
    granted: Cell<usize>,
}

impl Budget {
    /// Counts `n` more bytes kept; false, counting nothing, when a budget does not allow them.
    #[inline]
    fn charge(&self, n: usize) -> bool {
        let kept = self.kept.get() + n;
        if kept > self.granted.get() && !self.grant(kept) {
            return false;
        }
        self.kept.set(kept);
        true
    }

    /// Takes grants from the process's budget until they cover `kept` bytes, if the budgets allow.
    #[cold]
    #[inline(never)]
    fn grant(&self, kept: usize) -> bool {
        if kept > THREAD_BYTES {
            return false;
        }
        let more = (kept - self.granted.get()).div_ceil(GRANT) * GRANT;
        let taken = GRANTED.try_update(Ordering::Relaxed, Ordering::Relaxed, |granted| {
            (granted + more <= PROCESS_BYTES).then_some(granted + more)
        });
        if taken.is_err() {
            return false;
        }
        self.granted.set(self.granted.get() + more);
        true
    }

    /// Counts `n` bytes no longer kept, and gives back grants the thread no longer needs.
    #[inline]
    fn uncharge(&self, n: usize) {
        let kept = self.kept.get() - n;
        self.kept.set(kept);
        if self.granted.get() - kept >= 2 * GRANT {
            self.give_back_grants(GRANT);
        }
    }

    /// Gives back the grants that exceed the kept bytes by more than `spare`.
    #[cold]
    #[inline(never)]
    fn give_back_grants(&self, spare: usize) {
        let unused = (self.granted.get() - self.kept.get()).saturating_sub(spare) / GRANT * GRANT;
        GRANTED.fetch_sub(unused, Ordering::Relaxed);
        self.granted.set(self.granted.get() - unused);
    }
}

/// A thread's freed blocks, one array per size.
struct Cache {
    stacks: [Stack; CLASSES],
    budget: Budget,
    /// False once the thread has given its cache back for good.
    open: Cell<bool>,
}

thread_local! {
    static CACHE: Cache = const {
        Cache {
            stacks: [const {
                Stack {
                    blocks: Cell::new(ptr::null_mut()),
                    len: Cell::new(0),
                    capacity: Cell::new(0),
                    unsorted: Cell::new(0),
                }
            }; CLASSES],
            budget: Budget { kept: Cell::new(0), granted: Cell::new(0) },
            open: Cell::new(true),
        }
    };
}

/// The allocator: see the module's documentation.
pub struct Allocator;

/// The cached size class of a request, if it has one.
#[inline]
fn class_of(layout: &Layout) -> Option<usize> {
    let size = layout.size();
    (size <= SMALLEST + (CLASSES - 1) * STEP && layout.align() <= STEP)
        .then(|| size.saturating_sub(SMALLEST).div_ceil(STEP))
}

/// The layout of the system blocks of class `c`.
#[inline]
fn block(c: usize) -> Layout {
    // SAFETY: the alignment is a power of two and the size is small.
    unsafe { Layout::from_size_align_unchecked(SMALLEST + c * STEP, STEP) }
}

/// The layout of an array of `n` block addresses.
fn array(n: usize) -> Layout {
    Layout::array::<*mut u8>(n).expect("a cache array fits in memory")
}

impl Stack {
    /// A kept block, or null.
    #[inline]
    fn take(&self) -> *mut u8 {
        let len = self.len.get();
        if len == 0 {
            return ptr::null_mut();
        }
        let unsorted = self.unsorted.get();
        if unsorted >= SORT_FROM && unsorted * 2 >= len {
            self.sort();
        }
        self.len.set(len - 1);
        // SAFETY: the first `len` entries of the array are kept blocks.
        unsafe { *self.blocks.get().add(len - 1) }
    }

    /// Sorts the kept blocks so that they are taken in order of address. Many blocks freed at
    /// once are often the nodes of a structure the program dropped, freed in the order of its
    /// links; taken back in that order, they would place the next structure's nodes all over
    /// the memory they cover, and walking it would miss the processor's cache at every node
    /// (`benchmarks/NodeList.java` ran ten times slower). Taken in order of address, they are
    /// laid out as fresh memory would be. Sorting only when at least half the array is new
    /// keeps its cost a few comparisons per freed block (about 3% of binary trees' time).
    #[cold]
    #[inline(never)]
    fn sort(&self) {
        self.unsorted.set(0);
        // SAFETY: the first `len` entries are initialized, and nothing else uses the array.
        let blocks = unsafe { std::slice::from_raw_parts_mut(self.blocks.get(), self.len.get()) };
        // The last entry is taken first: lowest address last. `sort_unstable` allocates nothing.
        blocks.sort_unstable_by(|a, b| b.cmp(a));
    }

    /// Keeps a freed block of class `c`, already charged; false when the array is full.
    #[inline]
    fn keep(&self, c: usize, block: *mut u8, budget: &Budget) -> bool {
        let len = self.len.get();
        if len == self.capacity.get() && !self.grow(c, budget) {
            return false;
        }
        // SAFETY: `len` is below the capacity.
        unsafe { *self.blocks.get().add(len) = block };
        self.len.set(len + 1);
        self.unsorted.set(self.unsorted.get() + 1);
        true
    }

    /// Doubles the array, up to `THREAD_BYTES` of blocks, charging its growth to the budget;
    /// false when it cannot grow.
    #[cold]
    #[inline(never)]
    fn grow(&self, c: usize, budget: &Budget) -> bool {
        let most = THREAD_BYTES / block(c).size();
        let capacity = self.capacity.get();
        if capacity >= most {
            return false;
        }
        let wanted = (capacity * 2).clamp(1024, most);
        let growth = array(wanted).size() - array(capacity).size();
        if !budget.charge(growth) {
            return false;
        }
        let old = self.blocks.get();
        // SAFETY: the array, if any, came from `System` with the layout `array(capacity)`.
        let new = unsafe {
            if old.is_null() {
                System.alloc(array(wanted))
            } else {
                System.realloc(old as *mut u8, array(capacity), array(wanted).size())
            }
        };
        if new.is_null() {
            budget.uncharge(growth);
            return false;
        }
        self.blocks.set(new as *mut *mut u8);
        self.capacity.set(wanted);
        true
    }

    /// Gives every kept block, and the array, back to the system.
    fn give_back(&self, c: usize) {
        let blocks = self.blocks.replace(ptr::null_mut());
        if blocks.is_null() {
            return;
        }
        for i in 0..self.len.get() {
            // SAFETY: as in `take`; each block came from `System` with the layout `block(c)`.
            unsafe { System.dealloc(*blocks.add(i), block(c)) };
        }
        // SAFETY: the array came from `System` with this layout.
        unsafe { System.dealloc(blocks as *mut u8, array(self.capacity.get())) };
        self.len.set(0);
        self.capacity.set(0);
        self.unsorted.set(0);
    }
}

impl Cache {
    #[inline]
    fn take(&self, c: usize) -> *mut u8 {
        let block = self.stacks[c].take();
        if !block.is_null() {
            self.budget.uncharge(self::block(c).size());
        }
        block
    }

    #[inline]
    fn keep(&self, c: usize, block: *mut u8) -> bool {
        let size = self::block(c).size();
        if !self.open.get() || !self.budget.charge(size) {
            return false;
        }
        if self.stacks[c].keep(c, block, &self.budget) {
            return true;
        }
        self.budget.uncharge(size);
        false
    }

    /// Gives every cached block, and the thread's share of the process's budget, back, and
    /// caches nothing more.
    fn close(&self) {
        self.open.set(false);
        for (c, stack) in self.stacks.iter().enumerate() {
            stack.give_back(c);
        }
        self.budget.kept.set(0);
        self.budget.give_back_grants(0);
    }
}

// SAFETY: every block handed out is a live system block of at least the requested size and
// alignment, and a block is either in one thread's cache or in use, never both.
unsafe impl GlobalAlloc for Allocator {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let Some(c) = class_of(&layout) else { return unsafe { System.alloc(layout) } };
        let cached = CACHE.try_with(|cache| cache.take(c)).unwrap_or(ptr::null_mut());
        if cached.is_null() { unsafe { System.alloc(block(c)) } } else { cached }
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let Some(c) = class_of(&layout) else { return unsafe { System.dealloc(ptr, layout) } };
        if !CACHE.try_with(|cache| cache.keep(c, ptr)).unwrap_or(false) {
            unsafe { System.dealloc(ptr, block(c)) }
        }
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if class_of(&layout).is_none() {
            return unsafe { System.alloc_zeroed(layout) };
        }
        let block = unsafe { self.alloc(layout) };
        if !block.is_null() {
            unsafe { ptr::write_bytes(block, 0, layout.size()) };
        }
        block
    }

    #[inline]
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: the caller guarantees a valid new size for the alignment.
        let new = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        match (class_of(&layout), class_of(&new)) {
            (None, None) => unsafe { System.realloc(ptr, layout, new_size) },
            (Some(a), Some(b)) if a == b => ptr,
            _ => {
                let moved = unsafe { self.alloc(new) };
                if !moved.is_null() {
                    unsafe { ptr::copy_nonoverlapping(ptr, moved, layout.size().min(new_size)) };
                    unsafe { self.dealloc(ptr, layout) };
                }
                moved
            }
        }
    }
}

/// Gives the blocks that this thread cached back to the system: called when the body of a
/// thread that `Thread.start` started ends.
pub fn release_thread_cache() {
    let _ = CACHE.try_with(Cache::close);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_are_reused_and_released() {
        let a = Allocator;
        let layout = Layout::from_size_align(24, 8).unwrap();
        std::thread::spawn(move || unsafe {
            let first = a.alloc(layout);
            first.write_bytes(7, 24);
            a.dealloc(first, layout);
            let second = a.alloc(layout);
            assert_eq!(first, second, "the freed block is taken back");
            let grown = a.realloc(second, layout, 20);
            assert_eq!(grown, second, "a block of the same size class stays");
            let moved = a.realloc(grown, Layout::from_size_align(20, 8).unwrap(), 300);
            assert_eq!(*moved.add(19), 7);
            a.dealloc(moved, Layout::from_size_align(300, 8).unwrap());
            release_thread_cache();
            let zeroed = a.alloc_zeroed(layout);
            assert!((0..24).all(|i| *zeroed.add(i) == 0));
            a.dealloc(zeroed, layout);
        })
        .join()
        .unwrap();
    }

    #[test]
    fn a_thread_keeps_no_more_than_its_budget_and_gives_its_grants_back() {
        let a = Allocator;
        let layout = Layout::from_size_align(136, 8).unwrap();
        std::thread::spawn(move || unsafe {
            let blocks: Vec<*mut u8> = (0..THREAD_BYTES / 136 + 50_000).map(|_| a.alloc(layout)).collect();
            for &b in &blocks {
                a.dealloc(b, layout);
            }
            CACHE.with(|cache| {
                let budget = &cache.budget;
                assert!(budget.kept.get() <= budget.granted.get());
                assert!(budget.granted.get() <= THREAD_BYTES);
                assert!(cache.stacks[class_of(&layout).unwrap()].len.get() < blocks.len());
            });
            let again: Vec<*mut u8> = blocks.iter().map(|_| a.alloc(layout)).collect();
            for b in again {
                a.dealloc(b, layout);
            }
            release_thread_cache();
            CACHE.with(|cache| assert_eq!((cache.budget.kept.get(), cache.budget.granted.get()), (0, 0)));
        })
        .join()
        .unwrap();
    }

    #[test]
    fn many_blocks_come_back_in_order_of_address() {
        let a = Allocator;
        let layout = Layout::from_size_align(40, 8).unwrap();
        std::thread::spawn(move || unsafe {
            let mut blocks: Vec<*mut u8> = (0..2 * SORT_FROM).map(|_| a.alloc(layout)).collect();
            // Freed in a scrambled order.
            let n = blocks.len();
            for i in 0..n {
                blocks.swap(i, (i * 7919) % n);
            }
            for &b in &blocks {
                a.dealloc(b, layout);
            }
            let again: Vec<*mut u8> = (0..n).map(|_| a.alloc(layout)).collect();
            assert!(again.windows(2).all(|w| w[0] < w[1]), "taken in order of address");
            for b in again {
                a.dealloc(b, layout);
            }
            release_thread_cache();
        })
        .join()
        .unwrap();
    }
}
