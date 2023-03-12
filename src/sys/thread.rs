use alloc::sync::{Arc, Weak};
use bitflags::bitflags;
use core::{
    intrinsics::size_of,
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
};
use spin::Lazy;
use x86_64::{address::VirtualRange, cpu, paging::PAGE_SIZE, segment::Selector};

use crate::{mm::vmm, Spinlock};

use super::process::Process;

#[thread_local]
static CURRENT_THREAD: Lazy<Spinlock<Arc<Spinlock<Thread>>>> = Lazy::new(|| {
    Spinlock::new(Arc::new(Spinlock::new(
        Thread::builder()
            .entry_point(idle as usize)
            .kstack_size(PAGE_SIZE)
            .kind(Type::Kernel)
            .build()
            .unwrap(),
    )))
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
#[derive(Debug)]
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
    flags: Flags,
    exit_code: Option<i32>,
    exit_signal: Option<i32>,

    state: State,
    cpu_state: cpu::State,

    kstack: Option<VirtualRange>,
    mm: Option<Arc<Spinlock<Process>>>,
    process: Option<Weak<Spinlock<Process>>>,
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
    pub fn set_parent(&mut self, parent: Option<&Arc<Spinlock<Process>>>) {
        self.process = parent.map(Arc::downgrade);
    }

    /// Zombify the thread. This will set the exit code and signal, and will free the memory
    /// associated with the thread (kernel stack, memory manager, etc.)
    pub fn zombify(&mut self, exit_code: i32, exit_signal: i32) {
        self.exit_signal = Some(exit_signal);
        self.exit_code = Some(exit_code);
        self.state = State::Zombie;

        // Drop the memory manager, the kernel stack will
        vmm::deallocate(self.kstack.unwrap());
        self.kstack = None;
        self.mm = None;
    }

    /// Get a mutable reference to the CPU state of the thread. See `get_cpu_state()` for more
    /// information.
    #[must_use]
    pub fn cpu_state_mut(&mut self) -> &mut cpu::State {
        &mut self.cpu_state
    }

    /// Get a reference to the CPU state of the thread. This is used to save and restore the CPU
    /// state of the thread. The CPU state is only relevant when the thread is not running.
    #[must_use]
    pub fn cpu_state(&self) -> &cpu::State {
        &self.cpu_state
    }

    /// Set the reschedule flag for the thread. This will cause the thread to be rescheduled as soon
    /// as possible.
    pub fn set_need_rescheduling(&mut self) {
        self.flags |= Flags::NEED_SCHEDULING;
    }

    /// Check if the thread need to be rescheduled.
    #[must_use]
    pub fn need_rescheduling(&self) -> bool {
        self.flags.contains(Flags::NEED_SCHEDULING)
    }

    /// Returns the exit signal of the thread, if any.
    #[must_use]
    pub fn exit_signal(&self) -> Option<i32> {
        self.exit_signal
    }

    /// Returns the exit code of the thread, if any.
    #[must_use]
    pub fn exit_code(&self) -> Option<i32> {
        self.exit_code
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
                kind: Type::User,
                flags: Flags::NONE,
                process: None,
                mm: None,
                kstack: None,
                exit_code: None,
                exit_signal: None,
                state: State::Created,
                cpu_state: cpu::State::default(),
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
    pub fn mm(mut self, mm: &Arc<Spinlock<Process>>) -> Self {
        self.thread.mm = Some(Arc::clone(mm));
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
        self.thread.cpu_state.rip = self.entry_point as u64;
        match self.thread.kind {
            Type::User => {
                self.thread.cpu_state.cs = u64::from(Selector::USER_CODE64.value());
                self.thread.cpu_state.ss = u64::from(Selector::USER_DATA.value());
                self.thread.cpu_state.rsp = Thread::USER_STACK_TOP_ALIGNED as u64;
                todo!(); // Allocate the user stack
            }
            Type::Kernel => {
                self.thread.cpu_state.cs = u64::from(Selector::KERNEL_CODE64.value());
                self.thread.cpu_state.ss = u64::from(Selector::NULL.value());
                self.thread.cpu_state.rsp = self.thread.kstack.unwrap().end().as_u64();
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
            let tid = &mut TIDS.lock()[index];
            if *tid & (1 << off) == 0 {
                *tid |= 1 << off;
                return Some(Self(*tid));
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

/// Set the current thread.
pub fn set_current(thread: &Arc<Spinlock<Thread>>) {
    *CURRENT_THREAD.lock() = Arc::clone(thread);
}

/// Returns the current thread.
pub fn current() -> Arc<Spinlock<Thread>> {
    Arc::clone(&*CURRENT_THREAD.lock())
}

/// The idle thread. It's the thread that is executed when there is no other thread to execute.
fn idle() -> ! {
    x86_64::irq::enable();
    loop {
        unsafe {
            x86_64::cpu::hlt();
        }
    }
}
