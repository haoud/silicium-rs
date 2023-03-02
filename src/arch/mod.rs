pub mod address;
pub mod exception;
pub mod gdt;
pub mod idt;
pub mod paging;
pub mod tss;

/// Initialize the BSP. This is called by the kernel before any other initialization
/// is done. This function is responsible for setting up the GDT, IDT, TSS, exceptions...
pub fn init_bsp() {
    gdt::setup();
    idt::setup();
    tss::install();
    exception::setup();
    // TODO: Setup thread-local storage
}
