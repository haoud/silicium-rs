use core::sync::atomic::Ordering;

use ::log::error;
use x86_64::{
    self,
    lapic::{self, IpiDestination, IpiPriority},
};

use crate::{arch, log, EARLY};

#[cold]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    halt_other_core();
    log::on_panic();
    // TODO: Dump stack trace
    // TODO: Dump registers
    // TODO: Dump memory
    let cpu_id = if EARLY.load(Ordering::Relaxed) {
        0
    } else {
        arch::smp::current_id()
    };

    error!("CPU {cpu_id} {info}");
    error!("System halted");
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
