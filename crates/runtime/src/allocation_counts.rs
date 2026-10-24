//! Counts of the objects a program makes, by class, for finding what to make cheaper. The
//! translator emits a count in each constructor when it is asked to
//! (`JAVA_TO_RUST_COUNT_ALLOCATIONS=1` while translating); the program then prints the classes it
//! made most objects of on standard error when it exits. A translation made without it has no
//! counts and no cost.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// The count of one class's objects: a static of the generated code.
pub struct AllocationCount {
    pub name: &'static str,
    count: AtomicU64,
    listed: AtomicBool,
}

impl AllocationCount {
    pub const fn new(name: &'static str) -> Self {
        AllocationCount { name, count: AtomicU64::new(0), listed: AtomicBool::new(false) }
    }
}

/// The counts that have counted an object, in the order of their first.
static LISTED: Mutex<Vec<&'static AllocationCount>> = Mutex::new(Vec::new());

/// Counts one object of the class of `count`.
pub fn count_allocation(count: &'static AllocationCount) {
    count.count.fetch_add(1, Ordering::Relaxed);
    if !count.listed.load(Ordering::Relaxed) && !count.listed.swap(true, Ordering::Relaxed) {
        let mut listed = LISTED.lock().unwrap_or_else(|e| e.into_inner());
        if listed.is_empty() {
            unsafe extern "C" {
                fn atexit(f: extern "C" fn()) -> i32;
            }
            // SAFETY: `report` takes no arguments and may run at any exit.
            unsafe { atexit(report) };
        }
        listed.push(count);
    }
}

/// Prints the 60 classes with the most objects, and the total, on standard error.
extern "C" fn report() {
    let listed = LISTED.lock().unwrap_or_else(|e| e.into_inner());
    let mut counts: Vec<(u64, &str)> =
        listed.iter().map(|c| (c.count.load(Ordering::Relaxed), c.name)).collect();
    counts.sort_unstable_by(|a, b| b.cmp(a));
    let total: u64 = counts.iter().map(|c| c.0).sum();
    eprintln!("[allocations] {total} objects of {} classes", counts.len());
    for (count, name) in counts.iter().take(60) {
        eprintln!("[allocations] {count:>12} {:5.1}% {name}", 100.0 * *count as f64 / total.max(1) as f64);
    }
}
