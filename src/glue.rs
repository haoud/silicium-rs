use crate::x86_64;

#[cold]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    // TODO: Halt other cores
    // TODO: Dump stack trace
    // TODO: Dump registers
    // TODO: Dump memory
    log::error!("Panic: {}", info); // Should be safe to use log here
    log::error!("System halted");
    x86_64::cpu::freeze();
}
