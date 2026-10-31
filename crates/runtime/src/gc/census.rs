//! The census of the live heap (`JAVA_GC_CENSUS`): after each full collection, what the live
//! slots are and what their words hold, to measure where the live bytes go (per type, to compare
//! with a JVM's class histogram) and how much of them are references, and so what 4-byte
//! references would save (docs/native-speed.md, 1.6).
//!
//! **Words.** The heap knows sizes, not types, so a word of a slot that may hold pointers is
//! classified by its value and its neighbours:
//!
//! - **zero**: a null reference, or a number or flag that is 0;
//! - **thin**: a `java::Thin` of an object (a class number in the top 16 bits, an address of the
//!   start of a taken slot below): a reference to an object of a multi-class type;
//! - **thin value**: a class number above bits that are no object's address (an `int`, an enum
//!   constant held in the reference);
//! - **handle**: the address of the start of a taken slot, or of the first page of a large
//!   allocation, that is not the buffer of a `Vec` or a boxed slice: a `java::Arc`, a
//!   `java::JStr`, a `Box`;
//! - **buffer**: such an address in the shape of a `Vec` (capacity, pointer and length, the
//!   capacity at least the length and filling the slot it points to) or of a boxed slice (pointer
//!   and length, the length filling the slot): the elements of an array, a collection or a plain
//!   string, which an object owns rather than references;
//! - **interior**: an address in the heap that is not the start of a taken slot;
//! - **narrow**: a number below 2^32, such as an `int` with the 4 bytes of padding after it;
//! - **other**: anything else (wider numbers, addresses outside the heap).
//!
//! References are the thin and handle words; in a program with 4-byte references
//! (`java::CompactAllocator`), the halves of words that, times 8, are the start of a live slot are
//! counted besides, as the KB in 4-byte references. What 4-byte references save is estimated per slot,
//! as the slot's size against the size class that holds its size less what they free, three
//! ways (`saving`):
//!
//! - **references**: 4 bytes per reference, a low bound: a slot of 16 bytes with one reference
//!   and an `int` (a word of padding) saves nothing so;
//! - **paired**: also 4 bytes per narrow word, up to one per reference, as a 4-byte reference
//!   takes the padding beside an `int` (the slot of 16 bytes then takes 8);
//! - **with zeros**: also 4 bytes per zero word, a high bound: a null reference shrinks too, but
//!   a zero is as often a number, or the unused end of a slot.
//!
//! **Types.** With `JAVA_GC_CENSUS=3`, every allocation records the Rust type of what it holds,
//! from the program's start: `java::Arc::new` the type of the object (`gc::note_type`),
//! `java::JStr` its string, and every other allocation that it holds none (`note_untyped`), in a
//! table beside the heap of 2 bytes per 16 (`SHADOW`), a type's number in a table of type names
//! (`NAMES`). The census then counts each type's objects, and the buffers its objects own (the
//! slots their buffer words point to: the elements of an array, a `Vec`, a plain string), and
//! lists the slots of no type that no object owns by size. Off, the cost is one load and branch
//! per allocation.
//!
//! It reports, to standard error and without allocating:
//!
//! - a line per full collection with the totals (`JAVA_GC_CENSUS=1`);
//! - with `JAVA_GC_CENSUS=2`, also a line per slot size, and the classes (by class number) of the
//!   objects that thin references reach, those with the most bytes first, with their size now
//!   and with 4-byte references;
//! - with `JAVA_GC_CENSUS=3`, also the types, those with the most bytes first.

use super::*;

/// What a word of a live slot holds; see the module comment.
const ZERO: usize = 0;
const THIN: usize = 1;
const THIN_VALUE: usize = 2;
const HANDLE: usize = 3;
const BUFFER: usize = 4;
const INTERIOR: usize = 5;
const NARROW: usize = 6;
const OTHER: usize = 7;
const KINDS: usize = 8;

/// The classes of objects that thin references reach that the report lists, and the types.
const CLASSES_LISTED: usize = 60;
const TYPES_LISTED: usize = 150;

/// The most type names recorded; a type past them counts as the last.
pub(super) const MAX_TYPES: usize = 1 << 16;

/// The bytes of the heap per entry of `SHADOW`: every slot starts in an entry of its own, as
/// slots are 16 bytes or more.
pub(super) const GRANULE_SHIFT: usize = 4;

unsafe extern "C" {
    fn munmap(addr: *mut u8, len: usize) -> i32;
}

/// What the census reports: nothing (0), the totals (1), also the sizes and classes (2), also
/// the types (3), from `JAVA_GC_CENSUS`.
pub(super) static LEVEL: AtomicUsize = AtomicUsize::new(0);

/// The full collections so far, and the first whose census is reported (`JAVA_GC_CENSUS_FROM`).
static FULL: AtomicUsize = AtomicUsize::new(0);
pub(super) static FROM: AtomicUsize = AtomicUsize::new(0);

/// Whether allocations record their types (`JAVA_GC_CENSUS=3`).
pub(super) static TYPES: AtomicBool = AtomicBool::new(false);

/// Per 16 bytes of the heap's reservation, the number of the type of the slot that starts there
/// (0 for none).
static SHADOW: AtomicPtr<u16> = AtomicPtr::new(ptr::null_mut());
static SHADOW_BASE: AtomicUsize = AtomicUsize::new(0);

/// The type names, by number: the address of the name (a key, 0 for a free entry) and its length.
static NAMES: AtomicPtr<(AtomicUsize, AtomicUsize)> = AtomicPtr::new(ptr::null_mut());

/// Starts recording types, from the heap's start.
pub(super) fn start_types(base: usize) {
    SHADOW.store(reserve((HEAP_BYTES >> GRANULE_SHIFT) * 2) as *mut u16, Ordering::Relaxed);
    SHADOW_BASE.store(base, Ordering::Relaxed);
    NAMES.store(
        reserve(MAX_TYPES * std::mem::size_of::<(AtomicUsize, AtomicUsize)>()) as *mut _,
        Ordering::Relaxed,
    );
    TYPES.store(true, Ordering::Release);
}

/// The number of the type named `name`, which is registered the first time: an open table keyed
/// by the name's address (a type may have more than one copy of its name, one per crate, which the
/// report merges).
fn type_number(name: &'static str) -> u16 {
    let names = NAMES.load(Ordering::Relaxed);
    let key = name.as_ptr() as usize;
    let mut i = (key.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 48) & (MAX_TYPES - 1);
    for _ in 0..MAX_TYPES {
        // Number 0 is no type.
        if i != 0 {
            let (k, len) = unsafe { &*names.add(i) };
            match k.compare_exchange(0, key, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => {
                    len.store(name.len(), Ordering::Release);
                    return i as u16;
                }
                Err(found) if found == key => return i as u16,
                Err(_) => {}
            }
        }
        i = (i + 1) & (MAX_TYPES - 1);
    }
    (MAX_TYPES - 1) as u16
}

fn shadow_at(p: usize) -> Option<*mut u16> {
    let base = SHADOW_BASE.load(Ordering::Relaxed);
    let at = p.wrapping_sub(base);
    (at < HEAP_BYTES).then(|| unsafe { SHADOW.load(Ordering::Relaxed).add(at >> GRANULE_SHIFT) })
}

/// Records that the allocation at `p` holds a value of the type named `name`.
#[cold]
pub(crate) fn note(p: usize, name: &'static str) {
    if let Some(entry) = shadow_at(p) {
        unsafe { entry.write(type_number(name)) };
    }
}

/// Records that the allocation at `p` holds no type of its own (yet).
#[cold]
pub(super) fn note_untyped(p: usize) {
    if let Some(entry) = shadow_at(p) {
        unsafe { entry.write(0) };
    }
}

/// What the census counts, over a set of slots.
#[derive(Clone, Copy, Default)]
struct Counts {
    slots: u64,
    bytes: u64,
    words: [u64; KINDS],
    saved: u64,
    saved_paired: u64,
    saved_with_zeros: u64,
    /// The 4-byte references among the halves of the words, in a program that has them
    /// (`super::COMPACT`).
    compact: u64,
}

impl Counts {
    fn add(&mut self, other: &Counts) {
        self.slots += other.slots;
        self.bytes += other.bytes;
        for (total, n) in self.words.iter_mut().zip(other.words) {
            *total += n;
        }
        self.saved += other.saved;
        self.saved_paired += other.saved_paired;
        self.saved_with_zeros += other.saved_with_zeros;
        self.compact += other.compact;
    }

    fn references(&self) -> u64 {
        self.words[THIN] + self.words[HANDLE]
    }
}

/// What the census counts of a type: its objects, and the buffers they own.
#[derive(Clone, Copy, Default)]
struct TypeCounts {
    objects: Counts,
    buffers: Counts,
}

/// Where an address in the heap points.
#[derive(Clone, Copy)]
pub(super) enum Target {
    /// The start of a live small slot of `size` bytes, number `slot` of block `block`.
    Slot { block: usize, slot: usize, size: usize },
    /// The first page of a live large allocation of `blocks` blocks from `block`.
    Large { block: usize, blocks: usize },
    /// Anywhere else in the heap.
    Inside,
}

/// The positions of a `Vec`'s capacity, pointer and length among its three words, as this
/// compiler lays it out.
fn vec_layout() -> (usize, usize, usize) {
    // A `Vec` made of parts that are never used, so nothing is allocated or freed.
    let v = std::mem::ManuallyDrop::new(unsafe { Vec::from_raw_parts(0x1000 as *mut u64, 3, 7) });
    let words: [usize; 3] = unsafe { std::mem::transmute_copy(&*v) };
    let at = |value: usize| words.iter().position(|&w| w == value).unwrap_or(usize::MAX);
    (at(7), at(0x1000), at(3))
}

/// A bitmap with a bit per slot of the heap (`BITMAP_WORDS` words per block), outside the heap.
pub(super) struct Bits {
    words: *mut u64,
    bytes: usize,
}

impl Bits {
    pub(super) fn new(h: &Heap) -> Bits {
        let bytes = (h.top * BITMAP_WORDS * 8).max(PAGE);
        Bits { words: reserve(bytes) as *mut u64, bytes }
    }

    /// Sets the bit of `target`, returning whether it was clear.
    pub(super) fn set(&mut self, target: &Target) -> bool {
        let (block, slot) = match *target {
            Target::Slot { block, slot, .. } => (block, slot),
            Target::Large { block, .. } => (block, 0),
            Target::Inside => return false,
        };
        let word = unsafe { &mut *self.words.add(block * BITMAP_WORDS + slot / 64) };
        let bit = 1u64 << (slot % 64);
        let clear = *word & bit == 0;
        *word |= bit;
        clear
    }

    pub(super) fn get(&self, block: usize, slot: usize) -> bool {
        unsafe { *self.words.add(block * BITMAP_WORDS + slot / 64) & (1u64 << (slot % 64)) != 0 }
    }
}

impl Drop for Bits {
    fn drop(&mut self) {
        unsafe { munmap(self.words as *mut u8, self.bytes) };
    }
}

/// Memory outside the heap for `n` values of `T` that are zero at first.
pub(super) struct Table<T> {
    items: *mut T,
    n: usize,
}

impl<T> Table<T> {
    pub(super) fn new(n: usize) -> Table<T> {
        Table { items: reserve((n * std::mem::size_of::<T>()).max(PAGE)) as *mut T, n }
    }

    #[allow(clippy::mut_from_ref)]
    pub(super) fn all(&self) -> &mut [T] {
        unsafe { std::slice::from_raw_parts_mut(self.items, self.n) }
    }
}

impl<T> Drop for Table<T> {
    fn drop(&mut self) {
        unsafe { munmap(self.items as *mut u8, (self.n * std::mem::size_of::<T>()).max(PAGE)) };
    }
}

pub(super) struct Census<'h> {
    pub(super) h: &'h Heap,
    pub(super) end: usize,
    vec: (usize, usize, usize),
    /// The objects that thin references reach whose class has been counted.
    classes_seen: Bits,
    /// Per class number: the objects counted, when the classes are reported.
    classes: Option<Table<Counts>>,
}

impl Census<'_> {
    pub(super) fn target(&self, address: usize) -> Target {
        if address < self.h.base || address >= self.end {
            return Target::Inside;
        }
        let b = (address - self.h.base) >> BLOCK_SHIFT;
        let m = self.h.meta(b);
        match m.kind {
            SMALL => {
                let start = self.h.block_address(b) + m.start as usize;
                if address < start || !(address - start).is_multiple_of(m.size as usize) {
                    return Target::Inside;
                }
                let slot = (address - start) / m.size as usize;
                let (w, bit) = (slot / 64, 1u64 << (slot % 64));
                match slot < m.slots as usize && m.taken[w] & m.marks[w] & bit != 0 {
                    true => Target::Slot { block: b, slot, size: m.size as usize },
                    false => Target::Inside,
                }
            }
            // A large allocation starts within the first page of its first block (see `color`).
            LARGE if address - self.h.block_address(b) < PAGE && m.live > 0 => {
                Target::Large { block: b, blocks: m.size as usize }
            }
            _ => Target::Inside,
        }
    }

    /// Whether `count` elements of some size fill the target: its slot's size class is theirs, or
    /// its blocks are as many as they need.
    fn fits(count: usize, target: &Target) -> bool {
        if count == 0 || count > 1 << 40 {
            return false;
        }
        [1usize, 2, 4, 8, 16, 24, 32].iter().any(|&e| {
            let bytes = count * e;
            match *target {
                Target::Slot { size, .. } => bytes <= size && size_index(bytes, 8) == size_index(size, 8),
                Target::Large { blocks, .. } => {
                    bytes > LARGEST_SMALL && bytes <= blocks * BLOCK && bytes + BLOCK > blocks * BLOCK
                }
                Target::Inside => false,
            }
        })
    }

    /// Whether word `i` of `words`, which points to `target`, is the pointer of a `Vec` or of a
    /// boxed slice.
    pub(super) fn buffer(&self, words: &[usize], i: usize, target: &Target) -> bool {
        let (cap, ptr, len) = self.vec;
        if i >= ptr && i - ptr + 3 <= words.len() {
            let at = i - ptr;
            let (c, l) = (words[at + cap], words[at + len]);
            if c >= l && Census::fits(c, target) {
                return true;
            }
        }
        i + 1 < words.len() && Census::fits(words[i + 1], target)
    }

    /// Counts the words of the slot `words`, of a slot of `size` bytes (0 for a large
    /// allocation), into `counts`; counts the classes that its thin references reach when
    /// `classes` is set, and calls `owned` with each target of its buffer words.
    fn slot(
        &mut self,
        words: &[usize],
        size: usize,
        counts: &mut Counts,
        classes: bool,
        mut owned: impl FnMut(&mut Self, Target),
    ) {
        let mut kinds = [0u64; KINDS];
        if COMPACT.load(Ordering::Relaxed) {
            for &word in words {
                for half in [word as u32, (word >> 32) as u32] {
                    let target = self.target((half as usize) << 3);
                    counts.compact += matches!(target, Target::Slot { .. } | Target::Large { .. }) as u64;
                }
            }
        }
        for (i, &word) in words.iter().enumerate() {
            let kind = if word == 0 {
                ZERO
            } else if word >> 48 != 0 {
                let address = word & crate::thin::ADDRESS;
                match self.target(address) {
                    target @ (Target::Slot { .. } | Target::Large { .. }) => {
                        if classes {
                            self.class(word >> 48, &target);
                        }
                        THIN
                    }
                    Target::Inside => THIN_VALUE,
                }
            } else if word >= self.h.base && word < self.end {
                match self.target(word) {
                    target @ (Target::Slot { .. } | Target::Large { .. }) => {
                        match self.buffer(words, i, &target) {
                            true => {
                                owned(self, target);
                                BUFFER
                            }
                            false => HANDLE,
                        }
                    }
                    Target::Inside => INTERIOR,
                }
            } else if word >> 32 == 0 {
                NARROW
            } else {
                OTHER
            };
            kinds[kind] += 1;
        }
        let references = (kinds[THIN] + kinds[HANDLE]) as usize;
        counts.slots += 1;
        let bytes = if size == 0 { words.len() * 8 } else { size };
        counts.bytes += bytes as u64;
        for (total, n) in counts.words.iter_mut().zip(kinds) {
            *total += n;
        }
        let paired = references + (kinds[NARROW] as usize).min(references);
        counts.saved += saving(size, references * 4) as u64;
        counts.saved_paired += saving(size, paired * 4) as u64;
        counts.saved_with_zeros += saving(size, (paired + kinds[ZERO] as usize) * 4) as u64;
    }

    /// Counts a slot that cannot hold pointers: its bytes only.
    fn plain(counts: &mut Counts, bytes: usize) {
        counts.slots += 1;
        counts.bytes += bytes as u64;
    }

    /// Counts the object that a thin reference of class `class` reaches, the first time.
    fn class(&mut self, class: usize, target: &Target) {
        if self.classes.is_none() || !self.classes_seen.set(target) {
            return;
        }
        let (words, size, scanned) = self.words_of(target);
        let mut counts = Counts::default();
        match scanned {
            true => self.slot(words, size, &mut counts, false, |_, _| {}),
            false => Census::plain(&mut counts, words.len() * 8),
        }
        if let Some(classes) = &self.classes {
            classes.all()[class].add(&counts);
        }
    }

    /// The words of a target, its slot's size (0 for a large allocation), and whether they may
    /// hold pointers (an array of numbers counts as numbers, its header too).
    pub(super) fn words_of(&self, target: &Target) -> (&'static [usize], usize, bool) {
        match *target {
            Target::Slot { block, slot, size } => {
                let m = self.h.meta(block);
                let at = self.h.block_address(block) + m.start as usize + slot * size;
                (
                    unsafe { std::slice::from_raw_parts(at as *const usize, size / 8) },
                    size,
                    m.scan == SCAN_ALL,
                )
            }
            Target::Large { block, blocks } => {
                let at = self.h.block_address(block);
                let words = unsafe { std::slice::from_raw_parts(at as *const usize, blocks * BLOCK / 8) };
                (words, 0, self.h.meta(block).scan == SCAN_ALL)
            }
            Target::Inside => (&[], 0, false),
        }
    }

    /// The address where `target` starts.
    pub(super) fn start_of(&self, target: &Target) -> Option<usize> {
        match *target {
            Target::Slot { block, slot, size } => {
                Some(self.h.block_address(block) + self.h.meta(block).start as usize + slot * size)
            }
            Target::Large { block, .. } => {
                let (words, _, _) = self.words_of(target);
                let at = self.h.block_address(block);
                // A large allocation starts at the first entry of the shadow in its first page.
                let shadow = SHADOW.load(Ordering::Relaxed);
                (0..PAGE >> GRANULE_SHIFT)
                    .map(|g| at + (g << GRANULE_SHIFT))
                    .find(|&a| unsafe { *shadow.add((a - self.h.base) >> GRANULE_SHIFT) } != 0)
                    .or(Some(words.as_ptr() as usize))
            }
            Target::Inside => None,
        }
    }

    /// The type recorded for `target` (0 for none). A large allocation's entry is where it
    /// starts, within its first page.
    pub(super) fn type_of(&self, target: &Target) -> usize {
        let shadow = SHADOW.load(Ordering::Relaxed);
        let entry =
            |address: usize| unsafe { *shadow.add((address - self.h.base) >> GRANULE_SHIFT) } as usize;
        match *target {
            Target::Slot { block, slot, size } => {
                entry(self.h.block_address(block) + self.h.meta(block).start as usize + slot * size)
            }
            Target::Large { block, .. } => {
                let at = self.h.block_address(block);
                (0..PAGE >> GRANULE_SHIFT)
                    .map(|g| entry(at + (g << GRANULE_SHIFT)))
                    .find(|&t| t != 0)
                    .unwrap_or(0)
            }
            Target::Inside => 0,
        }
    }
}

/// The bytes saved when an object in a slot of `size` bytes (0 for a large allocation, whose
/// bytes shrink as they are) takes `less` bytes fewer: the slot's size less the size class that
/// then holds it, with a class of 8 bytes below the heap's smallest of 16.
fn saving(size: usize, less: usize) -> usize {
    if size == 0 {
        return less;
    }
    let wanted = size.saturating_sub(less).max(1);
    // Header-free objects of one or two 4-byte references would take slots of 8 bytes, a size
    // that the heap does not have now.
    let smaller = match wanted <= 8 {
        true => 8,
        false => size_index(wanted, 8).map_or(size, |c| SIZES[c] as usize),
    };
    size - smaller.min(size)
}

/// Every live slot and large allocation, in address order.
pub(super) fn each_live(h: &Heap, mut each: impl FnMut(Target)) {
    let mut b = 0;
    while b < h.top {
        let m = h.meta(b);
        match m.kind {
            SMALL => {
                for w in 0..(m.slots as usize).div_ceil(64) {
                    let mut live = m.taken[w] & m.marks[w];
                    while live != 0 {
                        let slot = w * 64 + live.trailing_zeros() as usize;
                        live &= live - 1;
                        each(Target::Slot { block: b, slot, size: m.size as usize });
                    }
                }
                b += 1;
            }
            LARGE => {
                let n = m.size as usize;
                if m.live > 0 {
                    each(Target::Large { block: b, blocks: n });
                }
                b += n;
            }
            _ => b += 1,
        }
    }
}

/// Counts the words of every live slot that may hold pointers, after a full collection (the
/// marks are those of the live slots), and reports them, at `level` (see `LEVEL`).
pub(super) fn census(h: &Heap, level: usize) {
    let n = FULL.fetch_add(1, Ordering::Relaxed) + 1;
    if n < FROM.load(Ordering::Relaxed) {
        return;
    }
    let mut census = Census {
        h,
        end: h.block_address(h.top),
        vec: vec_layout(),
        classes_seen: Bits::new(h),
        classes: (level >= 2).then(|| Table::new(65536)),
    };
    let mut by_size = [Counts::default(); SIZE_COUNT];
    let mut large = Counts::default();
    let mut unscanned = 0u64;
    each_live(h, |target| {
        let (words, size, scanned) = census.words_of(&target);
        match (scanned, target) {
            (false, _) => unscanned += words.len() as u64 * 8,
            (true, Target::Slot { block, .. }) => {
                let counts = &mut by_size[h.meta(block).class as usize / SCANS];
                census.slot(words, size, counts, level >= 2, |_, _| {})
            }
            (true, _) => census.slot(words, size, &mut large, level >= 2, |_, _| {}),
        }
    });
    let mut all = large;
    for counts in &by_size {
        all.add(counts);
    }
    line(format_args!(
        "[gc census: {} MB alive, {} MB in slots that may hold pointers ({} slots), {} MB that cannot]",
        (all.bytes + unscanned) >> 20,
        all.bytes >> 20,
        all.slots,
        unscanned >> 20
    ));
    report_counts(format_args!("all"), &all);
    lookalikes(&census);
    if level >= 2 {
        for (i, counts) in by_size.iter().enumerate() {
            if counts.slots > 0 {
                report_counts(format_args!("{} bytes", SIZES[i]), counts);
            }
        }
        if large.slots > 0 {
            report_counts(format_args!("large"), &large);
        }
        if let Some(classes) = &census.classes {
            report_classes(classes.all());
        }
    }
    if level >= 3 && TYPES.load(Ordering::Acquire) {
        census_types(&mut census);
        if level >= 4 {
            super::who::who(&census);
        }
    }
}

/// Where the heap would start in the reservation of a heap whose 4-byte references are addresses
/// divided by 8 (`lookalikes`).
const PLACEMENTS: [usize; 3] = [1 << 30, 4 << 30, 16 << 30];

/// Counts the 4-byte halves of the words of live slots that are not addresses in the heap and
/// that would read as a 4-byte reference to a live slot, were the heap placed so that its 4-byte
/// references are addresses divided by 8, from each of `PLACEMENTS`: the numbers that a scan of
/// slots for 4-byte references, without knowing their types, would take for references. This
/// census sees only live slots, so the count is a rate (lookalikes per half), which dead slots
/// share: each lookalike that hits a dead slot keeps it, and what it reaches, alive.
fn lookalikes(census: &Census) {
    let h = census.h;
    let span = census.end - h.base;
    let (mut halves, mut hits) = (0u64, [0u64; 3]);
    each_live(h, |target| {
        let (words, _, scanned) = census.words_of(&target);
        if !scanned {
            return;
        }
        for &word in words {
            if word == 0 || (word & crate::thin::ADDRESS).wrapping_sub(h.base) < span {
                continue;
            }
            for half in [word & 0xFFFF_FFFF, word >> 32] {
                halves += 1;
                let address = half << 3;
                for (i, &start) in PLACEMENTS.iter().enumerate() {
                    if address.wrapping_sub(start) < span
                        && !matches!(census.target(h.base + (address - start)), Target::Inside)
                    {
                        hits[i] += 1;
                    }
                }
            }
        }
    });
    line(format_args!(
        "[gc census lookalikes: of {} halves of words that are not addresses, {}, {} and {} read as a 4-byte reference to a live slot with the heap from 1, 4 and 16 GB]",
        halves, hits[0], hits[1], hits[2]
    ));
}

/// Counts each type's objects and the buffers they own, and the slots of no type that no object
/// owns, by size, and reports them.
fn census_types(census: &mut Census) {
    let h = census.h;
    let types: Table<TypeCounts> = Table::new(MAX_TYPES);
    let mut owned = Bits::new(h);
    // Each typed slot, and the buffers it owns, the first time one is reached.
    each_live(h, |target| {
        let t = census.type_of(&target);
        if t == 0 {
            return;
        }
        let (words, size, scanned) = census.words_of(&target);
        let counts = &mut types.all()[t];
        if !scanned {
            Census::plain(&mut counts.objects, words.len() * 8);
            return;
        }
        let mut buffers = Counts::default();
        census.slot(words, size, &mut counts.objects, false, |census, buffer| {
            if census.type_of(&buffer) != 0 || !owned.set(&buffer) {
                return;
            }
            let (words, size, scanned) = census.words_of(&buffer);
            match scanned {
                true => census.slot(words, size, &mut buffers, false, |_, _| {}),
                false => Census::plain(&mut buffers, words.len() * 8),
            }
        });
        counts.buffers.add(&buffers);
    });
    // The slots of no type that no typed object owns, by size.
    let mut untyped = [[Counts::default(); 2]; SIZE_COUNT];
    let mut untyped_large = [Counts::default(); 2];
    each_live(h, |target| {
        if census.type_of(&target) != 0 {
            return;
        }
        let (block, slot) = match target {
            Target::Slot { block, slot, .. } => (block, slot),
            Target::Large { block, .. } => (block, 0),
            Target::Inside => return,
        };
        if owned.get(block, slot) {
            return;
        }
        let (words, size, scanned) = census.words_of(&target);
        let counts = match target {
            Target::Slot { .. } => &mut untyped[h.meta(block).class as usize / SCANS][!scanned as usize],
            _ => &mut untyped_large[!scanned as usize],
        };
        match scanned {
            true => census.slot(words, size, counts, false, |_, _| {}),
            false => Census::plain(counts, words.len() * 8),
        }
    });
    report_types(&types);
    for (i, pair) in untyped.iter().enumerate() {
        for (no_scan, counts) in pair.iter().enumerate() {
            if counts.slots > 0 {
                let kind = if no_scan == 1 { " no pointers" } else { "" };
                report_counts(format_args!("untyped {} bytes{kind}", SIZES[i]), counts);
            }
        }
    }
    for (no_scan, counts) in untyped_large.iter().enumerate() {
        if counts.slots > 0 {
            let kind = if no_scan == 1 { " no pointers" } else { "" };
            report_counts(format_args!("untyped large{kind}"), counts);
        }
    }
}

/// The name of type `t`.
pub(super) fn type_name(t: usize) -> &'static str {
    let names = NAMES.load(Ordering::Acquire);
    let (key, len) = unsafe { &*names.add(t) };
    let (key, len) = (key.load(Ordering::Acquire), len.load(Ordering::Acquire));
    if key == 0 {
        return "(no name)";
    }
    unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(key as *const u8, len)) }
}

/// Lists the types, those with the most bytes (objects and buffers) first, merging the numbers of
/// one type whose name has more than one copy.
fn report_types(types: &Table<TypeCounts>) {
    let types = types.all();
    let order: Table<u32> = Table::new(MAX_TYPES);
    let order = order.all();
    let mut n = 0;
    for (t, counts) in types.iter().enumerate() {
        if counts.objects.slots > 0 {
            order[n] = t as u32;
            n += 1;
        }
    }
    let order = &mut order[..n];
    order.sort_unstable_by(|&a, &b| type_name(a as usize).cmp(type_name(b as usize)));
    for i in 1..n {
        let (first, this) = (order[i - 1] as usize, order[i] as usize);
        if type_name(first) == type_name(this) {
            let counts = types[first];
            types[this].objects.add(&counts.objects);
            types[this].buffers.add(&counts.buffers);
            types[first] = TypeCounts::default();
        }
    }
    let total = |t: &TypeCounts| t.objects.bytes + t.buffers.bytes;
    order.sort_unstable_by_key(|&t| std::cmp::Reverse(total(&types[t as usize])));
    let all = types.iter().fold(TypeCounts::default(), |mut all, t| {
        all.objects.add(&t.objects);
        all.buffers.add(&t.buffers);
        all
    });
    line(format_args!(
        "[gc census types: {} types, {} objects of {} KB, owning {} buffers of {} KB]",
        order.iter().filter(|&&t| types[t as usize].objects.slots > 0).count(),
        all.objects.slots,
        all.objects.bytes >> 10,
        all.buffers.slots,
        all.buffers.bytes >> 10
    ));
    for &t in order.iter().take(TYPES_LISTED) {
        let c = types[t as usize];
        if c.objects.slots == 0 {
            break;
        }
        let mut both = c.objects;
        both.add(&c.buffers);
        line(format_args!(
            "[gc census type {} KB: {} objects of {} KB, {} buffers of {} KB; references {} KB; 4-byte references save {} KB, {} KB paired, {} KB with zeros: {}]",
            total(&c) >> 10,
            c.objects.slots,
            c.objects.bytes >> 10,
            c.buffers.slots,
            c.buffers.bytes >> 10,
            (both.references() * 8) >> 10,
            both.saved >> 10,
            both.saved_paired >> 10,
            both.saved_with_zeros >> 10,
            Short(type_name(t as usize))
        ));
    }
}

/// A Rust type's name without its paths: `ArrayData<PlainHeader<i32>, i32>`.
pub(super) struct Short(pub(super) &'static str);

impl std::fmt::Display for Short {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let bytes = self.0.as_bytes();
        let mut start = 0;
        let mut i = 0;
        // Each identifier followed by `::` is left out.
        while i < bytes.len() {
            if bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' {
                let from = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                if bytes[i..].starts_with(b"::") {
                    f.write_str(&self.0[start..from])?;
                    i += 2;
                    start = i;
                }
            } else {
                i += 1;
            }
        }
        f.write_str(&self.0[start..])
    }
}

fn report_counts(name: std::fmt::Arguments, c: &Counts) {
    let w = &c.words;
    let kb = |words: u64| (words * 8) >> 10;
    let tenths = c.references() * 8 * 1000 / c.bytes.max(1);
    line(format_args!(
        "[gc census {name}: {} slots, {} KB; words in KB: {} zero, {} thin, {} thin value, {} handle, {} buffer, {} interior, {} narrow, {} other; references {} KB ({}.{}%); 4-byte references save {} KB, {} KB paired, {} KB with zeros; {} KB in 4-byte references]",
        c.slots,
        c.bytes >> 10,
        kb(w[ZERO]),
        kb(w[THIN]),
        kb(w[THIN_VALUE]),
        kb(w[HANDLE]),
        kb(w[BUFFER]),
        kb(w[INTERIOR]),
        kb(w[NARROW]),
        kb(w[OTHER]),
        kb(c.references()),
        tenths / 10,
        tenths % 10,
        c.saved >> 10,
        c.saved_paired >> 10,
        c.saved_with_zeros >> 10,
        (c.compact * 4) >> 10
    ));
}

/// Lists the classes whose objects thin references reach, those with the most bytes first.
fn report_classes(classes: &mut [Counts]) {
    let total: u64 = classes.iter().map(|c| c.bytes).sum();
    let count = classes.iter().filter(|c| c.slots > 0).count();
    line(format_args!(
        "[gc census by class: {} classes, {} KB of objects reached by thin references]",
        count,
        total >> 10
    ));
    for _ in 0..CLASSES_LISTED {
        let Some((number, c)) =
            classes.iter().enumerate().filter(|(_, c)| c.slots > 0).max_by_key(|(_, c)| c.bytes)
        else {
            break;
        };
        let c = *c;
        let per = |n: u64| n as f64 / c.slots as f64;
        line(format_args!(
            "[gc census class {number}: {} objects, {} KB, {:.1} bytes each, {:.2} references, {:.2} narrow and {:.2} zero words each; with 4-byte references {:.1} bytes each, {:.1} paired, {:.1} with zeros]",
            c.slots,
            c.bytes >> 10,
            per(c.bytes),
            per(c.references()),
            per(c.words[NARROW]),
            per(c.words[ZERO]),
            per(c.bytes - c.saved),
            per(c.bytes - c.saved_paired),
            per(c.bytes - c.saved_with_zeros)
        ));
        classes[number].slots = 0;
    }
}

/// Writes a line to standard error, without allocating; a longer line is cut.
pub(super) fn line(args: std::fmt::Arguments) {
    const LONGEST: usize = 1024;
    let mut buffer = [0u8; LONGEST];
    let len = {
        let mut out = &mut buffer[..LONGEST - 1];
        let _ = std::io::Write::write_fmt(&mut out, args);
        LONGEST - 1 - out.len()
    };
    buffer[len] = b'\n';
    unsafe { write(2, buffer.as_ptr().cast(), len + 1) };
}
