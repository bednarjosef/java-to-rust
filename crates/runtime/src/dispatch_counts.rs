//! Counts of the classes that the receiver of each call through a dispatcher has, by call, for a
//! translation that tests the classes a call mostly sees before it dispatches (docs/native-speed.md,
//! "Profiled calls"). The translator emits a count before each such call when it is asked to
//! (`JAVA_TO_RUST_COUNT_DISPATCH=1` while translating); the program then writes, when it exits,
//! a line per call to the file `JAVA_DISPATCH_COUNTS` names, with each class by its binary name,
//! which any translation of the program gives it, where its number would only fit a translation
//! that numbers the classes the same. A translation made without it has no counts and no cost.

use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

/// The classes a call kept counts for; any others are counted together.
const KEPT: usize = 4;

/// The counts of one call: a static of the generated code, named by the call's key
/// (`codegen::dispatch::profile_key`).
pub struct DispatchCount {
    key: &'static str,
    classes: [AtomicU16; KEPT],
    counts: [AtomicU64; KEPT],
    others: AtomicU64,
    listed: AtomicBool,
}

impl DispatchCount {
    pub const fn new(key: &'static str) -> Self {
        DispatchCount {
            key,
            classes: [const { AtomicU16::new(0) }; KEPT],
            counts: [const { AtomicU64::new(0) }; KEPT],
            others: AtomicU64::new(0),
            listed: AtomicBool::new(false),
        }
    }
}

/// The counts that have counted a call, in the order of their first.
static LISTED: Mutex<Vec<&'static DispatchCount>> = Mutex::new(Vec::new());

/// The binary name of each class by its number, from the program (empty where it has none).
static NAMES: OnceLock<&'static [&'static str]> = OnceLock::new();

/// Counts one call of `count`'s on an object of class number `class` (never 0), whose name
/// `names` gives (the program's table, the same at every call). Two threads that count a class
/// for the first time at once may each take a place for it: the file then has the class twice,
/// which the translator adds up.
pub fn count_dispatch(count: &'static DispatchCount, class: u16, names: &'static [&'static str]) {
    NAMES.get_or_init(|| names);
    for (c, n) in count.classes.iter().zip(&count.counts) {
        let held = match c.load(Ordering::Relaxed) {
            0 => c.compare_exchange(0, class, Ordering::Relaxed, Ordering::Relaxed).unwrap_or_else(|now| now),
            held => held,
        };
        if held == 0 || held == class {
            n.fetch_add(1, Ordering::Relaxed);
            return list(count);
        }
    }
    count.others.fetch_add(1, Ordering::Relaxed);
    list(count)
}

#[inline]
fn list(count: &'static DispatchCount) {
    if !count.listed.load(Ordering::Relaxed) && !count.listed.swap(true, Ordering::Relaxed) {
        let mut listed = LISTED.lock().unwrap_or_else(|e| e.into_inner());
        if listed.is_empty() {
            unsafe extern "C" {
                fn atexit(f: extern "C" fn()) -> i32;
            }
            // SAFETY: `write_counts` takes no arguments and may run at any exit.
            unsafe { atexit(write_counts) };
        }
        listed.push(count);
    }
}

/// Writes each call's counts to the file `JAVA_DISPATCH_COUNTS` names, a line per call: its key,
/// a tab, `class:count` for each class it kept, by its name (by its number where the table names
/// none), and `*:count` for the others.
extern "C" fn write_counts() {
    let Some(path) = std::env::var_os("JAVA_DISPATCH_COUNTS") else { return };
    let listed = LISTED.lock().unwrap_or_else(|e| e.into_inner());
    let mut text = String::new();
    for count in listed.iter() {
        text.push_str(count.key);
        text.push('\t');
        for (c, n) in count.classes.iter().zip(&count.counts) {
            let (c, n) = (c.load(Ordering::Relaxed), n.load(Ordering::Relaxed));
            if c != 0 && n != 0 {
                match name_of(c) {
                    Some(name) => text.push_str(&format!("{name}:{n} ")),
                    None => text.push_str(&format!("{c}:{n} ")),
                }
            }
        }
        text.push_str(&format!("*:{}\n", count.others.load(Ordering::Relaxed)));
    }
    if let Ok(mut file) = std::fs::File::create(path) {
        let _ = file.write_all(text.as_bytes());
    }
}

/// The name of class number `class` in the program's table, if it has one.
fn name_of(class: u16) -> Option<&'static str> {
    NAMES.get().and_then(|names| names.get(usize::from(class))).copied().filter(|name| !name.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_classes_are_counted_each_and_the_rest_together() {
        static COUNT: DispatchCount = DispatchCount::new("k");
        static NAMES: [&str; 14] = ["", "", "", "a.B", "", "a.C$D", "", "", "", "", "", "", "", ""];
        for class in [3, 3, 5, 7, 9, 11, 3, 13] {
            count_dispatch(&COUNT, class, &NAMES);
        }
        let kept: Vec<(u16, u64)> = COUNT
            .classes
            .iter()
            .zip(&COUNT.counts)
            .map(|(c, n)| (c.load(Ordering::Relaxed), n.load(Ordering::Relaxed)))
            .collect();
        assert_eq!(kept, [(3, 3), (5, 1), (7, 1), (9, 1)]);
        assert_eq!(COUNT.others.load(Ordering::Relaxed), 2);
        assert_eq!(
            (name_of(3), name_of(5), name_of(7), name_of(40)),
            (Some("a.B"), Some("a.C$D"), None, None)
        );
    }
}
