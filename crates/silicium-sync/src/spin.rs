use core::sync::atomic::{AtomicBool, Ordering};
use lock_api::{GuardSend, RawMutex};
use silicium_x86_64 as x86_64;

/// A spinlock that disables interrupts on the current core while it is locked.
/// 
/// # Why this spinlock disables interrupts during locking ?
/// If we lock a spinlock while interrupts are enabled, an interrupt handler could be called while
/// the spinlock is locked. If the interrupt handler also tries to lock the same spinlock AND the
/// interrupt handler is called from the same CPU core, we would have a guaranteed deadlock.
/// So if your lock is used in an interrupt handler, you should use this spinlock instead of the
/// `Spinlock` type.
pub struct RawSpinlockIrq(AtomicBool, AtomicBool);

unsafe impl RawMutex for RawSpinlockIrq {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: RawSpinlockIrq = RawSpinlockIrq(AtomicBool::new(false), AtomicBool::new(false));
    type GuardMarker = GuardSend;

    /// Disables interrupts and waits until the lock is acquired.
    fn lock(&self) {
        while !self.try_lock() {
            core::hint::spin_loop();
        }
    }

    /// Disables interrupts and tries to acquire the lock. Returns `true` if the lock was acquired.
    /// If the lock was already acquired by another thread, this function returns `false` and
    /// interrupts are restored to their previous state.
    fn try_lock(&self) -> bool {
        self.1.store(x86_64::interrupts::enabled(), Ordering::Relaxed);
        x86_64::interrupts::disable();
        let b = self.0
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok();
        if !b {
            x86_64::interrupts::restore(self.1.load(Ordering::Relaxed));
        }
        b
    }

    /// Releases the lock and restores the interrupt state before the lock was acquired.
    unsafe fn unlock(&self) {
        self.0.store(false, Ordering::Release);
        x86_64::interrupts::restore(self.1.load(Ordering::Relaxed));
    }

    /// Returns `true` if the lock is currently acquired, `false` otherwise.
    fn is_locked(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

pub type Spinlock<T> = lock_api::Mutex<RawSpinlock, T>;
pub type SpinlockGuard<'a, T> = lock_api::MutexGuard<'a, RawSpinlock, T>;

pub struct RawSpinlock(AtomicBool);

unsafe impl RawMutex for RawSpinlock {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: RawSpinlock = RawSpinlock(AtomicBool::new(false));
    type GuardMarker = GuardSend;

    /// Waits until the lock is acquired and locks it.
    fn lock(&self) {
        while !self.try_lock() {
            core::hint::spin_loop();
        }
    }

    /// Tries to acquire the lock. Returns `true` if the lock was acquired, `false` otherwise.
    fn try_lock(&self) -> bool {
        self.0
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    /// Releases the lock
    unsafe fn unlock(&self) {
        self.0.store(false, Ordering::Release);
    }

    /// Returns `true` if the lock is currently acquired, `false` otherwise.
    fn is_locked(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

pub type SpinlockIrq<T> = lock_api::Mutex<RawSpinlockIrq, T>;
pub type SpinlockIrqGuard<'a, T> = lock_api::MutexGuard<'a, RawSpinlockIrq, T>;
