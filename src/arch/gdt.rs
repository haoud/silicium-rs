use x86_64::gdt;
use x86_64::segment;

use crate::{config, Spinlock};

pub static GDT: Spinlock<gdt::Table<{ 6 + config::MAX_CPU * 2 }>> =
    Spinlock::new(gdt::Table::new());

/// Onlu used during the boot process, this function setup the GDT for the BSP and load it into the
/// CPU. This allow to have early interrupts and exceptions handling, when we not yet have a
/// TLS.
pub fn setup() {
    let mut gdt = GDT.lock();
    gdt.set_descriptor(0, &gdt::Descriptor::NULL);
    gdt.set_descriptor(1, &gdt::Descriptor::KERNEL_CODE64);
    gdt.set_descriptor(2, &gdt::Descriptor::KERNEL_DATA);
    gdt.set_descriptor(3, &gdt::Descriptor::USER_CODE64);
    gdt.set_descriptor(4, &gdt::Descriptor::USER_DATA);
    gdt.flush();
    unsafe {
        segment::reload(
            &segment::Selector::KERNEL_CODE64,
            &segment::Selector::KERNEL_DATA,
        );
        core::arch::asm!("mov fs, {0:e}", in(reg) 0);
        core::arch::asm!("mov gs, {0:e}", in(reg) 0);
    }
}

/// Reload the GDT and all the segment registers, and load 0 in the FS and GS registers
pub fn reload() {
    GDT.lock().flush();
    unsafe {
        segment::reload(
            &segment::Selector::KERNEL_CODE64,
            &segment::Selector::KERNEL_DATA,
        );
        core::arch::asm!("mov fs, {0:e}", in(reg) 0);
        core::arch::asm!("mov gs, {0:e}", in(reg) 0);
    }
}
