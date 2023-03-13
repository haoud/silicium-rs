use crate::{arch::paging::TableRoot, sys::thread::Thread, Spinlock};
use alloc::{sync::Arc, vec::Vec};
use core::{
    intrinsics::size_of,
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
};
use hashbrown::HashMap;
use spin::{Lazy, RwLock};
use x86_64::cpu::State;

use super::{
    schedule::{Scheduler, SCHEDULER},
    thread::{self, Tid},
};

/// A vector to track all the processes in the system
static PROCESSES: Lazy<RwLock<HashMap<Pid, Arc<Process>>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

/// A bitmap to track the PIDs status (free or used)
static PIDS: Spinlock<[u64; Pid::MAX / size_of::<u64>()]> =
    Spinlock::new([0; Pid::MAX / size_of::<u64>()]);

// An offset to start searching for free PIDs
static PIDS_OFFSET: AtomicU64 = AtomicU64::new(0);

// The number of used PIDs, to avoid searching the whole bitmap when there are no free PIDs
static PIDS_USED: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug)]
pub struct Process {
    pid: Pid,
    mm: Arc<Spinlock<TableRoot>>,
    parent: Spinlock<Option<Pid>>,
    children: Spinlock<Vec<Arc<Process>>>,
    threads: Spinlock<Vec<Arc<Thread>>>,
}

impl Process {
    /// Create a builder to create a new process
    #[must_use]

    pub fn builder(&self) -> Builder {
        Builder::new()
    }

    /// Add a thread to the process
    pub fn add_thread(&self, thread: Thread) {
        let thread = Arc::new(thread);
        self.threads.lock().push(Arc::clone(&thread));
        SCHEDULER.add_thread(thread);
    }

    /// Add a child to the process
    pub fn add_child(&self, child: Pid) {
        self.children.lock().push(find(child).unwrap());
    }

    // Find a thread in the process by its TID. If the thread doesn't exist, return `None`,
    // otherwise return the thread.
    pub fn thread(&self, tid: Tid) -> Option<Arc<Thread>> {
        self.threads
            .lock()
            .iter()
            .find(|t| t.tid() == tid)
            .map(Arc::clone)
    }

    /// Remove a thread from the process and drop it
    pub fn remove_thread(&self, tid: Tid) {
        self.threads.lock().retain_mut(|t| {
            if t.tid() == tid {
                t.set_parent(None);
                false
            } else {
                true
            }
        });
    }

    pub fn set_parent(&self, parent: Option<&Arc<Process>>) {
        *self.parent.lock() = parent.map(|p| p.pid);
    }

    /// Remove a child from the process
    pub fn remove_child(&self, child: Pid) {
        self.children.lock().retain(|c| c.pid != child);
    }

    /// Get the memory manager of the process
    #[must_use]
    pub const fn mm(&self) -> &Arc<Spinlock<TableRoot>> {
        &self.mm
    }

    /// Get the list of children of the process
    pub fn children(&self) -> Vec<Arc<Process>> {
        self.children.lock().clone()
    }

    /// Get a child of the process by its PID. If the child doesn't exist, return `None`, otherwise
    /// return the child.
    pub fn child(&self, pid: Pid) -> Option<Arc<Process>> {
        self.children
            .lock()
            .iter()
            .find(|c| c.pid == pid)
            .map(Arc::clone)
    }

    /// Get the parent of the process. If the parent doesn't exist, return `None`, otherwise return
    /// the parent.
    #[must_use]
    pub fn parent(&self) -> Option<Arc<Process>> {
        self.parent_id().and_then(find)
    }

    /// Get the PID of the parent of the process. If the parent doesn't exist, return `None`,
    /// otherwise return the PID of the parent.
    #[must_use]
    pub fn parent_id(&self) -> Option<Pid> {
        *self.parent.lock()
    }

    /// Get the PID of the process
    #[must_use]
    pub const fn pid(&self) -> &Pid {
        &self.pid
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        // Add all the children to the init process, to avoid orphan processes. Therefore, all
        // processes will have a parent, and we can safely use `unwrap` to access the parent every
        // time.

        self.pid.release();
    }
}

#[derive(Debug)]
pub struct Builder {
    process: Process,
}

impl Builder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            process: Process {
                parent: Spinlock::new(None),
                mm: Arc::new(Spinlock::new(TableRoot::new())),
                pid: Pid::generate().unwrap(),
                threads: Spinlock::new(Vec::new()),
                children: Spinlock::new(Vec::new()),
            },
        }
    }

    /// Add a thread to the process, and put it in the ready queue.
    #[must_use]
    pub fn add_thread(self, thread: Thread) -> Self {
        self.process.add_thread(thread);
        self
    }

    /// Set the parent of the process.
    #[must_use]
    pub fn parent(mut self, parent: &Arc<Process>) -> Self {
        self.process.parent = Spinlock::new(Some(parent.pid));
        self
    }

    /// Create a new process, add it to the list of processes and return its PID.
    pub fn build(self) -> Pid {
        let mut processes = PROCESSES.write();
        let pid = self.process.pid;
        processes.insert(pid, Arc::new(self.process));
        pid
    }
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents a PID. A PID is a 64-bit unsigned integer, but only the first 15 bits are used (the
/// maximum number of processes is 32768). The PID is used to identify a process and therefore it's
/// unique for each process.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pid(u64);

impl Pid {
    pub const MAX: usize = 32768;

    /// Create a new PID from a raw value.
    #[must_use]
    pub fn new(pid: u64) -> Option<Self> {
        if pid >= Self::MAX as u64 {
            return None;
        }
        Some(Self(pid))
    }

    /// Generate a new unique PID. If all the PIDs are used, return `None`.
    #[must_use]
    fn generate() -> Option<Self> {
        // If all the PIDs are used, return `None`
        if (PIDS_USED.fetch_add(1, Ordering::SeqCst) + 1) >= Self::MAX {
            PIDS_USED.fetch_sub(1, Ordering::SeqCst);
            return None;
        }

        // Find a free pid starting from the offset, and wrap around if the PID is marked
        // as used in the bitmap.
        loop {
            let pid = PIDS_OFFSET.fetch_add(1, Ordering::SeqCst) % Self::MAX as u64;
            let index = usize::try_from(pid).unwrap() / size_of::<u64>();
            let off = usize::try_from(pid).unwrap() % size_of::<u64>();
            let x = &mut PIDS.lock()[index];
            if *x & (1 << off) == 0 {
                *x |= 1 << off;
                return Some(Self(pid));
            }
        }
    }

    /// Release the PID, so it can be used again.
    fn release(self) {
        let index = usize::try_from(self.0).unwrap() / size_of::<u64>();
        let off = usize::try_from(self.0).unwrap() % size_of::<u64>();
        let pid = &mut PIDS.lock()[index];
        *pid &= !(1 << off);
    }
}

/// Borrow a process to do some computation on it with the closure, and return the result.
/// During all the computation, the process is locked and no other thread can access it, so
/// the closure must be as short as possible.
pub fn borrow<C, R>(pid: Pid, closure: C) -> Option<R>
where
    C: FnOnce(&Process) -> R,
{
    let process = { Arc::clone(PROCESSES.read().get(&pid)?) };
    Some(closure(&process))
}

/// Find a process by its PID and return a Arc to it.
pub fn find(pid: Pid) -> Option<Arc<Process>> {
    let processes = PROCESSES.read();
    Some(Arc::clone(processes.get(&pid)?))
}

/// Check if a process exists.
pub fn exists(pid: Pid) -> bool {
    PROCESSES.read().contains_key(&pid)
}

/// Delete a process from the list of processes. If the process is not found, nothing happens.
/// If this was the last reference to the process, it will be dropped.
pub fn delete(pid: Pid) {
    let mut processes = PROCESSES.write();
    let process = processes.remove(&pid).unwrap();

    let init = find(Pid(1)).unwrap();
    for child in process.children.lock().drain(..) {
        child.set_parent(Some(&init));
        init.add_child(child.pid);
    }
}

unsafe fn a() -> ! {
    loop {
        log::info!("A");

    }
}

unsafe fn b() -> ! {
    loop {
        log::info!("B");

     }
}

unsafe fn c() -> ! {
    loop {
        log::info!("C");
    }
}

pub fn setup() {
    // Create the idle process
    Builder::new().build();

    // Create the init process
    Builder::new()
        .add_thread(
            super::thread::Builder::new()
                .entry_point(a as usize)
                .kind(thread::Type::Kernel)
                .build()
                .unwrap(),
        )
        .build();

    Builder::new()
        .add_thread(
            super::thread::Builder::new()
                .entry_point(b as usize)
                .kind(thread::Type::Kernel)
                .build()
                .unwrap(),
        )
        .build();

    Builder::new()
        .add_thread(
            super::thread::Builder::new()
                .entry_point(c as usize)
                .kind(thread::Type::Kernel)
                .build()
                .unwrap(),
        )
        .build();
}

pub fn run_idle() -> ! {
    let mut state = State::default();
    let idle = thread::current();
    unsafe {
        let idle_state = idle.cpu_state().write();
        idle.cpu_state().force_write_unlock();
        x86_64::cpu::switch(&mut state, &idle_state);
        unreachable!();
    }
}
