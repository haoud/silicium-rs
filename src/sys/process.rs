use crate::{arch::paging::TableRoot, sys::thread::Thread, Spinlock};
use alloc::{
    sync::{Arc, Weak},
    vec::Vec,
};
use core::{
    intrinsics::size_of,
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
};
use hashbrown::HashMap;
use spin::{Lazy, RwLock};

/// A vector to track all the processes in the system
static PROCESSES: Lazy<RwLock<HashMap<Pid, Arc<Spinlock<Process>>>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

/// A bitmap to track the PIDs status (free or used)
static PIDS: Spinlock<[u64; Pid::MAX / size_of::<u64>()]> =
    Spinlock::new([0; Pid::MAX / size_of::<u64>()]);

// An offset to start searching for free PIDs
static PIDS_OFFSET: AtomicU64 = AtomicU64::new(0);

// The number of used PIDs, to avoid searching the whole bitmap when there are no free PIDs
static PIDS_USED: AtomicUsize = AtomicUsize::new(0);

pub struct Process {
    pid: Pid,
    mm: Arc<Spinlock<TableRoot>>,
    threads: Spinlock<Vec<Thread>>,
    parent: Option<Weak<Spinlock<Process>>>,
    children: Spinlock<Vec<Arc<Spinlock<Process>>>>,
}

impl Process {
    /// Create a builder to create a new process
    #[must_use]
    pub fn builder(&self) -> Builder {
        Builder::new()
    }

    /// Add a thread to the process
    pub fn add_thread(&self, thread: Thread) {
        self.threads.lock().push(thread);
    }

    /// Add a child to the process
    pub fn add_child(&self, child: Pid) {
        self.children.lock().push(find(child).unwrap());
    }

    /// Remove a child from the process
    pub fn remove_child(&self, child: Pid) {
        self.children.lock().retain(|c| c.lock().pid != child);
    }

    /// Get the list of threads of the process
    #[must_use]
    pub const fn threads(&self) -> &Spinlock<Vec<Thread>> {
        &self.threads
    }

    /// Get the memory manager of the process
    #[must_use]
    pub const fn mm(&self) -> &Arc<Spinlock<TableRoot>> {
        &self.mm
    }

    /// Get the list of children of the process
    pub fn children(&self) -> Vec<Arc<Spinlock<Process>>> {
        self.children.lock().clone()
    }

    /// Get a child of the process by its PID. If the child doesn't exist, return `None`, otherwise
    /// return the child.
    pub fn child(&self, pid: Pid) -> Option<Arc<Spinlock<Process>>> {
        self.children
            .lock()
            .iter()
            .find(|c| c.lock().pid == pid)
            .map(Arc::clone)
    }

    /// Get the parent of the process. If the parent doesn't exist, return `None`, otherwise return
    /// the parent.
    #[must_use]
    pub fn parent(&self) -> Option<&Weak<Spinlock<Process>>> {
        self.parent.as_ref()
    }

    /// Get the PID of the parent of the process. If the parent doesn't exist, return `None`,
    /// otherwise return the PID of the parent.
    #[must_use]
    pub fn parent_id(&self) -> Option<Pid> {
        self.parent
            .as_ref()
            .map(|p| p.upgrade().unwrap().lock().pid)
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
        let init = find(Pid(1)).unwrap();
        for child in self.children.lock().drain(..) {
            init.lock().add_child(child.lock().pid);
            child.lock().parent = Some(Arc::downgrade(&init));
        }
        self.pid.release();
    }
}

pub struct Builder {
    process: Process,
}

impl Builder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            process: Process {
                parent: None,
                mm: Arc::new(Spinlock::new(TableRoot::new())),
                pid: Pid::generate().unwrap(),
                threads: Spinlock::new(Vec::new()),
                children: Spinlock::new(Vec::new()),
            },
        }
    }

    /// Add a bunch of threads to the process.
    #[must_use]
    pub fn add_threads(self, threads: Vec<Thread>) -> Self {
        self.process.threads.lock().extend(threads);
        self
    }

    /// Add a thread to the process.
    #[must_use]
    pub fn add_thread(self, thread: Thread) -> Self {
        self.process.threads.lock().push(thread);
        self
    }

    /// Set the parent of the process.
    #[must_use]
    pub fn parent(mut self, parent: &Arc<Spinlock<Process>>) -> Self {
        self.process.parent = Some(Arc::downgrade(parent));
        self
    }

    /// Create a new process, add it to the list of processes and return its PID.
    pub fn build(self) -> Pid {
        let mut processes = PROCESSES.write();
        let pid = self.process.pid;
        processes.insert(pid, Arc::new(Spinlock::new(self.process)));
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
            let pid = &mut PIDS.lock()[index];
            if *pid & (1 << off) == 0 {
                *pid |= 1 << off;
                return Some(Self(*pid));
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
    let locked = process.lock();
    Some(closure(&locked))
}

/// Find a process by its PID and return a Arc to it.
pub fn find(pid: Pid) -> Option<Arc<Spinlock<Process>>> {
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
    processes.remove(&pid);
}
