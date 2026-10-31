//! Arrays in one slot of the collected heap (`array.rs`, `gc::alloc_array`): a pointer into an
//! array's elements, as a view of them or a lent slice holds on the stack, keeps the array whole
//! across collections, small or large; and the elements of an array of numbers are not scanned,
//! so a number that is an object's address keeps nothing alive, while the header and the
//! elements of an array of references are.

use java::Elements;
use java_runtime as java;
use std::hint::black_box;

#[global_allocator]
static ALLOCATOR: java::Allocator = java::Allocator;

struct Payload {
    value: u64,
}

struct Reference {
    referent: java::WeakField<Option<java::Arc<Payload>>>,
}

/// Overwrites the stack below the caller's frame, where copies of handles may be.
#[inline(never)]
fn scrub() {
    black_box([0u64; 4096]);
}

/// Collects, and allocates arrays of every size meanwhile, which take any slot freed.
fn churn() {
    scrub();
    for round in 0..3 {
        java::gc::collect_now();
        for n in [1, 7, 100, 3000, 20_000] {
            black_box(java::filled(n, round as i64 - 1));
        }
    }
}

/// The start and the length of the elements of a new array of `n` numbers, `k * 3` each: the
/// only thing left of it is a pointer into its elements, as a view holds.
#[inline(never)]
fn elements_only(n: i32) -> (usize, usize) {
    let a = java::tabulate(n, |k| k as i64 * 3);
    let items = a.in_place().unwrap();
    (items.as_ptr() as usize, items.len())
}

#[test]
fn a_pointer_into_the_elements_keeps_the_array() {
    let _main = java::main_thread();
    for n in [5, 1000, 100_000] {
        let (start, len) = elements_only(n);
        // The pointer stays in a register or on the stack, the only reference.
        let start = black_box(start);
        churn();
        let items = unsafe { std::slice::from_raw_parts(start as *const i64, len) };
        assert!(items.iter().enumerate().all(|(k, &v)| v == k as i64 * 3), "an array of {n} lost");
    }
}

/// Weak references to `n` new payloads; the address of payload `k` is stored as a number in
/// element `k` of `numbers`, if given.
#[inline(never)]
fn weak_payloads(n: usize, numbers: Option<&java::Array<i64>>) -> Vec<java::Arc<Reference>> {
    (0..n)
        .map(|k| {
            let payload = java::Arc::new(Payload { value: 7 });
            if let Some(numbers) = numbers {
                numbers.put(k as i32, java::Arc::as_ptr(&payload) as i64);
            }
            let r = java::Arc::new(Reference { referent: java::WeakField::new(Some(payload)) });
            java::register_reference(&r, &r.referent, 2, false);
            r
        })
        .collect()
}

fn cleared(references: &[java::Arc<Reference>]) -> usize {
    references.iter().filter(|r| r.referent.get().is_none()).count()
}

#[test]
fn the_elements_of_an_array_of_numbers_keep_nothing() {
    let _main = java::main_thread();
    // A small slot and a large allocation.
    for n in [200, 50_000] {
        let numbers = java::filled(n, 0i64);
        let weak = weak_payloads(n as usize, Some(&numbers));
        churn();
        // The conservative scan of the stack may keep a few.
        assert!(
            cleared(&weak) >= n as usize * 9 / 10,
            "{} of {n} objects whose addresses are numbers freed",
            cleared(&weak)
        );
        black_box(&numbers);
    }
}

#[test]
fn the_elements_of_an_array_of_references_keep_their_objects() {
    let _main = java::main_thread();
    for n in [4, 50_000] {
        let objects: java::Array<Option<java::Arc<Payload>>> = java::filled(n, None);
        let weak = weak_payloads(1, None).pop().unwrap();
        objects.put(n - 1, weak.referent.get());
        churn();
        assert_eq!(weak.referent.get().map(|p| p.value), Some(7), "an element of {n} lost its object");
        assert_eq!(objects.element(n - 1).map(|p| p.value), Some(7));
    }
}
