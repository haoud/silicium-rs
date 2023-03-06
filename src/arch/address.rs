use x86_64::address::{Physical, Virtual};

#[must_use]
pub const fn phys_to_virt(virt: Physical) -> Virtual {
    Virtual::new(virt.as_u64() + 0xFFFF_8000_0000_0000)
}

/// Return the physical address corresponding to the virtual address, assuming that the virtual
/// address is in the HHDM. If you want to get the physical address of a virtual address that is not
/// in the HHDM, you should use the `translate` function instead (paging.rs)
///
/// # Safety
/// Physical addresses must be in the HHDM, and the resulting physical address could not exist !
#[must_use]
pub const fn virt_to_phys(virt: Virtual) -> Physical {
    assert!(virt.as_u64() >= 0xFFFF_8000_0000_0000 && virt.as_u64() < 0xFFFF_8FFF_FFFF_FFFF);
    Physical::new(virt.as_u64() - 0xFFFF_8000_0000_0000)
}
