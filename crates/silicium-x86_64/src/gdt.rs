use bitfield::BitRangeMut;
use bitflags::bitflags;

use crate::{cpu, tss::TaskStateSegment};

#[derive(Debug, Clone)]
pub struct Table<const N: usize> {
    descriptors: [Entry; N],
    register: Register,
}

impl<const N: usize> Table<N> {
    pub const MAX_SIZE: usize = 8192;
    const MAX_SIZE_ASSERT: () =
        assert!(N <= Self::MAX_SIZE, "GDT can't be larger than 8192 entries");

    /// Creates a new empty GDT. All entries are set to the NULL descriptor by default
    #[must_use]
    #[allow(clippy::let_unit_value)]
    pub const fn new() -> Self {
        let _ = Self::MAX_SIZE_ASSERT; // Check that the GDT isn't too large
        Self {
            descriptors: [Entry::NULL; N],
            register: Register::null(),
        }
    }

    /// Returns the total number of entries in the GDT.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Set the GDT entry at the given index to the given descriptor.
    /// 
    /// # Warning
    /// If you set a system descriptor (i.e. a TSS descriptor), remember that it requires two GDT
    /// entries ! If you want to add a descriptor after a system descriptor, you need increment the
    /// index by 2.
    /// ```rust
    /// let mut gdt = Table::<8>::new();
    /// gdt.set_descriptor(0, &Descriptor::NULL);
    /// gdt.set_descriptor(1, &Descriptor::KERNEL_CODE64);
    /// gdt.set_descriptor(2, &Descriptor::KERNEL_DATA);
    /// gdt.set_descriptor(3, &Descriptor::USER_CODE64);
    /// gdt.set_descriptor(4, &Descriptor::USER_DATA);
    /// // This is a system descriptor (TSS descriptor), not initialized for simplicity
    /// // of the example. It requires two GDT entries.
    /// gdt.set_descriptor(5, &Descriptor::System(0, 0)));
    /// // The 6th entry is used by the TSS descriptor
    /// // The 7th entry is available and can be used by a new descriptor like this:
    /// gdt.set_descriptor(7, &Descriptor::KERNEL_CODE64);
    /// ```
    ///
    /// # Panics
    /// This function panics if the index is out of bounds (i.e. greater than or equal to the
    /// GDT's capacity) or if the entry is already in use.
    pub fn set_descriptor(&mut self, index: usize, descriptor: &Descriptor) {
        assert!(index < N, "out of bounds index when setting a GDT entry");
        if let Descriptor::Segment(x) = descriptor {
            assert!(
                self.descriptors[index] == Entry::NULL,
                "GDT entry is already in use"
            );
            self.descriptors[index] = Entry::new(*x);
        } else if let Descriptor::System(x, y) = descriptor {
            assert!(
                self.descriptors[index + 1] == Entry::NULL,
                "GDT entry is already in use"
            );
            assert!(
                self.descriptors[index] == Entry::NULL,
                "GDT entry is already in use"
            );
            self.descriptors[index + 1] = Entry::new(*y);
            self.descriptors[index] = Entry::new(*x);
        }
    }

    /// Clear the GDT entry at the given index.
    ///
    /// # Panics
    /// This function panics if the index is out of bounds (i.e. greater than or equal to the
    /// GDT's capacity)
    pub fn clear_entry(&mut self, index: usize) {
        assert!(index < N, "out of bounds index when clearing a GDT entry");
        self.descriptors[index] = Entry::NULL;
    }

    /// Set the GDT register to point to the GDT and load it into the CPU.
    #[allow(clippy::cast_possible_truncation)]
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
    pub fn pointer(&self) -> u64 {
        self as *const Self as u64
    }

    /// Load the GDT register into the CPU.
    pub fn load(&self) {
        unsafe {
            cpu::lgdt(self.pointer());
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
    pub const KERNEL_CODE64: Self = Self::Segment(0x00af_9b00_0000_ffff);
    pub const KERNEL_DATA: Self = Self::Segment(0x00cf_9300_0000_ffff);
    pub const USER_CODE64: Self = Self::Segment(0x00af_9b00_0000_ffff);
    pub const USER_DATA: Self = Self::Segment(0x00cf_9300_0000_ffff);

    /// Create a new TSS descriptor.
    #[must_use]
    pub fn tss(tss: &TaskStateSegment) -> Self {
        let mut low = DescriptorFlags::PRESENT.bits();
        let ptr = tss.as_ptr() as u64;

        // Set the limit to the size of the TSS minus 1 (because the limit is inclusive)
        low.set_bit_range(15, 0, (core::mem::size_of::<TaskStateSegment>() - 1) as u64);

        // Set the low 32 bits of the base address
        low.set_bit_range(39, 16, ptr & 0xFF_FFFF);
        low.set_bit_range(63, 56, (ptr >> 24) & 0xFF);

        // Set the type to 0b1001 (x86_64 available TSS)
        low.set_bit_range(43, 40, 0b1001);

        Self::System(low, (tss.as_ptr() as u64 >> 32) & 0xFFFF_FFFF)
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
    #[must_use]
    pub const fn new() -> Self {
        Self::empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(transparent)]
struct Entry(u64);

impl Entry {
    const NULL: Self = Self(0);
    const fn new(x: u64) -> Self {
        Self(x)
    }
}

#[cfg(test)]
mod test {
    use core::mem::size_of;

    #[test]
    fn struct_size_checks() {
        assert_eq!(size_of::<super::Register>(), 10);
        assert_eq!(size_of::<super::Entry>(), 8);
    }

    #[test]
    #[should_panic]
    fn gdt_out_of_bounds_access() {
        let mut gdt = super::Table::<8192>::new();
        gdt.set_descriptor(8192, &super::Descriptor::NULL);
    }
}
