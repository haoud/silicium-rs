use super::address::phys_to_virt;
use core::ptr::NonNull;
use x86_64::address::Physical;

#[derive(Debug, Clone, Copy, Hash)]
pub struct AcpiHandler {}

impl AcpiHandler {
    #[must_use]
    pub const fn new() -> Self {
        Self {}
    }
}

impl acpi::AcpiHandler for AcpiHandler {
    unsafe fn map_physical_region<T>(
        &self,
        phys: usize,
        size: usize,
    ) -> acpi::PhysicalMapping<Self, T> {
        // Simply return the physical address via the HHDM
        let start = Physical::new(phys as u64);
        acpi::PhysicalMapping::new(
            usize::try_from(start.as_u64()).unwrap(),
            NonNull::new(phys_to_virt(start).as_mut_ptr()).unwrap(),
            size,
            size,
            *self,
        )
    }

    fn unmap_physical_region<T>(_: &acpi::PhysicalMapping<Self, T>) {
        // Nothing to do here, because we don't map anything
    }
}
