use core::sync::atomic::Ordering;

use x86_64::{
    self,
    lapic::{self, IpiDestination, IpiPriority},
};

use crate::{arch, EARLY};

#[cold]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    halt_other_core();
    // TODO: Dump stack trace
    // TODO: Dump registers
    // TODO: Dump memory
    let cpu_id = if EARLY.load(Ordering::Relaxed) {
        0
    } else {
        arch::smp::current_id()
    };

    log::error!("CPU {cpu_id} {info}");
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
