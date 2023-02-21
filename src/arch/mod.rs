pub mod gdt;

pub fn init_bsp() {
    gdt::setup();
    // TODO: Setup IDT
    // TODO: Setup TSS
    // TODO: Setup paging
    // TODO: Setup exceptions
    // TODO: Setup thread-local storage
}
