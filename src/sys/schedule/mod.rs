use alloc::sync::Arc;

use crate::arch::paging;

use super::thread::{self, Thread, Tid};

pub mod round_robin;

pub static SCHEDULER: round_robin::Scheduler = round_robin::Scheduler::new();

pub trait Scheduler {
    fn pick_next(&self) -> Option<Arc<Thread>>;
    fn pick_idle(&self) -> Arc<Thread>;

    fn add_thread(&self, thread: Arc<Thread>);
    fn remove_thread(&self, tid: Tid);

    fn redistribute(&self);
    fn timer_tick(&self);

    /// Schedule the current thread, and run the next thread.
    ///
    /// TODO: Use a variable to disable preemption, to avoid being preempted while we are
    /// in kernel code. Preempt point will be the only place where we can be preempted, and
    /// will be added in the future. This will greatly improve the stability and the simplicity
    /// of the kernel.
    ///
    /// # Safety
    /// This is probably the most unsafe function in the whole kernel and relies on a few tricks to
    /// work. If a unexpected behavior occurs in the kernel, this function is the first place to
    /// look.
    unsafe fn schedule(&self) {
        x86_64::irq::without(|| {
            let current = thread::current();
            // This is a closure that will redistribute the threads if the run queue is empty, and
            // finally pick the next thread to run. If the run queue is still empty, it will return
            // `None`. In that case, we panic if the current thread is not the idle thread, because
            // this is not supposed to happen (idle threads should be always ready to run). If the
            // current thread is the idle thread, we just return, because there is nothing better to
            // do.
            let retry = || {
                self.redistribute();
                self.pick_next()
            };

            let next: Arc<Thread> = if let Some(next) = self.pick_next().or_else(retry) {
                next
            } else {
                if current.priority() == thread::Priority::Idle {
                    current.clear_need_rescheduling();
                    return;
                }
                self.pick_idle()
            };

            if current.tid() != next.tid() {
                log::debug!(
                    "Switching from thread {:?} to {:?}",
                    current.tid(),
                    next.tid()
                );
                // Change the mm if necessary.
                if let Some(mm) = next.mm() {
                    paging::set_current_table(mm);
                }

                current.clear_need_rescheduling();
                thread::set_current(&next);

                next.cpu_state().force_write_unlock();
                current.cpu_state().force_write_unlock();
                let next_state = next.cpu_state().write();
                let mut current_state = current.cpu_state().write();

                //
                x86_64::cpu::switch(&mut current_state, &next_state);
                core::mem::forget(current_state);
                core::mem::forget(next_state);
            }
        });
    }
}
