pub const PAGE_SHIFT: usize = 12;
pub const PAGE_SIZE: usize = 4096;
pub const PAGE_MASK: usize = !(PAGE_SIZE - 1);
pub const PAGE_OFFSET_MASK: usize = PAGE_SIZE - 1;

use crate::address::Physical;
use bitflags::bitflags;
use core::ops::{Index, IndexMut};

#[derive(Debug)]
#[repr(C, align(8))]
pub struct PageEntry(u64);

impl PageEntry {
    const ADDR_MASK: u64 = 0x000F_FFFF_FFFF_F000;
    const EMPTY: Self = Self(0);

    #[must_use]
    pub const fn new(addr: Physical, flags: PageEntryFlags) -> Self {
        assert!(addr.is_page_aligned(), "Address is not page aligned");
        Self((addr.as_u64() & Self::ADDR_MASK) | flags.bits())
    }

    pub fn set_address(&mut self, addr: Physical) {
        assert!(
            addr.is_page_aligned(),
            "Address {:016x} is not page aligned",
            addr.as_u64()
        );
        self.0 = (self.0 & !Self::ADDR_MASK) | (addr.as_u64() & Self::ADDR_MASK);
    }

    pub fn set_flags(&mut self, flags: PageEntryFlags) {
        self.0 = (self.0 & Self::ADDR_MASK) | flags.bits();
    }

    pub fn clear_flags(&mut self, flags: PageEntryFlags) {
        self.0 &= !flags.bits();
    }

    pub fn add_flags(&mut self, flags: PageEntryFlags) {
        self.0 |= flags.bits();
    }

    /// Returns `true` if the page is present in memory, `false` otherwise.
    #[must_use]
    pub const fn is_present(&self) -> bool {
        self.flags().contains(PageEntryFlags::PRESENT)
    }

    /// Returns `true` if the page is executable, `false` otherwise.
    #[must_use]
    pub const fn is_executable(&self) -> bool {
        !self.flags().contains(PageEntryFlags::NO_EXECUTE)
    }

    /// Returns `true` if the page is writable, `false` otherwise.
    #[must_use]
    pub const fn is_writable(&self) -> bool {
        self.flags().contains(PageEntryFlags::WRITABLE)
    }

    /// Returns `true` if the page is user accessible, `false` otherwise.
    #[must_use]
    pub const fn is_user(&self) -> bool {
        self.flags().contains(PageEntryFlags::USER)
    }

    /// Set the entry to 0, indicating that the page is not present in memory.
    pub fn clear(&mut self) {
        self.0 = 0;
    }

    /// Returns the flags of this entry.
    #[must_use]
    pub const fn flags(&self) -> PageEntryFlags {
        PageEntryFlags::from_bits_truncate(self.0)
    }

    /// Returns the physical address of the page mapped by this entry. If the entry is not present,
    /// `None` is returned.
    #[must_use]
    pub const fn address(&self) -> Option<Physical> {
        if self.flags().contains(PageEntryFlags::PRESENT) {
            Some(Physical::new(self.0 & Self::ADDR_MASK))
        } else {
            None
        }
    }
}

bitflags! {
    pub struct PageEntryFlags: u64 {
        const PRESENT = 1 << 0;
        const WRITABLE = 1 << 1;
        const USER = 1 << 2;
        const WRITE_THROUGH = 1 << 3;
        const NO_CACHE = 1 << 4;
        const ACCESSED = 1 << 5;
        const DIRTY = 1 << 6;
        const HUGE_PAGE = 1 << 7;
        const GLOBAL = 1 << 8;
        const BIT_9  = 1 << 9;
        const BIT_10 = 1 << 10;
        const BIT_11 = 1 << 11;
        const BIT_52 = 1 << 52;
        const BIT_53 = 1 << 53;
        const BIT_54 = 1 << 54;
        const BIT_55 = 1 << 55;
        const BIT_56 = 1 << 56;
        const BIT_57 = 1 << 57;
        const BIT_58 = 1 << 58;
        const BIT_59 = 1 << 59;
        const BIT_60 = 1 << 60;
        const BIT_61 = 1 << 61;
        const BIT_62 = 1 << 62;
        const NO_EXECUTE = 1 << 63;
    }
}

/// A page table with 512 entries.
#[derive(Debug)]
#[repr(C, align(4096))]
pub struct PageTable([PageEntry; 512]);

impl PageTable {
    pub const COUNT: usize = 512;

    /// Creates a new empty page table.
    #[must_use]
    pub const fn new() -> Self {
        Self([PageEntry::EMPTY; Self::COUNT])
    }

    /// Clears all entries in the page table. This does not free any memory, it just marks all
    /// entries as not present ans clears all flags and addresses.
    pub fn clear(&mut self) {
        for entry in self.0.iter_mut() {
            entry.clear();
        }
    }

    #[must_use]
    pub fn as_ptr(&self) -> *const PageEntry {
        self.0.as_ptr()
    }

    pub fn iter(&self) -> impl Iterator<Item = &PageEntry> {
        self.0.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut PageEntry> {
        self.0.iter_mut()
    }

    /// Returns `true` if all entries in the page table are empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.iter().all(PageEntry::is_present)
    }
}

impl Default for PageTable {
    fn default() -> Self {
        Self::new()
    }
}

impl Index<usize> for PageTable {
    type Output = PageEntry;

    fn index(&self, index: usize) -> &Self::Output {
        assert!(
            index < Self::COUNT,
            "Index {index}/{} out of bounds",
            Self::COUNT
        );
        &self.0[index]
    }
}

impl IndexMut<u64> for PageTable {
    #[allow(clippy::cast_possible_truncation)]
    fn index_mut(&mut self, index: u64) -> &mut Self::Output {
        &mut self[index as usize]
    }
}

impl IndexMut<usize> for PageTable {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        assert!(
            index < Self::COUNT,
            "Index {index}/{} out of bounds",
            Self::COUNT
        );
        &mut self.0[index]
    }
}

impl Index<u64> for PageTable {
    type Output = PageEntry;

    #[allow(clippy::cast_possible_truncation)]
    fn index(&self, index: u64) -> &Self::Output {
        &self[index as usize]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    PageTable = 1,
    PageDirectory = 2,
    PageTableDirectoryPointer = 3,
    PageMapLevel4 = 4,
}

impl Level {
    /// Returns the previous level in the paging hierarchy, or `None` if this is the lowest level.
    /// The first level is [`Level::PageMapLevel4`], the last is [`Level::PageTable`].
    #[must_use]
    pub const fn prev(&self) -> Option<Self> {
        match self {
            Self::PageTable => Some(Self::PageDirectory),
            Self::PageDirectory => Some(Self::PageTableDirectoryPointer),
            Self::PageTableDirectoryPointer => Some(Self::PageMapLevel4),
            Self::PageMapLevel4 => None,
        }
    }

    /// Returns the next level in the paging hierarchy, or `None` if this is the highest level.
    /// The first level is [`Level::PageMapLevel4`], the last is [`Level::PageTable`].
    #[must_use]
    pub const fn next(&self) -> Option<Self> {
        match self {
            Self::PageTable => None,
            Self::PageDirectory => Some(Self::PageTable),
            Self::PageTableDirectoryPointer => Some(Self::PageDirectory),
            Self::PageMapLevel4 => Some(Self::PageTableDirectoryPointer),
        }
    }
}

bitflags! {
    /// Represents a set of flags pushed onto the stack by the CPU when a page fault occurs,
    /// indicating the cause of the fault.
    #[repr(transparent)]
    pub struct PageFaultErrorCode: u64 {
        const PROTECTION_VIOLATION = 1 << 0;
        const WRITE_ACCESS = 1 << 1;
        const CPU_USER_MODE = 1 << 2;
        const MALFORMED_TABLE = 1 << 3;
        const INSTRUCTION_FETCH = 1 << 4;
        const PROTECTION_KEY = 1 << 5;
        const SHADOW_STACK = 1 << 6;
        const SGX = 1 << 15;
    }
}
