//! A new object's slot, taken inline by `java::Arc::new`, on the collector as the program's
//! allocator: every byte the object's value leaves unwritten is zero, not a word of the slot's
//! last object.

use java_runtime as java;
use std::hint::black_box;

#[global_allocator]
static ALLOCATOR: java::Allocator = java::Allocator;

#[allow(dead_code)]
enum Value {
    Empty,
    Pair(u64, u64),
}

#[allow(dead_code)]
struct Holder {
    value: Value,
    key: u32,
}

#[inline(never)]
fn holder(key: u32) -> java::Arc<Holder> {
    java::Arc::new(Holder { value: Value::Empty, key })
}

#[test]
fn a_new_object_keeps_no_word_of_its_slots_last_object() {
    let _main = java::main_thread();
    let word = 0x7f00_dead_beef_0001u64;
    for _ in 0..4 {
        // Slots of the holder's size, filled with a word and freed onto the thread's free list,
        // where the next object of that size takes one.
        let old: Vec<Box<[u64; 4]>> = (0..64).map(|_| black_box(Box::new([0; 4]))).collect();
        for slot in &old {
            let p = &**slot as *const [u64; 4] as *mut u64;
            for i in 0..4 {
                unsafe { p.add(i).write_volatile(word) };
            }
        }
        drop(old);
        let h = holder(7);
        let p = &*h as *const Holder as *const u64;
        let words: Vec<u64> =
            (0..size_of::<Holder>() / 8).map(|i| unsafe { p.add(i).read_volatile() }).collect();
        assert!(!words.contains(&word), "{words:x?}");
        assert!(words.iter().all(|w| w >> 32 != word >> 32), "{words:x?}");
    }
}
