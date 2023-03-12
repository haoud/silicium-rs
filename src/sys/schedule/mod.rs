use alloc::sync::Arc;

use crate::arch::paging;

use super::thread::{self, Thread, Tid};

pub mod round_robin;

pub static SCHEDULER: round_robin::Scheduler = round_robin::Scheduler::new();

pub trait Scheduler {
    fn pick_next(&self) -> Option<Arc<Thread>>;

    fn add_thread(&self, thread: Arc<Thread>);
    fn remove_thread(&self, tid: Tid);

    fn redistribute(&self);
    fn timer_tick(&self);

    unsafe fn schedule(&self) {
        let current = thread::current();
        current.clear_need_rescheduling();

        let next = self
            .pick_next()
            .or_else(|| {
                self.redistribute();
                self.pick_next()
            })
            .unwrap();

        let current_tid = current.tid();
        let next_tid = next.tid();

        if current_tid != next_tid {
            if current.state() == thread::State::Running {
                current.set_state(thread::State::Ready);
            }

            // Change the mm if necessary.
            if let Some(mm) = next.mm() {
                paging::set_current_table(mm);
            }

            thread::set_current(&next);

            // TODO: Explain why this is safe to look and then switch to.
            x86_64::cpu::switch(&mut current.cpu_state().write(), &next.cpu_state().read());
        }
    }
}
