use core::alloc::{GlobalAlloc, Layout};
use core::ops::Deref;
use core::ptr;

use x86_64::address::Virtual;
use x86_64::paging::PageTable;

use crate::arch::paging::{self, MapError, MapFlags, PageFaultError};
use crate::mm::FRAME_ALLOCATOR;
use crate::Spinlock;

use super::frame::{self, Allocator};

pub struct Locked {
    inner: Spinlock<linked_list_allocator::Heap>,
}

impl Locked {
    #[must_use]
    pub const fn new(inner: linked_list_allocator::Heap) -> Self {
        Locked {
            inner: Spinlock::new(inner),
        }
    }
}

impl Deref for Locked {
    type Target = Spinlock<linked_list_allocator::Heap>;

    fn deref(&self) -> &Spinlock<linked_list_allocator::Heap> {
        &self.inner
    }
}

unsafe impl GlobalAlloc for Locked {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        x86_64::irq::without(|| {
            self.inner
                .lock()
                .allocate_first_fit(layout)
                .ok()
                .map_or(ptr::null_mut(), core::ptr::NonNull::as_ptr)
        })
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        x86_64::irq::without(|| {
            self.inner
                .lock()
                .deallocate(ptr::NonNull::new_unchecked(ptr), layout);
        });
    }
}

/// Handle a page fault that occured in the heap area that is a demand-paging request. It simply
/// allocates a new frame and maps it to the requested address with RW permissions, and enable the
/// NX bit to avoid code execution from the heap.
///
/// # Errors
/// - `PageFaultError::OUT_OF_MEMORY` if the allocator is out of memory.
/// - `PageFaultError::ALREADY_MAPPED` if the page is already mapped, which should not happen
/// in a demand-paging request.
pub fn handle_demand_paging(table: &mut PageTable, addr: Virtual) -> Result<(), PageFaultError> {
    let paging_flags: MapFlags = MapFlags::PRESENT | MapFlags::WRITABLE | MapFlags::NO_EXECUTE;
    let alloc_flags = frame::AllocationFlags::KERNEL | frame::AllocationFlags::ZEROED;

    unsafe {
        let frame = x86_64::irq::without(|| {
            FRAME_ALLOCATOR
                .lock()
                .allocate(alloc_flags)
                .ok_or(PageFaultError::OUT_OF_MEMORY)
        })?;

        paging::map(table, addr, frame, paging_flags).map_err(|err| match err {
            MapError::OutOfMemory => PageFaultError::OUT_OF_MEMORY,
            MapError::AlreadyMapped => PageFaultError::ALREADY_MAPPED,
        })?;
    }
    Ok(())
}
