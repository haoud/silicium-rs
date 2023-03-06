use crate::arch::address::phys_to_virt;
use x86_64::paging::PAGE_SIZE;

use super::{AllocationFlags, Frame, FrameFlags, Stats};

/// A dummy allocator that allocates frames from the frame state. This allocator is very inefficient
/// and should only be used when no other allocator is available. But it could be easily improved,
/// by saving the last allocated frame index to avoid searching the frame state from the beginning.
///
/// For now, the allocator is used as the global allocator, but it will be replaced by a more
/// efficient allocator in the future, when performance becomes a concern.
pub struct Allocator {
    statistic: Stats,
}

impl Allocator {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            statistic: Stats::new(),
        }
    }
}

unsafe impl super::Allocator for Allocator {
    fn setup(&mut self, statistic: Stats) {
        self.statistic = statistic;
    }

    /// Allocates a frame from the frame state. Returns `None` if no frame is available, or a copy
    /// of the frame if a frame was successfully allocated.
    ///
    /// # Warning
    /// This method should only be used when no allocator is available because it is very, very
    /// inefficient, especially when the frame state is large and when low memory is available.
    /// Furthermore, many allocations flags are not supported (e.g. `AllocationFlags::BIOS`,
    /// `AllocationFlags::ISA`, `AllocationFlags::X86`)
    unsafe fn allocate(&mut self, flags: super::AllocationFlags) -> Option<Frame> {
        // Acquire the frame state and the frame statistics, the order is important and should be
        // consistent in all functions that use the frame state and the frame statistics.
        x86_64::irq::without(|| {
            let mut state = crate::mm::FRAME_STATE.lock();
            state
                .get_state_array_mut()
                .iter_mut()
                .find(|frame| frame.get_flags().contains(FrameFlags::FREE))
                .map(|frame| {
                    self.statistic.allocated += 1;
                    if flags.contains(AllocationFlags::KERNEL) {
                        frame.get_flags_mut().insert(FrameFlags::KERNEL);
                        self.statistic.kernel += 1;
                    }
                    if flags.contains(AllocationFlags::ZEROED) {
                        let frame = phys_to_virt(frame.get_frame().start()).as_mut_ptr::<u8>();
                        frame.write_bytes(0, PAGE_SIZE);
                    }
                    frame.get_flags_mut().remove(FrameFlags::FREE);
                    frame.retain();
                    frame
                })
                .map(|f| *f.get_frame())
        })
    }

    /// Allocates a range of free frames from the frame state. Returns `None` if no frame is
    /// available, or a range of frames if a range of frames was successfully allocated.
    ///
    /// # Warning
    /// Please, do not use this method. It is super, super inefficient, and should only be used
    /// when no allocator is available and for initialization purposes, when allocation speed is
    /// not important.
    /// Furthermore, many allocations flags are not supported (e.g. `AllocationFlags::BIOS`,
    /// `AllocationFlags::ISA`, `AllocationFlags::X86`)
    unsafe fn allocate_range(
        &mut self,
        count: usize,
        flags: AllocationFlags,
    ) -> Option<super::Range> {
        x86_64::irq::without(|| {
            // Find `count` contiguous frames that are free
            let mut state = crate::mm::FRAME_STATE.lock();
            let len = state.get_state_array().len();
            let array = state.get_state_array_mut();
            let mut i = 0;
            while i + count <= len {
                if array[i..i + count]
                    .iter()
                    .all(|e| e.get_flags().contains(super::FrameFlags::FREE))
                {
                    for frame in array[i..i + count].iter_mut() {
                        self.statistic.allocated += 1;
                        if flags.contains(AllocationFlags::KERNEL) {
                            frame.get_flags_mut().insert(FrameFlags::KERNEL);
                            self.statistic.kernel += 1;
                        }
                        if flags.contains(AllocationFlags::ZEROED) {
                            let frame = phys_to_virt(frame.get_frame().start()).as_mut_ptr::<u8>();
                            frame.write_bytes(0, PAGE_SIZE);
                        }
                        frame.get_flags_mut().remove(FrameFlags::FREE);
                        frame.retain();
                    }

                    return Some(super::Range {
                        start: *array[i].get_frame(),
                        end: *array[i + count].get_frame(),
                    });
                }
                i += 1;
            }
            None
        })
    }

    /// Reference a frame in the frame state, meaning that the frame is used many times. This method
    /// is unsafe because it can cause memory leaks if the frame is not freed the same number of
    /// times it is referenced.
    ///
    /// # Safety
    /// This method is unsafe because it can cause memory leaks if the frame is not freed the same
    /// number of times it is referenced.
    ///
    /// # Panics
    /// This method panics if the frame is not allocated (i.e. if the frame count is 0)
    unsafe fn reference(&mut self, frame: Frame) {
        x86_64::irq::without(|| {
            let mut state = crate::mm::FRAME_STATE.lock();
            let frame = state.get_frame_info_mut(frame.start()).unwrap();
            assert!(
                frame.get_count() > 0,
                "Referencing a frame that is not allocated"
            );
            frame.retain();
        });
    }

    /// Free a frame in the frame state. The frame is freed only if the frame count is 0, so you
    /// should not assume that the frame is freed after calling this method.
    ///
    /// # Safety
    /// This method is unsafe because it can cause a use-after-free if the frame is freed but
    /// used after this method is called. Double free are not possible because the frame count is
    /// checked, and panics if the frame is already free.
    ///
    /// # Panics
    /// This method panics if the frame is already free.
    unsafe fn deallocate(&mut self, frame: Frame) {
        // Acquire the frame state and the frame statistics, the order is important and should be
        // consistent in all functions that use the frame state and the frame statistics.
        x86_64::irq::without(|| {
            let mut state = crate::mm::FRAME_STATE.lock();

            let frame = state
                .get_frame_info_mut(frame.start())
                .expect("Invalid frame address");

            assert!(
                frame.get_count() != 0,
                "Physical frame deallocated too many times"
            );
            frame.release();
            if frame.get_count() == 0 {
                if frame.get_flags().contains(FrameFlags::KERNEL) {
                    frame.get_flags_mut().remove(FrameFlags::KERNEL);
                    self.statistic.kernel -= 1;
                }
                frame.get_flags_mut().remove(FrameFlags::KERNEL);
                frame.get_flags_mut().insert(FrameFlags::FREE);
                self.statistic.allocated -= 1;
            }
        });
    }

    /// Free a range of frames in the frame state. The frames are freed only if the frame count is 0,
    /// so you should not assume that the frames are freed after calling this method.
    ///
    /// # Safety
    /// This method is unsafe because it can cause a use-after-free if the frame range is freed but
    /// used after this method is called. Double free are not possible because the frame count is
    /// checked, and panics if a frame is already free.
    ///
    /// # Panics
    /// This method panics if one or more frames in the range are already free.
    unsafe fn deallocate_range(&mut self, range: super::Range) {
        for frame in range {
            self.deallocate(frame);
        }
    }

    fn statistics(&self) -> Stats {
        self.statistic
    }
}
