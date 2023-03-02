use core::mem::size_of;

use crate::{
    arch::address::{phys_to_virt, virt_to_phys},
    x86_64::address::{Physical, Virtual},
};
use limine::{LimineMemmapEntry, LimineMemoryMapEntryType, NonNullPtr};

use crate::mm::frame::FrameFlags;

use super::{Frame, Stats};

/// Represents the state of all physical memory frames. This state is used to keep track of which
/// frames are allocated, free, etc.
/// It is important to note that this state only store information about regular memory frames, and
/// should not be used to keep track of special frames such as the ACPI tables or framebuffer. To
/// avoid allocation a overly large array when there is few memory and there is a lot of special
/// frames (such as the framebuffer) at high addresses, frame out of the range of the array are
/// considered as reserved/poisoned and should only be used if you know what you are doing.
pub struct State<'a> {
    state: &'a mut [Frame],
}

impl<'a> State<'a> {
    /// Creates a new empty frame state. This state will be filled by the [`setup`] method.
    /// Attempting to use the state before calling [`setup`] will result in undefined behavior.
    #[must_use]
    pub const fn uninitialized() -> Self {
        Self { state: &mut [] }
    }

    /// Setup the frame state by parsing the memory map and filling the frame array.
    /// This method should only be called once, and should be called before using the frame state.
    ///
    /// # Panics
    /// Panics if the frame state is already initialized or if the frame array cannot be placed in
    /// the memory
    pub fn setup(&mut self, mmap: &[NonNullPtr<LimineMemmapEntry>]) -> Stats {
        assert!(self.state.is_empty(), "Frame state already initialized!");

        let last = Self::find_last_usable_frame_index(mmap);
        let array_location = Self::find_array_location(mmap, last);
        assert!(
            !array_location.is_null(),
            "Could not find a free region to place the frame array!"
        );

        let array: &mut [Frame] =
            unsafe { core::slice::from_raw_parts_mut(array_location.as_mut_ptr(), last) };
        let mut stats = Stats::new();

        // By default, all frames are marked as poisoned. After this loop, we will update the flags
        // for each frame accordingly to the memory map. If a frame is not in the memory map, it
        // will remain poisoned and will not be usable, to prevent any potential issues.
        for (i, frame) in array.iter_mut().enumerate() {
            let mut flags = FrameFlags::POISONED;
            let addr = (i as u64) << 12;
            if addr < 0x10_0000 {
                flags.insert(FrameFlags::BIOS);
            }
            if addr < 0x100_0000 {
                flags.insert(FrameFlags::ISA);
            }
            if addr < 0x1000_0000 {
                flags.insert(FrameFlags::X86);
            }
            *frame = Frame::new(Physical::new(addr), flags);
            stats.poisoned += 1;
            stats.total += 1;
        }

        // Update the flags for each frame according to the memory map.
        for entry in mmap {
            let start = super::index(entry.base).min(last);
            let end = super::index(entry.base + entry.len).min(last);

            for frame in &mut array[start..end] {
                match entry.typ {
                    LimineMemoryMapEntryType::Usable => {
                        frame.flags.remove(FrameFlags::POISONED);
                        frame.flags.insert(FrameFlags::FREE);
                        stats.poisoned -= 1;
                        stats.usable += 1;
                    }
                    LimineMemoryMapEntryType::KernelAndModules
                    | LimineMemoryMapEntryType::BootloaderReclaimable => {
                        frame.flags.remove(FrameFlags::POISONED);
                        frame.flags.insert(FrameFlags::KERNEL);
                        stats.allocated += 1;
                        stats.poisoned -= 1;
                        stats.kernel += 1;
                        stats.usable += 1;
                    }
                    LimineMemoryMapEntryType::BadMemory => (),
                    _ => {
                        if !frame.flags.contains(FrameFlags::POISONED) {
                            frame.flags.remove(FrameFlags::POISONED);
                            frame.flags.insert(FrameFlags::RESERVED);
                            stats.poisoned -= 1;
                            stats.reserved += 1;
                        }
                    }
                }
            }
        }

        // Mark the frames used by the frame array as reserved. After this loop, all kernel frames
        // are marked as used.
        let start = super::index(virt_to_phys(array_location).as_u64());
        let end = start + array.len() * size_of::<Frame>() / 4096;
        for frame in &mut array[start..=end] {
            frame.flags.remove(FrameFlags::FREE);
            frame.flags.insert(FrameFlags::KERNEL);
            stats.allocated += 1;
            stats.kernel += 1;
        }

        *self = State { state: array };
        stats
    }

    #[must_use]
    pub fn get_frame_info_mut(&mut self, address: u64) -> Option<&mut Frame> {
        self.state.get_mut(super::index(address))
    }

    #[must_use]
    pub fn get_frame_info(&self, address: u64) -> Option<&Frame> {
        self.state.get(super::index(address))
    }

    #[must_use]
    pub fn get_state_array_mut(&mut self) -> &mut [Frame] {
        self.state
    }

    #[must_use]
    pub fn get_state_array(&self) -> &[Frame] {
        self.state
    }

    /// Find in the memory map a free region that is big enough to hold the frame array. This is
    /// used to place the frame array in a free region of memory.
    /// If no such region is found, a null virtual address is returned.
    #[must_use]
    fn find_array_location(mmap: &[NonNullPtr<LimineMemmapEntry>], last: usize) -> Virtual {
        // Find in the memory map a free region that is big enough to hold the frame array
        let size = last * size_of::<Frame>();
        mmap.iter()
            .filter(|entry| entry.typ == LimineMemoryMapEntryType::Usable)
            .find(|entry| entry.len >= size as u64)
            .map_or(Virtual::null(), |entry| {
                phys_to_virt(Physical::new(entry.base))
            })
    }

    /// Find the last usable frame index of regular memory. This is used to determine the size of the
    /// frame array. As being say in the documentation of the [`State`] struct, frames out of the
    /// range of the array are considered as reserved/poisoned.
    #[must_use]
    fn find_last_usable_frame_index(mmap: &[NonNullPtr<LimineMemmapEntry>]) -> usize {
        mmap.iter()
            .filter(|entry| {
                entry.typ == LimineMemoryMapEntryType::Usable
                    || entry.typ == LimineMemoryMapEntryType::KernelAndModules
                    || entry.typ == LimineMemoryMapEntryType::BootloaderReclaimable
            })
            .map(|entry| entry.base + entry.len)
            .max()
            .map_or(0, super::index)
    }
}