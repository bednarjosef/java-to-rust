//! Steps of the analyses that run on a thread of their own while the others go on, where none of
//! them reads what the others find. A step's result is taken where the analyses ran it one after
//! another (`Alongside::join`), so the analyses find the same: a panic of the step comes out
//! there, with its place in the translator (`lower::panic_message`), and the result of a step
//! that the analyses never get to, as a check before it rejected the program, is dropped, as if
//! it had not run.

use std::any::Any;
use std::panic::AssertUnwindSafe;
use std::thread::{Scope, ScopedJoinHandle};

/// The stack of each such thread: as large as that of the translator's own (`main.rs`), as the
/// analyses recurse over the code.
const STACK: usize = 1 << 30;

/// A step running alongside the others, or done already when they run one after another
/// (`one_after_another`).
pub(super) enum Alongside<'scope, T> {
    Running(ScopedJoinHandle<'scope, Result<T, Panicked>>),
    Done(Done<T>),
}

thread_local! {
    /// Whether the analyses of this thread run their steps one after another (`one_after_another`).
    static ONE_AFTER_ANOTHER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Runs `f` with the steps of the analyses on this thread one after another, each where it is
/// started, rather than alongside each other: for a test that they find the same either way.
pub fn one_after_another<T>(f: impl FnOnce() -> T) -> T {
    let before = ONE_AFTER_ANOTHER.replace(true);
    let out = f();
    ONE_AFTER_ANOTHER.set(before);
    out
}

/// A panic of a step, with where in the translator it happened.
pub(super) struct Panicked {
    payload: Box<dyn Any + Send>,
    place: Option<String>,
}

impl<'scope, T: Send + 'scope> Alongside<'scope, T> {
    /// Starts `step` on a thread of its own.
    pub(super) fn start<'env>(
        scope: &'scope Scope<'scope, 'env>,
        step: impl FnOnce() -> T + Send + 'scope,
    ) -> Self {
        if ONE_AFTER_ANOTHER.get() {
            return Alongside::Done(Done::of(step));
        }
        let run = move || Done::of(step).0;
        let thread = std::thread::Builder::new().name("analyses".to_string()).stack_size(STACK);
        Alongside::Running(thread.spawn_scoped(scope, run).expect("a thread for the analyses"))
    }

    /// The result of the step, once it is done; its panic, if it panicked.
    pub(super) fn join(self) -> T {
        self.finish().take()
    }

    /// The step, once it is done, to be taken later.
    pub(super) fn finish(self) -> Done<T> {
        match self {
            Alongside::Running(thread) => Done(thread.join().expect("the step's panic is caught")),
            Alongside::Done(done) => done,
        }
    }
}

/// A step that is done (`Alongside::finish`).
pub struct Done<T>(Result<T, Panicked>);

impl<T> Done<T> {
    /// Runs `step` here and now, to be taken later: a step of a thread that runs others too.
    pub(super) fn of(step: impl FnOnce() -> T) -> Self {
        Done(
            std::panic::catch_unwind(AssertUnwindSafe(step))
                .map_err(|payload| Panicked { payload, place: crate::lower::panic_place() }),
        )
    }

    /// The result of the step; its panic, if it panicked.
    pub(super) fn take(self) -> T {
        match self.0 {
            Ok(value) => value,
            Err(Panicked { payload, place }) => {
                crate::lower::panic_carried(place);
                std::panic::resume_unwind(payload)
            }
        }
    }
}
