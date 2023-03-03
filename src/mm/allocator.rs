use core::alloc::{GlobalAlloc, Layout};
use core::ops::Deref;
use core::ptr;
use sync::spin::SpinlockIrq;

pub struct Locked {
    inner: SpinlockIrq<linked_list_allocator::Heap>,
}

impl Locked {
    #[must_use]
    pub const fn new(inner: linked_list_allocator::Heap) -> Self {
        Locked {
            inner: SpinlockIrq::new(inner),
        }
    }
}

impl Deref for Locked {
    type Target = SpinlockIrq<linked_list_allocator::Heap>;

    fn deref(&self) -> &SpinlockIrq<linked_list_allocator::Heap> {
        &self.inner
    }
}

unsafe impl GlobalAlloc for Locked {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.inner
            .lock()
            .allocate_first_fit(layout)
            .ok()
            .map_or(ptr::null_mut(), core::ptr::NonNull::as_ptr)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.inner
            .lock()
            .deallocate(ptr::NonNull::new_unchecked(ptr), layout);
    }
}
