//! The cell holding the plugin state. REAPER calls the plugin back from
//! inside its own calls (a pad button can trigger one of this plugin's
//! actions), and a bug must not take REAPER down, so access is guarded
//! against re-entrancy and panics, and work that arrives while the state is
//! in use is queued.

use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// Why [`Guarded::with`] did not run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unavailable {
    /// Called from inside another `with` (re-entrancy).
    Busy,
    /// The closure panicked; the state has been dropped.
    Panicked,
}

/// Optional state `T`, plus work `A` deferred while the state was in use.
pub struct Guarded<T, A> {
    state: RefCell<Option<T>>,
    deferred: RefCell<Vec<A>>,
}

impl<T, A> Guarded<T, A> {
    pub const fn new() -> Self {
        Guarded {
            state: RefCell::new(None),
            deferred: RefCell::new(Vec::new()),
        }
    }

    /// Run `f` on the state. If `f` panics, the state is dropped (so a
    /// broken state is not used again) and `Panicked` is returned.
    pub fn with<R>(&self, f: impl FnOnce(&mut Option<T>) -> R) -> Result<R, Unavailable> {
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.state.try_borrow_mut().ok().map(|mut st| f(&mut st))
        }));
        match result {
            Ok(Some(r)) => Ok(r),
            Ok(None) => Err(Unavailable::Busy),
            Err(_) => {
                // The borrow was released while unwinding. Dropping the state
                // may panic too; that must not escape either.
                if let Ok(mut st) = self.state.try_borrow_mut() {
                    let _ = catch_unwind(AssertUnwindSafe(|| drop(st.take())));
                }
                Err(Unavailable::Panicked)
            }
        }
    }

    /// Whether the state is in use, i.e. we are inside [`Self::with`].
    pub fn busy(&self) -> bool {
        self.state.try_borrow_mut().is_err()
    }

    /// Run `action` now, or queue it if the state is in use; queued work is
    /// collected with [`Self::take_deferred`].
    pub fn run_or_defer(&self, action: A, run: impl FnOnce(A)) {
        if self.busy() {
            self.deferred.borrow_mut().push(action);
        } else {
            run(action);
        }
    }

    pub fn take_deferred(&self) -> Vec<A> {
        std::mem::take(&mut *self.deferred.borrow_mut())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn guarded(v: i32) -> Guarded<i32, &'static str> {
        let g = Guarded::new();
        g.with(|s| *s = Some(v)).unwrap();
        g
    }

    #[test]
    fn gives_access_to_the_state() {
        let g = guarded(1);
        assert_eq!(g.with(|s| s.map(|v| v + 1)), Ok(Some(2)));
        g.with(|s| *s = None).unwrap();
        assert_eq!(g.with(|s| s.is_none()), Ok(true));
    }

    #[test]
    fn nested_access_is_refused_not_a_panic() {
        let g = guarded(1);
        let inner = g.with(|s| {
            *s = Some(5);
            g.with(|_| ())
        });
        assert_eq!(inner, Ok(Err(Unavailable::Busy)));
        assert_eq!(g.with(|s| *s), Ok(Some(5)), "outer call completed");
    }

    #[test]
    fn panic_drops_the_state_and_recovers() {
        let g = guarded(1);
        let r = g.with(|_| -> () { panic!("bug") });
        assert_eq!(r, Err(Unavailable::Panicked));
        assert_eq!(g.with(|s| *s), Ok(None), "state dropped");
        g.with(|s| *s = Some(2)).unwrap();
        assert_eq!(g.with(|s| *s), Ok(Some(2)), "usable again");
    }

    #[test]
    fn work_is_deferred_only_while_busy() {
        let g = guarded(1);
        let ran = Cell::new(0);
        g.run_or_defer("now", |_| ran.set(ran.get() + 1));
        assert_eq!(ran.get(), 1);
        assert!(g.take_deferred().is_empty());

        g.with(|_| {
            assert!(g.busy());
            g.run_or_defer("later", |_| ran.set(ran.get() + 1));
        })
        .unwrap();
        assert_eq!(ran.get(), 1, "not run while busy");
        assert!(!g.busy());
        assert_eq!(g.take_deferred(), vec!["later"]);
        assert!(g.take_deferred().is_empty(), "taken once");
    }
}
