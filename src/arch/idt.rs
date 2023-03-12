use crate::arch::acpi::{CLOCK_TICK_VECTOR, TLB_SHOOTDOWN_VECTOR};
use crate::sys::schedule::{Scheduler, SCHEDULER};
use crate::sys::thread;
use x86_64::cpu::{Privilege, State};
use x86_64::idt::{Descriptor, DescriptorFlags};
use x86_64::interrupt_handler;
use x86_64::{idt, lapic};

use crate::Spinlock;

use super::paging;

pub static IDT: Spinlock<idt::Table> = Spinlock::new(idt::Table::new());

/// Initializes the IDT and load it. This function must be called before enabling interrupts and
/// install a default handler for all interrupts (see [`unknown_interrupt_handler`]).
/// Each interrupt handler must be generated with the [`interrupt_handler!`] macro.
pub fn setup() {
    let mut idt = IDT.lock();
    let flags = DescriptorFlags::new()
        .set_privilege_level(Privilege::KERNEL)
        .present(true)
        .build();

    for i in 0..idt.capacity() {
        let descriptor = Descriptor::new()
            .set_handler_addr(unknown_interrupt as usize as u64)
            .set_options(flags)
            .build();
        idt.set_descriptor(
            u8::try_from(i).expect("IDT index should fit in u8"),
            descriptor,
        );
    }

    // Set the TLB shootdown handler
    let descriptor = Descriptor::new()
        .set_handler_addr(tlb_shootdown as usize as u64)
        .set_options(flags)
        .build();
    idt.set_descriptor(TLB_SHOOTDOWN_VECTOR, descriptor);

    // Set the clock tick handler
    let descriptor = Descriptor::new()
        .set_handler_addr(clock_tick as usize as u64)
        .set_options(flags)
        .build();
    idt.set_descriptor(CLOCK_TICK_VECTOR, descriptor);

    idt.load();
}

/// Reload the current IDT into the current CPU.
pub fn reload() {
    IDT.lock().load();
}

/// Default handler for all interrupts. This function is called when an interrupt occurs but no
/// handler is installed for it. Currently, this function only panics but it should not panic in the
/// future, only a debug message should be printed and eventually count the number of times the
/// interrupt occurred.
pub extern "C" fn unknown_interrupt_handler(_state: State) {
    panic!("Unknown interrupt");
}

/// Handler for the TLB shootdown interrupt. This interrupt is triggered when a TLB entry must be
/// invalidated. This function will invalidate all TLB entries on the current CPU by simplicity,
/// but it should be improved in the future to avoid unnecessary invalidations (and performance
/// penalties)
pub extern "C" fn tlb_shootdown_handler(_state: State) {
    paging::tlb::flush_all();
    lapic::send_eoi();
}

pub extern "C" fn clock_tick_handler(_state: State) {
    lapic::send_eoi();
    SCHEDULER.timer_tick();

    if thread::current().need_rescheduling() {
        unsafe {
            log::debug!("Scheduling CPU {}", crate::arch::smp::current_id());
            SCHEDULER.schedule();
        }
    }
}

interrupt_handler!(-1, unknown_interrupt, unknown_interrupt_handler, 0);
interrupt_handler!(
    TLB_SHOOTDOWN_VECTOR,
    tlb_shootdown,
    tlb_shootdown_handler,
    0
);
interrupt_handler!(CLOCK_TICK_VECTOR, clock_tick, clock_tick_handler, 0);
