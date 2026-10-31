//! What holds the objects of chosen types alive (`JAVA_GC_CENSUS=4`, census.rs): for each type
//! that `JAVA_GC_CENSUS_WHO` names, after the census of types, the slots whose words point to its
//! objects, by the holder's type (an object, the buffer that an object of a type owns, or a slot
//! of no type), the objects that no slot holds and that the roots therefore keep (a thread's
//! stack, static data, thread-local storage), and for two of them a chain of holders back to a
//! root.
//!
//! `JAVA_GC_CENSUS_WHO` is a comma-separated list of up to 8 parts of type names
//! (`StringTag,ListTag`), or `untyped:<size>` for the slots of `size` bytes of no type that no
//! object owns. Each step of a chain is a pass over the heap, so a census at this level takes
//! seconds; `JAVA_GC_CENSUS_FROM=<n>` skips the census of the full collections before the nth.

use super::census::{Bits, Census, GRANULE_SHIFT, MAX_TYPES, Short, Table, Target, each_live, type_name};
use super::*;

const MAX_SELECTED: usize = 8;
const HOLDERS_LISTED: usize = 10;
const CHAIN_STEPS: usize = 24;
const CHAINS: usize = 2;

/// What is selected: a part of a type name, or the unowned untyped slots of one size.
#[derive(Clone, Copy)]
enum Selector {
    Name(&'static [u8]),
    Untyped(usize),
}

fn selectors() -> ([Option<Selector>; MAX_SELECTED], usize) {
    let mut out = [None; MAX_SELECTED];
    let text = unsafe { getenv(c"JAVA_GC_CENSUS_WHO".as_ptr()) };
    if text.is_null() {
        return (out, 0);
    }
    let all: &'static [u8] = unsafe { std::ffi::CStr::from_ptr(text) }.to_bytes();
    let mut n = 0;
    for part in all.split(|&c| c == b',') {
        if part.is_empty() || n == MAX_SELECTED {
            continue;
        }
        out[n] = Some(match part.strip_prefix(b"untyped:") {
            Some(size) => {
                Selector::Untyped(std::str::from_utf8(size).ok().and_then(|s| s.parse().ok()).unwrap_or(0))
            }
            None => Selector::Name(part),
        });
        n += 1;
    }
    (out, n)
}

/// The kind of a holder: an object of type `t` (`t`), a buffer an object of type `t` owns
/// (`MAX_TYPES + t`), or a slot of no type that no object owns, by size class
/// (`2 * MAX_TYPES + class`, the last for a large allocation).
const KEYS: usize = 2 * MAX_TYPES + SIZE_COUNT + 1;

struct Who<'c, 'h> {
    census: &'c Census<'h>,
    /// Per 16 bytes of the heap in use: 1 + the offset / 8 of the object that owns the buffer
    /// that starts there, 0 for none.
    owner: Table<u32>,
    /// Per type: 0 not yet looked at, 1 not selected, 2 + i selected by selector i.
    chosen: Table<u8>,
    selected: [Option<Selector>; MAX_SELECTED],
    n: usize,
}

impl Who<'_, '_> {
    fn granule(&self, target: &Target) -> Option<usize> {
        self.census.start_of(target).map(|a| (a - self.census.h.base) >> GRANULE_SHIFT)
    }

    fn owner_of(&self, target: &Target) -> Option<Target> {
        let g = self.granule(target)?;
        let o = self.owner.all()[g];
        (o != 0).then(|| self.census.target(self.census.h.base + ((o as usize - 1) << 3)))
    }

    /// The selector that `target` falls under.
    fn selection(&self, target: &Target) -> Option<usize> {
        let t = self.census.type_of(target);
        if t == 0 {
            if self.owner_of(target).is_some() {
                return None;
            }
            let size = match *target {
                Target::Slot { size, .. } => size,
                _ => 0,
            };
            return (0..self.n)
                .find(|&i| matches!(self.selected[i], Some(Selector::Untyped(s)) if s == size));
        }
        let chosen = &mut self.chosen.all()[t];
        if *chosen == 0 {
            let name = type_name(t).as_bytes();
            *chosen = (0..self.n)
                .find(|&i| match self.selected[i] {
                    Some(Selector::Name(part)) => name.windows(part.len()).any(|w| w == part),
                    _ => false,
                })
                .map_or(1, |i| 2 + i as u8);
        }
        (*chosen >= 2).then(|| *chosen as usize - 2)
    }

    /// The kind of holder `target` is (see `KEYS`).
    fn key(&self, target: &Target) -> usize {
        let t = self.census.type_of(target);
        if t != 0 {
            return t;
        }
        if let Some(owner) = self.owner_of(target) {
            return MAX_TYPES + self.census.type_of(&owner);
        }
        match *target {
            Target::Slot { block, .. } => 2 * MAX_TYPES + self.census.h.meta(block).class as usize / SCANS,
            _ => KEYS - 1,
        }
    }

    fn describe(&self, key: usize) -> Holder {
        Holder(key)
    }
}

struct Holder(usize);

impl std::fmt::Display for Holder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let k = self.0;
        if k < MAX_TYPES {
            write!(f, "{}", Short(type_name(k)))
        } else if k < 2 * MAX_TYPES {
            write!(f, "buffer of {}", Short(type_name(k - MAX_TYPES)))
        } else if k < KEYS - 1 {
            write!(f, "untyped {} bytes", SIZES[k - 2 * MAX_TYPES])
        } else {
            write!(f, "untyped large")
        }
    }
}

/// Where the census's words point: the target of a handle or of a thin reference.
fn pointee(census: &Census, word: usize) -> Target {
    census.target(word & crate::thin::ADDRESS)
}

/// The ranges of the roots, with what they are: 0 static data, 1 thread-local storage, 2 + i the
/// stack of thread i.
fn roots(each: &mut dyn FnMut(usize, usize, usize)) {
    let statics = statics();
    for &(from, to) in &statics.ranges[..statics.count] {
        each(from, to, 0);
    }
    let tls = statics.tls.0.next_multiple_of(statics.tls.1);
    let here = &statics as *const Statics as usize;
    for (i, t) in THREADS.iter().enumerate() {
        if !t.used.load(Ordering::Acquire) {
            continue;
        }
        let tp = if t.thread_pointer.get() != 0 { t.thread_pointer.get() } else { thread_pointer() };
        if tls > 0 && tp != 0 {
            each(tp - tls, tp, 1);
        }
        let top = t.stack_top.get();
        let sp = t.stopped_sp.get();
        if sp != 0 && sp < top {
            each(sp, top, 2 + i);
        } else if here < top && top - here < 1 << 30 {
            each(here, top, 2 + i);
        }
    }
}

fn root_name(kind: usize) -> RootName {
    RootName(kind)
}

struct RootName(usize);

impl std::fmt::Display for RootName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            0 => f.write_str("static data"),
            1 => f.write_str("thread-local storage"),
            k => write!(f, "the stack of thread {}", k - 2),
        }
    }
}

/// Reports what holds the selected objects (see the module comment).
pub(super) fn who(census: &Census) {
    let (selected, n) = selectors();
    if n == 0 {
        return;
    }
    let h = census.h;
    let granules = (census.end - h.base) >> GRANULE_SHIFT;
    let w = Who { census, owner: Table::new(granules), chosen: Table::new(MAX_TYPES), selected, n };
    // The owners of buffers: each typed object's buffer words.
    each_live(h, |holder| {
        if census.type_of(&holder) == 0 {
            return;
        }
        let (words, _, scanned) = census.words_of(&holder);
        if !scanned {
            return;
        }
        let at = census.start_of(&holder).unwrap();
        for (i, &word) in words.iter().enumerate() {
            let target = pointee(census, word);
            if matches!(target, Target::Inside)
                || census.type_of(&target) != 0
                || !census.buffer(words, i, &target)
            {
                continue;
            }
            if let Some(g) = w.granule(&target) {
                let entry = &mut w.owner.all()[g];
                if *entry == 0 {
                    *entry = 1 + ((at - h.base) >> 3) as u32;
                }
            }
        }
    });
    // The holders of the selected objects, by kind.
    let holders: Table<u32> = Table::new(MAX_SELECTED * KEYS);
    let mut held = Bits::new(h);
    each_live(h, |holder| {
        let (words, _, scanned) = census.words_of(&holder);
        if !scanned {
            return;
        }
        let mut key = None;
        for &word in words {
            let target = pointee(census, word);
            if matches!(target, Target::Inside) {
                continue;
            }
            if let Some(s) = w.selection(&target) {
                let k = *key.get_or_insert_with(|| w.key(&holder));
                holders.all()[s * KEYS + k] += 1;
                held.set(&target);
            }
        }
    });
    // The selected objects, those no slot holds, and the samples for chains.
    let mut objects = [0u64; MAX_SELECTED];
    let mut bytes = [0u64; MAX_SELECTED];
    let mut unheld = [0u64; MAX_SELECTED];
    let mut samples = [[None::<Target>; CHAINS]; MAX_SELECTED];
    let mut seen = [0usize; MAX_SELECTED];
    each_live(h, |target| {
        let Some(s) = w.selection(&target) else { return };
        objects[s] += 1;
        bytes[s] += (census.words_of(&target).0.len() * 8) as u64;
        let (block, slot) = match target {
            Target::Slot { block, slot, .. } => (block, slot),
            Target::Large { block, .. } => (block, 0),
            Target::Inside => return,
        };
        if !held.get(block, slot) {
            unheld[s] += 1;
        }
        // Samples spread over the heap: the 1st and the 1001st.
        if seen[s] % 1000 == 0 && seen[s] / 1000 < CHAINS {
            samples[s][seen[s] / 1000] = Some(target);
        }
        seen[s] += 1;
    });
    for s in 0..n {
        let what = match selected[s] {
            Some(Selector::Name(part)) => std::str::from_utf8(part).unwrap_or("?"),
            _ => "untyped",
        };
        super::census::line(format_args!(
            "[gc census who {what}: {} objects of {} KB, {} held by no slot]",
            objects[s],
            bytes[s] >> 10,
            unheld[s]
        ));
        let row = &holders.all()[s * KEYS..(s + 1) * KEYS];
        let mut listed = [usize::MAX; HOLDERS_LISTED];
        for i in 0..HOLDERS_LISTED {
            let best = (0..KEYS).filter(|k| row[*k] > 0 && !listed.contains(k)).max_by_key(|&k| row[k]);
            match best {
                Some(k) => listed[i] = k,
                None => break,
            }
        }
        for &k in listed.iter().take_while(|&&k| k != usize::MAX) {
            super::census::line(format_args!(
                "[gc census who {what} held by {} words of {}]",
                row[k],
                w.describe(k)
            ));
        }
        for (c, sample) in samples[s].iter().enumerate() {
            if let Some(sample) = *sample {
                chain(&w, what, c, sample);
            }
        }
    }
}

/// Follows holders from `start` back towards a root, one pass over the heap per step, and reports
/// each.
fn chain(w: &Who, what: &str, c: usize, start: Target) {
    let census = w.census;
    let mut path = [0usize; CHAIN_STEPS + 1];
    let mut current = start;
    path[0] = census.start_of(&current).unwrap_or(0);
    for step in 0..CHAIN_STEPS {
        let address = path[step];
        // A holder that is an object is preferred to a buffer, and a new one to one on the path.
        let mut found: Option<(Target, usize)> = None;
        each_live(census.h, |holder| {
            if found.is_some_and(|(_, rank)| rank == 0) {
                return;
            }
            let (words, _, scanned) = census.words_of(&holder);
            if !scanned || !words.iter().any(|&word| word & crate::thin::ADDRESS == address) {
                return;
            }
            let at = census.start_of(&holder).unwrap_or(0);
            let rank = if path[..=step].contains(&at) {
                2
            } else if census.type_of(&holder) != 0 {
                0
            } else {
                1
            };
            if found.is_none_or(|(_, r)| rank < r) {
                found = Some((holder, rank));
            }
        });
        match found {
            Some((holder, rank)) if rank < 2 => {
                // A buffer is followed to the object that owns it.
                let (next, through) = match (census.type_of(&holder), w.owner_of(&holder)) {
                    (0, Some(owner)) => (owner, true),
                    _ => (holder, false),
                };
                let via = if through { " (its buffer)" } else { "" };
                super::census::line(format_args!(
                    "[gc census chain {what} {c} step {}: {}{via}]",
                    step + 1,
                    w.describe(w.key(&next))
                ));
                current = next;
                path[step + 1] = census.start_of(&current).unwrap_or(0);
            }
            _ => {
                let mut kinds = [0usize; 4];
                let mut n = 0;
                roots(&mut |from, to, kind| {
                    let mut p = from & !7;
                    while p + 8 <= to {
                        if unsafe { (p as *const usize).read() } & crate::thin::ADDRESS == address
                            && n < 4
                            && !kinds[..n].contains(&kind)
                        {
                            kinds[n] = kind;
                            n += 1;
                        }
                        p += 8;
                    }
                });
                match n {
                    0 => super::census::line(format_args!(
                        "[gc census chain {what} {c} step {}: no holder found]",
                        step + 1
                    )),
                    _ => {
                        for &k in &kinds[..n] {
                            super::census::line(format_args!(
                                "[gc census chain {what} {c} step {}: root, {}]",
                                step + 1,
                                root_name(k)
                            ));
                        }
                    }
                }
                return;
            }
        }
    }
}
