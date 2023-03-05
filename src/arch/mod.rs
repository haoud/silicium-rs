pub mod address;
pub mod exception;
pub mod gdt;
pub mod idt;
pub mod paging;
pub mod smp;
pub mod tss;

#[no_mangle]
pub unsafe extern "C" fn _start() -> ! {
    core::arch::asm!("xor rbp, rbp"); // Clear the base pointer (useful for backtraces)
    crate::log::init();
    crate::start();
}

/// Initialize the BSP. This is called by the kernel before any other initialization
/// is done. This function is responsible for setting up the GDT, IDT, TSS, exceptions...
pub fn init_bsp() {
    // Install IDT and exceptions as soon as possible to be able to handle interrupts
    idt::setup();
    exception::setup();

    smp::bsp_setup();
    gdt::setup();
    tss::install();
}
