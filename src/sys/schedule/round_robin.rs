use alloc::{sync::Arc, vec::Vec};

use crate::{
    sys::thread::{self, State, Thread, Tid},
    Spinlock,
};

/// Represents a thread with some additional information used by the scheduler.
struct ThreadInfo {
    thread: Arc<Thread>,
    quantum: u64,
}

pub struct Scheduler {
    run_list: Spinlock<Vec<ThreadInfo>>,
}

impl Scheduler {
    const QUANTUM: u64 = 20;

    /// Create a new scheduler, with an empty run list.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            run_list: Spinlock::new(Vec::new()),
        }
    }
}

impl super::Scheduler for Scheduler {
    fn pick_idle(&self) -> Arc<Thread> {
        x86_64::irq::without(|| {
            self.run_list
                .lock()
                .iter()
                .filter(|rt| rt.thread.priority() == thread::Priority::Idle)
                .find(|rt| {
                    let mut state = rt.thread.state_locked();
                    if *state == State::Ready {
                        *state = State::Running;
                        return true;
                    }
                    false
                })
                .map(|rt| Arc::clone(&rt.thread))
        })
        .unwrap()
    }

    fn pick_next(&self) -> Option<Arc<Thread>> {
        x86_64::irq::without(|| {
            self.run_list
                .lock()
                .iter()
                .filter(|rt| rt.quantum > 0)
                .filter(|rt| rt.thread.priority() != thread::Priority::Idle)
                .find(|rt| {
                    let mut state = rt.thread.state_locked();
                    if *state == State::Ready {
                        *state = State::Running;
                        return true;
                    }
                    false
                })
                .map(|rt| Arc::clone(&rt.thread))
        })
    }

    /// Add a thread to the scheduler. The thread is added to the run list and its state is set to
    /// `Ready`.
    fn add_thread(&self, thread: Arc<Thread>) {
        log::debug!("Adding thread {:?} to the scheduler", thread.tid());
        thread.set_state(State::Ready);
        x86_64::irq::without(|| {
            self.run_list.lock().push(ThreadInfo {
                quantum: Self::QUANTUM,
                thread,
            });
        });
    }

    /// Remove a thread from the scheduler. The thread is removed from the run list and cannot be
    /// run anymore until it is added again.
    ///
    /// # Panics
    /// This function panics if the thread to remove is in the `Running` state.
    fn remove_thread(&self, tid: Tid) {
        x86_64::irq::without(|| {
            self.run_list.lock().retain(|rt| {
                if rt.thread.tid() == tid {
                    assert!(rt.thread.state() != State::Running);
                    return false;
                }
                true
            });
        });
    }

    fn redistribute(&self) {
        x86_64::irq::without(|| {
            self.run_list
                .lock()
                .iter_mut()
                .filter(|rt| rt.thread.priority() != thread::Priority::Idle)
                .for_each(|rt| rt.quantum = Self::QUANTUM);
        });
    }

    fn timer_tick(&self) {
        x86_64::irq::without(|| {
            let current_tid = thread::current().tid();
            let mut run_list = self.run_list.lock();
            let running = run_list
                .iter_mut()
                .find(|rt| rt.thread.tid() == current_tid)
                .unwrap();
            match running.quantum {
                0 => running.thread.set_need_rescheduling(),
                _ => running.quantum -= 1,
            }
        });
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}
