//! Soft, weak and phantom references (docs/collector.md, "References"): the registry of the
//! references whose referents the collector clears, and what each collection does with them.
//!
//! A reference registers itself as its constructor ends (`java::register_reference`), under the
//! heap's lock, so no collection runs meanwhile: its handle's word, the address of its referent's
//! cell (`java::WeakField`), the cell's word for null, its kind, and whether it has a queue.
//! After marking, before the sweep, a collection (`process`):
//!
//! 1. marks the references that wait to be enqueued (`PENDING`), which nothing else may hold;
//! 2. keeps the soft references' referents that the policy keeps, and marks what they reach,
//!    again until no more are kept: what a kept soft referent reaches is softly reachable, so a
//!    weak reference to it is not cleared in step 3;
//! 3. forgets the references that are not marked, which die, and those whose referent is null;
//!    clears the cell of every other reference whose referent is not marked, adding the reference
//!    to `PENDING` if it has a queue, for the program to put on it (`take_pending`), and forgets
//!    it too, as nothing sets a referent again.
//!
//! In a minor collection, the old objects keep their marks, so only young referents are cleared,
//! and every soft referent is kept. The soft policy is HotSpot's: at a full collection, a soft
//! referent that nothing else keeps is cleared when the live heap is past `JAVA_GC_SOFT_LIMIT`
//! MB (a quarter of the machine's memory by default), or when it was last read longer ago than
//! `JAVA_GC_SOFT_MS_PER_MB` (1000 by default) milliseconds per MB that the live heap is below
//! that limit. The clock is `CLOCK`, the milliseconds the collector last read, which
//! `WeakField::get` stores in the cell.

use super::*;

/// The collector's clock: the milliseconds from the heap's start to the start of the last
/// collection.
pub static CLOCK: AtomicU64 = AtomicU64::new(0);

/// Whether any reference was registered, which has the collector read the roots inverted too.
pub(super) static USED: AtomicBool = AtomicBool::new(false);

const SOFT: u32 = 1;
const WEAK: u32 = 2;
const PHANTOM: u32 = 3;
/// The bit of an entry's kind for a reference with a queue.
const QUEUED: u32 = 8;

/// A registered reference.
#[derive(Clone, Copy)]
#[repr(C)]
struct Entry {
    reference: u64,
    cell: usize,
    null: u64,
    kind: u32,
}

/// The most references registered at once, and the most pending.
const MOST: usize = 1 << 25;

/// The registry and the pending references, outside the heap, under the heap's lock.
struct References {
    entries: *mut Entry,
    len: usize,
    pending: *mut u64,
    pending_len: usize,
}

struct Shared(std::cell::UnsafeCell<References>);

// Only used under the heap's lock.
unsafe impl Sync for Shared {}

static REFERENCES: Shared = Shared(std::cell::UnsafeCell::new(References {
    entries: ptr::null_mut(),
    len: 0,
    pending: ptr::null_mut(),
    pending_len: 0,
}));

/// Counts the references added to `PENDING`, for the reference handler thread to wait on.
static PENDING_SIGNAL: AtomicU32 = AtomicU32::new(0);
static PENDING_COUNT: AtomicUsize = AtomicUsize::new(0);

/// The references, while the heap's lock is held.
#[allow(clippy::mut_from_ref)]
fn references(_: &Guard) -> &mut References {
    let r = unsafe { &mut *REFERENCES.0.get() };
    if r.entries.is_null() {
        r.entries = reserve(MOST * std::mem::size_of::<Entry>()) as *mut Entry;
        r.pending = reserve(MOST * 8) as *mut u64;
    }
    r
}

/// Registers a reference (`java::register_reference`).
pub(crate) fn register(reference: u64, cell: usize, null: u64, kind: i32, queued: bool) {
    let kind = kind as u32;
    if !matches!(kind, SOFT | WEAK | PHANTOM) {
        return;
    }
    let kind = if queued { kind | QUEUED } else { kind };
    let g = lock();
    let h = heap(&g);
    let address = reference as usize & crate::thin::ADDRESS;
    if address.wrapping_sub(h.base) >= HEAP_BYTES {
        return;
    }
    let r = references(&g);
    if r.len == MOST {
        // Past the most, a reference keeps its referent, as before.
        return;
    }
    unsafe { r.entries.add(r.len).write(Entry { reference, cell, null, kind }) };
    r.len += 1;
    USED.store(true, Ordering::Release);
}

/// The word of a pending reference, the latest added, if there is one
/// (`java::take_pending_reference`). Without one, the heap's lock is not taken.
pub(crate) fn take_pending() -> Option<u64> {
    if !has_pending_references() {
        return None;
    }
    let g = lock();
    let r = references(&g);
    if r.pending_len == 0 {
        return None;
    }
    r.pending_len -= 1;
    PENDING_COUNT.store(r.pending_len, Ordering::Release);
    Some(unsafe { *r.pending.add(r.pending_len) })
}

/// Whether a reference is pending (`Reference.hasReferencePendingList()`).
pub fn has_pending_references() -> bool {
    PENDING_COUNT.load(Ordering::Acquire) > 0
}

/// `Reference.waitForReferencePendingList()`: waits until a reference is pending.
pub fn wait_for_pending_references() {
    loop {
        let seen = PENDING_SIGNAL.load(Ordering::Acquire);
        if has_pending_references() {
            return;
        }
        futex_wait(&PENDING_SIGNAL, seen);
    }
}

/// The soft policy's settings: the limit in bytes, and the milliseconds per MB below it.
fn soft_policy() -> (usize, u64) {
    static POLICY: std::sync::OnceLock<(usize, u64)> = std::sync::OnceLock::new();
    *POLICY.get_or_init(|| {
        let memory = crate::vm::os_total_memory().max(0) as usize;
        let limit = number_from(c"JAVA_GC_SOFT_LIMIT").map_or(memory / 4, |mb| mb << 20);
        let ms = number_from(c"JAVA_GC_SOFT_MS_PER_MB").unwrap_or(1000) as u64;
        (limit, ms)
    })
}

/// The monotonic milliseconds when the heap started, from which `CLOCK` counts.
static STARTED: AtomicU64 = AtomicU64::new(0);

fn monotonic_ms() -> u64 {
    let mut t = [0i64; 2];
    unsafe { clock_gettime(1, &mut t) };
    t[0] as u64 * 1000 + t[1] as u64 / 1_000_000
}

/// Starts the clock, with the heap.
pub(super) fn start_clock() {
    STARTED.store(monotonic_ms(), Ordering::Relaxed);
}

/// Sets the clock, at the start of a collection.
pub(super) fn tick() {
    CLOCK.store(monotonic_ms().saturating_sub(STARTED.load(Ordering::Relaxed)), Ordering::Relaxed);
}

/// The referent's address in a cell's word, if it is a slot of the heap that is not marked
/// (`None` for null, a value held in the reference, or anything outside the heap).
fn unmarked_referent(marker: &Marker, cell: usize, null: u64) -> Option<usize> {
    let word = unsafe { (*(cell as *const AtomicU64)).load(Ordering::Relaxed) };
    if word == null || word == 0 {
        return None;
    }
    let referent = (!word) as usize & crate::thin::ADDRESS;
    (marker.marked(referent) == Some(false)).then_some(referent)
}

/// What a collection does with the references, after marking and before the sweep (see the
/// module comment): `alive` is the bytes alive after the last collection.
pub(super) fn process(g: &Guard, marker: &mut Marker, minor: bool, alive: usize) {
    if !USED.load(Ordering::Acquire) {
        return;
    }
    let r = references(g);
    // 1. The pending references stay until they are enqueued.
    for i in 0..r.pending_len {
        marker.candidate(unsafe { *r.pending.add(i) } as usize);
    }
    marker.drain();
    // 2. Soft references first, so that what a kept soft referent reaches is not cleared through
    // a weak reference in step 3.
    let (limit, ms_per_mb) = soft_policy();
    let clock = CLOCK.load(Ordering::Relaxed);
    let free_mb = (limit.saturating_sub(alive) >> 20) as u64;
    loop {
        let mut kept = false;
        for i in 0..r.len {
            let e = unsafe { *r.entries.add(i) };
            if e.kind & !QUEUED != SOFT
                || marker.marked(e.reference as usize & crate::thin::ADDRESS) != Some(true)
            {
                continue;
            }
            let Some(referent) = unmarked_referent(marker, e.cell, e.null) else { continue };
            let read = unsafe { (*((e.cell + 8) as *const AtomicU64)).load(Ordering::Relaxed) };
            let keep = minor || (alive <= limit && clock.saturating_sub(read) <= free_mb * ms_per_mb);
            if keep {
                marker.candidate(referent);
                marker.drain();
                kept = true;
            }
        }
        if !kept {
            break;
        }
    }
    // 3. The references that died are forgotten; the others' referents that nothing keeps are
    // cleared, and the references wait to be enqueued.
    let mut live = 0;
    let mut added = 0;
    for i in 0..r.len {
        let e = unsafe { *r.entries.add(i) };
        if marker.marked(e.reference as usize & crate::thin::ADDRESS) != Some(true) {
            continue;
        }
        let word = unsafe { (*(e.cell as *const AtomicU64)).load(Ordering::Relaxed) };
        if word == e.null || word == 0 {
            // Cleared by the program: nothing sets a referent again.
            continue;
        }
        if unmarked_referent(marker, e.cell, e.null).is_some() {
            unsafe { (*(e.cell as *const AtomicU64)).store(e.null, Ordering::Relaxed) };
            if e.kind & QUEUED != 0 && r.pending_len < MOST {
                unsafe { r.pending.add(r.pending_len).write(e.reference) };
                r.pending_len += 1;
                added += 1;
            }
            continue;
        }
        unsafe { r.entries.add(live).write(e) };
        live += 1;
    }
    r.len = live;
    if added > 0 {
        PENDING_COUNT.store(r.pending_len, Ordering::Release);
        PENDING_SIGNAL.fetch_add(1, Ordering::AcqRel);
        futex_wake(&PENDING_SIGNAL);
    }
}
