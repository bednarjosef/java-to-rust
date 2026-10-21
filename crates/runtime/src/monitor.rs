//! Java's monitors: `synchronized`, `wait`, `notify`, `notifyAll` and `Thread.holdsLock`.
//!
//! An object that threads share is an `Arc<java::Locked<C>>`, and the reentrant lock of its
//! `Locked` is its monitor as well as its data lock (see docs/multi-threading.md, 3.2): a
//! `synchronized (o) { .. }` holds the lock for the block (`let _monitor = java::enter(&o);`),
//! and the borrows of the object's fields inside the block take it once more, which a reentrant
//! lock allows. So the lock is taken in the order the Java program takes its monitors, and a
//! deadlock of the translation is one the Java program has.
//!
//! An object of one thread (an `Rc`) has no lock, since no other thread can contend for it. Its
//! monitor is only the record that the thread holds it, a list of the addresses of the objects
//! that the thread has entered, so that `wait`, `notify` and `holdsLock` answer as Java's do.

use crate::Locked;
use crate::locked::{RawLock, Waited};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

/// The monitor of an object: the lock of one that threads share, or the address of one that a
/// single thread holds.
pub enum MonitorOf<'a> {
    Shared(&'a RawLock),
    Local(usize),
}

/// A value whose object has a monitor: a handle, or the enum of a class type, which the
/// translation implements it for by matching the variant.
pub trait HasMonitor {
    fn monitor(&self) -> MonitorOf<'_>;
}

impl<T: ?Sized> HasMonitor for Locked<T> {
    #[inline]
    fn monitor(&self) -> MonitorOf<'_> {
        MonitorOf::Shared(self.raw())
    }
}

impl<H: HasMonitor + ?Sized> HasMonitor for Arc<H> {
    #[inline]
    fn monitor(&self) -> MonitorOf<'_> {
        (**self).monitor()
    }
}

impl<H: HasMonitor + ?Sized> HasMonitor for &H {
    #[inline]
    fn monitor(&self) -> MonitorOf<'_> {
        (**self).monitor()
    }
}

impl<T: ?Sized> HasMonitor for Rc<T> {
    #[inline]
    fn monitor(&self) -> MonitorOf<'_> {
        MonitorOf::Local(Rc::as_ptr(self) as *const u8 as usize)
    }
}

thread_local! {
    /// The objects of this thread alone whose monitors it holds, by address, once per entry.
    static ENTERED: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
}

fn holds_local(address: usize) -> bool {
    ENTERED.with_borrow(|e| e.contains(&address))
}

/// A `synchronized` region on an object, which it holds; the monitor is exited when it drops,
/// however the region ends.
pub struct MonitorGuard<H: HasMonitor> {
    held: H,
}

/// Enters the monitor of `held`'s object: `synchronized (o)`, or a synchronized method. The
/// guard keeps its own handle, so the variable that held the object may change in the region.
#[inline]
pub fn enter<H: HasMonitor + Clone>(held: &H) -> MonitorGuard<H> {
    enter_held(held.clone())
}

/// Enters the monitor of a class: a static synchronized method, or `synchronized (C.class)`.
/// The monitor is a static of the translation, `static C_MONITOR: java::Locked<()>`.
#[inline]
pub fn enter_class(monitor: &'static Locked<()>) -> MonitorGuard<&'static Locked<()>> {
    enter_held(monitor)
}

#[inline]
fn enter_held<H: HasMonitor>(held: H) -> MonitorGuard<H> {
    match held.monitor() {
        MonitorOf::Shared(lock) => lock.enter(),
        MonitorOf::Local(address) => ENTERED.with_borrow_mut(|e| e.push(address)),
    }
    MonitorGuard { held }
}

impl<H: HasMonitor> Drop for MonitorGuard<H> {
    #[inline]
    fn drop(&mut self) {
        match self.held.monitor() {
            MonitorOf::Shared(lock) => lock.exit(),
            MonitorOf::Local(address) => ENTERED.with_borrow_mut(|e| {
                if let Some(at) = e.iter().rposition(|&a| a == address) {
                    e.remove(at);
                }
            }),
        }
    }
}

/// `Thread.holdsLock(o)`.
pub fn holds_lock(held: &impl HasMonitor) -> bool {
    match held.monitor() {
        MonitorOf::Shared(lock) => lock.held_by_current(),
        MonitorOf::Local(address) => holds_local(address),
    }
}

const NOT_OWNER: &str = "current thread is not owner";

/// `o.wait(millis)` (`o.wait()` is a wait of 0 milliseconds, which has no timeout), where the
/// program observes its exceptions.
pub fn try_monitor_wait(held: &impl HasMonitor, millis: i64) -> Result<(), crate::Exception> {
    if millis < 0 {
        return Err(crate::Exception::new("java.lang.IllegalArgumentException", "timeout value is negative"));
    }
    let timeout = (millis > 0).then(|| Duration::from_millis(millis as u64));
    let waited = match held.monitor() {
        MonitorOf::Shared(lock) => match lock.wait_notified(timeout) {
            Waited::Done => true,
            Waited::NotOwner => false,
            Waited::Interrupted => return Err(crate::Exception::bare("java.lang.InterruptedException")),
        },
        // No other thread can reach the object to notify it, so the wait lasts its timeout, or
        // for ever, as Java's does.
        MonitorOf::Local(address) if holds_local(address) => {
            // Only an interrupt, from another thread or from before the wait, ends a wait
            // without a timeout.
            if crate::thread::take_interrupt()
                || crate::thread::interruptible_sleep(timeout.unwrap_or(Duration::MAX))
            {
                return Err(crate::Exception::bare("java.lang.InterruptedException"));
            }
            true
        }
        MonitorOf::Local(_) => false,
    };
    if waited {
        Ok(())
    } else {
        Err(crate::Exception::new("java.lang.IllegalMonitorStateException", NOT_OWNER))
    }
}

/// `o.wait(millis)`.
pub fn monitor_wait(held: &impl HasMonitor, millis: i64) {
    if let Err(e) = try_monitor_wait(held, millis) {
        crate::uncaught::<()>(e);
    }
}

/// `o.notify()` or, with `all`, `o.notifyAll()`, where the program observes the
/// `IllegalMonitorStateException`.
pub fn try_monitor_notify(held: &impl HasMonitor, all: bool) -> Result<(), crate::Exception> {
    let owner = match held.monitor() {
        MonitorOf::Shared(lock) => lock.notify(all),
        MonitorOf::Local(address) => holds_local(address),
    };
    if owner {
        Ok(())
    } else {
        Err(crate::Exception::new("java.lang.IllegalMonitorStateException", NOT_OWNER))
    }
}

/// `o.notify()` or `o.notifyAll()`.
pub fn monitor_notify(held: &impl HasMonitor, all: bool) {
    if let Err(e) = try_monitor_notify(held, all) {
        crate::uncaught::<()>(e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_monitor_is_reentrant_and_held_by_its_thread() {
        let o = Arc::new(Locked::new(0));
        assert!(!holds_lock(&o));
        let outer = enter(&o);
        let inner = enter(&o);
        *o.borrow_mut() += 1;
        assert!(holds_lock(&o));
        drop(inner);
        assert!(holds_lock(&o));
        drop(outer);
        assert!(!holds_lock(&o));
        let local = Rc::new(1);
        let guard = enter(&local);
        assert!(holds_lock(&local));
        drop(guard);
        assert!(!holds_lock(&local));
    }

    #[test]
    fn wait_and_notify_need_the_monitor() {
        let o = Arc::new(Locked::new(()));
        assert!(try_monitor_notify(&o, false).is_err());
        assert!(try_monitor_wait(&o, 1).is_err());
        assert!(try_monitor_wait(&o, -1).is_err());
        let _guard = enter(&o);
        assert!(try_monitor_notify(&o, true).is_ok());
        assert!(try_monitor_wait(&o, 1).is_ok());
        assert!(holds_lock(&o));
    }

    #[test]
    fn a_notified_thread_wakes_with_the_monitor() {
        let queue = Arc::new(Locked::new(Vec::new()));
        let consumer = {
            let queue = queue.clone();
            std::thread::spawn(move || {
                let mut taken = Vec::new();
                while taken.len() < 1000 {
                    let _guard = enter(&queue);
                    while queue.borrow().is_empty() {
                        monitor_wait(&queue, 0);
                    }
                    let first = queue.borrow_mut().remove(0);
                    taken.push(first);
                    monitor_notify(&queue, true);
                }
                taken
            })
        };
        for i in 0..1000 {
            let _guard = enter(&queue);
            while queue.borrow().len() >= 4 {
                monitor_wait(&queue, 0);
            }
            queue.borrow_mut().push(i);
            monitor_notify(&queue, false);
        }
        assert_eq!(consumer.join().unwrap(), (0..1000).collect::<Vec<_>>());
    }

    #[test]
    fn each_notify_wakes_a_waiter_that_a_later_one_cannot_take() {
        // Several consumers, one `notify` per item: a waiter that comes late must not take the
        // notification meant for one that waited before it, or both would wait for ever.
        let queue = Arc::new(Locked::new((Vec::new(), 0)));
        let consumers: Vec<_> = (0..3)
            .map(|_| {
                let queue = queue.clone();
                std::thread::spawn(move || {
                    let mut sum = 0i64;
                    loop {
                        let _guard = enter(&queue);
                        while queue.borrow().0.is_empty() && queue.borrow().1 == 0 {
                            monitor_wait(&queue, 0);
                        }
                        match queue.borrow_mut().0.pop() {
                            Some(v) => sum += v,
                            None => return sum,
                        }
                    }
                })
            })
            .collect();
        for i in 0..3000i64 {
            let _guard = enter(&queue);
            queue.borrow_mut().0.push(i);
            monitor_notify(&queue, false);
        }
        {
            let _guard = enter(&queue);
            queue.borrow_mut().1 = 1;
            monitor_notify(&queue, true);
        }
        let total: i64 = consumers.into_iter().map(|c| c.join().unwrap()).sum();
        assert_eq!(total, (0..3000).sum::<i64>());
    }

    #[test]
    fn a_timed_wait_ends_without_a_notify() {
        let o = Arc::new(Locked::new(()));
        let _guard = enter(&o);
        let started = std::time::Instant::now();
        monitor_wait(&o, 20);
        assert!(started.elapsed() >= Duration::from_millis(20));
        assert!(holds_lock(&o));
    }
}
