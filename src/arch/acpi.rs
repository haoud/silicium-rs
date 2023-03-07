use crate::{
    mm::{
        frame::Frame,
        vmm::{self, AllocationFlags},
    },
    LIMINE_RSDP,
};

use super::{
    address::virt_to_phys,
    paging::{self, MapFlags},
};
use acpi::{madt::Madt, sdt::Signature};
use core::ptr::NonNull;
use x86_64::{
    address::{Virtual, VirtualRange},
    paging::PAGE_SIZE,
};

pub const TLB_SHOOTDOWN_VECTOR: u8 = 0xF0;
pub const CLOCK_TICK_VECTOR: u8 = 0xF1;

#[derive(Debug, Clone, Copy, Hash)]
struct AcpiHandler {}

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
        let aligned_size = size + (PAGE_SIZE - (size % PAGE_SIZE));
        let aligned_phys = phys - (phys % PAGE_SIZE);
        let offset = phys - aligned_phys;
        let flags = MapFlags::PRESENT
            | MapFlags::WRITABLE
            | MapFlags::NO_EXECUTE
            | MapFlags::NO_CACHE
            | MapFlags::WRITE_THROUGH;
        let virt = vmm::allocate(aligned_size, AllocationFlags::NONE)
            .unwrap()
            .start();

        for i in (aligned_phys..(aligned_phys + aligned_size)).step_by(PAGE_SIZE) {
            paging::map(
                &mut *paging::active_table_mut(),
                virt + (i - aligned_phys),
                Frame::from_u64(i as u64),
                flags,
            )
            .unwrap();
        }

        acpi::PhysicalMapping::new(
            aligned_phys + offset,
            NonNull::new((virt + offset).as_mut_ptr()).unwrap(),
            size,
            aligned_size - offset,
            *self,
        )
    }

    fn unmap_physical_region<T>(mapping: &acpi::PhysicalMapping<Self, T>) {
        let start = Virtual::new(mapping.virtual_start().as_ptr() as u64).page_align_down();
        let end = (start + mapping.region_length() as u64).page_align_up();

        for i in (start..end).step_by(PAGE_SIZE) {
            unsafe {
                paging::unmap(&mut *paging::active_table_mut(), i);
            }
        }

        let range = VirtualRange::new(start, end);
        vmm::deallocate(range);
    }
}

/// Setup ACPI and everything related to it.
/// Currently, this function only initializes the LAPIC, and enable it on the core which called this
/// function. Other cores will have to call this function themselves.
pub fn setup() {
    let address = usize::try_from(
        virt_to_phys(Virtual::new(
            LIMINE_RSDP
                .get_response()
                .get_mut()
                .unwrap()
                .address
                .as_ptr()
                .unwrap() as u64,
        ))
        .as_u64(),
    )
    .unwrap();
    let rsdp = unsafe {
        match acpi::AcpiTables::from_rsdp(AcpiHandler::new(), address) {
            Ok(x) => x,
            Err(e) => panic!("Failed to initialize ACPI: {:#?}", e),
        }
    };

    // Find the MADT
    let madt = unsafe {
        match rsdp.get_sdt::<Madt>(Signature::MADT) {
            Ok(option) => match option {
                Some(madt) => madt,
                None => panic!("No MADT found"),
            },
            Err(e) => panic!("Failed to find the MADT: {:#?}", e),
        }
    };

    // Parse the interrupt model
    let apic = match madt.parse_interrupt_model() {
        Ok((model, _)) => match model {
            acpi::InterruptModel::Unknown => {
                panic!("Unknown interrupt model: ACPI is not supported")
            }
            acpi::InterruptModel::Apic(apic) => apic,
            _ => panic!("Unsupported interrupt model"),
        },
        Err(e) => panic!("Failed to parse the interrupt model: {:#?}", e),
    };

    unsafe {
        x86_64::lapic::setup(remap_lapic(apic.local_apic_address).unwrap());
        x86_64::lapic::enable();
    }
}

/// Remap the LAPIC to a virtual address.
///
/// # Errors
/// If an error occurs, the LAPIC is not remapped and `None` is returned. Otherwise, the virtual
/// address of the LAPIC is returned, wrapped in `Some`. The LAPIC base address is page aligned
/// and is mapped on one page.
#[must_use]
unsafe fn remap_lapic(base: u64) -> Option<Virtual> {
    let aligned_base = base - (base % PAGE_SIZE as u64);
    let offset = base - aligned_base;
    let flags = MapFlags::PRESENT
        | MapFlags::WRITABLE
        | MapFlags::NO_EXECUTE
        | MapFlags::NO_CACHE
        | MapFlags::WRITE_THROUGH;
    let virt = vmm::allocate(PAGE_SIZE, AllocationFlags::NONE)
        .ok()?
        .start();
    paging::map(
        &mut *paging::active_table_mut(),
        virt,
        Frame::from_u64(aligned_base),
        flags,
    )
    .ok()?;
    Some(virt + offset)
}
