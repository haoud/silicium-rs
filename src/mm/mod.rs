use crate::{arch::paging, sync::spin::SpinlockIrq};
use frame::Allocator;
use limine::LimineMemmapRequest;

pub mod allocator;
pub mod frame;

pub const KERNEL_BASE: u64 = 0xFFFF_8000_0000_0000;

pub const HHDM_START: u64 = 0xFFFF_8000_0000_0000;
pub const HHDM_END: u64 = 0xFFFF_9000_0000_0000;
pub const HEAP_START: u64 = 0xFFFF_9000_0000_0000;
pub const HEAP_END: u64 = 0xFFFF_A000_0000_0000;
pub const VMALLOC_START: u64 = 0xFFFF_A000_0000_0000;
pub const VMALLOC_END: u64 = 0xFFFF_B000_0000_0000;

#[allow(clippy::cast_possible_truncation)]
pub const HHDM_SIZE: usize = (HEAP_END - HEAP_START) as usize;
#[allow(clippy::cast_possible_truncation)]
pub const HEAP_SIZE: usize = (HEAP_END - HEAP_START) as usize;
#[allow(clippy::cast_possible_truncation)]
pub const VMALLOC_SIZE: usize = (VMALLOC_END - VMALLOC_START) as usize;

pub static FRAME_STATE: SpinlockIrq<frame::state::State> =
    SpinlockIrq::new(frame::state::State::uninitialized());
pub static FRAME_ALLOCATOR: SpinlockIrq<frame::dummy_allocator::Allocator> =
    SpinlockIrq::new(frame::dummy_allocator::Allocator::new());

#[global_allocator]
static HEAP_ALLOCATOR: allocator::Locked =
    allocator::Locked::new(linked_list_allocator::Heap::empty());

/// Setup the memory manager of the kernel. Currently, this function is responsible for setting up
/// the frame allocator, initializing the heap and terminating the paging initialization.
pub fn setup(mmap_request: &LimineMemmapRequest) {
    let statistic = FRAME_STATE
        .lock()
        .setup(mmap_request.get_response().get().unwrap().memmap());
    FRAME_ALLOCATOR.lock().setup(statistic);
    unsafe {
        HEAP_ALLOCATOR.lock().init(HEAP_START as *mut u8, HEAP_SIZE);
    }

    // We initialize the paging system here (and not in `arch::init_bsp()`) because we need a
    // frame allocator to do so. Paging are mostly initialized by Limine when it loads the kernel,
    // but we need to terminate the initialization here.
    paging::setup();
}