pub mod address;
pub mod exception;
pub mod gdt;
pub mod idt;
pub mod paging;
pub mod smp;
pub mod tss;

#[no_mangle]
pub unsafe extern "C" fn _start() -> ! {
    core::arch::asm!("xor rbp, rbp");   // Clear the base pointer (useful for backtraces)
    // Clear FS and GS registers
    core::arch::asm!("mov fs, {0:e}", in(reg) 0);
    core::arch::asm!("mov gs, {0:e}", in(reg) 0);
    crate::log::init();
    crate::start();
}

/// Initialize the BSP
pub fn init_bsp() {
    smp::bsp_setup();
    tss::install();
    gdt::ap_setup();
}
