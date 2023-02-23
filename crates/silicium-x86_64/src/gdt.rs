use bitfield::BitRangeMut;
use bitflags::bitflags;

use crate::{cpu, tss::TaskStateSegment};

#[derive(Debug, Clone)]
pub struct Table<const N: usize> {
    descriptors: [Entry; N],
    register: Register,
}

impl<const N: usize> Table<N> {
    /// Creates a new empty GDT. All entries are set to the NULL descriptor by default
    pub const fn new() -> Self {
        Self {
            descriptors: [Entry::NULL; N],
            register: Register::null(),
        }
    }

    /// Returns the total number of entries in the GDT.
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Set the GDT entry at the given index to the given descriptor.
    ///
    /// # Panics
    /// This function panics if the index is out of bounds.
    pub fn set(&mut self, index: usize, descriptor: Descriptor) {
        assert!(index < N, "out of bounds index when setting a GDT entry");
        if let Descriptor::Segment(x) = descriptor {
            self.descriptors[index] = Entry::new(x, 0);
        } else if let Descriptor::System(x, y) = descriptor {
            self.descriptors[index] = Entry::new(x, y);
        }
    }

    /// Set the GDT register to point to the GDT and load it into the CPU.
    pub fn flush(&mut self) {
        self.register.limit = (N * core::mem::size_of::<Entry>() - 1) as u16;
        self.register.base = self.descriptors.as_ptr() as u64;
        self.register.load();
    }
}

#[derive(Debug, Clone)]
#[repr(C, packed)]
struct Register {
    limit: u16,
    base: u64,
}

impl Register {
    /// Create a new GDT register which points to NULL.
    pub const fn null() -> Self {
        Self { limit: 0, base: 0 }
    }

    /// Returns a raw pointer to the GDT register.
    pub const fn as_ptr(&self) -> *const Self {
        self as *const Self
    }

    /// Load the GDT register into the CPU.
    pub fn load(&self) {
        unsafe {
            cpu::lgdt(self.as_ptr() as *const u64);
        }
    }
}

#[derive(Debug, Clone)]
pub enum Descriptor {
    System(u64, u64),
    Segment(u64),
}

impl Descriptor {
    pub const NULL: Self = Self::Segment(0);
    pub const KERNEL_CODE64: Self = Self::Segment(0x00af9b000000ffff);
    pub const KERNEL_DATA: Self = Self::Segment(0x00cf93000000ffff);
    pub const USER_CODE64: Self = Self::Segment(0x00af9b000000ffff);
    pub const USER_DATA: Self = Self::Segment(0x00cf93000000ffff);

    /// Create a new TSS descriptor.
    pub fn tss(tss: &TaskStateSegment) -> Self {
        let mut low = DescriptorFlags::PRESENT.bits();
        let ptr = tss.as_ptr() as u64;

        // Set the limit to the size of the TSS minus 1 (because the limit is inclusive)
        low.set_bit_range(15, 0, (core::mem::size_of::<TaskStateSegment>() - 1) as u64);

        // Set the low 32 bits of the base address
        low.set_bit_range(39, 16, ptr & 0xFFFFFF);
        low.set_bit_range(63, 56, (ptr >> 24) & 0xFF);

        // Set the type to 0b1001 (x86_64 available TSS)
        low.set_bit_range(43, 40, 0b1001);

        Self::System(low, (tss.as_ptr() as u64 >> 32) as u32 as u64)
    }
}

bitflags! {
    pub struct DescriptorFlags: u64 {
        const ACCESSED          = 1 << 40;
        const WRITABLE          = 1 << 41;
        const CONFORMING        = 1 << 42;
        const EXECUTABLE        = 1 << 43;
        const USER_SEGMENT      = 1 << 44;
        const DPL_RING_3        = 3 << 45;
        const PRESENT           = 1 << 47;
        const AVAILABLE         = 1 << 52;
        const LONG_MODE         = 1 << 53;
        const DEFAULT_SIZE      = 1 << 54;
        const GRANULARITY       = 1 << 55;
    }
}

impl DescriptorFlags {
    pub const fn new() -> Self {
        Self::empty()
    }
}

#[derive(Debug, Clone)]
#[repr(C, align(8))]
struct Entry(u64, u64);

impl Entry {
    const NULL: Self = Self(0, 0);
    const fn new(x: u64, y: u64) -> Self {
        Self(x, y)
    }
}
