use core::alloc::{GlobalAlloc, Layout};
use core::ops::Deref;
use core::ptr;

use crate::Spinlock;

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
