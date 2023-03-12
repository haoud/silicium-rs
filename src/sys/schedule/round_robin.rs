use alloc::{sync::Arc, vec::Vec};

use crate::{
    sys::thread::{self, State, Thread, Tid},
    Spinlock,
};

pub struct RunnableThread {
    thread: Arc<Thread>,
    quantum: u64,
}

pub struct Scheduler {
    run_list: Spinlock<Vec<RunnableThread>>,
}

impl Scheduler {
    const QUANTUM: u64 = 100;

    #[must_use]
    pub const fn new() -> Self {
        Self {
            run_list: Spinlock::new(Vec::new()),
        }
    }
}

impl super::Scheduler for Scheduler {
    fn pick_next(&self) -> Option<Arc<Thread>> {
        self.run_list
            .lock()
            .iter()
            .find(|rt| rt.quantum > 0)
            .map(|rt| Arc::clone(&rt.thread))
    }

    fn add_thread(&self, thread: Arc<Thread>) {
        thread.set_state(State::Ready);
        self.run_list.lock().push(RunnableThread {
            quantum: Self::QUANTUM,
            thread,
        });
    }

    fn remove_thread(&self, tid: Tid) {
        self.run_list.lock().retain(|rt| rt.thread.tid() != tid);
    }

    fn redistribute(&self) {
        self.run_list
            .lock()
            .iter_mut()
            .for_each(|rt| rt.quantum = Self::QUANTUM);
    }

    fn timer_tick(&self) {
        let current_tid = thread::current().tid();
        let mut run_list = self.run_list.lock();
        let running = run_list
            .iter_mut()
            .find(|rt| rt.thread.tid() == current_tid)
            .unwrap();

        if running.quantum == 0 {
            running.thread.set_need_rescheduling();
        } else {
            running.quantum -= 1;
        }
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}
