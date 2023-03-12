use alloc::sync::{Arc, Weak};
use bitflags::bitflags;
use core::{
    intrinsics::size_of,
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
};
use spin::{Lazy, RwLock};
use x86_64::{address::VirtualRange, cpu, paging::PAGE_SIZE, segment::Selector};

use crate::{arch::paging::TableRoot, mm::vmm, Spinlock};

use super::{process::{self, Pid, Process}, schedule::{SCHEDULER, Scheduler}};

#[thread_local]
static CURRENT_THREAD: Lazy<Spinlock<Arc<Thread>>> = Lazy::new(|| {
    let parent = process::find(Pid::new(0).unwrap()).unwrap();
    parent.add_thread(Thread::builder()
    .entry_point(idle as usize)
    .priority(Priority::Idle)
    .kstack_size(PAGE_SIZE)
    .kind(Type::Kernel)
    .build()
    .unwrap());
    let thread = parent.thread(Tid::new(0).unwrap()).unwrap();
    SCHEDULER.add_thread(Arc::clone(&thread));
    Spinlock::new(thread)
});

/// A bitmap to track the TIDs status (free or used)
static TIDS: Spinlock<[u64; Tid::MAX / size_of::<u64>()]> =
    Spinlock::new([0; Tid::MAX / size_of::<u64>()]);

// An offset to start searching for free TIDs
static TIDS_OFFSET: AtomicU64 = AtomicU64::new(0);

// The number of used TIDs, to avoid searching the whole bitmap when there are no free TIDs
static TIDS_USED: AtomicUsize = AtomicUsize::new(0);

/// The type of a thread
#[derive(Debug)]
pub enum Type {
    User,
    Kernel,
}

/// The state of a thread
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// The thread is created, but not yet ready to run
    Created,

    /// The thread is ready to run
    Ready,

    /// The thread is running
    Running,

    /// The thread is blocked
    Blocked,

    /// The thread sleeps and can be woken up by a signal
    Waiting,

    // The thread sleeps, but cannot be woken up by an signal
    Sleeping,

    /// The thread is terminated, but we need to keep it in the thread list
    Zombie,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

bitflags! {
    /// A set of flags for a thread
    pub struct Flags : u64 {
        const NONE = 0;

        /// Set if the thread need to be rescheduled
        const NEED_SCHEDULING = 1 << 0;
    }
}

/// Represents a thread. A thread is a lightweight process that shares the same address space with
/// other threads, or can even use the address space of another process in the case of a kernel
/// thread.
#[derive(Debug)]
pub struct Thread {
    tid: Tid,
    kind: Type,
    flags: Spinlock<Flags>,
    priority: Spinlock<Priority>,
    exit_code: Spinlock<Option<i32>>,
    exit_signal: Spinlock<Option<i32>>,

    state: Spinlock<State>,
    cpu_state: RwLock<cpu::State>,

    kstack: Option<VirtualRange>,
    process: Spinlock<Option<Weak<Process>>>,
    mm: Option<Arc<Spinlock<TableRoot>>>,
}

impl Thread {
    pub const USER_STACK_BOTTOM: usize = Self::USER_STACK_TOP - Self::USER_STACK_SIZE;
    pub const USER_STACK_TOP_ALIGNED: usize = Self::USER_STACK_TOP & !0xF;
    pub const USER_STACK_TOP: usize = 0x0000_7FFF_FFFF_FFFF;
    pub const USER_STACK_SIZE: usize = 8 * 1024 * 1024;

    pub const DEFAULT_KSTACK_SIZE: usize = 32 * 1024;

    /// Returns a builder to create a new thread.
    #[must_use]
    pub fn builder() -> Builder {
        Builder::new()
    }

    /// Set the parent process of the thread.
    pub fn set_parent(&self, parent: Option<&Arc<Process>>) {
        *self.process.lock() = parent.map(Arc::downgrade);
    }

    /// Zombify the thread. This will set the exit code and signal, and will free the memory
    /// associated with the thread (kernel stack, memory manager, etc.)
    pub fn zombify(&mut self, exit_code: i32, exit_signal: i32) {
        *self.exit_signal.lock() = Some(exit_signal);
        *self.exit_code.lock() = Some(exit_code);
        self.set_state(State::Zombie);

        // Drop the memory manager, the kernel stack will
        vmm::deallocate(self.kstack.unwrap());
        self.kstack = None;
        self.mm = None;
    }

    /// Get a reference to the CPU state of the thread. This is used to save and restore the CPU
    /// state of the thread. The CPU state is only relevant when the thread is not running.
    #[must_use]
    pub fn cpu_state(&self) -> &RwLock<cpu::State> {
        &self.cpu_state
    }

    /// Set the reschedule flag for the thread. This will cause the thread to be rescheduled as soon
    /// as possible.
    pub fn set_need_rescheduling(&self) {
        *self.flags.lock() |= Flags::NEED_SCHEDULING;
    }

    /// Clear the reschedule flag for the thread.
    pub fn clear_need_rescheduling(&self) {
        *self.flags.lock() &= !Flags::NEED_SCHEDULING;
    }

    /// Check if the thread need to be rescheduled.
    #[must_use]
    pub fn need_rescheduling(&self) -> bool {
        self.flags.lock().contains(Flags::NEED_SCHEDULING)
    }

    /// Returns the exit signal of the thread, if any.
    #[must_use]
    pub fn exit_signal(&self) -> Option<i32> {
        *self.exit_signal.lock()
    }

    /// Returns the exit code of the thread, if any.
    #[must_use]
    pub fn exit_code(&self) -> Option<i32> {
        *self.exit_code.lock()
    }

    /// Set the state of the thread.
    pub fn set_state(&self, state: State) {
        *self.state.lock() = state;
    }

    pub fn mm(&self) -> Option<&Arc<Spinlock<TableRoot>>> {
        self.mm.as_ref()
    }

    /// Returns the state of the thread.
    #[must_use]
    pub fn state(&self) -> State {
        *self.state.lock()
    }

    /// Returns the kind of the thread.
    #[must_use]
    pub fn kind(&self) -> &Type {
        &self.kind
    }

    /// Returns the TID of the thread.
    #[must_use]
    pub fn tid(&self) -> Tid {
        self.tid
    }
}

impl Drop for Thread {
    fn drop(&mut self) {
        self.tid.release();
        if let Some(stack) = self.kstack {
            vmm::deallocate(stack);
        }
    }
}

/// Represents all possible errors when creating a thread.
#[derive(Debug)]
pub enum CreationError {
    OutOfMemory,
    NoFreeTid,
}

/// A builder to create a new thread.
pub struct Builder {
    entry_point: usize,
    kstack_size: usize,
    thread: Thread,
}

impl Builder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            thread: Thread {
                tid: Tid(0),
                mm: None,
                kstack: None,
                kind: Type::User,
                flags: Spinlock::new(Flags::NONE),
                process: Spinlock::new(None),
                exit_code: Spinlock::new(None),
                exit_signal: Spinlock::new(None),
                state: Spinlock::new(State::Created),
                priority: Spinlock::new(Priority::Normal),
                cpu_state: RwLock::new(cpu::State::default()),
            },
            entry_point: 0,
            kstack_size: 0,
        }
    }

    /// Set the kind of the thread.
    #[must_use]
    pub fn kind(mut self, kind: Type) -> Self {
        self.thread.kind = kind;
        self
    }

    /// Set the memory manager of the thread.
    #[must_use]
    pub fn mm(mut self, mm: &Arc<Spinlock<TableRoot>>) -> Self {
        self.thread.mm = Some(Arc::clone(mm));
        self
    }

    /// Set the priority of the thread.
    #[must_use]
    #[allow(unused_mut)]
    pub fn priority(mut self, priority: Priority) -> Self {
        *self.thread.priority.lock() = priority;
        self
    }

    /// Set the entry point of the thread.
    #[must_use]
    pub fn entry_point(mut self, entry_point: usize) -> Self {
        self.entry_point = entry_point;
        self
    }

    /// Set the size of the kernel stack.
    #[must_use]
    pub fn kstack_size(mut self, kstack_size: usize) -> Self {
        self.kstack_size = kstack_size;
        self
    }

    /// # Errors
    /// - `NoFreeTid`: There is no free TID, the maximum number of threads has been reached.
    /// - `OutOfMemory`: The kernel stack could not be allocated because there is no more memory.
    pub fn build(mut self) -> Result<Thread, CreationError> {
        // Allocate a TID
        self.thread.tid = Tid::generate().ok_or(CreationError::NoFreeTid)?;

        // Allocate the kernel stack
        let alloc_flags = vmm::AllocationFlags::NONE
            | vmm::AllocationFlags::ATOMIC
            | vmm::AllocationFlags::MAP
            | vmm::AllocationFlags::ZEROED;
        let kstack = vmm::allocate(self.kstack_size, alloc_flags).map_err(|_| {
            self.thread.tid.release();
            CreationError::OutOfMemory
        })?;
        self.thread.kstack = Some(kstack);

        // Set the CPU state
        {
            let mut cpu_state = self.thread.cpu_state.write();
            cpu_state.rip = self.entry_point as u64;
            match self.thread.kind {
                Type::User => {
                    cpu_state.cs = u64::from(Selector::USER_CODE64.value());
                    cpu_state.ss = u64::from(Selector::USER_DATA.value());
                    cpu_state.rsp = Thread::USER_STACK_TOP_ALIGNED as u64;
                    todo!(); // Allocate the user stack
                }
                Type::Kernel => {
                    cpu_state.cs = u64::from(Selector::KERNEL_CODE64.value());
                    cpu_state.ss = u64::from(Selector::NULL.value());
                    cpu_state.rsp = self.thread.kstack.unwrap().end().as_u64();
                }
            }
        }
        Ok(self.thread)
    }
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents a tid. A tid is a 64-bit unsigned integer, but only the first 15 bits are used (the
/// maximum number of thread is 32768). The TID is used to identify a thread and therefore it's
/// unique for each thread.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tid(u64);

impl Tid {
    pub const MAX: usize = 32768;

    /// Create a new TID from a raw value.
    #[must_use]
    pub fn new(tid: u64) -> Option<Self> {
        if tid >= Self::MAX as u64 {
            return None;
        }
        Some(Self(tid))
    }

    /// Generate a new unique TID. If all the tids are used, return `None`.
    #[must_use]
    fn generate() -> Option<Self> {
        // If all the tids are used, return `None`
        if (TIDS_USED.fetch_add(1, Ordering::SeqCst) + 1) >= Self::MAX {
            TIDS_USED.fetch_sub(1, Ordering::SeqCst);
            return None;
        }

        // Find a free TID starting from the offset, and wrap around if the TID is marked
        // as used in the bitmap.
        loop {
            let tid = TIDS_OFFSET.fetch_add(1, Ordering::SeqCst) % Self::MAX as u64;
            let index = usize::try_from(tid).unwrap() / size_of::<u64>();
            let off = usize::try_from(tid).unwrap() % size_of::<u64>();
            let x = &mut TIDS.lock()[index];
            if *x & (1 << off) == 0 {
                *x |= 1 << off;
                return Some(Self(tid));
            }
        }
    }

    /// Release the TID, so it can be used again.
    fn release(self) {
        let index = usize::try_from(self.0).unwrap() / size_of::<u64>();
        let off = usize::try_from(self.0).unwrap() % size_of::<u64>();
        let tid = &mut TIDS.lock()[index];
        *tid &= !(1 << off);
    }
}

/// Set the thread as the current thread, and set its state to `Running`.
pub fn set_current(thread: &Arc<Thread>) {
    *CURRENT_THREAD.lock() = Arc::clone(thread);
    thread.set_state(State::Running);
}

/// Returns the current thread.
pub fn current() -> Arc<Thread> {
    Arc::clone(&*CURRENT_THREAD.lock())
}

/// The idle thread. It's the thread that is executed when there is no other thread to execute.
pub fn idle() -> ! {
    x86_64::irq::enable();
    loop {
        unsafe {
            log::debug!("Idle thread (CPU {})", crate::arch::smp::current_id());
            x86_64::cpu::hlt();
        }
    }
}
