use x86_64::{
    self,
    lapic::{self, IpiDestination, IpiPriority},
};

#[cold]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    halt_other_core();
    // TODO: Dump stack trace
    // TODO: Dump registers
    // TODO: Dump memory
    log::error!("Panic: {}", info); // Should be safe to use log here
    log::error!("System halted");
    x86_64::cpu::freeze();
}

#[cold]
#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("Allocation error: {:?}", layout)
}

fn halt_other_core() {
    if lapic::initialized() {
        unsafe {
            x86_64::lapic::send_ipi(IpiDestination::OtherCores, IpiPriority::Nmi, 2);
        }
    }
}
