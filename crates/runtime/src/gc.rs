//! The collected heap: the memory allocator of translated programs, which also frees what the
//! program can no longer reach.
//!
//! Java objects are not counted: copying a handle copies a pointer, and dropping one does nothing
//! (see `shared.rs`). An object nothing reaches any more is found and freed here, as a JVM's
//! collector does. Everything else Rust allocates (a `Vec`'s buffer, a `String`) comes from the
//! same heap and is still freed by Rust as usual, the moment it is dropped. Memory that an
//! unreachable object owned, and that Rust therefore never freed, is found unreachable too.
//!
//! The heap is one range of address space, reserved at start and used as needed, cut into
//! blocks of 64 KB. A small block holds slots of one size: one of `CLASSES` sizes from 16 bytes
//! to 32 KB, for allocations that may hold pointers or, in blocks of their own, for those that
//! cannot (an alignment below 8 rules out a pointer, as in the bytes of a string). A larger
//! allocation takes whole blocks. Each block has a bit per slot saying that the slot is taken
//! (an object, or a free slot that a thread holds) and a bit per slot for marking.
//!
//! **Allocation** takes a slot from the calling thread's list of free slots of its size, a few
//! instructions. A freed slot goes on the list of the thread that frees it. When a list is empty,
//! the thread takes all the untaken slots of a block of that size under the heap's lock. A slot
//! that may hold pointers is zeroed as it is handed out, so that no old word in it keeps an object
//! alive; a pointer-free one only for `alloc_zeroed`.
//!
//! **Collection** runs when the program has taken half as many bytes as were alive after the last
//! one (at least `MIN_COLLECT`; `JAVA_GC_GROWTH` sets the share). It stops every other thread
//! (with `SIGPWR`; each records where its stack is and waits), marks every slot reachable from the
//! roots, and frees the taken slots left unmarked. With a large heap, helper threads
//! mark and sweep alongside (see "Marking"). The roots are scanned conservatively: any aligned word that points into a
//! taken slot keeps it, which makes interior pointers and pointers in registers work. They are
//! - the stacks of the threads (from the stack pointer recorded when it stopped, which is below
//!   the registers that the signal saved), and of the collecting thread, with its registers;
//! - the program's static data, the writable segments of every loaded object;
//! - each thread's static thread-local storage, where the slots its free lists start from are;
//! - the free lists of every thread, whose slots are marked without being scanned.
//!
//! Slots are scanned whole, word by word, except in blocks that cannot hold pointers.
//!
//! A thread is registered (its stack and thread pointer found) as the first thing it does, by
//! this crate's `pthread_create`, which takes the C library's place, and unregistered when it
//! ends. No collection runs between the call and the registration, while the closure the thread
//! runs is only known to the C library.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::{Cell, UnsafeCell};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, AtomicU64, AtomicUsize, Ordering, fence};

/// The heap's blocks are `1 << BLOCK_SHIFT` bytes, aligned to their size.
const BLOCK_SHIFT: usize = 16;
const BLOCK: usize = 1 << BLOCK_SHIFT;

/// The address space reserved for the heap.
const HEAP_BYTES: usize = 64 << 30;
const MAX_BLOCKS: usize = HEAP_BYTES >> BLOCK_SHIFT;

/// The most slots a block has (of 16 bytes), and the words of a bitmap over them.
const MAX_SLOTS: usize = BLOCK / 16;
const BITMAP_WORDS: usize = MAX_SLOTS / 64;

/// The sizes of the slots of small blocks. Larger allocations take whole blocks.
const SIZES: [u32; 40] = [
    16, 32, 48, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384, 448, 512, 640, 768, 896, 1024, 1280,
    1536, 1792, 2048, 2560, 3072, 3584, 4096, 5120, 6144, 7168, 8192, 10240, 12288, 14336, 16384, 20480,
    24576, 28672, 32768,
];
const SIZE_COUNT: usize = SIZES.len();
const LARGEST_SMALL: usize = 32768;

/// A class is a size and whether its slots are scanned: `size_index * 2 + no_scan`.
const CLASSES: usize = SIZE_COUNT * 2;

/// The fewest bytes taken between collections.
const MIN_COLLECT: usize = 64 << 20;

/// How many slots of one class a thread's list keeps before freed ones go back to their blocks.
const LIST_MAX_BYTES: usize = 1 << 20;

/// The class of a size up to 1024 bytes, by `(size + 15) / 16`.
static SMALL_CLASS: [u8; 65] = small_classes();

const fn small_classes() -> [u8; 65] {
    let mut table = [0u8; 65];
    let mut i = 0;
    while i <= 64 {
        let size = i * 16;
        let mut c = 0;
        while (SIZES[c] as usize) < size {
            c += 1;
        }
        table[i] = c as u8;
        i += 1;
    }
    table
}

/// The size index of a slot for `size` bytes aligned to `align`, or `None` for a large
/// allocation. A slot is aligned to 16 bytes, and to its size when that is a power of two, so an
/// alignment above 16 takes the smallest power of two that holds the size.
#[inline(always)]
fn size_index(size: usize, align: usize) -> Option<usize> {
    if align > 16 {
        let wanted = size.max(align).next_power_of_two();
        if wanted > LARGEST_SMALL {
            return None;
        }
        return SIZES.iter().position(|&s| s as usize == wanted);
    }
    if size <= 1024 {
        return Some(SMALL_CLASS[size.div_ceil(16)] as usize);
    }
    if size > LARGEST_SMALL {
        return None;
    }
    SIZES.iter().position(|&s| s as usize >= size)
}

/// What a block holds.
const FREE: u8 = 0;
const SMALL: u8 = 1;
/// The first block of a large allocation, and the ones after it.
const LARGE: u8 = 2;
const LARGE_REST: u8 = 3;

/// A block's description, in a table beside the heap.
#[repr(C)]
struct Meta {
    kind: u8,
    /// The class of a small block.
    class: u8,
    /// Whether the slots can hold no pointers, so marking does not scan them.
    no_scan: bool,
    /// Whether the block is on its class's list of blocks with untaken slots.
    listed: bool,
    /// The slot size of a small block; the number of blocks of a large allocation (on its first
    /// block), or the index of that first block (on the others).
    size: u32,
    slots: u32,
    /// The next block on its class's list.
    next: u32,
    /// `slot = offset * magic >> 32`, a division by the slot size.
    magic: u64,
    taken: [u64; BITMAP_WORDS],
    marks: [u64; BITMAP_WORDS],
    /// What the sweep found: the slots alive in a small block (1 for a large allocation that
    /// lives), and whether a small block has untaken slots.
    live: u32,
    untaken: bool,
}

/// No block.
const NONE: u32 = u32::MAX;

/// The heap's shared state, behind `LOCK`.
struct Heap {
    /// The first block's address, and the end of the blocks used so far.
    base: usize,
    meta: *mut Meta,
    /// Blocks `0..top` have been used.
    top: usize,
    /// Per class, the blocks with untaken slots.
    lists: [u32; CLASSES],
    /// Free blocks below `top`, as runs (start, length), in an array of `MAX_BLOCKS` entries.
    runs: *mut (u32, u32),
    run_count: usize,
    /// Bytes handed to threads or large allocations since the last collection, and the amount
    /// that starts the next one.
    taken: usize,
    next_collection: usize,
    /// Bytes alive after the last collection.
    alive: usize,
    collections: usize,
    /// The bytes taken between collections at least: `MIN_COLLECT`, or what the variable
    /// `JAVA_GC_EVERY` sets, for testing the collector.
    every: usize,
    /// Whether each collection is reported on standard error (`JAVA_GC_LOG`).
    log: bool,
    /// The bytes taken between collections, in percent of the bytes alive after the last
    /// (`JAVA_GC_GROWTH`).
    growth: usize,
    /// Whether `JAVA_GC_THREADS` sets the markers, which then mark every collection, however
    /// small (to test marking with helpers).
    threads_set: bool,
}

struct Shared(UnsafeCell<Heap>);
unsafe impl Sync for Shared {}

static HEAP: Shared = Shared(UnsafeCell::new(Heap {
    base: 0,
    meta: ptr::null_mut(),
    top: 0,
    lists: [NONE; CLASSES],
    runs: ptr::null_mut(),
    run_count: 0,
    taken: 0,
    next_collection: MIN_COLLECT,
    alive: 0,
    collections: 0,
    every: MIN_COLLECT,
    log: false,
    growth: 50,
    threads_set: false,
}));

/// The heap's lock: a spin lock that yields, since it is taken rarely and briefly, and must not
/// allocate.
static LOCK: AtomicBool = AtomicBool::new(false);

/// The first address of the heap, and its end so far (`base + top * BLOCK`), for the quick test
/// of a candidate pointer. Written under the lock, read while the world is stopped.
static BASE: AtomicUsize = AtomicUsize::new(0);
static END: AtomicUsize = AtomicUsize::new(0);

struct Guard;

impl Drop for Guard {
    fn drop(&mut self) {
        LOCK.store(false, Ordering::Release);
    }
}

fn lock() -> Guard {
    let mut spins = 0u32;
    while LOCK.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
        spins += 1;
        if spins < 64 {
            std::hint::spin_loop();
        } else {
            unsafe { sched_yield() };
        }
    }
    Guard
}

/// The heap, while the lock is held.
#[allow(clippy::mut_from_ref)]
fn heap(_: &Guard) -> &mut Heap {
    unsafe { &mut *HEAP.0.get() }
}

#[repr(C)]
struct DlPhdrInfo {
    addr: usize,
    name: *const u8,
    phdr: *const ProgramHeader,
    phnum: u16,
}

#[repr(C)]
struct ProgramHeader {
    kind: u32,
    flags: u32,
    offset: u64,
    vaddr: u64,
    paddr: u64,
    filesz: u64,
    memsz: u64,
    align: u64,
}

#[repr(C)]
struct SigAction {
    handler: usize,
    mask: [u64; 16],
    flags: i32,
    restorer: usize,
}

unsafe extern "C" {
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, off: i64) -> *mut u8;
    fn madvise(addr: *mut u8, len: usize, advice: i32) -> i32;
    fn sched_yield() -> i32;
    fn dl_iterate_phdr(
        callback: extern "C" fn(*const DlPhdrInfo, usize, *mut u8) -> i32,
        data: *mut u8,
    ) -> i32;
    fn pthread_self() -> usize;
    fn pthread_getattr_np(thread: usize, attr: *mut [u64; 8]) -> i32;
    fn pthread_attr_getstack(attr: *const [u64; 8], addr: *mut usize, size: *mut usize) -> i32;
    fn pthread_attr_destroy(attr: *mut [u64; 8]) -> i32;
    fn pthread_kill(thread: usize, sig: i32) -> i32;
    fn sigaction(sig: i32, act: *const SigAction, old: *mut SigAction) -> i32;
    fn sigemptyset(set: *mut [u64; 16]) -> i32;
    fn syscall(n: i64, ...) -> i64;
    fn __errno_location() -> *mut i32;
    fn write(fd: i32, buf: *const std::ffi::c_void, len: usize) -> isize;
    fn _exit(code: i32) -> !;
    fn getenv(name: *const std::ffi::c_char) -> *const std::ffi::c_char;
    fn dlsym(handle: *mut u8, name: *const std::ffi::c_char) -> *mut u8;
    fn sched_getaffinity(pid: i32, size: usize, mask: *mut u64) -> i32;
}

const PROT_READ_WRITE: i32 = 3;
const MAP_PRIVATE_ANONYMOUS_NORESERVE: i32 = 0x02 | 0x20 | 0x4000;
const MADV_DONTNEED: i32 = 4;
const SIGPWR: i32 = 30;
const SA_SIGINFO_RESTART: i32 = 4 | 0x1000_0000;
const SYS_FUTEX: i64 = 202;
const FUTEX_WAIT_PRIVATE: i32 = 128;
const FUTEX_WAKE_PRIVATE: i32 = 129;
const PT_LOAD: u32 = 1;
const PT_TLS: u32 = 7;
const PF_W: u32 = 2;

fn futex_wait(word: &AtomicU32, value: u32) {
    unsafe { syscall(SYS_FUTEX, word as *const AtomicU32, FUTEX_WAIT_PRIVATE, value, ptr::null::<u8>()) };
}

fn futex_wake(word: &AtomicU32) {
    unsafe { syscall(SYS_FUTEX, word as *const AtomicU32, FUTEX_WAKE_PRIVATE, i32::MAX) };
}

/// Maps `len` bytes of zeroed memory that is only committed when touched.
fn reserve(len: usize) -> *mut u8 {
    let p = unsafe { mmap(ptr::null_mut(), len, PROT_READ_WRITE, MAP_PRIVATE_ANONYMOUS_NORESERVE, -1, 0) };
    if p as isize == -1 {
        std::process::abort();
    }
    p
}

/// The number an environment variable holds, if it is set to one.
fn number_from(name: &std::ffi::CStr) -> Option<usize> {
    let text = unsafe { getenv(name.as_ptr()) };
    if text.is_null() {
        return None;
    }
    unsafe { std::ffi::CStr::from_ptr(text) }.to_str().ok()?.parse().ok()
}

/// Sets the heap up, on the first allocation.
#[cold]
fn start(h: &mut Heap) {
    let raw = reserve(HEAP_BYTES + BLOCK) as usize;
    h.base = (raw + BLOCK - 1) & !(BLOCK - 1);
    h.meta = reserve(MAX_BLOCKS * std::mem::size_of::<Meta>()) as *mut Meta;
    h.runs = reserve(MAX_BLOCKS * std::mem::size_of::<(u32, u32)>()) as *mut (u32, u32);
    BASE.store(h.base, Ordering::Release);
    END.store(h.base, Ordering::Release);
    install_signal();
    h.log = !unsafe { getenv(c"JAVA_GC_LOG".as_ptr()) }.is_null();
    h.threads_set = number_from(c"JAVA_GC_THREADS").is_some_and(|n| n > 1);
    if let Some(n) = number_from(c"JAVA_GC_GROWTH") {
        h.growth = n.max(1);
    }
    if let Some(n) = number_from(c"JAVA_GC_EVERY") {
        h.every = n.max(1);
        h.next_collection = h.every;
    }
}

impl Heap {
    /// The description of block `b`, which lives as long as the heap, apart from `self`.
    #[allow(clippy::mut_from_ref)]
    fn meta<'m>(&self, b: usize) -> &'m mut Meta {
        unsafe { &mut *self.meta.add(b) }
    }

    fn block_address(&self, b: usize) -> usize {
        self.base + (b << BLOCK_SHIFT)
    }

    /// `n` free blocks in a row: from a run of free ones, or new ones at the top.
    fn take_blocks(&mut self, n: usize) -> usize {
        for i in 0..self.run_count {
            let (start, len) = unsafe { *self.runs.add(i) };
            if len as usize >= n {
                let rest = (start as usize + n, len as usize - n);
                unsafe {
                    if rest.1 == 0 {
                        self.run_count -= 1;
                        *self.runs.add(i) = *self.runs.add(self.run_count);
                    } else {
                        *self.runs.add(i) = (rest.0 as u32, rest.1 as u32);
                    }
                }
                return start as usize;
            }
        }
        if self.top + n > MAX_BLOCKS {
            out_of_memory();
        }
        let start = self.top;
        self.top += n;
        END.store(self.block_address(self.top), Ordering::Release);
        start
    }

    /// Gives `n` blocks from `start` back: their memory goes back to the system, zeroed.
    fn free_blocks(&mut self, start: usize, n: usize) {
        for b in start..start + n {
            let m = self.meta(b);
            m.kind = FREE;
            m.listed = false;
        }
        unsafe { madvise(self.block_address(start) as *mut u8, n << BLOCK_SHIFT, MADV_DONTNEED) };
        unsafe { *self.runs.add(self.run_count) = (start as u32, n as u32) };
        self.run_count += 1;
    }

    /// A new small block of class `class`, all its slots untaken.
    fn new_block(&mut self, class: usize) -> usize {
        let b = self.take_blocks(1);
        let size = SIZES[class / 2];
        let m = self.meta(b);
        m.kind = SMALL;
        m.class = class as u8;
        m.no_scan = class % 2 == 1;
        m.listed = false;
        m.size = size;
        m.slots = (BLOCK as u32) / size;
        m.next = NONE;
        m.magic = (1u64 << 32) / size as u64 + 1;
        m.taken = [0; BITMAP_WORDS];
        m.marks = [0; BITMAP_WORDS];
        b
    }

    /// Takes every untaken slot of a block of `class` and links them into a list, which it
    /// returns with the bytes it holds.
    fn refill(&mut self, class: usize) -> (*mut u8, usize) {
        let b = loop {
            let b = self.lists[class];
            if b == NONE {
                break self.new_block(class);
            }
            let m = self.meta(b as usize);
            self.lists[class] = m.next;
            m.listed = false;
            if m.kind == SMALL && m.class as usize == class {
                break b as usize;
            }
        };
        let start = self.block_address(b);
        let m = self.meta(b);
        let size = m.size as usize;
        let mut head: *mut u8 = ptr::null_mut();
        let mut count = 0;
        // From the last slot down, so that the list hands out slots in order of address.
        for slot in (0..m.slots as usize).rev() {
            let (w, bit) = (slot / 64, 1u64 << (slot % 64));
            if m.taken[w] & bit == 0 {
                m.taken[w] |= bit;
                let p = (start + slot * size) as *mut u8;
                unsafe { (p as *mut *mut u8).write(head) };
                head = p;
                count += 1;
            }
        }
        self.taken += count * size;
        (head, count * size)
    }

    /// Puts a block with untaken slots on its class's list, unless it is there.
    fn list(&mut self, b: usize) {
        let m = self.meta(b);
        if !m.listed {
            m.listed = true;
            let class = m.class as usize;
            m.next = self.lists[class];
            self.lists[class] = b as u32;
        }
    }
}

/// Ends the program with Java's message, without allocating (the heap's lock may be held).
#[cold]
fn out_of_memory() -> ! {
    fail(b"Exception in thread \"main\" java.lang.OutOfMemoryError: Java heap space\n");
}

#[cold]
fn fail(message: &[u8]) -> ! {
    unsafe {
        write(2, message.as_ptr().cast(), message.len());
        _exit(1)
    }
}

/// A thread's free slots: per class, a list linked through the slots' first words.
struct Lists {
    heads: [Cell<*mut u8>; CLASSES],
    bytes: [Cell<usize>; CLASSES],
    /// This thread's entry in `THREADS`, once registered.
    entry: Cell<usize>,
    /// Set when the thread has unregistered at its end: it allocates without lists from then on.
    ended: Cell<bool>,
}

thread_local! {
    static LISTS: Lists = const {
        Lists {
            heads: [const { Cell::new(ptr::null_mut()) }; CLASSES],
            bytes: [const { Cell::new(0) }; CLASSES],
            entry: Cell::new(NO_ENTRY),
            ended: Cell::new(false),
        }
    };
    static UNREGISTER: Unregister = const { Unregister };
}

const NO_ENTRY: usize = usize::MAX;

/// Unregisters the thread when its thread-local storage is dropped.
struct Unregister;

impl Drop for Unregister {
    fn drop(&mut self) {
        let _ = LISTS.try_with(unregister);
    }
}

/// A registered thread.
struct Thread {
    used: AtomicBool,
    pthread: Cell<usize>,
    stack_top: Cell<usize>,
    /// Where its stack pointer and thread pointer were when it stopped.
    stopped_sp: Cell<usize>,
    thread_pointer: Cell<usize>,
    /// Its free lists.
    lists: Cell<*const Lists>,
}

unsafe impl Sync for Thread {}

const MAX_THREADS: usize = 4096;

static THREADS: [Thread; MAX_THREADS] = [const {
    Thread {
        used: AtomicBool::new(false),
        pthread: Cell::new(0),
        stack_top: Cell::new(0),
        stopped_sp: Cell::new(0),
        thread_pointer: Cell::new(0),
        lists: Cell::new(ptr::null()),
    }
}; MAX_THREADS];

/// Threads announced by `spawning` that have not registered yet.
static SPAWNING: AtomicUsize = AtomicUsize::new(0);

/// The stop protocol: stopped threads count up `STOPPED` and wait while `EPOCH` is unchanged.
static STOPPED: AtomicU32 = AtomicU32::new(0);
static EPOCH: AtomicU32 = AtomicU32::new(0);

// The current thread's entry, set while it is registered (read by the signal handler).
thread_local! {
    static ENTRY: Cell<usize> = const { Cell::new(NO_ENTRY) };
}

/// The thread pointer: the address that `%fs` points at, below which the static thread-local
/// storage of the program lies.
#[inline(always)]
fn thread_pointer() -> usize {
    let tp: usize;
    unsafe { std::arch::asm!("mov {}, fs:0", out(reg) tp, options(nostack, readonly, preserves_flags)) };
    tp
}

/// Registers the calling thread, under the heap's lock.
fn register(h: &Guard, lists: &Lists) {
    let _ = h;
    let me = unsafe { pthread_self() };
    let mut attr = [0u64; 8];
    let (mut addr, mut size) = (0usize, 0usize);
    unsafe {
        pthread_getattr_np(me, &mut attr);
        pthread_attr_getstack(&attr, &mut addr, &mut size);
        pthread_attr_destroy(&mut attr);
    }
    let Some(i) = THREADS.iter().position(|t| !t.used.load(Ordering::Relaxed)) else {
        fail(b"too many threads for the collector\n");
    };
    let t = &THREADS[i];
    t.pthread.set(me);
    t.stack_top.set(addr + size);
    t.lists.set(lists);
    t.used.store(true, Ordering::Release);
    lists.entry.set(i);
    let _ = ENTRY.try_with(|e| e.set(i));
    let _ = UNREGISTER.try_with(|_| ());
}

/// Unregisters the calling thread, giving its free slots back to their blocks.
fn unregister(lists: &Lists) {
    let g = lock();
    let h = heap(&g);
    lists.ended.set(true);
    let i = lists.entry.get();
    if i == NO_ENTRY {
        return;
    }
    for class in 0..CLASSES {
        let mut p = lists.heads[class].get();
        while !p.is_null() {
            let next = unsafe { (p as *mut *mut u8).read() };
            unsafe { (p as *mut *mut u8).write(ptr::null_mut()) };
            untake(h, p as usize);
            p = next;
        }
        lists.heads[class].set(ptr::null_mut());
        lists.bytes[class].set(0);
    }
    THREADS[i].used.store(false, Ordering::Release);
    lists.entry.set(NO_ENTRY);
    let _ = ENTRY.try_with(|e| e.set(NO_ENTRY));
}

/// Marks a slot untaken, so that its block's next refill hands it out again.
fn untake(h: &mut Heap, p: usize) {
    let b = (p - h.base) >> BLOCK_SHIFT;
    let start = h.block_address(b);
    let m = h.meta(b);
    let slot = (((p - start) as u64 * m.magic) >> 32) as usize;
    m.taken[slot / 64] &= !(1u64 << (slot % 64));
    h.list(b);
}

/// What a thread that `pthread_create` starts runs first.
struct Start {
    routine: extern "C" fn(*mut u8) -> *mut u8,
    argument: *mut u8,
}

/// `pthread_create`, in place of the C library's, for every thread of the program: Rust's
/// threads and any other. The new thread registers before it runs anything, and no collection
/// runs between the call and that, while the argument (Rust's closure) is only in memory of the
/// C library, which is not scanned.
///
/// # Safety
///
/// As the C library's `pthread_create`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_create(
    thread: *mut usize,
    attr: *const u8,
    routine: extern "C" fn(*mut u8) -> *mut u8,
    argument: *mut u8,
) -> i32 {
    let real = real_pthread_create();
    if BASE.load(Ordering::Acquire) == 0 {
        // The program does not allocate from this heap.
        return unsafe { real(thread, attr, routine, argument) };
    }
    let start = unsafe { System.alloc(Layout::new::<Start>()) } as *mut Start;
    unsafe { start.write(Start { routine, argument }) };
    SPAWNING.fetch_add(1, Ordering::SeqCst);
    let result = unsafe { real(thread, attr, begin, start as *mut u8) };
    if result != 0 {
        SPAWNING.fetch_sub(1, Ordering::SeqCst);
        unsafe { System.dealloc(start as *mut u8, Layout::new::<Start>()) };
    }
    result
}

type Create = unsafe extern "C" fn(*mut usize, *const u8, extern "C" fn(*mut u8) -> *mut u8, *mut u8) -> i32;

/// The C library's `pthread_create`, which this crate's takes the place of.
fn real_pthread_create() -> Create {
    unsafe { std::mem::transmute(dlsym(-1isize as *mut u8, c"pthread_create".as_ptr())) }
}

/// The start of every thread: registers it, then runs what `pthread_create` was given.
extern "C" fn begin(start: *mut u8) -> *mut u8 {
    let Start { routine, argument } = unsafe { (start as *mut Start).read() };
    unsafe { System.dealloc(start, Layout::new::<Start>()) };
    let _ = LISTS.try_with(|lists| {
        let g = lock();
        if lists.entry.get() == NO_ENTRY {
            register(&g, lists);
        }
    });
    SPAWNING.fetch_sub(1, Ordering::SeqCst);
    routine(argument)
}

/// The collector of translated programs, installed as the global allocator.
pub struct Allocator;

unsafe impl GlobalAlloc for Allocator {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let Some(index) = size_index(layout.size(), layout.align()) else {
            return allocate_large(layout);
        };
        let class = index * 2 + (layout.align() < 8) as usize;
        let p = LISTS
            .try_with(|lists| {
                let head = lists.heads[class].get();
                if head.is_null() {
                    return if lists.ended.get() {
                        allocate_without_lists(class)
                    } else {
                        refill(lists, class)
                    };
                }
                let next = unsafe { (head as *mut *mut u8).read() };
                lists.heads[class].set(next);
                lists.bytes[class].set(lists.bytes[class].get() - SIZES[index] as usize);
                head
            })
            .unwrap_or_else(|_| allocate_without_lists(class));
        // A slot that may hold pointers is zeroed whole as it is handed out, so that no old word
        // in a part the program does not write keeps an object alive.
        if layout.align() >= 8 {
            unsafe { ptr::write_bytes(p, 0, SIZES[index] as usize) };
        }
        p
    }

    #[inline]
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        // The layout's class is the slot's: `realloc` keeps a slot only for the same class.
        let Some(index) = size_index(layout.size(), layout.align()) else {
            return free_large(p, layout);
        };
        let class = index * 2 + (layout.align() < 8) as usize;
        let size = SIZES[index] as usize;
        let kept = LISTS.try_with(|lists| {
            let bytes = lists.bytes[class].get();
            if bytes >= LIST_MAX_BYTES || lists.entry.get() == NO_ENTRY {
                return false;
            }
            unsafe { (p as *mut *mut u8).write(lists.heads[class].get()) };
            lists.heads[class].set(p);
            lists.bytes[class].set(bytes + size);
            true
        });
        if kept != Ok(true) {
            let g = lock();
            untake(heap(&g), p as usize);
        }
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { self.alloc(layout) };
        // A slot that may hold pointers, and a large allocation, are handed out zeroed already.
        if layout.align() < 8 && size_index(layout.size(), layout.align()).is_some() {
            unsafe { ptr::write_bytes(p, 0, layout.size()) };
        }
        p
    }

    #[inline]
    unsafe fn realloc(&self, p: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // The same slot serves a size of the same class, and the same blocks a large size that
        // needs as many: `dealloc` finds the slot's class from the size it is given.
        let same = match (size_index(layout.size(), layout.align()), size_index(new_size, layout.align())) {
            (Some(a), Some(b)) => a == b,
            (None, None) => layout.size().div_ceil(BLOCK) == new_size.div_ceil(BLOCK),
            _ => false,
        };
        if same {
            if new_size < layout.size() {
                // The bytes past the new end are zeroed, as at allocation: no old word there
                // keeps an object alive.
                unsafe { ptr::write_bytes(p.add(new_size), 0, layout.size() - new_size) };
            }
            return p;
        }
        let new = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        let moved = unsafe { self.alloc(new) };
        unsafe { ptr::copy_nonoverlapping(p, moved, layout.size().min(new_size)) };
        unsafe { self.dealloc(p, layout) };
        moved
    }
}

/// The lock, with the heap set up, the calling thread registered and a collection run when
/// enough has been taken since the last.
fn prepare() -> Guard {
    let g = lock();
    let h = heap(&g);
    if h.base == 0 {
        start(h);
    }
    let registered = LISTS
        .try_with(|lists| {
            if lists.entry.get() == NO_ENTRY && !lists.ended.get() {
                register(&g, lists);
            }
            lists.entry.get() != NO_ENTRY
        })
        .unwrap_or(false);
    // A thread that is not registered cannot collect: its own stack would not be scanned.
    if registered && heap(&g).taken >= heap(&g).next_collection {
        collect(&g);
    }
    g
}

/// Refills an empty list of `class`, collecting first when enough has been taken.
#[cold]
#[inline(never)]
fn refill(lists: &Lists, class: usize) -> *mut u8 {
    let g = prepare();
    let h = heap(&g);
    let (head, bytes) = h.refill(class);
    let next = unsafe { (head as *mut *mut u8).read() };
    unsafe { (head as *mut *mut u8).write(ptr::null_mut()) };
    lists.heads[class].set(next);
    lists.bytes[class].set(bytes - SIZES[class / 2] as usize);
    head
}

/// An allocation on a thread whose free lists are gone (while its thread-local storage is
/// dropped): one slot taken from a block under the lock.
#[cold]
fn allocate_without_lists(class: usize) -> *mut u8 {
    let g = lock();
    let h = heap(&g);
    if h.base == 0 {
        start(h);
    }
    let (head, _) = h.refill(class);
    // Gives back all the slots but the first.
    let mut p = unsafe { (head as *mut *mut u8).read() };
    unsafe { (head as *mut *mut u8).write(ptr::null_mut()) };
    while !p.is_null() {
        let next = unsafe { (p as *mut *mut u8).read() };
        unsafe { (p as *mut *mut u8).write(ptr::null_mut()) };
        untake(h, p as usize);
        h.taken -= SIZES[class / 2] as usize;
        p = next;
    }
    head
}

#[cold]
fn allocate_large(layout: Layout) -> *mut u8 {
    if layout.align() > BLOCK {
        return ptr::null_mut();
    }
    let n = layout.size().div_ceil(BLOCK).max(1);
    let g = prepare();
    let h = heap(&g);
    let b = h.take_blocks(n);
    let m = h.meta(b);
    m.kind = LARGE;
    m.no_scan = layout.align() < 8;
    m.size = n as u32;
    m.taken[0] = 1;
    m.marks[0] = 0;
    for rest in b + 1..b + n {
        let m = h.meta(rest);
        m.kind = LARGE_REST;
        m.size = b as u32;
    }
    h.taken += n << BLOCK_SHIFT;
    h.block_address(b) as *mut u8
}

#[cold]
fn free_large(p: *mut u8, _layout: Layout) {
    let g = lock();
    let h = heap(&g);
    let b = (p as usize - h.base) >> BLOCK_SHIFT;
    let n = h.meta(b).size as usize;
    h.free_blocks(b, n);
}

// ---- Stopping the world ----

fn install_signal() {
    let mut action = SigAction {
        handler: stopped as *const () as usize,
        mask: [0; 16],
        flags: SA_SIGINFO_RESTART,
        restorer: 0,
    };
    unsafe {
        sigemptyset(&mut action.mask);
        sigaction(SIGPWR, &action, ptr::null_mut());
    }
}

/// The handler of `SIGPWR`: records the stack pointer and thread pointer, and waits for the
/// collection to end. The registers of the interrupted code are in the signal frame, above the
/// recorded stack pointer.
extern "C" fn stopped(_sig: i32, _info: *mut u8, _context: *mut u8) {
    let errno = unsafe { *__errno_location() };
    let epoch = EPOCH.load(Ordering::Acquire);
    let marker = 0usize;
    let sp = &marker as *const usize as usize;
    if let Ok(i) = ENTRY.try_with(|e| e.get())
        && i != NO_ENTRY
    {
        THREADS[i].stopped_sp.set(sp);
        THREADS[i].thread_pointer.set(thread_pointer());
    }
    fence(Ordering::SeqCst);
    STOPPED.fetch_add(1, Ordering::SeqCst);
    futex_wake(&STOPPED);
    while EPOCH.load(Ordering::Acquire) == epoch {
        futex_wait(&EPOCH, epoch);
    }
    STOPPED.fetch_sub(1, Ordering::SeqCst);
    futex_wake(&STOPPED);
    unsafe { *__errno_location() = errno };
}

/// Stops every registered thread but the calling one; returns how many were stopped.
fn stop_others(me: usize) -> u32 {
    let mut n = 0;
    for (i, t) in THREADS.iter().enumerate() {
        if i != me && t.used.load(Ordering::Acquire) && unsafe { pthread_kill(t.pthread.get(), SIGPWR) } == 0
        {
            n += 1;
        }
    }
    loop {
        let stopped = STOPPED.load(Ordering::SeqCst);
        if stopped >= n {
            break;
        }
        futex_wait(&STOPPED, stopped);
    }
    n
}

fn resume_others(n: u32) {
    let _ = n;
    EPOCH.fetch_add(1, Ordering::SeqCst);
    futex_wake(&EPOCH);
    loop {
        let stopped = STOPPED.load(Ordering::SeqCst);
        if stopped == 0 {
            break;
        }
        futex_wait(&STOPPED, stopped);
    }
}

// ---- Marking ----

/// A stack of slots marked but not yet scanned, as (address, length), in memory outside the heap.
/// Items `low..len` are on it: other markers are handed the oldest, from `low`, which tend to lead
/// to the most work.
struct MarkStack {
    items: *mut (usize, usize),
    low: usize,
    len: usize,
    capacity: usize,
}

impl MarkStack {
    fn reserved(capacity: usize) -> MarkStack {
        MarkStack { items: reserve(capacity * 16) as *mut (usize, usize), low: 0, len: 0, capacity }
    }

    fn count(&self) -> usize {
        self.len - self.low
    }

    #[inline]
    fn push(&mut self, item: (usize, usize)) {
        if self.len == self.capacity {
            // The stack is reserved large; past that, the program is out of memory anyway.
            out_of_memory();
        }
        unsafe { self.items.add(self.len).write(item) };
        self.len += 1;
    }

    #[inline]
    fn pop(&mut self) -> Option<(usize, usize)> {
        if self.len == self.low {
            self.low = 0;
            self.len = 0;
            return None;
        }
        self.len -= 1;
        Some(unsafe { self.items.add(self.len).read() })
    }

    /// Takes the oldest item.
    fn pop_oldest(&mut self) -> Option<(usize, usize)> {
        if self.len == self.low {
            return None;
        }
        self.low += 1;
        Some(unsafe { self.items.add(self.low - 1).read() })
    }
}

/// The items of each marker's stack.
const MARK_STACK_ITEMS: usize = 1 << 27;

/// The collecting thread's mark stack, reserved at the first collection.
static MARK_STACK: AtomicPtr<(usize, usize)> = AtomicPtr::new(ptr::null_mut());

/// The most bytes scanned as one piece of work: the rest of a longer range (a large array) goes
/// back on the stack first, where another marker can take it.
const SCAN_CHUNK: usize = 64 << 10;

/// The slots a marker holds while their memory is fetched (see `Marker::drain`).
const PREFETCH: usize = 8;

/// The markers of a collection are the collecting thread and up to this many helper threads.
const MAX_HELPERS: usize = 15;

/// Marking is shared with the helpers when at least this much was alive after the last
/// collection; a smaller heap is marked faster than the helpers wake.
const SHARED_MARKING_FROM: usize = 16 << 20;

/// The work markers hand to each other: items moved off a marker's stack while another marker is
/// idle. Behind its own spin lock; `len` can be read without it.
struct Pool {
    lock: AtomicBool,
    len: AtomicUsize,
    stack: UnsafeCell<MarkStack>,
}

unsafe impl Sync for Pool {}

static POOL: Pool = Pool {
    lock: AtomicBool::new(false),
    len: AtomicUsize::new(0),
    stack: UnsafeCell::new(MarkStack { items: ptr::null_mut(), low: 0, len: 0, capacity: 0 }),
};

/// The markers of the current collection, and how many of them have no work. Marking ends when
/// all of them are idle, as none can then make more work.
static MARKERS: AtomicUsize = AtomicUsize::new(1);
static IDLE: AtomicUsize = AtomicUsize::new(0);

/// The helper threads started; they wait for `MARK_ROUND` to change, mark, and count up
/// `MARK_DONE`. `META` is the block table they read.
static HELPERS: AtomicUsize = AtomicUsize::new(0);
static MARK_ROUND: AtomicU32 = AtomicU32::new(0);
static MARK_DONE: AtomicU32 = AtomicU32::new(0);
static META: AtomicPtr<Meta> = AtomicPtr::new(ptr::null_mut());

/// Whether the helpers' round is a sweep: they take `SWEEP_CHUNK` blocks at a time from
/// `SWEEP_NEXT` up to `SWEEP_TOP` and add the bytes alive in them to `SWEEP_ALIVE`.
static SWEEPING: AtomicBool = AtomicBool::new(false);
static SWEEP_NEXT: AtomicUsize = AtomicUsize::new(0);
static SWEEP_TOP: AtomicUsize = AtomicUsize::new(0);
static SWEEP_ALIVE: AtomicUsize = AtomicUsize::new(0);
const SWEEP_CHUNK: usize = 256;

/// The helpers that the machine's processors allow, at most `MAX_HELPERS`, or what
/// `JAVA_GC_THREADS` (the markers in all) sets.
fn helpers_wanted() -> usize {
    if let Some(n) = number_from(c"JAVA_GC_THREADS") {
        return n.saturating_sub(1).min(MAX_HELPERS);
    }
    let mut mask = [0u64; 16];
    if unsafe { sched_getaffinity(0, std::mem::size_of_val(&mask), mask.as_mut_ptr()) } != 0 {
        return 0;
    }
    let cpus: u32 = mask.iter().map(|w| w.count_ones()).sum();
    (cpus as usize).saturating_sub(1).min(MAX_HELPERS)
}

/// Starts the helpers, with the C library's `pthread_create`: they are not the program's threads,
/// are never stopped, and never allocate.
fn start_helpers() {
    let wanted = helpers_wanted();
    let round = MARK_ROUND.load(Ordering::Acquire);
    for _ in 0..wanted {
        let mut thread = 0usize;
        let started =
            unsafe { real_pthread_create()(&mut thread, ptr::null(), helper, round as usize as *mut u8) };
        if started != 0 {
            break;
        }
        HELPERS.fetch_add(1, Ordering::AcqRel);
    }
}

/// A helper thread: marks, then sweeps, with the collecting thread at each collection.
extern "C" fn helper(round: *mut u8) -> *mut u8 {
    let mut round = round as usize as u32;
    let mut stack = MarkStack::reserved(MARK_STACK_ITEMS);
    loop {
        let mut now = MARK_ROUND.load(Ordering::Acquire);
        while now == round {
            futex_wait(&MARK_ROUND, round);
            now = MARK_ROUND.load(Ordering::Acquire);
        }
        round = now;
        if SWEEPING.load(Ordering::Acquire) {
            sweep_share();
        } else {
            let mut marker = Marker {
                meta: META.load(Ordering::Acquire),
                base: BASE.load(Ordering::Acquire),
                end: END.load(Ordering::Acquire),
                stack,
                shares: true,
            };
            marker.finish();
            stack = marker.stack;
        }
        MARK_DONE.fetch_add(1, Ordering::AcqRel);
        futex_wake(&MARK_DONE);
    }
}

/// Sets the bit of a mark word; whether it was clear.
#[inline(always)]
fn set_mark(word: *mut u64, bit: u64, shared: bool) -> bool {
    let marks = unsafe { AtomicU64::from_ptr(word) };
    let old = marks.load(Ordering::Relaxed);
    if old & bit != 0 {
        return false;
    }
    if shared {
        marks.fetch_or(bit, Ordering::Relaxed) & bit == 0
    } else {
        marks.store(old | bit, Ordering::Relaxed);
        true
    }
}

/// One thread's marking: the collecting thread's or a helper's.
struct Marker {
    meta: *mut Meta,
    base: usize,
    end: usize,
    stack: MarkStack,
    /// Whether other markers run, so that mark bits are set atomically and work is offered.
    shares: bool,
}

impl Marker {
    /// Marks the slot that `word` points into, if it is a taken slot.
    #[inline(always)]
    fn candidate(&mut self, word: usize) {
        // A `java::Thin` keeps its class number in the top bits of a handle.
        let word = word & crate::thin::ADDRESS;
        if word.wrapping_sub(self.base) >= self.end - self.base {
            return;
        }
        let mut b = (word - self.base) >> BLOCK_SHIFT;
        let mut m = unsafe { self.meta.add(b) };
        // The table is read through raw places: other markers set mark bits beside these fields.
        match unsafe { (*m).kind } {
            SMALL => {
                let start = self.base + (b << BLOCK_SHIFT);
                let slot = (((word - start) as u64 * unsafe { (*m).magic }) >> 32) as usize;
                if slot >= unsafe { (*m).slots } as usize {
                    return;
                }
                let (w, bit) = (slot / 64, 1u64 << (slot % 64));
                if unsafe { (*m).taken[w] } & bit == 0
                    || !set_mark(unsafe { &raw mut (*m).marks[w] }, bit, self.shares)
                {
                    return;
                }
                if !unsafe { (*m).no_scan } {
                    let size = unsafe { (*m).size } as usize;
                    self.stack.push((start + slot * size, size));
                }
            }
            LARGE | LARGE_REST => {
                if unsafe { (*m).kind } == LARGE_REST {
                    b = unsafe { (*m).size } as usize;
                    m = unsafe { self.meta.add(b) };
                }
                if unsafe { (*m).taken[0] } & 1 == 0
                    || !set_mark(unsafe { &raw mut (*m).marks[0] }, 1, self.shares)
                {
                    return;
                }
                if !unsafe { (*m).no_scan } {
                    self.stack.push((
                        self.base + (b << BLOCK_SHIFT),
                        (unsafe { (*m).size } as usize) << BLOCK_SHIFT,
                    ));
                }
            }
            _ => {}
        }
    }

    /// Scans the words of `[from, to)`.
    fn range(&mut self, from: usize, to: usize) {
        let mut p = (from + 7) & !7;
        while p + 8 <= to {
            let word = unsafe { (p as *const usize).read() };
            self.candidate(word);
            p += 8;
        }
    }

    /// Scans everything on this marker's stack, offering work to idle markers now and then.
    ///
    /// A slot taken off the stack waits in a ring of `PREFETCH` slots while its memory is fetched
    /// (a prefetch as it goes in), and is scanned as it comes out: the misses of reading many
    /// slots overlap instead of following one another.
    fn drain(&mut self) {
        let mut scanned = 0u32;
        let mut ring = [(0usize, 0usize); PREFETCH];
        let (mut first, mut held) = (0usize, 0usize);
        loop {
            let Some((start, mut len)) = self.stack.pop() else {
                if held == 0 {
                    return;
                }
                // Scanning what is held may push more.
                let (start, len) = ring[first];
                first = (first + 1) % PREFETCH;
                held -= 1;
                self.range(start, start + len);
                continue;
            };
            if len > SCAN_CHUNK {
                // The other pieces go on the stack at once, where other markers can take them.
                let mut at = start + len.div_ceil(SCAN_CHUNK) * SCAN_CHUNK;
                while at - SCAN_CHUNK > start {
                    at -= SCAN_CHUNK;
                    self.stack.push((at, (start + len - at).min(SCAN_CHUNK)));
                }
                len = SCAN_CHUNK;
            }
            unsafe {
                std::arch::x86_64::_mm_prefetch::<{ std::arch::x86_64::_MM_HINT_T0 }>(start as *const i8)
            };
            if held == PREFETCH {
                let (start, len) = ring[first];
                first = (first + 1) % PREFETCH;
                held -= 1;
                self.range(start, start + len);
            }
            ring[(first + held) % PREFETCH] = (start, len);
            held += 1;
            scanned += 1;
            if self.shares && scanned.is_multiple_of(64) {
                self.offer();
            }
        }
    }

    /// Moves half of this marker's stack to the pool when a marker is idle and the pool is low.
    fn offer(&mut self) {
        if self.stack.count() < 2
            || IDLE.load(Ordering::Relaxed) == 0
            || POOL.len.load(Ordering::Relaxed) >= 256
        {
            return;
        }
        let n = (self.stack.count() / 2).min(4096);
        lock_pool();
        let pool = unsafe { &mut *POOL.stack.get() };
        for _ in 0..n {
            let Some(item) = self.stack.pop_oldest() else { break };
            pool.push(item);
        }
        POOL.len.store(pool.count(), Ordering::Release);
        unlock_pool();
    }

    /// Takes work from the pool; whether there was any.
    fn take(&mut self) -> bool {
        if POOL.len.load(Ordering::Acquire) == 0 {
            return false;
        }
        lock_pool();
        let pool = unsafe { &mut *POOL.stack.get() };
        let n = pool.count().min(256);
        for _ in 0..n {
            let Some(item) = pool.pop_oldest() else { break };
            self.stack.push(item);
        }
        if pool.count() == 0 {
            pool.low = 0;
            pool.len = 0;
        }
        POOL.len.store(pool.count(), Ordering::Release);
        unlock_pool();
        n > 0
    }

    /// Marks until every marker is out of work.
    fn finish(&mut self) {
        let markers = MARKERS.load(Ordering::Acquire);
        loop {
            self.drain();
            if self.take() {
                continue;
            }
            IDLE.fetch_add(1, Ordering::SeqCst);
            let mut spins = 0u32;
            loop {
                if POOL.len.load(Ordering::Acquire) > 0 {
                    // Not idle while taking, so that nobody sees every marker idle meanwhile.
                    IDLE.fetch_sub(1, Ordering::SeqCst);
                    if self.take() {
                        break;
                    }
                    IDLE.fetch_add(1, Ordering::SeqCst);
                }
                if IDLE.load(Ordering::SeqCst) == markers {
                    return;
                }
                spins += 1;
                if spins < 256 {
                    std::hint::spin_loop();
                } else {
                    unsafe { sched_yield() };
                }
            }
        }
    }

    /// Marks a slot on a free list without scanning it.
    fn keep(&mut self, p: usize) {
        let b = (p - self.base) >> BLOCK_SHIFT;
        let start = self.base + (b << BLOCK_SHIFT);
        let m = unsafe { self.meta.add(b) };
        let slot = (((p - start) as u64 * unsafe { (*m).magic }) >> 32) as usize;
        set_mark(unsafe { &raw mut (*m).marks[slot / 64] }, 1u64 << (slot % 64), self.shares);
    }
}

fn lock_pool() {
    while POOL.lock.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
        std::hint::spin_loop();
    }
}

fn unlock_pool() {
    POOL.lock.store(false, Ordering::Release);
}

/// The program's writable static data, and the size and alignment of its static thread-local
/// storage.
struct Statics {
    ranges: [(usize, usize); 64],
    count: usize,
    tls: (usize, usize),
}

extern "C" fn each_object(info: *const DlPhdrInfo, _size: usize, data: *mut u8) -> i32 {
    let statics = unsafe { &mut *(data as *mut Statics) };
    let info = unsafe { &*info };
    let first = unsafe { *info.name } == 0 && statics.tls == (0, 0);
    for i in 0..info.phnum as usize {
        let ph = unsafe { &*info.phdr.add(i) };
        if ph.kind == PT_LOAD && ph.flags & PF_W != 0 && statics.count < statics.ranges.len() {
            let from = info.addr + ph.vaddr as usize;
            statics.ranges[statics.count] = (from, from + ph.memsz as usize);
            statics.count += 1;
        }
        if ph.kind == PT_TLS && first {
            statics.tls = (ph.memsz as usize, ph.align.max(1) as usize);
        }
    }
    0
}

fn statics() -> Statics {
    let mut s = Statics { ranges: [(0, 0); 64], count: 0, tls: (0, 0) };
    unsafe { dl_iterate_phdr(each_object, &mut s as *mut Statics as *mut u8) };
    s
}

/// Spills the callee-saved registers onto the stack, then scans the stack from there.
#[inline(never)]
fn scan_own_stack(marker: &mut Marker, top: usize) {
    let mut registers = [0usize; 6];
    unsafe {
        std::arch::asm!(
            "mov [{0}], rbx", "mov [{0} + 8], rbp", "mov [{0} + 16], r12",
            "mov [{0} + 24], r13", "mov [{0} + 32], r14", "mov [{0} + 40], r15",
            in(reg) registers.as_mut_ptr(), options(nostack, preserves_flags),
        )
    };
    let here = registers.as_ptr() as usize;
    marker.range(here, here + std::mem::size_of_val(&registers));
    std::hint::black_box(&registers);
    marker.range(here, top);
}

/// A collection, with the lock held.
#[cold]
#[inline(never)]
fn collect(g: &Guard) {
    if SPAWNING.load(Ordering::SeqCst) > 0 {
        // A thread is starting, and the closure it runs is only known to the C library: try
        // again a little later.
        let h = heap(g);
        h.next_collection = h.taken + h.every / 4;
        return;
    }
    let me = ENTRY.try_with(|e| e.get()).unwrap_or(NO_ENTRY);
    let started = std::time::Instant::now();
    let taken = heap(g).taken;
    // The helpers mark too once the heap is large enough to be worth waking them. They start
    // before the program's threads stop, since a stopped thread may hold a lock of the C
    // library's that starting a thread needs.
    let h = heap(g);
    let shares = (h.alive >= SHARED_MARKING_FROM && h.every >= MIN_COLLECT) || h.threads_set;
    if shares && HELPERS.load(Ordering::Acquire) == 0 {
        start_helpers();
    }
    let stopped = stop_others(me);
    let h = heap(g);
    if MARK_STACK.load(Ordering::Relaxed).is_null() {
        MARK_STACK.store(MarkStack::reserved(MARK_STACK_ITEMS).items, Ordering::Relaxed);
        unsafe { *POOL.stack.get() = MarkStack::reserved(MARK_STACK_ITEMS) };
    }
    // The marks are clear: the last sweep cleared them, and blocks never used are zero.
    let (base, end) = (h.base, h.block_address(h.top));
    let helpers = if shares { HELPERS.load(Ordering::Acquire) } else { 0 };
    MARKERS.store(helpers + 1, Ordering::Release);
    IDLE.store(0, Ordering::Release);
    META.store(h.meta, Ordering::Release);
    if helpers > 0 {
        MARK_DONE.store(0, Ordering::Release);
        MARK_ROUND.fetch_add(1, Ordering::AcqRel);
        futex_wake(&MARK_ROUND);
    }
    let stack =
        MarkStack { items: MARK_STACK.load(Ordering::Relaxed), low: 0, len: 0, capacity: MARK_STACK_ITEMS };
    let mut marker = Marker { meta: h.meta, base, end, stack, shares: helpers > 0 };
    let statics = statics();
    for &(from, to) in &statics.ranges[..statics.count] {
        marker.range(from, to);
        marker.drain();
    }
    let tls = statics.tls.0.next_multiple_of(statics.tls.1);
    for (i, t) in THREADS.iter().enumerate() {
        if !t.used.load(Ordering::Acquire) {
            continue;
        }
        let lists = unsafe { &*t.lists.get() };
        for class in 0..CLASSES {
            let mut p = lists.heads[class].get() as usize;
            while p != 0 {
                marker.keep(p);
                p = unsafe { (p as *const usize).read() };
            }
        }
        let tp = if i == me { thread_pointer() } else { t.thread_pointer.get() };
        if tls > 0 && tp != 0 {
            marker.range(tp - tls, tp);
            marker.drain();
        }
        if i == me {
            scan_own_stack(&mut marker, t.stack_top.get());
        } else if t.stopped_sp.get() != 0 {
            marker.range(t.stopped_sp.get(), t.stack_top.get());
        }
        marker.drain();
    }
    marker.finish();
    loop {
        let done = MARK_DONE.load(Ordering::Acquire);
        if helpers == 0 || done as usize >= helpers {
            break;
        }
        futex_wait(&MARK_DONE, done);
    }
    let h = heap(g);
    let marked = started.elapsed();
    sweep(h, helpers);
    h.collections += 1;
    if h.log {
        report(h, taken, marked, started.elapsed());
    }
    for t in &THREADS {
        t.stopped_sp.set(0);
        t.thread_pointer.set(0);
    }
    resume_others(stopped);
}

/// Writes a line about the collection that just ended to standard error, without allocating.
fn report(h: &Heap, taken: usize, marked: std::time::Duration, total: std::time::Duration) {
    use std::io::Write;
    let mut buffer = [0u8; 256];
    let mut out = &mut buffer[..];
    let _ = writeln!(
        out,
        "[gc {}: took {} MB, {} MB alive, heap {} MB, mark {} ms, sweep {} ms]",
        h.collections,
        taken >> 20,
        h.alive >> 20,
        (h.top << BLOCK_SHIFT) >> 20,
        marked.as_millis(),
        (total - marked).as_millis()
    );
    let len = 256 - out.len();
    unsafe { write(2, buffer.as_ptr().cast(), len) };
}

/// Frees every taken slot left unmarked, and lists the blocks with untaken slots. The blocks are
/// swept in chunks by the collecting thread and the `helpers` together; then this thread lists
/// the blocks and frees the empty ones.
fn sweep(h: &mut Heap, helpers: usize) {
    SWEEP_NEXT.store(0, Ordering::Release);
    SWEEP_TOP.store(h.top, Ordering::Release);
    SWEEP_ALIVE.store(0, Ordering::Release);
    if helpers > 0 {
        SWEEPING.store(true, Ordering::Release);
        MARK_DONE.store(0, Ordering::Release);
        MARK_ROUND.fetch_add(1, Ordering::AcqRel);
        futex_wake(&MARK_ROUND);
    }
    sweep_share();
    loop {
        let done = MARK_DONE.load(Ordering::Acquire);
        if helpers == 0 || done as usize >= helpers {
            break;
        }
        futex_wait(&MARK_DONE, done);
    }
    SWEEPING.store(false, Ordering::Release);
    h.lists = [NONE; CLASSES];
    let mut alive = SWEEP_ALIVE.load(Ordering::Acquire);
    // Neighbouring empty blocks are freed together, in one call to the system.
    let mut empty: Option<(usize, usize)> = None;
    let mut b = 0;
    while b < h.top {
        let m = h.meta(b);
        let (n, free) = match m.kind {
            SMALL => {
                m.listed = false;
                if m.live > 0 && m.untaken {
                    h.list(b);
                }
                (1, m.live == 0)
            }
            LARGE => {
                let n = m.size as usize;
                if m.live > 0 {
                    alive += n << BLOCK_SHIFT;
                }
                (n, m.live == 0)
            }
            _ => (1, false),
        };
        if free {
            empty = match empty {
                Some((start, len)) if start + len == b => Some((start, len + n)),
                Some((start, len)) => {
                    h.free_blocks(start, len);
                    Some((b, n))
                }
                None => Some((b, n)),
            };
        }
        b += n;
    }
    if let Some((start, len)) = empty {
        h.free_blocks(start, len);
    }
    merge_runs(h);
    h.alive = alive;
    h.taken = 0;
    h.next_collection = if h.every < MIN_COLLECT { h.every } else { (alive / 100 * h.growth).max(h.every) };
}

/// Sweeps chunks of blocks until none are left (see `SWEEP_NEXT`).
fn sweep_share() {
    let meta = META.load(Ordering::Acquire);
    let top = SWEEP_TOP.load(Ordering::Acquire);
    let mut alive = 0;
    loop {
        let from = SWEEP_NEXT.fetch_add(SWEEP_CHUNK, Ordering::AcqRel);
        if from >= top {
            break;
        }
        for b in from..(from + SWEEP_CHUNK).min(top) {
            alive += sweep_block(unsafe { meta.add(b) });
        }
    }
    SWEEP_ALIVE.fetch_add(alive, Ordering::AcqRel);
}

/// Sweeps the block described by `m`: frees the dead slots of a small block, records
/// what lives (`live`, `untaken`), and clears the marks for the next collection. Returns the
/// bytes alive in a small block.
fn sweep_block(m: *mut Meta) -> usize {
    let m = unsafe { &mut *m };
    let alive = match m.kind {
        SMALL => {
            let size = m.size as usize;
            let mut live = 0u32;
            let mut untaken = false;
            for w in 0..(m.slots as usize).div_ceil(64) {
                let dead = m.taken[w] & !m.marks[w];
                m.taken[w] &= !dead;
                live += m.taken[w].count_ones();
                let valid = if (w + 1) * 64 <= m.slots as usize {
                    u64::MAX
                } else {
                    (1u64 << (m.slots as usize % 64)) - 1
                };
                untaken |= m.taken[w] & valid != valid;
            }
            m.live = live;
            m.untaken = untaken;
            live as usize * size
        }
        LARGE => {
            m.live = (m.marks[0] & 1) as u32;
            0
        }
        _ => 0,
    };
    m.marks = [0; BITMAP_WORDS];
    alive
}

/// Rebuilds the runs of free blocks from the blocks' kinds, merging neighbours.
fn merge_runs(h: &mut Heap) {
    h.run_count = 0;
    let mut b = 0;
    while b < h.top {
        if h.meta(b).kind == FREE {
            let start = b;
            while b < h.top && h.meta(b).kind == FREE {
                b += 1;
            }
            unsafe { *h.runs.add(h.run_count) = (start as u32, (b - start) as u32) };
            h.run_count += 1;
        } else {
            b += 1;
        }
    }
}

/// Runs a collection now (for tests and `System.gc()`).
pub fn collect_now() {
    let g = lock();
    if heap(&g).base != 0 {
        collect(&g);
    }
}

/// The bytes alive after the last collection, and the number of collections.
pub fn statistics() -> (usize, usize) {
    let g = lock();
    let h = heap(&g);
    (h.alive, h.collections)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_division_by_a_slot_size_is_exact() {
        for &size in &SIZES {
            let magic = (1u64 << 32) / size as u64 + 1;
            for offset in 0..BLOCK as u64 {
                assert_eq!((offset * magic) >> 32, offset / size as u64, "size {size} offset {offset}");
            }
        }
    }

    #[test]
    fn sizes_take_the_smallest_class_that_holds_them() {
        assert_eq!(size_index(1, 1), Some(0));
        assert_eq!(size_index(16, 8), Some(0));
        assert_eq!(size_index(17, 8), Some(1));
        assert_eq!(size_index(1000, 8).map(|i| SIZES[i]), Some(1024));
        assert_eq!(size_index(1025, 8).map(|i| SIZES[i]), Some(1280));
        assert_eq!(size_index(32768, 8).map(|i| SIZES[i]), Some(32768));
        assert_eq!(size_index(32769, 8), None);
        assert_eq!(size_index(40, 32).map(|i| SIZES[i]), Some(64));
    }
}
