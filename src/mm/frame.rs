use core::ops::{Add, AddAssign, Sub, SubAssign};

use bitflags::bitflags;

use crate::x86_64::address::Physical;

/// Represents an error when a physical address is not page aligned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NotAligned(Physical, usize);

/// Represents a physical memory frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Frame {
    address: Physical,
    flags: FrameFlags,
}

impl Frame {
    /// Creates a new frame
    /// 
    /// # Panics
    /// Panics if the address is not page aligned (4 KiB aligned).
    #[must_use]
    pub fn new(address: Physical, flags: FrameFlags) -> Self {
        assert!(
            address.is_page_aligned(),
            "Frame address must be page aligned!"
        );
        Self { address, flags }
    }

    /// Try to create a new frame.
    /// 
    /// # Errors
    /// Returns an [`NotAligned`] error if the address is not page aligned
    pub fn try_new(address: Physical, flags: FrameFlags) -> Result<Self, NotAligned> {
        if address.is_page_aligned() {
            Ok(Self { address, flags })
        } else {
            Err(NotAligned(address, 4096usize))
        }
    }

    /// Check if the frame contains the given address.
    #[must_use]
    pub fn contains(&self, address: Physical) -> bool {
        address >= self.address && address < self.address + 4096usize
    }

    #[must_use]
    pub const fn address(&self) -> Physical {
        self.address
    }

    pub fn remove_flags(&mut self, flags: FrameFlags) {
        self.flags &= !flags;
    }

    pub fn add_flags(&mut self, flags: FrameFlags) {
        self.flags |= flags;
    }

    pub fn set_flags(&mut self, flags: FrameFlags) {
        self.flags = flags;
    }

    #[must_use]
    pub const fn flags(&self) -> FrameFlags {
        self.flags
    }

    /// Create a range of frames. The range is semi-inclusive, meaning that the end frame is not
    /// included in the range.
    #[must_use]
    pub fn range(start: Frame, end: Frame) -> Range {
        Range { start, end }
    }
}

impl Add<u64> for Frame {
    type Output = Self;

    fn add(self, rhs: u64) -> Self::Output {
        Self::new(self.address + rhs * 4096u64, self.flags)
    }
}

impl AddAssign<u64> for Frame {
    fn add_assign(&mut self, rhs: u64) {
        self.address += rhs * 4096u64;
    }
}

impl Sub<u64> for Frame {
    type Output = Self;

    fn sub(self, rhs: u64) -> Self::Output {
        Self::new(self.address - rhs * 4096u64, self.flags)
    }
}

impl SubAssign<u64> for Frame {
    fn sub_assign(&mut self, rhs: u64) {
        self.address -= rhs * 4096u64;
    }
}

#[derive(Debug, Clone, Copy, Hash)]
pub struct Stat {
    total: usize,     // Total number of frames
    free: usize,      // Total number of usable frames for allocation
    allocated: usize, // Total number of allocated frames
    reserved: usize,  // Total number of reserved frames
    kernel: usize,    // Total number of kernel frames
    borrowed: usize,  // Total number of borrowed frames
}

bitflags! {
    pub struct FrameFlags : u64 {
        const NONE = 0;
        const RESERVED = 1 << 0;
        const ALLOCATED = 1 << 1;
        const ZEROED = 1 << 2;
        const DIRTY = 1 << 3;
        const KERNEL = 1 << 4;
        const BORROWED = 1 << 5;
        const BIOS = 1 << 5;
        const ISA = 1 << 6;
        const X86 = 1 << 7;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Range {
    pub start: Frame, // Inclusive
    pub end: Frame,   // Exclusive
}

impl Range {
    /// Check if the range is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }
}

impl Iterator for Range {
    type Item = Frame;

    fn next(&mut self) -> Option<Self::Item> {
        if self.is_empty() {
            None
        } else {
            let frame = self.start;
            self.start += 1;
            Some(frame)
        }
    }
}

bitflags! {
    pub struct AllocationFlags : u64 {
        const NONE = FrameFlags::NONE.bits;
        const ZEROED =  FrameFlags::ZEROED.bits;
        const KERNEL = FrameFlags::KERNEL.bits;
        const BIOS = FrameFlags::BIOS.bits;
        const ISA = FrameFlags::ISA.bits;
        const X86 = FrameFlags::X86.bits;
    }
}

pub unsafe trait Allocator {
    fn allocate(&mut self, flags: AllocationFlags) -> Option<Frame>;
    fn allocate_range(&mut self, n: usize, flags: AllocationFlags) -> Option<Range>;
}

pub unsafe trait Deallocator {
    fn deallocate(&mut self, frame: Frame);
    fn deallocate_range(&mut self, range: Range);
}
