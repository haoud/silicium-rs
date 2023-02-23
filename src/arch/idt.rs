use crate::sync::spin::SpinlockIrq;
use crate::x86_64::cpu::{Privilege, State};
use crate::x86_64::interrupt_handler;
use crate::x86_64::idt;
use crate::x86_64::idt::{Descriptor, DescriptorFlags};

pub static IDT: SpinlockIrq<idt::Table> = SpinlockIrq::new(idt::Table::new());

/// Initializes the IDT. This function must be called before enabling interrupts and install
/// a default handler for all interrupts (see [`unknown_interrupt_handler`]).
/// Each interrupt handler must be generated with the [`interrupt_handler!`] macro.
pub fn setup() {
    let mut idt = IDT.lock();
    for i in 0..idt.capacity() {
        let flags = DescriptorFlags::new()
            .set_privilege_level(Privilege::KERNEL)
            .present(true)
            .build();
        let descriptor = Descriptor::new()
            .set_handler_addr(unknown_interrupt as u64)
            .set_options(flags)
            .build();
        idt.set_descriptor(i as u8, descriptor);
    }
    idt.load();
}

/// Default handler for all interrupts. This function is called when an interrupt occurs but no
/// handler is installed for it. Currently, this function only panics but it should not panic in the
/// future, only a debug message should be printed and eventually count the number of times the
/// interrupt occurred.
pub extern "C" fn unknown_interrupt_handler(_state: State) {
    panic!("Unknown interrupt");
}

interrupt_handler!(-1, unknown_interrupt, unknown_interrupt_handler, 0);
