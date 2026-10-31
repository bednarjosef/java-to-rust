//! Soft and weak references (`java::WeakField`, `gc::references`): a referent that nothing else
//! keeps is cleared by a collection and its reference made pending, one that something keeps is
//! not, a soft referent read lately stays, and the same holds in the young generation's minor
//! collections. The cases that need the collector's settings run in a child process of this
//! test, which sets them before its heap starts.

use java_runtime as java;
use std::hint::black_box;

#[global_allocator]
static ALLOCATOR: java::Allocator = java::Allocator;

struct Payload {
    value: u64,
    _padding: [u64; 3],
}

struct Reference {
    referent: java::WeakField<Option<java::Arc<Payload>>>,
}

const N: usize = 200;

/// `N` references of `kind` to payloads that only they hold, with queues.
#[inline(never)]
fn references(kind: i32) -> Vec<java::Arc<Reference>> {
    references_queued(kind, true)
}

#[inline(never)]
fn references_queued(kind: i32, queued: bool) -> Vec<java::Arc<Reference>> {
    (0..N)
        .map(|i| {
            let payload = java::Arc::new(Payload { value: i as u64, _padding: [0; 3] });
            let reference = java::Arc::new(Reference { referent: java::WeakField::new(Some(payload)) });
            java::register_reference(&reference, &reference.referent, kind, queued);
            reference
        })
        .collect()
}

/// Overwrites the stack below the caller's frame, where copies of the payloads' handles may be.
#[inline(never)]
fn scrub() {
    black_box([0u64; 4096]);
}

fn collect() {
    scrub();
    for _ in 0..3 {
        java::gc::collect_now();
    }
}

fn cleared(references: &[java::Arc<Reference>]) -> usize {
    references.iter().filter(|r| r.referent.get().is_none()).count()
}

/// The scenarios, each run in a child process with its settings: `weak`, `soft-kept`,
/// `soft-cleared` and `minor`.
fn scenario(name: &str) {
    let _main = java::main_thread();
    match name {
        "weak" => {
            let held: Vec<java::Arc<Payload>> =
                (0..N).map(|i| java::Arc::new(Payload { value: i as u64, _padding: [0; 3] })).collect();
            let strong: Vec<java::Arc<Reference>> = held
                .iter()
                .map(|p| {
                    let r = java::Arc::new(Reference { referent: java::WeakField::new(Some(p.clone())) });
                    java::register_reference(&r, &r.referent, 2, true);
                    r
                })
                .collect();
            let weak = references(2);
            let unqueued = references_queued(2, false);
            collect();
            assert!(cleared(&weak) >= N * 9 / 10, "{} of {N} weak referents cleared", cleared(&weak));
            assert!(cleared(&unqueued) >= N * 9 / 10, "{} of {N} weak referents cleared", cleared(&unqueued));
            assert_eq!(cleared(&strong), 0, "a referent held elsewhere was cleared");
            let mut pending = 0;
            while let Some(r) = java::take_pending_reference::<java::Arc<Reference>>() {
                assert!(r.referent.get().is_none(), "a pending reference still has its referent");
                assert!(
                    weak.iter().any(|w| java::Arc::ptr_eq(w, &r)),
                    "a reference without a queue is pending"
                );
                pending += 1;
            }
            assert!(!java::has_pending_references());
            assert_eq!(pending, cleared(&weak), "every cleared reference with a queue is pending once");
            assert!(held.iter().zip(&strong).all(|(p, r)| r.referent.get().unwrap().value == p.value));
        }
        "soft-kept" => {
            let soft = references(1);
            collect();
            assert_eq!(cleared(&soft), 0, "soft referents read lately were cleared");
        }
        "soft-cleared" => {
            let soft = references(1);
            std::thread::sleep(std::time::Duration::from_millis(20));
            collect();
            assert!(cleared(&soft) >= N * 9 / 10, "{} of {N} soft referents cleared", cleared(&soft));
        }
        "minor" => {
            // References that live through a full collection are old; payloads made after it
            // are young, and a minor collection clears those that only the old references hold.
            // (The old references' first referents are held elsewhere; Java sets a referent only
            // in the constructor, but the collector does not rely on that once one is set.)
            let first = java::Arc::new(Payload { value: 0, _padding: [0; 3] });
            let old: Vec<java::Arc<Reference>> = (0..N)
                .map(|_| {
                    let r = java::Arc::new(Reference { referent: java::WeakField::new(Some(first.clone())) });
                    java::register_reference(&r, &r.referent, 2, true);
                    r
                })
                .collect();
            collect();
            #[inline(never)]
            fn fill(old: &[java::Arc<Reference>]) -> Vec<java::Arc<Payload>> {
                let mut kept = Vec::new();
                for (i, r) in old.iter().enumerate() {
                    let p = java::Arc::new(Payload { value: i as u64, _padding: [0; 3] });
                    if i % 2 == 0 {
                        kept.push(p.clone());
                    }
                    r.referent.set(Some(p));
                }
                kept
            }
            let kept = fill(&old);
            scrub();
            java::gc::collect_now();
            let (_, collections) = java::gc::statistics();
            let odd_cleared = old.iter().skip(1).step_by(2).filter(|r| r.referent.get().is_none()).count();
            let even_cleared = old.iter().step_by(2).filter(|r| r.referent.get().is_none()).count();
            assert_eq!(even_cleared, 0, "a young referent held elsewhere was cleared");
            assert!(odd_cleared >= N / 2 * 9 / 10, "{odd_cleared} of {} young referents cleared", N / 2);
            assert!(collections >= 2);
            black_box(kept);
        }
        _ => panic!("no scenario {name}"),
    }
}

fn run(name: &str, settings: &[(&str, &str)]) {
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child", "--nocapture", "--test-threads=1"])
        .env("REFERENCES_SCENARIO", name)
        .envs(settings.iter().copied())
        .status()
        .unwrap();
    assert!(status.success(), "scenario {name} failed");
}

#[test]
fn child() {
    if let Ok(name) = std::env::var("REFERENCES_SCENARIO") {
        scenario(&name);
    }
}

#[test]
fn weak_referents_that_nothing_keeps_are_cleared_and_pending() {
    run("weak", &[]);
}

#[test]
fn soft_referents_read_lately_stay() {
    run("soft-kept", &[]);
}

#[test]
fn soft_referents_not_read_are_cleared_by_the_policy() {
    run("soft-cleared", &[("JAVA_GC_SOFT_MS_PER_MB", "0")]);
}

#[test]
fn a_minor_collection_clears_young_referents_of_old_references() {
    run("minor", &[("JAVA_GC_GENERATIONS", "1")]);
}
