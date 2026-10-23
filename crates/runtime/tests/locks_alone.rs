//! The plain path of `java::Locked` and `java::Arc` while one thread runs, and the moment that
//! thread starts a second: the locks it took alone stay its own, and the new thread waits for
//! them. In a process of its own, as the unit tests run on many threads that the runtime never
//! started.

use java_runtime as java;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[test]
fn locks_taken_alone_are_still_held_once_a_thread_starts() {
    let shared = java::Arc::new(java::Locked::new(0));
    // Alone, borrows conflict as in a `RefCell`, and a refused one leaves the lock as it was.
    let conflict = |f: &dyn Fn()| std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).is_err();
    {
        let _reading = shared.borrow();
        assert!(conflict(&|| drop(shared.borrow_mut())));
        let _again = shared.borrow();
    }
    {
        let _changing = shared.borrow_mut();
        assert!(conflict(&|| drop(shared.borrow())));
        assert!(conflict(&|| drop(shared.borrow_mut())));
    }
    {
        let _monitor = java::enter(&shared);
        *shared.borrow_mut() += 0;
    }
    // Taken alone: as the monitor, then borrowed to change and given back, then borrowed to read
    // across the start.
    let monitor = java::enter(&shared);
    *shared.borrow_mut() += 1;
    let reading = shared.borrow();
    let started = std::sync::Arc::new(AtomicBool::new(false));
    let tid = java::next_thread_id();
    let (other, signal) = (shared.clone(), started.clone());
    java::start_thread(tid, "other".to_string(), false, move || {
        signal.store(true, Ordering::SeqCst);
        *other.borrow_mut() += 10;
    });
    while !started.load(Ordering::SeqCst) {
        std::thread::yield_now();
    }
    std::thread::sleep(Duration::from_millis(50));
    // The other thread waits for the lock, and this one still holds it, so takes it again.
    assert_eq!(*reading, 1);
    assert_eq!(*shared.borrow(), 1);
    assert!(java::holds_lock(&shared));
    drop(reading);
    drop(monitor);
    java::join_thread(tid, 0);
    assert_eq!(*shared.borrow(), 11);
    assert!(!java::holds_lock(&shared));
    assert_eq!(java::Arc::strong_count(&shared), 1);
}
