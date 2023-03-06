use crate::{arch::paging, Spinlock};
use frame::Allocator;

pub mod allocator;
pub mod frame;
pub mod vmm;

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

pub static FRAME_STATE: Spinlock<frame::state::State> =
    Spinlock::new(frame::state::State::uninitialized());
pub static FRAME_ALLOCATOR: Spinlock<frame::dummy_allocator::Allocator> =
    Spinlock::new(frame::dummy_allocator::Allocator::new());

#[global_allocator]
static HEAP_ALLOCATOR: allocator::Locked =
    allocator::Locked::new(linked_list_allocator::Heap::empty());

/// Setup the memory manager of the kernel. Currently, this function is responsible for setting up
/// the frame allocator, initializing the heap and terminating the paging initialization.
///
/// # Warning
/// If an interrupt occurs during this function, the kernel will panic, because this function is
/// called before everything is initialized, and therefore, the interrupt handler will not be
/// initialized.
pub fn setup() {
    let mmap = crate::LIMINE_MEMMAP.get_response().get().unwrap().memmap();
    let statistic = FRAME_STATE.lock().setup(mmap);
    FRAME_ALLOCATOR.lock().setup(statistic);

    unsafe {
        HEAP_ALLOCATOR.lock().init(HEAP_START as *mut u8, HEAP_SIZE);
    }

    // We initialize the paging system here (and not in `arch::init_bsp()`) because we need a
    // frame allocator to do so. Paging are mostly initialized by Limine when it loads the kernel,
    // but we need to terminate the initialization here.
    paging::setup();
    vmm::setup();
}
