use core::ops::{Add, AddAssign, Sub, SubAssign};

use bitflags::bitflags;

use x86_64::address::{Physical, Address};

pub mod dummy_allocator;
pub mod state;

/// Represents an error when a physical address is not page aligned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NotAligned(Physical, usize);

/// Represents a physical memory frame.
#[derive(Debug, Clone, Copy, Hash)]
pub struct Frame {
    address: Physical,
    flags: FrameFlags,
    count: usize,
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
        Self {
            address,
            flags,
            count: 0,
        }
    }

    /// Try to create a new frame.
    ///
    /// # Errors
    /// Returns an [`NotAligned`] error if the address is not page aligned
    pub fn try_new(address: Physical, flags: FrameFlags) -> Result<Self, NotAligned> {
        if address.is_page_aligned() {
            Ok(Self {
                address,
                flags,
                count: 0,
            })
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

    #[must_use]
    pub const fn count(&self) -> usize {
        self.count
    }

    pub fn increment_count(&mut self) {
        self.count += 1;
    }

    pub fn decrement_count(&mut self) {
        self.count -= 1;
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
pub struct Stats {
    pub total: usize,     // Total number of frames
    pub usable: usize,    // Total number of usable frames for allocation
    pub allocated: usize, // Total number of allocated frames
    pub reserved: usize,  // Total number of reserved frames
    pub kernel: usize,    // Total number of kernel frames
    pub borrowed: usize,  // Total number of borrowed frames
    pub poisoned: usize,  // Total number of poisoned frames
}

impl Stats {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            total: 0,
            usable: 0,
            allocated: 0,
            reserved: 0,
            kernel: 0,
            borrowed: 0,
            poisoned: 0,
        }
    }
}

bitflags! {
    pub struct FrameFlags : u64 {
        const NONE = 0;
        const POISONED = 1 << 0;
        const RESERVED = 1 << 1;
        const FREE = 1 << 2;
        const ZEROED = 1 << 3;
        const DIRTY = 1 << 4;
        const KERNEL = 1 << 5;
        const BORROWED = 1 << 6;
        const BIOS = 1 << 7;
        const ISA = 1 << 8;
        const X86 = 1 << 9;
    }
}

#[derive(Debug, Clone, Hash)]
pub struct Range {
    pub start: Frame, // Inclusive
    pub end: Frame,   // Exclusive
}

impl Range {
    #[must_use]
    pub const fn new(start: Frame, end: Frame) -> Self {
        Self { start, end }
    }

    /// Check if the range contains the given address.
    #[must_use]
    pub fn contains_address(&self, address: Physical) -> bool {
        address.as_u64() >= self.start.address.as_u64()
            && address.as_u64() < self.end.address.as_u64()
    }

    /// Check if the range contains the given frame address.
    #[must_use]
    pub fn contains(&self, frame: Frame) -> bool {
        frame.address.as_u64() >= self.start.address.as_u64()
            && frame.address.as_u64() < self.end.address.as_u64()
    }

    /// Returns the number of frames in the range.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn count(&self) -> usize {
        (self.end.address.as_u64() - self.start.address.as_u64()) as usize / 4096
    }

    /// Returns the length of the range, in frames.
    #[must_use]
    pub fn len(&self) -> usize {
        self.count()
    }

    /// Check if the range is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.start.address.as_u64() >= self.end.address.as_u64()
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
    fn setup(&mut self, statistics: Stats);
    unsafe fn allocate(&mut self, flags: AllocationFlags) -> Option<Frame>;
    unsafe fn allocate_range(&mut self, count: usize, flags: AllocationFlags) -> Option<Range>;
    unsafe fn reference(&mut self, frame: Frame);
    unsafe fn deallocate(&mut self, frame: Frame);
    unsafe fn deallocate_range(&mut self, range: Range);
    fn statistics(&self) -> Stats;
}

#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub const fn index(address: u64) -> usize {
    Physical::new(address).frame_index() as usize
}
