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
//! blocks of 64 KB. A small block holds slots of one size: one of `SIZES` from 16 bytes to 32 KB,
//! and one kind of scanning (`Scan`): for allocations that may hold pointers anywhere, for those
//! that cannot (an alignment below 8 rules out a pointer, as in the bytes of a string), and for
//! arrays of numbers, which may hold pointers only in their header (`alloc_array`). A larger
//! allocation takes whole blocks. Each block has a bit per slot saying that the slot is taken
//! (an object, or a free slot that a thread holds) and a bit per slot for marking.
//!
//! **Allocation** takes a slot of its size from the calling thread's own free slots, a few
//! instructions: from the list of slots it freed, or from its span, a run of neighbouring untaken
//! slots of a block that it took under the heap's lock (no more than fit in `SPAN_BYTES`, one at
//! least), handed out in order by moving a pointer. A slot that may hold pointers is zeroed before
//! it is handed out (a span's all at once as the thread takes it, one of the free list as it is
//! taken), so that no old word in it keeps an object alive; a pointer-free one only for
//! `alloc_zeroed`.
//!
//! **Collection** runs when the program has taken as many bytes as were alive after the last one
//! (at least `MIN_COLLECT`; `JAVA_GC_GROWTH` sets the share), times a scale that grows, up to
//! twice or, for a small heap, to `SMALL_ROOM` bytes, while each collection takes more than half
//! as much processor time as the program used since the last (`TIME_SHARE`; `JAVA_GC_TIME` sets
//! the share, 0 turns the scale off). It stops every other thread
//! (with `SIGPWR`; each records where its stack is and waits), marks every slot reachable from the
//! roots, and frees the taken slots left unmarked. With a large heap, helper threads
//! mark and sweep alongside (see "Marking"). The roots are scanned conservatively: any aligned word that points into a
//! taken slot keeps it, which makes interior pointers and pointers in registers work. They are
//! - the stacks of the threads (from the stack pointer recorded when it stopped, which is below
//!   the registers that the signal saved), and of the collecting thread, from where the
//!   collection began, with its registers there (`OwnStack`);
//! - the program's static data, the writable segments of every loaded object;
//! - each thread's static thread-local storage, where the slots its free lists start from are;
//! - the free lists of every thread, whose slots are marked without being scanned, and the slots
//!   of their spans not handed out yet, and the last one that was (all that were, in a span that
//!   handed out slots to Rust code), which are marked once marking is done (`keep_spans`).
//!
//! Slots are scanned whole, word by word, except in blocks that cannot hold pointers.
//!
//! **Generations**, with `JAVA_GC_GENERATIONS=1`: an object that lives through two collections
//! keeps its mark and is old (`age`). A minor collection marks from the roots and from the old
//! objects on the pages written since the last collection or protected by it, stops at marked
//! objects, and frees the young ones left unmarked; a full one clears the marks first, and runs once a quarter more (plus 1 MB) is
//! alive than after the last full one, at once after the minor one that finds it. Which pages
//! were written the kernel says: the heap is registered with a `userfaultfd` in asynchronous
//! write-protect mode, each collection protects the pages that hold old objects, and
//! `PAGEMAP_SCAN` reports those written since (`track_writes`, `written`, `protect_old`). Every
//! write counts, whatever code or thread makes it, so nothing is emitted in the program. Memory
//! outside the heap that can hold a reference is the roots, scanned whole at every collection.
//! Where the kernel refuses, every collection is full.
//!
//! A thread is registered (its stack and thread pointer found) as the first thing it does, by
//! this crate's `pthread_create`, which takes the C library's place, and unregistered when it
//! ends. Between the call and the registration, the closure the thread runs is kept by an entry
//! of `STARTING`, in the static data, which collections scan.

mod census;
pub(crate) mod references;
mod who;

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::{Cell, UnsafeCell};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, AtomicU64, AtomicUsize, Ordering, fence};

/// The heap's blocks are `1 << BLOCK_SHIFT` bytes, aligned to their size.
const BLOCK_SHIFT: usize = 16;
const BLOCK: usize = 1 << BLOCK_SHIFT;

/// The address space reserved for the heap: from `LOW_HEAP` to 32 GB when that range is free,
/// where a 4-byte reference (`java::RefField`) is an object's address divided by 8.
const HEAP_BYTES: usize = 28 << 30;
const LOW_HEAP: usize = 4 << 30;
/// The end of the addresses that 4-byte references reach.
pub(crate) const COMPACT_END: usize = 32 << 30;
const MAX_BLOCKS: usize = HEAP_BYTES >> BLOCK_SHIFT;

/// The most slots a block has (of 16 bytes), and the words of a bitmap over them.
const MAX_SLOTS: usize = BLOCK / 16;
const BITMAP_WORDS: usize = MAX_SLOTS / 64;

/// The sizes of the slots of small blocks. Larger allocations take whole blocks. Up to 128 bytes,
/// where most objects are, the sizes grow by 8 bytes, a handle or a `long`, so that an object
/// takes at most 7 bytes more than it needs; above, by a quarter of a power of two, with a size
/// of a power of two and 16 bytes beside each up to 8 KB: an array whose elements take a power
/// of two, as Java programs' arrays often do, and its 16-byte header (`array.rs`). Without it,
/// the `long[256]` of BlockReads' sections took a slot of 2,560 bytes for its 2,064. None at 16
/// and 32 KB, where a block holds no more slots of 16,400 bytes than of 20,480, nor of 32,784
/// than one large allocation.
const SIZES: [u32; 54] = [
    16, 24, 32, 40, 48, 56, 64, 72, 80, 88, 96, 104, 112, 120, 128, 144, 160, 192, 224, 256, 272, 320, 384,
    448, 512, 528, 640, 768, 896, 1024, 1040, 1280, 1536, 1792, 2048, 2064, 2560, 3072, 3584, 4096, 4112,
    5120, 6144, 7168, 8192, 8208, 10240, 12288, 14336, 16384, 20480, 24576, 28672, 32768,
];
const SIZE_COUNT: usize = SIZES.len();
const LARGEST_SMALL: usize = 32768;

/// How marking scans a slot: whole (`SCAN_ALL`), not at all (`SCAN_NONE`: an alignment below 8,
/// which rules out a pointer), or only its first `HEAD_BYTES` (`SCAN_HEAD`: an array of numbers,
/// whose header may hold pointers and whose elements cannot, `alloc_array`).
type Scan = u8;
const SCAN_ALL: Scan = 0;
const SCAN_NONE: Scan = 1;
pub(crate) const SCAN_HEAD: Scan = 2;
const SCANS: usize = 3;
/// The bytes of a `SCAN_HEAD` slot that marking scans: an array's header (`array.rs`).
pub(crate) const HEAD_BYTES: usize = 16;

/// A class is a size and a kind of scanning: `size_index * SCANS + scan`.
const CLASSES: usize = SIZE_COUNT * SCANS;

/// The kind of scanning of an allocation of `layout` through the global allocator.
#[inline(always)]
const fn scan_of(align: usize) -> Scan {
    if align < 8 { SCAN_NONE } else { SCAN_ALL }
}

/// The bytes of a slot of `size` bytes that marking scans, from its start.
#[inline(always)]
fn scanned_bytes(scan: Scan, size: usize) -> usize {
    match scan {
        SCAN_ALL => size,
        SCAN_HEAD => HEAD_BYTES.min(size),
        _ => 0,
    }
}

/// The fewest bytes taken between collections.
const MIN_COLLECT: usize = 64 << 20;

/// While a collection takes more processor time than this percentage of what the program used
/// since the last one, the bytes taken between collections grow (`scale_by_time`), by at most `SCALE_STEP` a collection and to
/// `MAX_SCALE` percent at most, or to `SMALL_ROOM` bytes where that is more; while they take
/// less, the growth shrinks back, by half at most.
/// Processor time rather than time on the clock, so that threads taking memory side by side do
/// not count as collecting more. The small step keeps a short phase of costly collections from
/// growing the heap: the share must stay high for several collections in a row. BinaryTrees and
/// SmallObjects, whose marking on every processor takes several times the program's own time
/// between collections, grow to twice; ManyThreadsAllocating, whose costly collections are the
/// last two, stays at its peak memory. Twice at most, since a large program that collects as
/// often (the translated javac) then holds about as much as with `JAVA_GC_GROWTH=200`, 2.0 GB
/// against the Java VM's 4.1 GB (docs/collector.md).
const TIME_SHARE: usize = 50;

/// Seconds between the times a collection gives the program's code pages back to the system
/// (`give_code_pages_back`), once the program has run a while. The first time is after a second,
/// and the wait doubles from there up to this: startup runs most of the code that is run once,
/// within its first seconds, and the Minecraft server held 172 MB of code and constants until its
/// first give-back at 10 seconds, its peak once the heap is kept smaller (`JAVA_GC_GROWTH=50`).
const CODE_PAGES_EVERY: usize = 10;
const SCALE_STEP: f64 = 1.1;
const MAX_SCALE: usize = 200;

/// The bytes taken between collections that the scale may reach past `MAX_SCALE`, for a program
/// that collects often and has little alive: BinaryTrees marks a tree of up to 85 MB on every
/// collection; with up to twice as much taken between them it collects 38 times at a peak of
/// 245 MB, with up to 256 MB 25 times at 331 MB, in a fifth fewer instructions (17.7 G against
/// 22.1 G; the Java VM's peak is 2 GB).
const SMALL_ROOM: usize = 256 << 20;

/// How many slots of one class a thread's list keeps before freed ones go back to their blocks.
const LIST_MAX_BYTES: usize = 1 << 20;

/// The most bytes of a new span fetched into the cache ahead of the program's writes.
const PREFETCH_SPAN: usize = 4096;

/// The most bytes of slots a span takes, one slot at least. Each thread keeps a span in every
/// class it allocates in: with spans of whole 64 KB blocks, the Minecraft server's 41 threads held
/// 28 MB of slots taken and never handed out.
const SPAN_BYTES: usize = 8192;

/// The size index of a size up to 1024 bytes, by `(size + 7) / 8`.
static SMALL_CLASS: [u8; 129] = small_classes();

const fn small_classes() -> [u8; 129] {
    let mut table = [0u8; 129];
    let mut i = 0;
    while i <= 128 {
        let size = i * 8;
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
/// allocation. A slot is aligned to 8 bytes, to 16 when its size is a multiple of 16, and to its
/// size when that is a power of two, so an alignment of 16 takes a multiple of 16 and one above
/// 16 the smallest power of two that holds the size.
#[inline(always)]
fn size_index(size: usize, align: usize) -> Option<usize> {
    if align > 16 {
        let wanted = size.max(align).next_power_of_two();
        if wanted > LARGEST_SMALL {
            return None;
        }
        return SIZES.iter().position(|&s| s as usize == wanted);
    }
    let size = if align > 8 { size.next_multiple_of(16) } else { size };
    if size <= 1024 {
        return Some(SMALL_CLASS[size.div_ceil(8)] as usize);
    }
    if size > LARGEST_SMALL {
        return None;
    }
    SIZES.iter().position(|&s| s as usize >= size)
}

/// `size_index` at compile time, for a type's size and alignment (`take_slot`): the same answer,
/// by a search of `SIZES` rather than a table that a `const fn` cannot read.
const fn size_index_of(size: usize, align: usize) -> Option<usize> {
    let wanted = if align > 16 {
        (if size > align { size } else { align }).next_power_of_two()
    } else if align > 8 {
        size.next_multiple_of(16)
    } else {
        size
    };
    let mut i = 0;
    while i < SIZE_COUNT {
        let s = SIZES[i] as usize;
        if s >= wanted && (align <= 16 || s == wanted) {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Where the first slot of small block `b` of slots of `size` bytes is, past the block's
/// address: one of eight cache lines, by the block's number. Blocks are aligned to 64 KB, so
/// without this the first slots of every block would share their offset in a page, and the
/// objects a program makes first, often the ones it uses most, would sit at the same offset in
/// their pages whatever their size. A processor then takes a store to one for a store that a
/// later load of another may depend on (both addresses agree in their low 12 bits) and makes the
/// load wait. Each large allocation starts at another cache line of the page for the same reason
/// (`allocate_large`). A slot whose size is a power of two stays aligned to it, which an
/// allocation aligned to more than 16 bytes needs (`size_index`); from 1 KB, such a block is not
/// moved, as a whole slot would be lost.
fn color(b: usize, size: usize) -> usize {
    let start = b % 8 * 64;
    match size.is_power_of_two() {
        false => start,
        true if size < 1024 => start.next_multiple_of(size),
        true => 0,
    }
}

/// What a block holds.
const FREE: u8 = 0;
const SMALL: u8 = 1;
/// The first block of a large allocation, and the ones after it.
const LARGE: u8 = 2;
const LARGE_REST: u8 = 3;
/// The last block of a large allocation whose end leaves room in it (`allocate_large`): until a
/// small block or the next large allocation takes that room, which starts at `start`.
const TAIL: u8 = 4;

/// The least room a large allocation's last block must leave for the allocation to share it.
const TAIL_ROOM: usize = 1024;
/// How many of the latest tails a large allocation tries to start in.
const TAILS_TRIED: usize = 8;

/// A block's description, in a table beside the heap.
#[repr(C)]
struct Meta {
    kind: u8,
    /// The class of a small block.
    class: u8,
    /// How marking scans the slots (`Scan`).
    scan: Scan,
    /// Whether the block is on its class's list of blocks with untaken slots.
    listed: bool,
    /// The slot size of a small block; the number of blocks a large allocation spans (on its
    /// first block), or the index of that first block (on the others).
    size: u32,
    slots: u32,
    /// Where a small block's first slot is, past the block's address (see `color`); where a
    /// large allocation starts in its first block; where a tail's room starts.
    start: u32,
    /// The bytes of each slot of a small block that marking scans (`scanned_bytes`); the bytes
    /// of a large allocation (0 for one of 4 GB or more, which shares no block).
    scanned: u32,
    /// With blocks shared by large allocations (`allocate_large`): one more than the first block
    /// of the large allocation whose end is in this block, below `start`, or 0. The block is
    /// then a tail, a small block or the first block of another large allocation.
    tail_of: u32,
    /// Whether a large allocation's last block is shared: a tail, a small block or another large
    /// allocation's first block, whose `tail_of` names this one.
    shared_end: bool,
    /// Whether the block is on the heap's list of tails (`Heap::tails`), and the next one there.
    in_tails: bool,
    next_tail: u32,
    /// Whether a free block's memory holds what it last held rather than zeros (`free_blocks`).
    dirty: bool,
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

/// With generations, a block's ages, in a table of their own beside `Meta`'s, which only a
/// program with generations reserves: the slots old after the last collection, and those that
/// lived through one collection and are not old yet (`age`). In `Meta`, they took 1 KB of every
/// 64 KB block whether or not the program had generations.
#[repr(C)]
struct Ages {
    old: [u64; BITMAP_WORDS],
    once: [u64; BITMAP_WORDS],
}

/// No block.
const NONE: u32 = u32::MAX;

/// The heap's shared state, behind `LOCK`.
struct Heap {
    /// The first block's address, and the end of the blocks used so far.
    base: usize,
    meta: *mut Meta,
    /// The blocks' `Ages`, with generations; null without.
    ages: *mut Ages,
    /// Blocks `0..top` have been used.
    top: usize,
    /// Per class, the blocks with untaken slots.
    lists: [u32; CLASSES],
    /// The tails (`TAIL`), latest first, through `Meta::next_tail`. A block that stopped being a
    /// tail may stay on the list until it is next walked (`Meta::in_tails`).
    tails: u32,
    /// Free blocks below `top`, as runs (start, length), in an array of `MAX_BLOCKS` entries.
    runs: *mut (u32, u32),
    run_count: usize,
    /// Whether the runs are in descending order of their start (`take_blocks`), as they are but
    /// while a sweep frees blocks.
    runs_descending: bool,
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
    /// How much more than that the program takes between collections, in percent, from 100 to
    /// `MAX_SCALE`, or to what takes `SMALL_ROOM` bytes where that is more (`scale_by_time`).
    scale: usize,
    /// A collection's processor time, in percent of the program's since the last one, above
    /// which `scale` grows: `TIME_SHARE`, or what `JAVA_GC_TIME` sets (0 keeps `scale` at 100).
    time_share: usize,
    /// The process's processor time when the last collection ended (`process_time`), 0 before
    /// the first.
    last_end: u64,
    /// Whether `JAVA_GC_THREADS` sets the markers, which then mark every collection, however
    /// small (to test marking with helpers).
    threads_set: bool,
    /// The large allocations made, which choose where the next one starts.
    larges: usize,
    /// Whether the system reports the pages of the heap written since the last collection, so
    /// that collections can be minor (see "Generations" above): `/proc/self/pagemap`, read with
    /// `PAGEMAP_SCAN` into `regions`, outside the heap, and the userfaultfd that protects pages.
    /// -1 when it does not.
    pagemap: i32,
    uffd: i32,
    regions: *mut PageRegion,
    /// The runs of pages the last collection protected, which the next minor one scans as if
    /// written (an old object there may point to one that lived through one collection, see
    /// `age`), and room for those this collection protects; `PROTECTED_RUNS` of each at most. When they
    /// do not fit (`protected_lost`), the next collection is full.
    previous: *mut (usize, usize),
    previous_count: usize,
    protected: *mut (usize, usize),
    protected_lost: bool,
    /// The next collection is a full one once this many bytes are alive.
    next_full: usize,
    /// Whether the last collection was minor.
    minor: bool,
    /// The most seconds between the times a collection gives the program's code pages back to the
    /// system (`CODE_PAGES_EVERY`, or `JAVA_GC_CODE_PAGES`; 0 never), the monotonic second it last
    /// did, and the seconds until the next time, from 1 doubling up to `code_pages`.
    code_pages: u64,
    code_pages_at: u64,
    code_pages_wait: u64,
    /// Whether each collection gives back the pages of the threads' stacks below where each
    /// stopped (`give_stack_pages_back`; `JAVA_GC_STACK_PAGES=0` turns it off).
    stack_pages: bool,
    /// Whether small slots and free blocks are taken lowest address first (`JAVA_GC_LOW_FIRST=0`
    /// turns it off).
    low_first: bool,
    /// Whether each full collection also reports the bytes alive per slot size
    /// (`JAVA_GC_LOG=2`).
    log_sizes: bool,
    /// The bytes of roots the last collection scanned and the nanoseconds the scans took, before
    /// what they reach is marked: of the statics, the thread-locals and the stacks (`JAVA_GC_LOG`).
    roots: [(usize, u64); 3],
}

struct Shared(UnsafeCell<Heap>);
unsafe impl Sync for Shared {}

static HEAP: Shared = Shared(UnsafeCell::new(Heap {
    base: 0,
    meta: ptr::null_mut(),
    ages: ptr::null_mut(),
    top: 0,
    lists: [NONE; CLASSES],
    tails: NONE,
    runs: ptr::null_mut(),
    run_count: 0,
    runs_descending: true,
    taken: 0,
    next_collection: MIN_COLLECT,
    alive: 0,
    collections: 0,
    every: MIN_COLLECT,
    log: false,
    growth: 100,
    scale: 100,
    time_share: TIME_SHARE,
    last_end: 0,
    threads_set: false,
    larges: 0,
    pagemap: -1,
    uffd: -1,
    regions: ptr::null_mut(),
    previous: ptr::null_mut(),
    previous_count: 0,
    protected: ptr::null_mut(),
    protected_lost: false,
    next_full: 0,
    minor: false,
    code_pages: 0,
    code_pages_at: 0,
    code_pages_wait: 1,
    stack_pages: true,
    low_first: false,
    log_sizes: false,
    roots: [(0, 0); 3],
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
    fn open(path: *const std::ffi::c_char, flags: i32, ...) -> i32;
    fn ioctl(fd: i32, request: u64, ...) -> i32;
    fn clock_gettime(clock: i32, time: *mut [i64; 2]) -> i32;
}

/// The seconds since some fixed time (`CLOCK_MONOTONIC`).
fn monotonic_seconds() -> u64 {
    let mut t = [0i64; 2];
    unsafe { clock_gettime(1, &mut t) };
    t[0] as u64
}

/// The processor time the process has used, every thread's, in nanoseconds.
fn process_time() -> u64 {
    const CLOCK_PROCESS_CPUTIME_ID: i32 = 2;
    let mut t = [0i64; 2];
    unsafe { clock_gettime(CLOCK_PROCESS_CPUTIME_ID, &mut t) };
    t[0] as u64 * 1_000_000_000 + t[1] as u64
}

const PROT_READ_WRITE: i32 = 3;
const MAP_PRIVATE_ANONYMOUS_NORESERVE: i32 = 0x02 | 0x20 | 0x4000;
const MADV_DONTNEED: i32 = 4;
const MADV_HUGEPAGE: i32 = 14;
const MADV_NOHUGEPAGE: i32 = 15;
/// The size of a huge page, to which the heap is aligned.
const HUGE_PAGE: usize = 2 << 20;
const SIGPWR: i32 = 30;
const SA_SIGINFO_RESTART: i32 = 4 | 0x1000_0000;
const SYS_FUTEX: i64 = 202;
const SYS_GETPID: i64 = 39;
const SYS_GETTID: i64 = 186;
const FUTEX_WAIT_PRIVATE: i32 = 128;
const FUTEX_WAKE_PRIVATE: i32 = 129;
const PT_LOAD: u32 = 1;
const PT_TLS: u32 = 7;
const PF_W: u32 = 2;
const O_RDONLY_CLOEXEC: i32 = 0x80000;
const O_NONBLOCK: i32 = 0x800;
const SYS_USERFAULTFD: i64 = 323;
const UFFD_USER_MODE_ONLY: i64 = 1;
const UFFD_API: u64 = 0xaa;
const UFFDIO_API: u64 = 0xc018_aa3f;
const UFFDIO_REGISTER: u64 = 0xc020_aa00;
const UFFDIO_REGISTER_MODE_WP: u64 = 2;
const UFFD_FEATURE_WP_ASYNC: u64 = 0x8000;
const UFFDIO_WRITEPROTECT: u64 = 0xc018_aa06;
const UFFDIO_WRITEPROTECT_MODE_WP: u64 = 1;
const PAGE: usize = 4096;
const PAGEMAP_SCAN: u64 = 0xc060_6610;
const PM_SCAN_CHECK_WPASYNC: u64 = 2;
const PAGE_IS_WRITTEN: u64 = 2;

/// A range of pages that `PAGEMAP_SCAN` reports, and its argument (`linux/fs.h`).
#[repr(C)]
#[derive(Clone, Copy)]
struct PageRegion {
    start: u64,
    end: u64,
    categories: u64,
}

#[repr(C)]
struct PmScanArg {
    size: u64,
    flags: u64,
    start: u64,
    end: u64,
    walk_end: u64,
    vec: u64,
    vec_len: u64,
    max_pages: u64,
    category_inverted: u64,
    category_mask: u64,
    category_anyof_mask: u64,
    return_mask: u64,
}

/// The ranges one call of `PAGEMAP_SCAN` reports at most.
const REGIONS: usize = 1 << 14;
/// The most runs of protected pages a collection records for the next one (`Heap::protected`).
const PROTECTED_RUNS: usize = 1 << 20;

fn futex_wait(word: &AtomicU32, value: u32) {
    unsafe { syscall(SYS_FUTEX, word as *const AtomicU32, FUTEX_WAIT_PRIVATE, value, ptr::null::<u8>()) };
}

fn futex_wake(word: &AtomicU32) {
    futex_wake_some(word, i32::MAX);
}

/// Wakes at most `n` of the threads waiting on `word`.
fn futex_wake_some(word: &AtomicU32, n: i32) {
    unsafe { syscall(SYS_FUTEX, word as *const AtomicU32, FUTEX_WAKE_PRIVATE, n) };
}

/// Maps `len` bytes of zeroed memory that is only committed when touched, on the system's
/// default pages.
fn map(len: usize) -> *mut u8 {
    let p = unsafe { mmap(ptr::null_mut(), len, PROT_READ_WRITE, MAP_PRIVATE_ANONYMOUS_NORESERVE, -1, 0) };
    if p as isize == -1 {
        std::process::abort();
    }
    p
}

/// Maps `len` bytes as `map` does, on pages of 4 KB: a side table reserved large and touched
/// only at its start (a mark stack, the list of free runs, the references) would otherwise take
/// a whole huge page of 2 MB at its first write, on a system that gives huge pages to every
/// aligned range (`transparent_hugepage=always`). The Minecraft server's 17 mark stacks (the
/// collecting thread's, the shared pool's and 15 helpers') held 34 MB so, 2 MB each.
fn reserve(len: usize) -> *mut u8 {
    let p = map(len);
    unsafe { madvise(p, len, MADV_NOHUGEPAGE) };
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
    // From 4 GB to 32 GB when that is free, as it is in a Linux process (the program and its
    // libraries are mapped above or below it), else anywhere: a program with 4-byte references
    // then stops (`compact_references`).
    const MAP_FIXED_NOREPLACE: i32 = 0x10_0000;
    let low = unsafe {
        mmap(
            LOW_HEAP as *mut u8,
            HEAP_BYTES,
            PROT_READ_WRITE,
            MAP_PRIVATE_ANONYMOUS_NORESERVE | MAP_FIXED_NOREPLACE,
            -1,
            0,
        )
    } as usize;
    h.base = if low == LOW_HEAP {
        low
    } else {
        let raw = map(HEAP_BYTES + HUGE_PAGE) as usize;
        (raw + HUGE_PAGE - 1) & !(HUGE_PAGE - 1)
    };
    // The heap asks for huge pages, which the system then makes at a fault even when it would
    // have to compact memory first: without them, each 4 KB of the heap takes an entry of the
    // processor's address cache (TLB), which a program that follows pointers across a large heap
    // keeps missing. `JAVA_GC_HUGE_PAGES=0` leaves the system's default.
    if number_from(c"JAVA_GC_HUGE_PAGES") != Some(0) {
        unsafe { madvise(h.base as *mut u8, HEAP_BYTES, MADV_HUGEPAGE) };
    }
    // The blocks' descriptions are read at every candidate pointer marking finds, all over the
    // table, which is as dense as the heap: they stay on the system's default pages.
    h.meta = map(MAX_BLOCKS * std::mem::size_of::<Meta>()) as *mut Meta;
    h.runs = reserve(MAX_BLOCKS * std::mem::size_of::<(u32, u32)>()) as *mut (u32, u32);
    BASE.store(h.base, Ordering::Release);
    END.store(h.base, Ordering::Release);
    install_signal();
    h.log = !unsafe { getenv(c"JAVA_GC_LOG".as_ptr()) }.is_null();
    h.log_sizes = number_from(c"JAVA_GC_LOG") == Some(2);
    h.threads_set = number_from(c"JAVA_GC_THREADS").is_some_and(|n| n > 1);
    if let Some(n) = number_from(c"JAVA_GC_GROWTH") {
        h.growth = n.max(1);
    }
    if let Some(n) = number_from(c"JAVA_GC_TIME") {
        h.time_share = n;
    }
    if let Some(n) = number_from(c"JAVA_GC_EVERY") {
        h.every = n.max(1);
        h.next_collection = h.every;
    }
    START_DELAY.store(number_from(c"JAVA_GC_START_DELAY").unwrap_or(0), Ordering::Relaxed);
    if number_from(c"JAVA_GC_GENERATIONS") == Some(1) {
        track_writes(h);
    }
    h.code_pages = number_from(c"JAVA_GC_CODE_PAGES").unwrap_or(CODE_PAGES_EVERY) as u64;
    h.code_pages_at = monotonic_seconds();
    h.code_pages_wait = h.code_pages.min(1);
    h.stack_pages = number_from(c"JAVA_GC_STACK_PAGES") != Some(0);
    h.low_first = number_from(c"JAVA_GC_LOW_FIRST") != Some(0);
    let census = number_from(c"JAVA_GC_CENSUS").unwrap_or(0);
    census::LEVEL.store(census, Ordering::Relaxed);
    census::FROM.store(number_from(c"JAVA_GC_CENSUS_FROM").unwrap_or(0), Ordering::Relaxed);
    references::start_clock();
    if census >= 3 {
        census::start_types(h.base);
    }
}

/// Has the system track the writes into the heap's reservation: registers it with a
/// `userfaultfd` in asynchronous write-protect mode, in which a write to a protected page
/// unprotects it in the kernel, with no thread woken, and opens `/proc/self/pagemap`, whose
/// `PAGEMAP_SCAN` reports the unprotected pages. The userfaultfd stays open: closing it would
/// end the tracking. Where any step is refused, collections stay full.
#[cold]
fn track_writes(h: &mut Heap) {
    let uffd =
        unsafe { syscall(SYS_USERFAULTFD, (O_RDONLY_CLOEXEC | O_NONBLOCK) as i64 | UFFD_USER_MODE_ONLY) };
    if uffd < 0 {
        return;
    }
    let uffd = uffd as i32;
    let mut api = [UFFD_API, UFFD_FEATURE_WP_ASYNC, 0];
    let mut register = [h.base as u64, HEAP_BYTES as u64, UFFDIO_REGISTER_MODE_WP, 0];
    if unsafe { ioctl(uffd, UFFDIO_API, api.as_mut_ptr()) } != 0
        || api[1] & UFFD_FEATURE_WP_ASYNC == 0
        || unsafe { ioctl(uffd, UFFDIO_REGISTER, register.as_mut_ptr()) } != 0
    {
        return;
    }
    let pagemap = unsafe { open(c"/proc/self/pagemap".as_ptr(), O_RDONLY_CLOEXEC) };
    if pagemap < 0 {
        return;
    }
    h.pagemap = pagemap;
    h.uffd = uffd;
    h.ages = reserve(MAX_BLOCKS * std::mem::size_of::<Ages>()) as *mut Ages;
    h.regions = reserve(REGIONS * std::mem::size_of::<PageRegion>()) as *mut PageRegion;
    h.previous = reserve(PROTECTED_RUNS * std::mem::size_of::<(usize, usize)>()) as *mut (usize, usize);
    h.protected = reserve(PROTECTED_RUNS * std::mem::size_of::<(usize, usize)>()) as *mut (usize, usize);
    // A first scan, of no pages yet, finds whether the system answers it.
    if !written(h, |_, _| {}) {
        h.pagemap = -1;
    }
}

/// Calls `each` with every range of the heap's blocks not protected: written since it was
/// protected, or never protected. False when the system refuses.
fn written(h: &Heap, mut each: impl FnMut(usize, usize)) -> bool {
    let end = h.block_address(h.top) as u64;
    let mut at = h.base as u64;
    loop {
        let mut arg = PmScanArg {
            size: std::mem::size_of::<PmScanArg>() as u64,
            flags: PM_SCAN_CHECK_WPASYNC,
            start: at,
            end: end.max(at + 4096),
            walk_end: 0,
            vec: h.regions as u64,
            vec_len: REGIONS as u64,
            max_pages: 0,
            category_inverted: 0,
            category_mask: PAGE_IS_WRITTEN,
            category_anyof_mask: 0,
            return_mask: PAGE_IS_WRITTEN,
        };
        let n = unsafe { ioctl(h.pagemap, PAGEMAP_SCAN, &mut arg) };
        if n < 0 {
            return false;
        }
        for i in 0..n as usize {
            let r = unsafe { *h.regions.add(i) };
            each(r.start as usize, r.end as usize);
        }
        if arg.walk_end >= end {
            return true;
        }
        if arg.walk_end <= at {
            return false;
        }
        at = arg.walk_end;
    }
}

impl Heap {
    /// The description of block `b`, which lives as long as the heap, apart from `self`.
    #[allow(clippy::mut_from_ref)]
    fn meta<'m>(&self, b: usize) -> &'m mut Meta {
        unsafe { &mut *self.meta.add(b) }
    }

    /// Block `b`'s ages, in a program with generations.
    fn ages<'m>(&self, b: usize) -> Option<&'m mut Ages> {
        if self.ages.is_null() { None } else { Some(unsafe { &mut *self.ages.add(b) }) }
    }

    fn block_address(&self, b: usize) -> usize {
        self.base + (b << BLOCK_SHIFT)
    }

    /// `n` free blocks in a row: from a run of free ones, or new ones at the top. The runs are
    /// kept from the highest to the lowest (`runs_descending`), so the lowest run long enough is
    /// the last that is: one look for a single block, as most takes are, where a search of every
    /// run cost ManyThreadsAllocating, whose heap has hundreds of them, 8% of its instructions.
    fn take_blocks(&mut self, n: usize) -> usize {
        let low_first = self.low_first;
        let runs = self.sorted_runs();
        let found = match low_first {
            true => runs.iter().rposition(|&(_, len)| len as usize >= n),
            false => runs.iter().position(|&(_, len)| len as usize >= n),
        };
        if let Some(i) = found {
            let (start, len) = runs[i];
            if len as usize == n {
                self.remove_run(i);
            } else {
                // What is left of the run starts higher, still below the run before it.
                runs[i] = (start + n as u32, len - n as u32);
            }
            return start as usize;
        }
        if self.top + n > MAX_BLOCKS {
            out_of_memory();
        }
        let start = self.top;
        self.top += n;
        END.store(self.block_address(self.top), Ordering::Release);
        start
    }

    /// Takes the `n` blocks from `start` if they are free: in a run of free blocks, or at the top.
    fn take_blocks_at(&mut self, start: usize, n: usize) -> bool {
        if start == self.top {
            if self.top + n > MAX_BLOCKS {
                return false;
            }
            self.top += n;
            END.store(self.block_address(self.top), Ordering::Release);
            return true;
        }
        // The run that would hold them is the highest that starts at `start` or below.
        let runs = self.sorted_runs();
        let i = runs.partition_point(|&(from, _)| from as usize > start);
        let Some(&(from, len)) = runs.get(i) else {
            return false;
        };
        let (from, len) = (from as usize, len as usize);
        if start + n > from + len {
            return false;
        }
        let (before, after) = (start - from, from + len - start - n);
        // What is left keeps the order: the part after them in the run's place, the part before
        // just after it.
        match (before > 0, after > 0) {
            (false, false) => self.remove_run(i),
            (true, false) => runs[i] = (from as u32, before as u32),
            (false, true) => runs[i] = ((start + n) as u32, after as u32),
            (true, true) => {
                runs[i] = ((start + n) as u32, after as u32);
                self.insert_run(i + 1, (from as u32, before as u32));
            }
        }
        true
    }

    /// Puts block `b`, a tail, on the list of tails, unless it is there.
    fn push_tail(&mut self, b: usize) {
        let m = self.meta(b);
        if !m.in_tails {
            m.in_tails = true;
            m.next_tail = self.tails;
            self.tails = b as u32;
        }
    }

    /// Takes the latest tail with `TAIL_ROOM` at least off the list of tails. Those with less, the
    /// first blocks of large allocations that started late in a tail, leave the list too.
    fn pop_tail(&mut self) -> Option<usize> {
        while self.tails != NONE {
            let b = self.tails as usize;
            let m = self.meta(b);
            self.tails = m.next_tail;
            m.in_tails = false;
            if m.kind == TAIL && m.start as usize + TAIL_ROOM <= BLOCK {
                return Some(b);
            }
        }
        None
    }

    /// Frees the large allocation that starts in block `b`, and the blocks it shares: its first
    /// block becomes a tail again if an allocation still ends in it, and its last block is
    /// freed if it is a tail, which only this allocation used.
    fn release_large(&mut self, b: usize) {
        let m = self.meta(b);
        let n = m.size as usize;
        let (owner, shared_end) = (m.tail_of, m.shared_end);
        let mut from = b;
        if owner != 0 {
            m.kind = TAIL;
            m.shared_end = false;
            self.push_tail(b);
            from = b + 1;
        }
        let mut to = b + n;
        if shared_end {
            let last = self.meta(b + n - 1);
            if last.kind != TAIL {
                last.tail_of = 0;
                to -= 1;
            }
        }
        if from < to {
            self.free_blocks(from, to - from);
        }
    }

    /// Gives `n` blocks from `start` back to the heap. Their memory stays and keeps what it held
    /// (`dirty`): it goes back to the system only at a collection, beyond what the program takes
    /// before the next one (`give_back`).
    fn free_blocks(&mut self, start: usize, n: usize) {
        for b in start..start + n {
            let m = self.meta(b);
            m.kind = FREE;
            m.listed = false;
            m.dirty = true;
            m.tail_of = 0;
            m.shared_end = false;
        }
        // In its place among the runs; at the end during a sweep, which rebuilds them in order
        // (`merge_runs`) once it has freed every empty block.
        let at = match self.runs_descending {
            true => self.free_runs().partition_point(|&(from, _)| from as usize > start),
            false => self.run_count,
        };
        self.insert_run(at, (start as u32, n as u32));
    }

    /// The runs of free blocks.
    fn free_runs(&mut self) -> &mut [(u32, u32)] {
        match self.run_count {
            0 => &mut [],
            n => unsafe { std::slice::from_raw_parts_mut(self.runs, n) },
        }
    }

    /// The runs of free blocks, from the highest to the lowest.
    fn sorted_runs(&mut self) -> &mut [(u32, u32)] {
        if !self.runs_descending {
            self.free_runs().sort_unstable_by_key(|&(start, _)| std::cmp::Reverse(start));
            self.runs_descending = true;
        }
        self.free_runs()
    }

    /// Puts `run` at `i` among the runs of free blocks, moving those from there up one place.
    fn insert_run(&mut self, i: usize, run: (u32, u32)) {
        // At most one run per free block: there is room.
        unsafe {
            ptr::copy(self.runs.add(i), self.runs.add(i + 1), self.run_count - i);
            *self.runs.add(i) = run;
        }
        self.run_count += 1;
    }

    /// Removes the run of free blocks at `i`, moving those after it down one place.
    fn remove_run(&mut self, i: usize) {
        self.free_runs().copy_within(i + 1.., i);
        self.run_count -= 1;
    }

    /// A new small block of class `class`, all its slots untaken.
    fn new_block(&mut self, class: usize) -> usize {
        let size = SIZES[class / SCANS];
        // Slots below 1 KB go in a tail's room first, from its start rounded up to their alignment
        // (see `color`): a tail leaves `TAIL_ROOM` at least.
        let tail = if (size as usize) < TAIL_ROOM { self.pop_tail() } else { None };
        let (b, start) = match tail {
            Some(b) => {
                let align = match size as usize {
                    s if s.is_power_of_two() => s,
                    s if s.is_multiple_of(16) => 16,
                    _ => 8,
                };
                (b, (self.meta(b).start as usize).next_multiple_of(align))
            }
            None => {
                let b = self.take_blocks(1);
                (b, color(b, size as usize))
            }
        };
        let m = self.meta(b);
        m.kind = SMALL;
        m.class = class as u8;
        m.scan = (class % SCANS) as Scan;
        m.listed = false;
        m.size = size;
        m.start = start as u32;
        m.scanned = scanned_bytes(m.scan, size as usize) as u32;
        m.slots = (BLOCK as u32 - m.start) / size;
        m.next = NONE;
        m.magic = (1u64 << 32) / size as u64 + 1;
        m.taken = [0; BITMAP_WORDS];
        m.marks = [0; BITMAP_WORDS];
        if let Some(a) = self.ages(b) {
            a.old = [0; BITMAP_WORDS];
            a.once = [0; BITMAP_WORDS];
        }
        b
    }

    /// A block of `class` that may have untaken slots: the first on its class's list, or a new one.
    fn next_block(&mut self, class: usize) -> usize {
        loop {
            let b = self.lists[class];
            if b == NONE {
                return self.new_block(class);
            }
            let m = self.meta(b as usize);
            self.lists[class] = m.next;
            m.listed = false;
            if m.kind == SMALL && m.class as usize == class {
                return b as usize;
            }
        }
    }

    /// Takes the first run of untaken slots of block `b` from slot `from`, at most `most` of
    /// them: its first slot and its length.
    fn take_run(&mut self, b: usize, from: usize, most: usize) -> Option<(usize, usize)> {
        let m = self.meta(b);
        let slots = m.slots as usize;
        let valid = |w: usize| if (w + 1) * 64 <= slots { u64::MAX } else { (1u64 << (slots % 64)) - 1 };
        let mut w = from / 64;
        let mut free = 0;
        while w < slots.div_ceil(64) {
            free = valid(w) & !m.taken[w];
            if w == from / 64 {
                free &= u64::MAX << (from % 64);
            }
            if free != 0 {
                break;
            }
            w += 1;
        }
        if free == 0 {
            return None;
        }
        let first = w * 64 + free.trailing_zeros() as usize;
        // The run goes on while the slots are untaken, word by word.
        let mut n = 0;
        let mut at = first;
        while n < most && at < slots {
            let w = at / 64;
            let rest = (valid(w) & !m.taken[w]) >> (at % 64);
            let len = (rest.trailing_ones() as usize).min(64 - at % 64).min(most - n);
            if len == 0 {
                break;
            }
            let bits = if len == 64 { u64::MAX } else { ((1u64 << len) - 1) << (at % 64) };
            m.taken[w] |= bits;
            n += len;
            at += len;
            if !at.is_multiple_of(64) {
                break;
            }
        }
        self.taken += n * m.size as usize;
        Some((first, n))
    }

    /// Takes the untaken slots of block `b` from slot `from` up to slot `upto`, both in one word
    /// of its bitmap: their bits in that word.
    fn take_rest(&mut self, b: usize, from: usize, upto: usize) -> u64 {
        let m = self.meta(b);
        let w = from / 64;
        let upto = upto.min(m.slots as usize).min(w * 64 + 64);
        if upto <= from {
            return 0;
        }
        let window = (u64::MAX >> (64 - (upto - from))) << (from % 64);
        let bits = window & !m.taken[w];
        m.taken[w] |= bits;
        self.taken += bits.count_ones() as usize * m.size as usize;
        bits
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

/// A thread's free slots, per class: those Rust freed, in a list linked through the slots' first
/// words, and a span, a run of neighbouring untaken slots of a block that the thread took, handed
/// out in order by moving a pointer.
struct Lists {
    heads: [Cell<*mut u8>; CLASSES],
    bytes: [Cell<usize>; CLASSES],
    /// The address of the span's next slot, and the end of its last: the span is empty when they
    /// are equal.
    span_next: [Cell<usize>; CLASSES],
    span_end: [Cell<usize>; CLASSES],
    /// The address of the span's first slot (0 before the first span).
    span_start: [Cell<usize>; CLASSES],
    /// Whether `keep_spans` marked the slot before `span_next`, whose mark goes once the sweep is
    /// done, as do those of the slots not handed out.
    span_kept: [Cell<bool>; CLASSES],
    /// Whether the span handed out a slot to Rust code, through the global allocator's `alloc`,
    /// rather than only objects and arrays: then `keep_spans` keeps every slot it handed out.
    span_rust: [Cell<bool>; CLASSES],
    /// The block the span is in, and the slot of it to look for the next run from.
    span_block: [Cell<u32>; CLASSES],
    span_from: [Cell<u32>; CLASSES],
    /// Slots of the span's block that the thread took with the span's run, beyond it, and has not
    /// handed out: the untaken slots left in the run's word of the bitmap, as bits of that word,
    /// and the address of the word's first slot. Their runs become spans without the heap's
    /// lock (`span_from_rest`), which matters where the heap's holes are a few slots long.
    span_rest: [Cell<u64>; CLASSES],
    span_rest_at: [Cell<usize>; CLASSES],
    /// The spans the thread took since the last collection, those of them under the heap's lock,
    /// and their slots, for `JAVA_GC_LOG` (`span_counts`).
    spans: Cell<u64>,
    spans_locked: Cell<u64>,
    span_slots: Cell<u64>,
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
            span_next: [const { Cell::new(0) }; CLASSES],
            span_end: [const { Cell::new(0) }; CLASSES],
            span_start: [const { Cell::new(0) }; CLASSES],
            span_kept: [const { Cell::new(false) }; CLASSES],
            span_rust: [const { Cell::new(false) }; CLASSES],
            span_block: [const { Cell::new(NONE) }; CLASSES],
            span_from: [const { Cell::new(0) }; CLASSES],
            span_rest: [const { Cell::new(0) }; CLASSES],
            span_rest_at: [const { Cell::new(0) }; CLASSES],
            spans: Cell::new(0),
            spans_locked: Cell::new(0),
            span_slots: Cell::new(0),
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
    /// The lowest address of its stack, page-aligned; 0 for the process's first thread, whose
    /// stack the system grows down on demand (`give_stack_pages_back`).
    stack_low: Cell<usize>,
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
        stack_low: Cell::new(0),
        stopped_sp: Cell::new(0),
        thread_pointer: Cell::new(0),
        lists: Cell::new(ptr::null()),
    }
}; MAX_THREADS];

/// Threads started without an entry of `STARTING` that have not registered yet: no collection
/// runs while there are any.
static SPAWNING: AtomicUsize = AtomicUsize::new(0);

/// The arguments of the threads started and not registered yet (0 where free), in the static
/// data, which every collection scans: Rust's argument is a box of the closure the thread runs,
/// which until the thread registers is otherwise only in memory of the C library. So a collection
/// runs while threads start, rather than waiting until none is: ManyThreadsAllocating's 32
/// threads, which allocate while the main thread starts the next, put off the first collection
/// until 432-752 MB were taken instead of 64 when held to one or two processors (as on a loaded
/// machine), and peaked at 525-881 MB where it now peaks at 134-154 MB.
/// A thread started while every entry is taken counts in `SPAWNING` instead.
static STARTING: [AtomicUsize; STARTING_ENTRIES] = [const { AtomicUsize::new(0) }; STARTING_ENTRIES];
const STARTING_ENTRIES: usize = 64;
const NO_STARTING: usize = usize::MAX;

/// An entry of `STARTING` holding `argument`, or `NO_STARTING` (counted in `SPAWNING`).
fn starting(argument: *mut u8) -> usize {
    if !argument.is_null() {
        for (i, entry) in STARTING.iter().enumerate() {
            if entry.compare_exchange(0, argument as usize, Ordering::SeqCst, Ordering::Relaxed).is_ok() {
                return i;
            }
        }
    }
    SPAWNING.fetch_add(1, Ordering::SeqCst);
    NO_STARTING
}

/// The milliseconds a starting thread waits before it registers, for testing the collector
/// (`JAVA_GC_START_DELAY`): collections then run while threads start.
static START_DELAY: AtomicUsize = AtomicUsize::new(0);

/// Ends what `starting` began, once the thread is registered (or was not started).
fn started(entry: usize) {
    match entry {
        NO_STARTING => _ = SPAWNING.fetch_sub(1, Ordering::SeqCst),
        _ => STARTING[entry].store(0, Ordering::SeqCst),
    }
}

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
    let first = unsafe { syscall(SYS_GETTID) == syscall(SYS_GETPID) };
    t.stack_low.set(if first { 0 } else { addr.next_multiple_of(PAGE) });
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
        let size = SIZES[class / SCANS] as usize;
        let mut p = lists.span_next[class].get();
        while p < lists.span_end[class].get() {
            untake(h, p);
            p += size;
        }
        let mut rest = lists.span_rest[class].replace(0);
        while rest != 0 {
            untake(h, lists.span_rest_at[class].get() + rest.trailing_zeros() as usize * size);
            rest &= rest - 1;
        }
        lists.span_next[class].set(0);
        lists.span_end[class].set(0);
        lists.span_start[class].set(0);
        lists.span_kept[class].set(false);
        lists.span_rust[class].set(false);
        lists.span_block[class].set(NONE);
    }
    THREADS[i].used.store(false, Ordering::Release);
    lists.entry.set(NO_ENTRY);
    let _ = ENTRY.try_with(|e| e.set(NO_ENTRY));
}

/// Marks a slot untaken, so that its block's next refill hands it out again, and young: an old
/// object that Rust freed leaves its mark behind.
fn untake(h: &mut Heap, p: usize) {
    let b = (p - h.base) >> BLOCK_SHIFT;
    let m = h.meta(b);
    let start = h.block_address(b) + m.start as usize;
    let slot = (((p - start) as u64 * m.magic) >> 32) as usize;
    m.taken[slot / 64] &= !(1u64 << (slot % 64));
    m.marks[slot / 64] &= !(1u64 << (slot % 64));
    if let Some(a) = h.ages(b) {
        a.old[slot / 64] &= !(1u64 << (slot % 64));
        a.once[slot / 64] &= !(1u64 << (slot % 64));
    }
    h.list(b);
}

/// What a thread that `pthread_create` starts runs first.
struct Start {
    routine: extern "C" fn(*mut u8) -> *mut u8,
    argument: *mut u8,
    /// Its entry of `STARTING`, or `NO_STARTING`.
    entry: usize,
}

/// `pthread_create`, in place of the C library's, for every thread of the program: Rust's
/// threads and any other. The new thread registers before it runs anything; until then its
/// argument (Rust's closure), otherwise only in memory of the C library, which is not scanned,
/// is in an entry of `STARTING`.
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
    let entry = starting(argument);
    unsafe { start.write(Start { routine, argument, entry }) };
    let result = unsafe { real(thread, attr, begin, start as *mut u8) };
    if result != 0 {
        started(entry);
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
    let Start { routine, argument, entry } = unsafe { (start as *mut Start).read() };
    unsafe { System.dealloc(start, Layout::new::<Start>()) };
    let delay = START_DELAY.load(Ordering::Relaxed);
    if delay > 0 {
        std::thread::sleep(std::time::Duration::from_millis(delay as u64));
    }
    let _ = LISTS.try_with(|lists| {
        let g = lock();
        if lists.entry.get() == NO_ENTRY {
            register(&g, lists);
        }
    });
    // Registered: the argument is on its stack or in its registers, which collections now read.
    started(entry);
    routine(argument)
}

/// A slot (or a large allocation) for `layout`, of the kind of scanning `scan`, zeroed if it is
/// scanned whole; for Rust code (`RUST`, the global allocator), or for an object or array. A slot
/// of a span is zeroed already (`fill_span`); one of the free list is zeroed here.
#[inline(always)]
fn allocate<const RUST: bool>(layout: Layout, scan: Scan) -> *mut u8 {
    let Some(index) = size_index(layout.size(), layout.align()) else {
        let p = allocate_large(layout, scan);
        if census::TYPES.load(Ordering::Relaxed) {
            census::note_untyped(p as usize);
        }
        return p;
    };
    let class = index * SCANS + scan as usize;
    let size = SIZES[index] as usize;
    // The slot, and whether it is one of the free list's, which is zeroed after the closure: with
    // the zeroing in it, the closure grows past what is inlined, and every allocation calls it.
    let (p, freed) = LISTS
        .try_with(|lists| {
            let head = lists.heads[class].get();
            if !head.is_null() {
                let next = unsafe { (head as *mut *mut u8).read() };
                lists.heads[class].set(next);
                lists.bytes[class].set(lists.bytes[class].get() - size);
                return (head, true);
            }
            let next = lists.span_next[class].get();
            if next != lists.span_end[class].get() {
                lists.span_next[class].set(next + size);
                if RUST {
                    lists.span_rust[class].set(true);
                }
                return (next as *mut u8, false);
            }
            if lists.ended.get() {
                return (allocate_without_lists(class), false);
            }
            let p = refill(lists, class);
            if RUST {
                lists.span_rust[class].set(true);
            }
            (p, false)
        })
        .unwrap_or_else(|_| (allocate_without_lists(class), false));
    if freed && scan == SCAN_ALL {
        unsafe { zero_freed(p, size) };
    }
    if census::TYPES.load(Ordering::Relaxed) {
        census::note_untyped(p as usize);
    }
    p
}

/// Memory for an array of `size` bytes aligned to `align` (`array.rs`): scanned whole if its
/// elements may hold pointers (`references`), else only its header, its first `HEAD_BYTES`
/// (`SCAN_HEAD`), as its elements are numbers. Zeroed only if scanned whole; a pointer into its
/// elements keeps it, as any pointer into a slot does.
#[inline]
pub(crate) fn alloc_array(size: usize, align: usize, references: bool) -> *mut u8 {
    let layout = Layout::from_size_align(size, align).expect("an array's layout");
    let p = allocate::<false>(layout, if references { SCAN_ALL } else { SCAN_HEAD });
    if p.is_null() {
        std::alloc::handle_alloc_error(layout);
    }
    p
}

/// The collector of translated programs, installed as the global allocator.
pub struct Allocator;

unsafe impl GlobalAlloc for Allocator {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        allocate::<true>(layout, scan_of(layout.align()))
    }

    #[inline]
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        // The layout's class is the slot's: `realloc` keeps a slot only for the same class.
        let Some(index) = size_index(layout.size(), layout.align()) else {
            return free_large(p, layout);
        };
        let class = index * SCANS + scan_of(layout.align()) as usize;
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
            (None, None) => {
                let color = p as usize & (BLOCK - 1);
                (color + layout.size()).div_ceil(BLOCK) == (color + new_size).div_ceil(BLOCK)
                    && resize_large(p, layout.size(), new_size)
            }
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

/// The collector of a program whose objects hold 4-byte references (`compact.rs`), installed as
/// the global allocator in place of `Allocator`: `Allocator`, which its first allocation tells to
/// look for them (`compact_references`).
pub struct CompactAllocator;

unsafe impl GlobalAlloc for CompactAllocator {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !COMPACT.load(Ordering::Relaxed) {
            compact_references();
        }
        unsafe { Allocator.alloc(layout) }
    }

    #[inline]
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        unsafe { Allocator.dealloc(p, layout) }
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if !COMPACT.load(Ordering::Relaxed) {
            compact_references();
        }
        unsafe { Allocator.alloc_zeroed(layout) }
    }

    #[inline]
    unsafe fn realloc(&self, p: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        unsafe { Allocator.realloc(p, layout, new_size) }
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

/// Starts a new span of `class` when the free list and the span are empty: the next run of the
/// slots the thread took beyond its last span, or else the next run of untaken slots of the
/// span's block, or of a block from the heap, together with the untaken slots after it in its
/// word of the bitmap. Collects first when enough has been taken. Returns the first slot of the
/// span, handed out.
#[cold]
#[inline(never)]
fn refill(lists: &Lists, class: usize) -> *mut u8 {
    let size = SIZES[class / SCANS] as usize;
    let rest = lists.span_rest[class].get();
    if rest != 0 {
        return span_from_rest(lists, class, size, rest);
    }
    let (start, end) = {
        let g = prepare();
        take_span(heap(&g), lists, class, size)
    };
    fill_span(class, start, end);
    start as *mut u8
}

/// Takes a new span of `class` for the thread whose lists are `lists`, from the block its last
/// span was in or another, and makes it the thread's span with its first slot handed out: its
/// start and end. While the census of types runs, a span is one slot, so that every allocation
/// reaches `take_slot_cold`, which records its type.
fn take_span(h: &mut Heap, lists: &Lists, class: usize, size: usize) -> (usize, usize) {
    let types = census::TYPES.load(Ordering::Relaxed);
    loop {
        let b = lists.span_block[class].get();
        let most = if types { 1 } else { (SPAN_BYTES / size).max(1) };
        // The span's block may have been freed, and taken for another class, since: when every
        // slot the span handed out was freed.
        if b != NONE
            && h.meta(b as usize).kind == SMALL
            && h.meta(b as usize).class as usize == class
            && let Some((first, n)) = h.take_run(b as usize, lists.span_from[class].get() as usize, most)
        {
            let b = b as usize;
            let slot0 = h.block_address(b) + h.meta(b).start as usize;
            let start = slot0 + first * size;
            let end = start + n * size;
            let mut from = first + n;
            if !from.is_multiple_of(64) && !types {
                // The span and the rest together hold no more than a span may.
                let upto = (first + most).min(from / 64 * 64 + 64);
                let rest = h.take_rest(b, from, upto);
                if rest != 0 {
                    lists.span_rest[class].set(rest);
                    lists.span_rest_at[class].set(slot0 + from / 64 * 64 * size);
                }
                from = upto;
            }
            lists.span_from[class].set(from as u32);
            lists.span_start[class].set(start);
            lists.span_rust[class].set(false);
            lists.span_next[class].set(start + size);
            lists.span_end[class].set(end);
            lists.spans.set(lists.spans.get() + 1);
            lists.spans_locked.set(lists.spans_locked.get() + 1);
            lists.span_slots.set(lists.span_slots.get() + n as u64);
            return (start, end);
        }
        lists.span_block[class].set(h.next_block(class) as u32);
        lists.span_from[class].set(0);
    }
}

/// Readies the slots of a new span of `class`, `[start, end)`: zeroes them if they may hold
/// pointers, so that no old word in a part the program does not write keeps an object alive and
/// a slot handed out from the span needs no stores of its own (`take_slot`); else fetches them
/// into the cache before the program writes them. Run without the heap's lock: a collection
/// meanwhile keeps the span's slots, as at any time (`keep_spans`), and scans none of them.
#[inline(always)]
fn fill_span(class: usize, start: usize, end: usize) {
    if class % SCANS == SCAN_ALL as usize {
        unsafe { ptr::write_bytes(start as *mut u8, 0, end - start) };
    } else {
        prefetch_span(start, end);
    }
}

/// Starts a new span of `class` from the first run of the slots the thread took beyond its last
/// span (`rest`, see `Lists::span_rest`), without the heap's lock. A collection may stop the
/// thread at any point here, and then keeps the slots of the span not handed out and those of the
/// rest (`keep_spans`): so the span is empty, with no start, while it changes, and its run leaves
/// the rest only once it is the span, before its first slot is handed out.
#[inline(never)]
fn span_from_rest(lists: &Lists, class: usize, size: usize, rest: u64) -> *mut u8 {
    let first = rest.trailing_zeros() as usize;
    let n = (rest >> first).trailing_ones() as usize;
    let start = lists.span_rest_at[class].get() + first * size;
    let end = start + n * size;
    // Zeroed while the slots are still the rest, which a collection keeps.
    if class % SCANS == SCAN_ALL as usize {
        unsafe { ptr::write_bytes(start as *mut u8, 0, end - start) };
    }
    let order = || std::sync::atomic::compiler_fence(Ordering::SeqCst);
    lists.span_start[class].set(0);
    lists.span_rust[class].set(false);
    order();
    lists.span_next[class].set(start);
    lists.span_end[class].set(end);
    order();
    lists.span_start[class].set(start);
    order();
    lists.spans.set(lists.spans.get() + 1);
    lists.span_slots.set(lists.span_slots.get() + n as u64);
    let run = if n == 64 { u64::MAX } else { ((1u64 << n) - 1) << first };
    lists.span_rest[class].set(rest & !run);
    order();
    lists.span_next[class].set(start + size);
    if class % SCANS != SCAN_ALL as usize {
        prefetch_span(start, end);
    }
    start as *mut u8
}

/// Fetches a new span's slots into the cache, before the program writes them.
#[inline(always)]
fn prefetch_span(start: usize, end: usize) {
    for line in (start..end.min(start + PREFETCH_SPAN)).step_by(64) {
        unsafe { std::arch::x86_64::_mm_prefetch::<{ std::arch::x86_64::_MM_HINT_T0 }>(line as *const i8) };
    }
}

/// A slot for a new value of type `T`, zeroed, from the calling thread's span. Inlined into each
/// `Arc::new`, where the slot's class and size are constants, unlike a call of the global
/// allocator, which finds them from the layout it is given at run time: BinaryTrees spent about
/// 40% of its main thread in that call. The span's slots are zeroed already (`fill_span`), so
/// taking one is a load, a compare and a store; everything else is `take_slot_cold`, one function
/// given the class, which never returns null.
#[inline(always)]
pub(crate) fn take_slot<T>() -> *mut u8 {
    let Some(index) = (const { size_index_of(size_of::<T>(), align_of::<T>()) }) else {
        return take_large(Layout::new::<T>(), std::any::type_name::<T>());
    };
    let class = index * SCANS + scan_of(align_of::<T>()) as usize;
    let size = SIZES[index] as usize;
    LISTS
        .try_with(|lists| {
            let next = lists.span_next[class].get();
            if next != lists.span_end[class].get() {
                lists.span_next[class].set(next + size);
                return next as *mut u8;
            }
            take_slot_cold(lists, class, std::any::type_name::<T>())
        })
        .unwrap_or_else(|_| take_slot_without_lists(class, std::any::type_name::<T>()))
}

/// A slot of `class` for a new object of the type named `name` when the calling thread's span is
/// empty: from the thread's free list (the slots Rust code freed), or a new span (`refill`),
/// zeroed. Here, not inline, the census of types records the object's type: while it runs, each
/// span is one slot (`take_span`), so every object comes here.
#[cold]
#[inline(never)]
fn take_slot_cold(lists: &Lists, class: usize, name: &'static str) -> *mut u8 {
    let size = SIZES[class / SCANS] as usize;
    let head = lists.heads[class].get();
    let p = if !head.is_null() {
        let next = unsafe { (head as *mut *mut u8).read() };
        lists.heads[class].set(next);
        lists.bytes[class].set(lists.bytes[class].get() - size);
        if class % SCANS == SCAN_ALL as usize {
            unsafe { ptr::write_bytes(head, 0, size) };
        }
        head
    } else if lists.ended.get() {
        allocate_without_lists(class)
    } else {
        refill(lists, class)
    };
    if census::TYPES.load(Ordering::Relaxed) {
        census::note(p as usize, name);
    }
    p
}

/// `take_slot_cold` on a thread whose thread-local storage is gone.
#[cold]
#[inline(never)]
fn take_slot_without_lists(class: usize, name: &'static str) -> *mut u8 {
    let p = allocate_without_lists(class);
    if census::TYPES.load(Ordering::Relaxed) {
        census::note(p as usize, name);
    }
    p
}

/// Memory for an object of `layout` too large for a slot, of the type named `name`: as the global
/// allocator's `alloc`, but not counted as Rust code's (see `Lists::span_rust`).
#[cold]
#[inline(never)]
fn take_large(layout: Layout, name: &'static str) -> *mut u8 {
    let p = allocate::<false>(layout, scan_of(layout.align()));
    if p.is_null() {
        std::alloc::handle_alloc_error(layout);
    }
    if census::TYPES.load(Ordering::Relaxed) {
        census::note(p as usize, name);
    }
    p
}

/// Records, for the census of types (`JAVA_GC_CENSUS=3`), that the allocation at `p` holds a
/// `T`.
#[inline(always)]
pub(crate) fn note_type<T>(p: *const T) {
    if census::TYPES.load(Ordering::Relaxed) {
        census::note(p as usize, std::any::type_name::<T>());
    }
}

/// Records, for the census of types, that the allocation at `p` holds what `name` names.
#[inline(always)]
pub(crate) fn note_name(p: *const u8, name: &'static str) {
    if census::TYPES.load(Ordering::Relaxed) {
        census::note(p as usize, name);
    }
}

/// Zeroes the `size` bytes at `p`, a slot of a free list being handed out, all of them, as a span
/// is zeroed (`fill_span`). Inlined where the value is then stored, the zeroing would otherwise be
/// cut to the bytes that the store leaves alone: a store of a whole value counts as writing every
/// byte of it, those of an enum's smaller variant and the padding too, which it leaves as they
/// were, holding the words of the slot's last value, which then keep what they point to alive (a
/// shared object's `Option<i64>` that is `None`, over a dead object's pointer). The empty `asm!`,
/// which may read any memory, keeps every byte's zero. Up to 128 bytes, word by word rather than
/// with a call of `memset`: a slot of these sizes is a whole number of words.
#[inline(always)]
unsafe fn zero_freed(p: *mut u8, size: usize) {
    if size <= 128 {
        let mut q = p as *mut u64;
        let end = unsafe { p.add(size) } as *mut u64;
        while q < end {
            unsafe {
                q.write_volatile(0);
                q = q.add(1);
            }
        }
    } else {
        unsafe { ptr::write_bytes(p, 0, size) };
    }
    unsafe { std::arch::asm!("/* {0} */", in(reg) p, options(nostack, preserves_flags)) };
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
    loop {
        let b = h.next_block(class);
        if let Some((slot, _)) = h.take_run(b, 0, 1) {
            // The block may have more untaken slots.
            h.list(b);
            let m = h.meta(b);
            let p = h.block_address(b) + m.start as usize + slot * m.size as usize;
            fill_span(class, p, p + m.size as usize);
            return p as *mut u8;
        }
    }
}

#[cold]
fn allocate_large(layout: Layout, scan: Scan) -> *mut u8 {
    if layout.align() > BLOCK {
        return ptr::null_mut();
    }
    let g = prepare();
    let h = heap(&g);
    let size = layout.size();
    // Blocks are shared where a large allocation ends short of a block's end, as an array whose
    // elements fill whole blocks does by its 16-byte header: the rest of its last block, a tail,
    // goes to the next large allocation or to small slots (`new_block`). Not with generations,
    // whose minor collections find old objects by the blocks of written pages (`push_old`).
    let shares = h.ages.is_null() && layout.align() <= 64 && size <= u32::MAX as usize;
    // Where it starts: in the room of one of the latest tails, when the blocks after it are free.
    let mut placed = None;
    if shares {
        let (mut prev, mut t, mut tried) = (NONE, h.tails, 0);
        while t != NONE && tried < TAILS_TRIED {
            let m = h.meta(t as usize);
            let next = m.next_tail;
            if m.kind != TAIL {
                // No longer a tail: off the list.
                m.in_tails = false;
                if prev == NONE {
                    h.tails = next
                } else {
                    h.meta(prev as usize).next_tail = next
                }
                t = next;
                continue;
            }
            tried += 1;
            let at = (m.start as usize).next_multiple_of(64);
            if at < BLOCK
                && at + size > BLOCK
                && h.take_blocks_at(t as usize + 1, (at + size - BLOCK).div_ceil(BLOCK))
            {
                m.in_tails = false;
                if prev == NONE {
                    h.tails = next
                } else {
                    h.meta(prev as usize).next_tail = next
                }
                // The room was free, maybe dirty: handed out zeroed.
                unsafe { ptr::write_bytes((h.block_address(t as usize) + at) as *mut u8, 0, BLOCK - at) };
                placed = Some((t as usize, at, 1 + (at + size - BLOCK).div_ceil(BLOCK)));
                break;
            }
            prev = t;
            t = next;
        }
    }
    // Else in blocks of its own, at another of the 64 cache lines of a page (see `color`), within
    // the room that the blocks its size needs leave: a size of whole blocks, as a `Vec` that
    // doubled to 128 KB asks for, starts at its first block rather than taking one more block for
    // its color.
    let (b, at, n, fresh) = match placed {
        Some((b, at, n)) => (b, at, n, b + 1),
        None => {
            let n = size.div_ceil(BLOCK).max(1);
            let lines = (n * BLOCK - size) / 64 + 1;
            let color = if layout.align() <= 64 { (h.larges * 23 % 64 % lines) * 64 } else { 0 };
            h.larges += 1;
            let b = h.take_blocks(n);
            h.meta(b).tail_of = 0;
            (b, color, n, b)
        }
    };
    // A large allocation is handed out zeroed.
    for k in fresh..b + n {
        if h.meta(k).dirty {
            unsafe { ptr::write_bytes(h.block_address(k) as *mut u8, 0, BLOCK) };
            h.meta(k).dirty = false;
        }
    }
    // Its last block is shared when it leaves room enough, and is not its first.
    let end = at + size - (n - 1) * BLOCK;
    let shared_end = shares && n > 1 && end + TAIL_ROOM <= BLOCK;
    let m = h.meta(b);
    m.kind = LARGE;
    m.scan = scan;
    m.size = n as u32;
    // Where the allocation starts, for marking an array of numbers' header only.
    m.start = at as u32;
    m.scanned = if size <= u32::MAX as usize { size as u32 } else { 0 };
    m.shared_end = shared_end;
    m.taken[0] = 1;
    m.marks[0] = 0;
    if let Some(a) = h.ages(b) {
        a.old[0] = 0;
        a.once[0] = 0;
    }
    for rest in b + 1..b + n {
        let m = h.meta(rest);
        m.kind = LARGE_REST;
        m.size = b as u32;
        m.tail_of = 0;
    }
    if shared_end {
        let last = b + n - 1;
        let m = h.meta(last);
        m.kind = TAIL;
        m.tail_of = b as u32 + 1;
        m.start = end.next_multiple_of(64) as u32;
        h.push_tail(last);
    }
    h.taken += n << BLOCK_SHIFT;
    (h.block_address(b) + at) as *mut u8
}

/// Records that the large allocation at `p` now takes `new_size` bytes in the blocks it has,
/// for marking (`Meta::scanned`), unless it would grow into a last block that it shares
/// (`allocate_large`): false then.
fn resize_large(p: *mut u8, size: usize, new_size: usize) -> bool {
    let g = lock();
    let h = heap(&g);
    let m = h.meta((p as usize - h.base) >> BLOCK_SHIFT);
    if new_size > size && m.shared_end {
        return false;
    }
    m.scanned = if new_size <= u32::MAX as usize { new_size as u32 } else { 0 };
    true
}

#[cold]
fn free_large(p: *mut u8, _layout: Layout) {
    let g = lock();
    let h = heap(&g);
    let b = (p as usize - h.base) >> BLOCK_SHIFT;
    h.release_large(b);
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

    /// Moves the `n` oldest items, at most, onto `to`, in one copy; how many it moved.
    fn move_oldest(&mut self, n: usize, to: &mut MarkStack) -> usize {
        let n = n.min(self.count());
        if to.len + n > to.capacity {
            out_of_memory();
        }
        unsafe { ptr::copy_nonoverlapping(self.items.add(self.low), to.items.add(to.len), n) };
        self.low += n;
        to.len += n;
        if self.low == self.len {
            self.low = 0;
            self.len = 0;
        }
        n
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
const PREFETCH: usize = 16;

/// The most bytes of a slot whose last cache line is fetched as the slot goes in the ring.
const PREFETCH_LAST: usize = 256;

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

/// With `JAVA_GC_LOG`: the nanoseconds the markers of the current collection spent without work,
/// from the first time each ran out until marking ended or it took more (`Marker::finish`).
static LOG_IDLE: AtomicBool = AtomicBool::new(false);
static IDLE_NANOS: AtomicU64 = AtomicU64::new(0);

/// Counts up whenever a marker puts work in the pool and when marking ends: an idle marker that
/// has spun `IDLE_SPINS` times and yielded `IDLE_YIELDS` times sleeps on it until it changes.
/// Spinning on, idle helpers took 6 s of processor time in a run of 0.64 s that marked a long
/// linked list, which hands out no work, 22 times (`SmallObjects`).
static WORK: AtomicU32 = AtomicU32::new(0);
const IDLE_SPINS: u32 = 256;
const IDLE_YIELDS: u32 = 16;

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
                compact: COMPACT.load(Ordering::Acquire),
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

/// Whether the program holds 4-byte references (`compact_references`), which marking then looks
/// for in every half of every word it scans.
static COMPACT: AtomicBool = AtomicBool::new(false);

/// Says that the program holds references of 4 bytes in its objects (`compact.rs`), as
/// `CompactAllocator` does at the first allocation: the heap must lie below 32 GB, and marking
/// takes each 4 bytes of a word that, times 8, is the start of a taken slot for a reference to it.
#[cold]
pub fn compact_references() {
    let g = prepare();
    let h = heap(&g);
    if h.base != LOW_HEAP {
        fail(b"the heap could not be placed below 32 GB, which this program's 4-byte references need\n");
    }
    COMPACT.store(true, Ordering::Release);
}

/// One thread's marking: the collecting thread's or a helper's.
struct Marker {
    meta: *mut Meta,
    base: usize,
    end: usize,
    stack: MarkStack,
    /// Whether other markers run, so that mark bits are set atomically and work is offered.
    shares: bool,
    /// Whether words are also read as two 4-byte references (`COMPACT`).
    compact: bool,
}

impl Marker {
    /// Marks the slot that `word` points into, if it is a taken slot.
    fn candidate(&mut self, word: usize) {
        if self.shares { self.mark::<false, true>(word) } else { self.mark::<false, false>(word) }
    }

    /// Marks the slot that `word` points into, or with `EXACT` starts at, if it is a taken slot;
    /// with `SHARES`, setting its mark atomically, as other markers run. With `EXACT`, `word` is
    /// a 4-byte reference (`java::RefField`) times 8, which never points inside a slot.
    #[inline(always)]
    fn mark<const EXACT: bool, const SHARES: bool>(&mut self, word: usize) {
        // A `java::Thin` keeps its class number in the top bits of a handle.
        let word = word & crate::thin::ADDRESS;
        if word.wrapping_sub(self.base) >= self.end - self.base {
            return;
        }
        let b = (word - self.base) >> BLOCK_SHIFT;
        let m = unsafe { self.meta.add(b) };
        // The table is read through raw places: other markers set mark bits beside these fields.
        let kind = unsafe { (*m).kind };
        if kind == SMALL {
            let start = self.base + (b << BLOCK_SHIFT) + unsafe { (*m).start } as usize;
            if word < start {
                // Below the first slot may be the end of a large allocation (`Meta::tail_of`).
                let owner = unsafe { (*m).tail_of } as usize;
                if !EXACT && owner != 0 {
                    self.mark_large::<SHARES>(owner - 1);
                }
                return;
            }
            let slot = (((word - start) as u64 * unsafe { (*m).magic }) >> 32) as usize;
            let size = unsafe { (*m).size } as usize;
            if slot >= unsafe { (*m).slots } as usize || EXACT && start + slot * size != word {
                return;
            }
            // `slot` is below `slots`, so its word is within the bitmaps. Its mark is read first:
            // a marked slot is taken, and many words point to slots marked already, whose taken
            // bit, in another cache line, is then not read.
            let (w, bit) = (slot / 64, 1u64 << (slot % 64));
            let marks = unsafe { AtomicU64::from_ptr((&raw mut (*m).marks).cast::<u64>().add(w)) };
            let old = marks.load(Ordering::Relaxed);
            if old & bit != 0 || unsafe { *(&raw const (*m).taken).cast::<u64>().add(w) } & bit == 0 {
                return;
            }
            if SHARES {
                if marks.fetch_or(bit, Ordering::Relaxed) & bit != 0 {
                    return;
                }
            } else {
                marks.store(old | bit, Ordering::Relaxed);
            }
            let scanned = unsafe { (*m).scanned } as usize;
            if scanned > 0 {
                self.stack.push((start + slot * size, scanned));
            }
            return;
        }
        // Without this, the compiler tests the kind with a table of jumps, an indirect branch per
        // word, where a word that points into a small block, nearly every one, needs one test.
        std::hint::cold_path();
        if let Some(head) = self.large_head(word, b, kind) {
            // A 4-byte reference is to a large allocation's start (`allocate_large`).
            if EXACT
                && word
                    != self.base + (head << BLOCK_SHIFT) + unsafe { (*self.meta.add(head)).start } as usize
            {
                return;
            }
            self.mark_large::<SHARES>(head);
        }
    }

    /// The first block of the large allocation that `word`, in block `b` of kind `kind`, points
    /// into: below the start of a block's own allocation or room is the end of the large
    /// allocation that shares it (`Meta::tail_of`).
    #[inline(always)]
    fn large_head(&self, word: usize, b: usize, kind: u8) -> Option<usize> {
        let m = unsafe { self.meta.add(b) };
        let below = || word < self.base + (b << BLOCK_SHIFT) + unsafe { (*m).start } as usize;
        match kind {
            LARGE_REST => Some(unsafe { (*m).size } as usize),
            LARGE | TAIL | SMALL if unsafe { (*m).tail_of } != 0 && below() => {
                Some(unsafe { (*m).tail_of } as usize - 1)
            }
            LARGE => Some(b),
            _ => None,
        }
    }

    /// Marks the large allocation that starts in block `b`, if it is taken and not marked yet,
    /// and puts what to scan of it on the stack.
    #[inline(always)]
    fn mark_large<const SHARES: bool>(&mut self, b: usize) {
        let m = unsafe { self.meta.add(b) };
        if unsafe { (*m).taken[0] } & 1 == 0 || !set_mark(unsafe { &raw mut (*m).marks[0] }, 1, SHARES) {
            return;
        }
        let start = self.base + (b << BLOCK_SHIFT) + unsafe { (*m).start } as usize;
        match unsafe { (*m).scan } {
            SCAN_HEAD => self.stack.push((start, HEAD_BYTES)),
            SCAN_ALL => match unsafe { (*m).scanned } as usize {
                0 => self
                    .stack
                    .push((self.base + (b << BLOCK_SHIFT), (unsafe { (*m).size } as usize) << BLOCK_SHIFT)),
                bytes => self.stack.push((start, bytes)),
            },
            _ => {}
        }
    }

    /// Whether the taken slot that `word` points into is marked; `None` if it points into none.
    fn marked(&self, word: usize) -> Option<bool> {
        let word = word & crate::thin::ADDRESS;
        if word.wrapping_sub(self.base) >= self.end - self.base {
            return None;
        }
        let mut b = (word - self.base) >> BLOCK_SHIFT;
        let mut m = unsafe { self.meta.add(b) };
        let (w, bit) = match unsafe { (*m).kind } {
            SMALL
                if unsafe { (*m).tail_of } == 0
                    || word >= self.base + (b << BLOCK_SHIFT) + unsafe { (*m).start } as usize =>
            {
                let start = self.base + (b << BLOCK_SHIFT) + unsafe { (*m).start } as usize;
                if word < start {
                    return None;
                }
                let slot = (((word - start) as u64 * unsafe { (*m).magic }) >> 32) as usize;
                if slot >= unsafe { (*m).slots } as usize {
                    return None;
                }
                (slot / 64, 1u64 << (slot % 64))
            }
            kind => {
                b = self.large_head(word, b, kind)?;
                m = unsafe { self.meta.add(b) };
                (0, 1)
            }
        };
        if unsafe { (*m).taken[w] } & bit == 0 {
            return None;
        }
        Some(unsafe { (*m).marks[w] } & bit != 0)
    }

    /// Scans the words of `[from, to)` of a thread's stack or registers, inverted too, once the
    /// program has references (`references`): a thread stopped while it reads a referent
    /// (`java::WeakField`) may hold its word inverted.
    fn root_range(&mut self, from: usize, to: usize) {
        self.range(from, to);
        if references::USED.load(Ordering::Relaxed) {
            let mut p = (from + 7) & !7;
            while p + 8 <= to {
                self.candidate(!unsafe { (p as *const usize).read() });
                p += 8;
            }
        }
    }

    /// Scans the words of `[from, to)`.
    fn range(&mut self, from: usize, to: usize) {
        match (self.shares, self.compact) {
            (false, false) => self.scan::<false, false>(from, to),
            (false, true) => self.scan::<false, true>(from, to),
            (true, false) => self.scan::<true, false>(from, to),
            (true, true) => self.scan::<true, true>(from, to),
        }
    }

    /// Scans the words of `[from, to)`, with `SHARES` as `self.shares` and `COMPACT` as
    /// `self.compact`: also each half of each word as a 4-byte reference.
    #[inline(always)]
    fn scan<const SHARES: bool, const COMPACT: bool>(&mut self, from: usize, to: usize) {
        let mut p = (from + 7) & !7;
        while p + 8 <= to {
            let word = unsafe { (p as *const usize).read() };
            self.mark::<false, SHARES>(word);
            if COMPACT {
                self.mark::<true, SHARES>((word as u32 as usize) << 3);
                self.mark::<true, SHARES>((word >> 32) << 3);
            }
            p += 8;
        }
    }

    /// Scans everything on this marker's stack, offering work to idle markers now and then.
    ///
    /// A slot taken off the stack waits in a ring of `PREFETCH` slots while its memory is fetched
    /// (its first cache line and the line of its last word, as it goes in), and is scanned as it
    /// comes out: the misses of reading many slots overlap instead of following one another.
    fn drain(&mut self) {
        match (self.shares, self.compact) {
            (false, false) => self.drain_as::<false, false>(),
            (false, true) => self.drain_as::<false, true>(),
            (true, false) => self.drain_as::<true, false>(),
            (true, true) => self.drain_as::<true, true>(),
        }
    }

    /// `drain`, with `SHARES` as `self.shares` and `COMPACT` as `self.compact`, so that each slot
    /// is scanned inline, with neither tested per word.
    fn drain_as<const SHARES: bool, const COMPACT: bool>(&mut self) {
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
                self.scan::<SHARES, COMPACT>(start, start + len);
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
                use std::arch::x86_64::{_MM_HINT_T0, _mm_prefetch};
                _mm_prefetch::<_MM_HINT_T0>(start as *const i8);
                _mm_prefetch::<_MM_HINT_T0>((start + len.min(PREFETCH_LAST) - 1) as *const i8);
            };
            if held == PREFETCH {
                let (start, len) = ring[first];
                first = (first + 1) % PREFETCH;
                held -= 1;
                self.scan::<SHARES, COMPACT>(start, start + len);
            }
            ring[(first + held) % PREFETCH] = (start, len);
            held += 1;
            scanned += 1;
            if SHARES && scanned.is_multiple_of(64) {
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
        self.stack.move_oldest(n, pool);
        POOL.len.store(pool.count(), Ordering::Release);
        unlock_pool();
        // As many sleepers as the work feeds: each takes up to 256 items.
        WORK.fetch_add(1, Ordering::SeqCst);
        futex_wake_some(&WORK, n.div_ceil(256) as i32);
    }

    /// Takes work from the pool; whether there was any.
    fn take(&mut self) -> bool {
        if POOL.len.load(Ordering::Acquire) == 0 {
            return false;
        }
        lock_pool();
        let pool = unsafe { &mut *POOL.stack.get() };
        let n = pool.move_oldest(256, &mut self.stack);
        POOL.len.store(pool.count(), Ordering::Release);
        unlock_pool();
        n > 0
    }

    /// Marks until every marker is out of work. With `JAVA_GC_LOG`, adds the time this marker
    /// had no work to `IDLE_NANOS`.
    fn finish(&mut self) {
        let markers = MARKERS.load(Ordering::Acquire);
        let log = LOG_IDLE.load(Ordering::Relaxed);
        loop {
            self.drain();
            if self.take() {
                continue;
            }
            IDLE.fetch_add(1, Ordering::SeqCst);
            let idle_from = log.then(std::time::Instant::now);
            let idle_end = || {
                if let Some(from) = idle_from {
                    IDLE_NANOS.fetch_add(from.elapsed().as_nanos() as u64, Ordering::Relaxed);
                }
            };
            let mut spins = 0u32;
            loop {
                if POOL.len.load(Ordering::Acquire) > 0 {
                    // Not idle while taking, so that nobody sees every marker idle meanwhile.
                    IDLE.fetch_sub(1, Ordering::SeqCst);
                    if self.take() {
                        idle_end();
                        break;
                    }
                    IDLE.fetch_add(1, Ordering::SeqCst);
                }
                if IDLE.load(Ordering::SeqCst) == markers {
                    idle_end();
                    // The markers asleep see the end as they wake.
                    if self.shares {
                        WORK.fetch_add(1, Ordering::SeqCst);
                        futex_wake(&WORK);
                    }
                    return;
                }
                spins += 1;
                if spins < IDLE_SPINS {
                    std::hint::spin_loop();
                } else if spins < IDLE_SPINS + IDLE_YIELDS {
                    unsafe { sched_yield() };
                } else {
                    // The count is read before the pool and the idle markers, so that work
                    // offered or the end reached after them changes it, and the wait returns.
                    let seen = WORK.load(Ordering::SeqCst);
                    if POOL.len.load(Ordering::SeqCst) == 0 && IDLE.load(Ordering::SeqCst) != markers {
                        futex_wait(&WORK, seen);
                    }
                }
            }
        }
    }

    /// Marks a slot on a free list without scanning it.
    fn keep(&mut self, p: usize) {
        let b = (p - self.base) >> BLOCK_SHIFT;
        let m = unsafe { self.meta.add(b) };
        let start = self.base + (b << BLOCK_SHIFT) + unsafe { (*m).start } as usize;
        let slot = (((p - start) as u64 * unsafe { (*m).magic }) >> 32) as usize;
        set_mark(unsafe { &raw mut (*m).marks[slot / 64] }, 1u64 << (slot % 64), self.shares);
    }
}

/// Takes the pool's lock, waiting on a plain read of it, so that the waiters do not take the
/// lock's cache line from its holder over and over.
fn lock_pool() {
    while POOL.lock.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
        while POOL.lock.load(Ordering::Relaxed) {
            std::hint::spin_loop();
        }
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
fn scan_own_stack(marker: &mut Marker, own: &OwnStack, top: usize) {
    marker.root_range(own.from(), top);
}

/// Where the collecting thread's stack is scanned from: the callee-saved registers as `collect`
/// began, stored in its frame, and the frames above. The frames of the collection itself, below,
/// are left out: they reuse stack that the program's calls used before, and their unwritten words
/// still hold addresses from those calls, which would keep dead objects alive.
struct OwnStack {
    registers: [usize; 6],
}

impl OwnStack {
    #[inline(always)]
    fn here() -> OwnStack {
        let mut own = OwnStack { registers: [0; 6] };
        unsafe {
            std::arch::asm!(
                "mov [{0}], rbx", "mov [{0} + 8], rbp", "mov [{0} + 16], r12",
                "mov [{0} + 24], r13", "mov [{0} + 32], r14", "mov [{0} + 40], r15",
                in(reg) own.registers.as_mut_ptr(), options(nostack, preserves_flags),
            )
        };
        std::hint::black_box(&own.registers);
        own
    }

    fn from(&self) -> usize {
        self.registers.as_ptr() as usize
    }
}

/// A collection, with the lock held.
#[cold]
#[inline(never)]
fn collect(g: &Guard) {
    let own = OwnStack::here();
    if SPAWNING.load(Ordering::SeqCst) > 0 {
        // A thread started without an entry of `STARTING` has not registered, and the closure it
        // runs is only known to the C library: try again a little later.
        let h = heap(g);
        h.next_collection = h.taken + h.every / 4;
        return;
    }
    let me = ENTRY.try_with(|e| e.get()).unwrap_or(NO_ENTRY);
    references::tick();
    let started = std::time::Instant::now();
    let started_time = process_time();
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
    if MARK_STACK.load(Ordering::Relaxed).is_null() {
        MARK_STACK.store(MarkStack::reserved(MARK_STACK_ITEMS).items, Ordering::Relaxed);
        unsafe { *POOL.stack.get() = MarkStack::reserved(MARK_STACK_ITEMS) };
    }
    // A minor collection keeps the marks, which make the objects that lived through the last
    // collection old; a full one clears them first. Should a minor one leave as much alive as
    // starts a full one, the full one follows in the same pause, so that the old objects that
    // died do not decide when the next collection runs.
    let h = heap(g);
    let minor = h.pagemap >= 0 && h.alive < h.next_full && !h.protected_lost;
    if mark_and_sweep(g, me, &own, shares, minor, taken, started) && heap(g).alive >= heap(g).next_full {
        mark_and_sweep(g, me, &own, shares, false, taken, started);
    }
    std::hint::black_box(&own);
    let h = heap(g);
    if h.pagemap >= 0 {
        age(h);
        if !protect_old(h) {
            h.pagemap = -1;
        }
    }
    scale_by_time(heap(g), started_time);
    give_back(heap(g));
    if heap(g).stack_pages {
        give_stack_pages_back(me);
    }
    for t in &THREADS {
        t.stopped_sp.set(0);
        t.thread_pointer.set(0);
    }
    resume_others(stopped);
    let h = heap(g);
    if h.code_pages > 0 && monotonic_seconds() >= h.code_pages_at + h.code_pages_wait {
        h.code_pages_at = monotonic_seconds();
        h.code_pages_wait = (h.code_pages_wait * 2).min(h.code_pages);
        give_code_pages_back();
    }
}

/// Gives the memory of free blocks back to the system, beyond what the program takes before the
/// next collection: whole huge pages of free blocks, the highest first, since free blocks and
/// slots are taken lowest first. A huge page that one 64 KB block was cut out of would become
/// small pages, so a page with a block in use stays. Memory given back when a block emptied came
/// back at once, faulted in and zeroed by the system: BinaryTrees, whose trees die between
/// collections, spent 0.14 s of its 2.5 s in the system (with one marker), and up to 0.43 s
/// where the system had no huge pages to give.
fn give_back(h: &mut Heap) {
    const PER_PAGE: usize = HUGE_PAGE >> BLOCK_SHIFT;
    // The room the program takes from before the next collection: the untaken slots of small
    // blocks, the free blocks whose memory stays, and then whole free pages, lowest first.
    let mut room = 0;
    let mut whole = 0;
    for page in (0..h.top).step_by(PER_PAGE) {
        let mut free = 0;
        let mut dirty = 0;
        for b in page..page + PER_PAGE {
            let m = h.meta(b);
            match m.kind {
                FREE => {
                    free += 1;
                    dirty += m.dirty as usize;
                }
                SMALL => room += (m.slots - m.live) as usize * m.size as usize,
                TAIL => room += BLOCK - m.start as usize,
                _ => {}
            }
        }
        if free < PER_PAGE {
            room += dirty << BLOCK_SHIFT;
        } else if dirty > 0 {
            whole += 1;
        }
    }
    let needed = h.next_collection.saturating_sub(h.taken);
    let mut kept_pages = needed.saturating_sub(room).div_ceil(HUGE_PAGE);
    if whole <= kept_pages {
        return;
    }
    for page in (0..h.top).step_by(PER_PAGE) {
        if (page..page + PER_PAGE).any(|b| h.meta(b).kind != FREE)
            || !(page..page + PER_PAGE).any(|b| h.meta(b).dirty)
        {
            continue;
        }
        if kept_pages > 0 {
            kept_pages -= 1;
            continue;
        }
        unsafe { madvise(h.block_address(page) as *mut u8, HUGE_PAGE, MADV_DONTNEED) };
        (page..page + PER_PAGE).for_each(|b| h.meta(b).dirty = false);
    }
}

/// How far below the stack pointer a stopped thread recorded its stack is kept: the handler that
/// stopped it (`stopped`) goes on to wait on a futex and return, a few calls deeper.
const STACK_PAGES_KEPT: usize = 16 * PAGE;

/// Gives back to the system the pages of each stopped thread's stack below where it stopped:
/// nothing there is alive, and a page the thread reaches again comes back zeroed at its first
/// touch. A thread keeps every page it ever touched otherwise, so a deep call made once (a
/// recursion in world generation, a parser at startup) holds its pages for the rest of the run.
/// The threads are stopped, so none is running below its recorded stack pointer. The calling
/// thread's stack, and the process's first thread's (`stack_low` 0), are left as they are.
/// Pages not in memory cost the system nothing to give back, and a page given back costs a
/// fault of a zeroed page when the thread reaches it again, so this runs at every collection.
fn give_stack_pages_back(me: usize) {
    for (i, t) in THREADS.iter().enumerate() {
        let (low, sp) = (t.stack_low.get(), t.stopped_sp.get());
        if i == me || !t.used.load(Ordering::Acquire) || low == 0 || sp == 0 {
            continue;
        }
        let to = sp.saturating_sub(STACK_PAGES_KEPT) & !(PAGE - 1);
        if to > low {
            unsafe { madvise(low as *mut u8, to - low, MADV_DONTNEED) };
        }
    }
}

/// Gives the pages of the program's code and constants back to the system: they are the
/// executable's own file, unchanged (the loader writes only its writable segments), so a page
/// used again comes back from the file's cached pages. Startup runs code once that the program
/// never needs again, and those pages would otherwise stay resident.
fn give_code_pages_back() {
    extern "C" fn each(info: *const DlPhdrInfo, _size: usize, _data: *mut u8) -> i32 {
        let info = unsafe { &*info };
        for i in 0..info.phnum as usize {
            let ph = unsafe { &*info.phdr.add(i) };
            if ph.kind == PT_LOAD && ph.flags & PF_W == 0 {
                let from = (info.addr + ph.vaddr as usize).next_multiple_of(PAGE);
                let to = (info.addr + ph.vaddr as usize + ph.memsz as usize) & !(PAGE - 1);
                if from < to {
                    unsafe { madvise(from as *mut u8, to - from, MADV_DONTNEED) };
                }
            }
        }
        // The executable only: it is the first object.
        1
    }
    unsafe { dl_iterate_phdr(each, ptr::null_mut()) };
}

/// Scales the bytes taken before the next collection by the share of the processor time
/// collections take: the time this one took against the time the program's threads used since
/// the last one ended (`started`, `last_end`: `process_time`). Processor time rather than time
/// on the clock, so that a program whose threads take memory side by side is not counted as
/// collecting more often than it does per thread.
fn scale_by_time(h: &mut Heap, started: u64) {
    let end = process_time();
    let last = std::mem::replace(&mut h.last_end, end);
    if h.time_share == 0 || h.every < MIN_COLLECT {
        return;
    }
    if last != 0 {
        let pause = end.saturating_sub(started) as f64;
        let between = started.saturating_sub(last).max(1000) as f64;
        let ratio = (pause / between * 100.0 / h.time_share as f64).clamp(0.5, SCALE_STEP);
        // A small heap may grow to `SMALL_ROOM` between collections, past twice.
        let most = MAX_SCALE.max(SMALL_ROOM / (h.next_collection / 100).max(1));
        h.scale = ((h.scale as f64 * ratio) as usize).clamp(100, most);
    }
    h.next_collection = h.next_collection / 100 * h.scale;
}

/// Marks and sweeps, with the other threads stopped: a minor collection or a full one. Returns
/// whether it was minor (a minor one becomes full when the system stops answering).
fn mark_and_sweep(
    g: &Guard,
    me: usize,
    own: &OwnStack,
    shares: bool,
    minor: bool,
    taken: usize,
    started: std::time::Instant,
) -> bool {
    let h = heap(g);
    let mut minor = minor;
    if !minor {
        clear_marks(h);
    }
    let (base, end) = (h.base, h.block_address(h.top));
    let helpers = if shares { HELPERS.load(Ordering::Acquire) } else { 0 };
    MARKERS.store(helpers + 1, Ordering::Release);
    IDLE.store(0, Ordering::Release);
    IDLE_NANOS.store(0, Ordering::Relaxed);
    LOG_IDLE.store(h.log, Ordering::Relaxed);
    META.store(h.meta, Ordering::Release);
    if helpers > 0 {
        MARK_DONE.store(0, Ordering::Release);
        MARK_ROUND.fetch_add(1, Ordering::AcqRel);
        futex_wake(&MARK_ROUND);
    }
    let stack =
        MarkStack { items: MARK_STACK.load(Ordering::Relaxed), low: 0, len: 0, capacity: MARK_STACK_ITEMS };
    let mut marker = Marker {
        meta: h.meta,
        base,
        end,
        stack,
        shares: helpers > 0,
        compact: COMPACT.load(Ordering::Acquire),
    };
    // In a minor collection, the old objects on the pages written since the last collection
    // are scanned: they are where an old object may point to a young one. Should the system stop
    // answering, this and every later collection are full.
    if minor {
        let complete = {
            let h = &*h;
            written(h, |from, to| push_old(h, &mut marker, from, to))
                && (0..h.previous_count).all(|i| {
                    let (from, to) = unsafe { *h.previous.add(i) };
                    push_old(h, &mut marker, from, to);
                    true
                })
        };
        if !complete {
            h.pagemap = -1;
            marker.stack.low = 0;
            marker.stack.len = 0;
            clear_marks(h);
            minor = false;
        }
    }
    let log = h.log;
    let mut roots = [(0usize, 0u64); 3];
    // Scans a root with `scan`, counting its bytes and time in `roots[kind]` when logging.
    let mut timed = |kind: usize, bytes: usize, scan: &mut dyn FnMut()| {
        if !log {
            return scan();
        }
        let at = std::time::Instant::now();
        scan();
        roots[kind].0 += bytes;
        roots[kind].1 += at.elapsed().as_nanos() as u64;
    };
    let statics = statics();
    for &(from, to) in &statics.ranges[..statics.count] {
        timed(0, to - from, &mut || marker.range(from, to));
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
            // Not inverted: a load in progress is in a register or on the stack, not here.
            timed(1, tls, &mut || marker.range(tp - tls, tp));
            marker.drain();
        }
        if i == me {
            let top = t.stack_top.get();
            timed(2, top.saturating_sub(own.from()), &mut || scan_own_stack(&mut marker, own, top));
        } else if t.stopped_sp.get() != 0 {
            let (sp, top) = (t.stopped_sp.get(), t.stack_top.get());
            timed(2, top.saturating_sub(sp), &mut || marker.root_range(sp, top));
        }
        marker.drain();
    }
    heap(g).roots = roots;
    marker.finish();
    loop {
        let done = MARK_DONE.load(Ordering::Acquire);
        if helpers == 0 || done as usize >= helpers {
            break;
        }
        futex_wait(&MARK_DONE, done);
    }
    // The helpers are done: what references keep is marked by this thread alone.
    marker.shares = false;
    references::process(g, &mut marker, minor, heap(g).alive);
    let h = heap(g);
    keep_spans(h);
    let marked = started.elapsed();
    sweep(h, helpers);
    release_kept(h);
    let census = census::LEVEL.load(Ordering::Relaxed);
    if census > 0 && !minor {
        census::census(h, census);
    }
    h.minor = minor;
    if !minor {
        h.next_full = h.alive / 4 * 5 + (1 << 20);
    }
    h.collections += 1;
    if h.log {
        report(h, taken, marked, started.elapsed());
        if h.log_sizes && !minor {
            report_sizes(h);
        }
    }
    minor
}

/// Keeps the slots of each thread's spans not handed out yet and those it took beyond them
/// (`Lists::span_rest`), once marking is done, and the last one handed out, which a thread
/// stopped in `alloc` may have been handing out before its address was anywhere the marking
/// looks. A span that handed out slots to Rust code (`span_rust`) keeps all it handed out, until
/// the thread takes a new span: some are held only by memory the marking does not read, as a
/// thread's name is, which std keeps in the thread's `Thread`, allocated by the C library's
/// allocator. Spans of only objects keep only the last, so that what weak references refer to
/// dies. Marking scans each slot that something reaches the first time it marks it, which a mark
/// set here before would prevent.
fn keep_spans(h: &mut Heap) {
    for t in &THREADS {
        if !t.used.load(Ordering::Acquire) {
            continue;
        }
        let lists = unsafe { &*t.lists.get() };
        for class in 0..CLASSES {
            let start = lists.span_start[class].get();
            if start == 0 {
                continue;
            }
            let (next, end) = (lists.span_next[class].get(), lists.span_end[class].get());
            let size = SIZES[class / SCANS] as usize;
            let Some((m, first)) = span_slots(h, class, start) else {
                continue;
            };
            if next > start {
                let last = first + (next - start) / size - 1;
                let (w, bit) = (last / 64, 1u64 << (last % 64));
                let kept = m.taken[w] & bit != 0 && m.marks[w] & bit == 0;
                lists.span_kept[class].set(kept);
                m.marks[w] |= if kept { bit } else { 0 };
                if lists.span_rust[class].get() {
                    keep_taken(m, first, last - first);
                }
            }
            set_bits(&mut m.marks, first + (next - start) / size, (end - next) / size, true);
        }
        rest_marks(h, lists, true);
    }
}

/// Sets or clears the marks of the slots a thread took beyond its spans (`Lists::span_rest`).
fn rest_marks(h: &Heap, lists: &Lists, set: bool) {
    for class in 0..CLASSES {
        let rest = lists.span_rest[class].get();
        if rest == 0 {
            continue;
        }
        if let Some((m, first)) = span_slots(h, class, lists.span_rest_at[class].get()) {
            match set {
                true => m.marks[first / 64] |= rest,
                false => m.marks[first / 64] &= !rest,
            }
        }
    }
}

/// The description of the block of a span of `class` that starts at `start`, and the span's
/// first slot; `None` once the block holds other slots: a span all handed out whose objects all
/// died leaves its block free, to be used again.
fn span_slots<'m>(h: &Heap, class: usize, start: usize) -> Option<(&'m mut Meta, usize)> {
    let b = (start - h.base) >> BLOCK_SHIFT;
    let m = h.meta(b);
    if m.kind != SMALL || m.class as usize != class {
        return None;
    }
    let first = (start - h.block_address(b) - m.start as usize) / m.size as usize;
    Some((m, first))
}

/// Marks the taken slots among the `n` from slot `from`. Their marks stay until the next full
/// collection clears them, so in a minor one they count as old.
fn keep_taken(m: &mut Meta, from: usize, n: usize) {
    let mut at = from;
    while at < from + n {
        let len = (64 - at % 64).min(from + n - at);
        let mask = if len == 64 { u64::MAX } else { ((1u64 << len) - 1) << (at % 64) };
        m.marks[at / 64] |= m.taken[at / 64] & mask;
        at += len;
    }
}

/// Sets or clears `n` bits of a bitmap from bit `from`.
fn set_bits(bits: &mut [u64; BITMAP_WORDS], from: usize, n: usize, set: bool) {
    let mut at = from;
    while at < from + n {
        let len = (64 - at % 64).min(from + n - at);
        let mask = if len == 64 { u64::MAX } else { ((1u64 << len) - 1) << (at % 64) };
        if set {
            bits[at / 64] |= mask;
        } else {
            bits[at / 64] &= !mask;
        }
        at += len;
    }
}

/// Whether the page at `page` holds an old object that may hold pointers.
fn old_on(h: &Heap, page: usize) -> bool {
    let b = (page - h.base) >> BLOCK_SHIFT;
    let m = h.meta(b);
    match m.kind {
        SMALL if m.scan != SCAN_NONE => {
            let start = h.block_address(b) + m.start as usize;
            if page + PAGE <= start {
                return false;
            }
            let size = m.size as usize;
            let first = page.saturating_sub(start) / size;
            let last = ((page + PAGE - 1 - start) / size).min(m.slots as usize - 1);
            (first..=last).any(|slot| m.taken[slot / 64] & m.marks[slot / 64] & (1u64 << (slot % 64)) != 0)
        }
        LARGE | LARGE_REST => {
            let head = h.meta(if m.kind == LARGE { b } else { m.size as usize });
            head.scan != SCAN_NONE && head.marks[0] & 1 != 0
        }
        _ => false,
    }
}

/// Ages the objects that lived through this collection, with generations: one that was neither
/// old nor had lived through a collection before has now (`once`) and loses its mark, so that
/// it stays young until the next collection; one that had becomes old and keeps its mark, as
/// does an old one. Objects that die young therefore do not become old by being alive at one
/// collection: in BinaryTrees, the tree being built.
///
/// An old object may then point to one that lived through one collection. The pointer was
/// written after that object was made, so on a page written in the interval before the last
/// collection or since: the next minor collection scans, besides the pages written since, those
/// the last one protected (`Heap::previous`).
fn age(h: &mut Heap) {
    for b in 0..h.top {
        let m = h.meta(b);
        let Some(a) = h.ages(b) else { return };
        let words = match m.kind {
            SMALL => (m.slots as usize).div_ceil(64),
            LARGE => 1,
            _ => 0,
        };
        for w in 0..words {
            let reached = m.taken[w] & m.marks[w];
            let fresh = reached & !a.old[w] & !a.once[w];
            a.once[w] = fresh;
            m.marks[w] = reached & !fresh;
            a.old[w] = m.marks[w];
        }
    }
}

/// Protects the pages not protected that now hold old objects, in runs, so that a write to one
/// is reported at the next collection. Pages of young objects only stay writable, so that
/// allocation does not fault. False when the system refuses.
fn protect_old(h: &mut Heap) -> bool {
    let mut run: Option<(usize, usize)> = None;
    let mut refused = false;
    let (protected, mut count, mut lost) = (h.protected, 0, false);
    let complete = {
        let h = &*h;
        let mut protect = |from: usize, to: usize| {
            let mut range = [from as u64, (to - from) as u64, UFFDIO_WRITEPROTECT_MODE_WP];
            refused |= unsafe { ioctl(h.uffd, UFFDIO_WRITEPROTECT, range.as_mut_ptr()) } != 0;
            if count < PROTECTED_RUNS {
                unsafe { *protected.add(count) = (from, to) };
                count += 1;
            } else {
                lost = true;
            }
        };
        let complete = written(h, |from, to| {
            for page in (from..to).step_by(PAGE) {
                if old_on(h, page) {
                    run = match run {
                        Some((start, end)) if end == page => Some((start, page + PAGE)),
                        Some((start, end)) => {
                            protect(start, end);
                            Some((page, page + PAGE))
                        }
                        None => Some((page, page + PAGE)),
                    };
                }
            }
        });
        if let Some((start, end)) = run {
            protect(start, end);
        }
        complete
    };
    // What this collection protected, the next one scans as well as the pages written since.
    std::mem::swap(&mut h.previous, &mut h.protected);
    h.previous_count = count;
    h.protected_lost = lost;
    complete && !refused
}

/// Takes back the marks that the collection gave the slots threads hold, once the sweep has
/// kept them: those of the free lists (`Marker::keep`) and those `keep_spans` set. What a thread
/// makes in them is then young. A mark that marking set stays: its object is old, and what it
/// pointed to was marked through it.
fn release_kept(h: &mut Heap) {
    for t in &THREADS {
        if !t.used.load(Ordering::Acquire) {
            continue;
        }
        let lists = unsafe { &*t.lists.get() };
        for class in 0..CLASSES {
            let mut p = lists.heads[class].get() as usize;
            while p != 0 {
                let b = (p - h.base) >> BLOCK_SHIFT;
                let m = h.meta(b);
                let slot = (((p - h.block_address(b) - m.start as usize) as u64 * m.magic) >> 32) as usize;
                m.marks[slot / 64] &= !(1u64 << (slot % 64));
                p = unsafe { (p as *const usize).read() };
            }
            let start = lists.span_start[class].get();
            if start != 0
                && let Some((m, first)) = span_slots(h, class, start)
            {
                let (next, end) = (lists.span_next[class].get(), lists.span_end[class].get());
                let size = SIZES[class / SCANS] as usize;
                let unused = first + (next - start) / size;
                let kept = lists.span_kept[class].replace(false) as usize;
                set_bits(&mut m.marks, unused - kept, (end - next) / size + kept, false);
            }
        }
        rest_marks(h, lists, false);
    }
}

/// Clears every mark, for a full collection.
fn clear_marks(h: &mut Heap) {
    for b in 0..h.top {
        let m = h.meta(b);
        if m.kind == SMALL || m.kind == LARGE {
            m.marks = [0; BITMAP_WORDS];
        }
    }
}

/// Puts the old objects of `[from, to)`, a range of written pages, on the marker's stack: the
/// marked slots of the small blocks that may hold pointers that the range overlaps, and the part
/// of a marked large allocation within it. Their marks are set, so marking would not scan them.
fn push_old(h: &Heap, marker: &mut Marker, from: usize, to: usize) {
    let from = from.max(h.base);
    let to = to.min(h.block_address(h.top));
    let mut b = from.saturating_sub(h.base) >> BLOCK_SHIFT;
    while from < to && h.block_address(b) < to {
        let m = h.meta(b);
        match m.kind {
            SMALL if m.scan != SCAN_NONE => {
                let start = h.block_address(b) + m.start as usize;
                let size = m.size as usize;
                let scanned = scanned_bytes(m.scan, size);
                let (low, high) = (from.max(start), to.min(h.block_address(b + 1)));
                if low < high {
                    let first = (low - start) / size;
                    let last = ((high - 1 - start) / size).min(m.slots as usize - 1);
                    // A word of the bitmaps at a time, and only its old slots: the pages written
                    // since the last collection are mostly young objects, whose marks are clear.
                    for w in first / 64..=last / 64 {
                        let mut old = m.taken[w] & m.marks[w];
                        if w == first / 64 {
                            old &= u64::MAX << (first % 64);
                        }
                        if w == last / 64 && last % 64 < 63 {
                            old &= (1u64 << (last % 64 + 1)) - 1;
                        }
                        while old != 0 {
                            let slot = w * 64 + old.trailing_zeros() as usize;
                            old &= old - 1;
                            marker.stack.push((start + slot * size, scanned));
                        }
                    }
                }
                b += 1;
            }
            LARGE | LARGE_REST => {
                let head = if m.kind == LARGE { b } else { m.size as usize };
                let n = h.meta(head).size as usize;
                let (start, end) = (h.block_address(head), h.block_address(head + n));
                let hm = h.meta(head);
                if hm.scan != SCAN_NONE && hm.marks[0] & 1 != 0 {
                    // Of an array of numbers, only its header.
                    let (start, end) = match hm.scan {
                        SCAN_HEAD => (start + hm.start as usize, start + hm.start as usize + HEAD_BYTES),
                        _ => (start, end),
                    };
                    let (low, high) = (from.max(start), to.min(end));
                    if low < high {
                        marker.stack.push((low & !7, high - (low & !7)));
                    }
                }
                b = head + n;
            }
            _ => b += 1,
        }
    }
}

/// The spans the program's threads took since the last collection, those of them under the
/// heap's lock, and their slots; the counts start again.
fn span_counts() -> (u64, u64, u64) {
    let (mut spans, mut locked, mut slots) = (0, 0, 0);
    for t in &THREADS {
        if !t.used.load(Ordering::Acquire) {
            continue;
        }
        let lists = unsafe { &*t.lists.get() };
        spans += lists.spans.replace(0);
        locked += lists.spans_locked.replace(0);
        slots += lists.span_slots.replace(0);
    }
    (spans, locked, slots)
}

/// Writes a line about the collection that just ended to standard error, without allocating.
fn report(h: &Heap, taken: usize, marked: std::time::Duration, total: std::time::Duration) {
    use std::io::Write;
    let (spans, locked, slots) = span_counts();
    let mut buffer = [0u8; 512];
    let mut out = &mut buffer[..];
    let [statics, locals, stacks] = h.roots;
    let markers = MARKERS.load(Ordering::Relaxed);
    let idle =
        IDLE_NANOS.load(Ordering::Relaxed) as u128 * 100 / (marked.as_nanos() * markers as u128).max(1);
    let _ = writeln!(
        out,
        "[gc {} {}: took {} MB, {} MB alive, heap {} MB, mark {} ms, sweep {} ms, scale {}%, \
         spans {} ({} locked) of {:.1} slots, roots: statics {} KB {} us, thread-locals {} KB {} us, \
         stacks {} KB {} us, {} markers idle {}% of the marking]",
        h.collections,
        if h.minor { "minor" } else { "full" },
        taken >> 20,
        h.alive >> 20,
        (h.top << BLOCK_SHIFT) >> 20,
        marked.as_millis(),
        (total - marked).as_millis(),
        h.scale,
        spans,
        locked,
        slots as f64 / spans.max(1) as f64,
        statics.0 >> 10,
        statics.1 / 1000,
        locals.0 >> 10,
        locals.1 / 1000,
        stacks.0 >> 10,
        stacks.1 / 1000,
        markers,
        idle,
    );
    let len = 512 - out.len();
    unsafe { write(2, buffer.as_ptr().cast(), len) };
}

/// Writes the bytes alive in each slot size, and in large allocations, after a full collection,
/// with the blocks each takes, to standard error, without allocating.
fn report_sizes(h: &Heap) {
    use std::io::Write;
    let (mut alive, mut blocks) = ([0usize; CLASSES], [0usize; CLASSES]);
    let (mut large, mut large_blocks) = (0usize, 0usize);
    let mut b = 0;
    while b < h.top {
        let m = h.meta(b);
        match m.kind {
            SMALL => {
                alive[m.class as usize] += m.live as usize * m.size as usize;
                blocks[m.class as usize] += 1;
                b += 1;
            }
            LARGE => {
                let n = m.size as usize;
                if m.live > 0 {
                    large += n << BLOCK_SHIFT;
                    large_blocks += n;
                }
                // A shared last block is looked at on its own.
                b += n - m.shared_end as usize;
            }
            _ => b += 1,
        }
    }
    for class in 0..CLASSES {
        if blocks[class] == 0 {
            continue;
        }
        let mut buffer = [0u8; 128];
        let mut out = &mut buffer[..];
        let _ = writeln!(
            out,
            "[gc sizes {} bytes{}: {} KB alive in {} blocks]",
            SIZES[class / SCANS],
            match (class % SCANS) as Scan {
                SCAN_NONE => " no pointers",
                SCAN_HEAD => " pointers in the header",
                _ => "",
            },
            alive[class] >> 10,
            blocks[class]
        );
        let len = 128 - out.len();
        unsafe { write(2, buffer.as_ptr().cast(), len) };
    }
    let mut buffer = [0u8; 128];
    let mut out = &mut buffer[..];
    let _ = writeln!(out, "[gc sizes large: {} KB alive in {} blocks]", large >> 10, large_blocks);
    let len = 128 - out.len();
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
    // The blocks freed here go at the end of the runs, rebuilt in order below (`merge_runs`).
    h.runs_descending = false;
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
                if m.live == 0 && m.tail_of != 0 {
                    // The end of a large allocation that lives is in it: a tail again.
                    m.kind = TAIL;
                    (1, false)
                } else {
                    (1, m.live == 0)
                }
            }
            LARGE => {
                let n = m.size as usize;
                if m.live > 0 {
                    alive += n << BLOCK_SHIFT;
                }
                if m.live == 0 && (m.tail_of != 0 || m.shared_end) {
                    // Shares a block: freed on its own (`release_large`); its last block, if
                    // shared, is looked at next.
                    let next = b + n - m.shared_end as usize;
                    h.release_large(b);
                    b = next;
                    continue;
                }
                (n - m.shared_end as usize, m.live == 0 && !m.shared_end)
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
    // The list of tails, rebuilt: the lowest first.
    h.tails = NONE;
    for b in (0..h.top).rev() {
        h.meta(b).in_tails = false;
        if h.meta(b).kind == TAIL {
            h.push_tail(b);
        }
    }
    // Listed from the top down, a class's list hands out its lowest block first.
    if h.low_first {
        h.lists = [NONE; CLASSES];
        for b in (0..h.top).rev() {
            let m = h.meta(b);
            if m.kind == SMALL && m.live > 0 && m.untaken {
                m.listed = false;
                h.list(b);
            }
        }
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

/// Sweeps the block described by `m`: frees the dead slots of a small block and records what
/// lives (`live`, `untaken`). Returns the bytes alive in a small block.
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
    // The marks stay: an object that lives on is old (see "Generations" above).
    alive
}

/// Rebuilds the runs of free blocks from the blocks' kinds, merging neighbours, from the highest
/// to the lowest.
fn merge_runs(h: &mut Heap) {
    h.run_count = 0;
    let mut b = h.top;
    while b > 0 {
        if h.meta(b - 1).kind == FREE {
            let end = b;
            while b > 0 && h.meta(b - 1).kind == FREE {
                b -= 1;
            }
            unsafe { *h.runs.add(h.run_count) = (b as u32, (end - b) as u32) };
            h.run_count += 1;
        } else {
            b -= 1;
        }
    }
    h.runs_descending = true;
}

/// Runs a collection now, for tests (`System.gc()` does nothing, see `system::gc`). A collection
/// waits while a thread that found no entry of `STARTING` is starting (`collect`): so this tries
/// again, the lock released for the thread to register, until one ran.
pub fn collect_now() {
    loop {
        let g = lock();
        if heap(&g).base == 0 {
            return;
        }
        let before = heap(&g).collections;
        collect(&g);
        if heap(&g).collections != before {
            return;
        }
        drop(g);
        std::thread::yield_now();
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
        assert_eq!(size_index(17, 8).map(|i| SIZES[i]), Some(24));
        assert_eq!(size_index(17, 16).map(|i| SIZES[i]), Some(32));
        assert_eq!(size_index(24, 16).map(|i| SIZES[i]), Some(32));
        assert_eq!(size_index(121, 8).map(|i| SIZES[i]), Some(128));
        assert_eq!(size_index(129, 8).map(|i| SIZES[i]), Some(144));
        assert_eq!(size_index(145, 8).map(|i| SIZES[i]), Some(160));
        assert_eq!(size_index(1000, 8).map(|i| SIZES[i]), Some(1024));
        assert_eq!(size_index(1025, 8).map(|i| SIZES[i]), Some(1040));
        assert_eq!(size_index(1041, 8).map(|i| SIZES[i]), Some(1280));
        // A `long[256]` and its header.
        assert_eq!(size_index(2048 + 16, 16).map(|i| SIZES[i]), Some(2064));
        assert_eq!(size_index(16384 + 16, 8).map(|i| SIZES[i]), Some(20480));
        assert_eq!(size_index(32768, 8).map(|i| SIZES[i]), Some(32768));
        assert_eq!(size_index(32769, 8), None);
        for align in [1, 2, 4, 8, 16, 32, 64, 4096] {
            for size in 1..40000 {
                assert_eq!(size_index_of(size, align), size_index(size, align), "size {size}, align {align}");
            }
        }
        assert_eq!(size_index(40, 32).map(|i| SIZES[i]), Some(64));
    }
}
