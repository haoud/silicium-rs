pub mod address;
pub mod exception;
pub mod gdt;
pub mod idt;
pub mod tss;

pub fn init_bsp() {
    gdt::setup();
    idt::setup();
    tss::install();
    exception::setup();
    // TODO: Setup TSS
    // TODO: Setup paging
    // TODO: Setup exceptions
    // TODO: Setup thread-local storage
}
