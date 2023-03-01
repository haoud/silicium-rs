use crate::sync::spin::SpinlockIrq;
use frame::Allocator;
use limine::LimineMemmapRequest;

pub mod frame;

pub static FRAME_STATE: SpinlockIrq<frame::state::State> =
    SpinlockIrq::new(frame::state::State::uninitialized());
pub static FRAME_ALLOCATOR: SpinlockIrq<frame::dummy_allocator::Allocator> =
    SpinlockIrq::new(frame::dummy_allocator::Allocator::new());

pub fn setup(mmap_request: &LimineMemmapRequest) {
    let statistic = FRAME_STATE
        .lock()
        .setup(mmap_request.get_response().get().unwrap().memmap());
    FRAME_ALLOCATOR.lock().setup(statistic);
    // Use the dummy allocator to early allocate frames
    // Initialise the buddy frame allocator, and throw away the dummy allocator
}
