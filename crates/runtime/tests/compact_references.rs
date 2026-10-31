//! Objects reached only through 4-byte references (`java::RefField`, `java::SharedRefField`) live
//! through collections: the collector reads every 4 bytes of a slot as a possible reference.

use java_runtime as java;
use std::hint::black_box;

#[global_allocator]
static ALLOCATOR: java::Allocator = java::Allocator;

struct Node {
    value: u64,
    next: java::SharedRefField<Option<java::Arc<Node>>>,
    name: java::RefField<Option<java::JStr>>,
    small: u32,
}

/// A chain of `n` nodes, each reached only by the 4-byte field of the one before; the handle of
/// the first.
#[inline(never)]
fn chain(n: u64) -> java::Arc<Node> {
    let mut head: Option<java::Arc<Node>> = None;
    for value in (0..n).rev() {
        let node = Node {
            value,
            next: java::SharedRefField::new(head.take()),
            name: java::RefField::new(Some(java::JStr::new(java::wtf8(&format!("node {value}"))))),
            small: value as u32 * 3,
        };
        head = Some(java::Arc::new(node));
    }
    head.unwrap()
}

/// Makes and drops objects of the nodes' size and of other sizes, which take any slot that a
/// collection freed.
#[inline(never)]
fn churn() {
    for i in 0..200_000u64 {
        black_box(java::Arc::new([i; 4]));
        black_box(java::Arc::new((i, i + 1, i as u32)));
        black_box(vec![i; 3]);
    }
}

#[test]
fn objects_reached_only_by_4_byte_references_live() {
    let _main = java::main_thread();
    java::compact_references();
    let head = chain(20_000);
    for _ in 0..3 {
        churn();
        java::gc::collect_now();
    }
    let mut node = Some(head);
    let mut count = 0u64;
    while let Some(n) = node {
        assert_eq!(n.value, count);
        assert_eq!(n.small, count as u32 * 3);
        assert_eq!(n.name.get().unwrap().to_string(), format!("node {count}"));
        count += 1;
        node = n.next.get();
    }
    assert_eq!(count, 20_000);
}

#[test]
fn a_shared_reference_field_swaps_by_identity() {
    let a = java::Arc::new(1u64);
    let b = java::Arc::new(2u64);
    let f = java::SharedRefField::new(Some(a.clone()));
    assert!(!f.compare_and_set_by(Some(b.clone()), None, |x, y| x.as_ref().map(java::Arc::as_ptr)
        == y.as_ref().map(java::Arc::as_ptr)));
    assert!(f.compare_and_set_by(Some(a.clone()), Some(b.clone()), |x, y| x.as_ref().map(java::Arc::as_ptr)
        == y.as_ref().map(java::Arc::as_ptr)));
    assert_eq!(*f.get().unwrap(), 2);
    assert_eq!(*f.replace(None).unwrap(), 2);
    assert!(f.get().is_none());
    let g: java::RefField<Option<java::Arc<u64>>> = Default::default();
    assert!(g.get().is_none());
    g.set(Some(a));
    assert_eq!(*g.clone().get().unwrap(), 1);
}
