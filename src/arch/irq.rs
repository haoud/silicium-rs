use crate::config;
use core::sync::atomic::{AtomicU64, Ordering};
use x86_64::{
    cpu::Privilege,
    idt::{Descriptor, DescriptorFlags},
    interrupt_handler, pic,
};

static TICKS: AtomicU64 = AtomicU64::new(0);

/// Setup the IRQs handlers
#[allow(clippy::fn_to_numeric_cast)]
pub fn setup() {
    let flags = DescriptorFlags::new()
        .set_privilege_level(Privilege::KERNEL)
        .present(true)
        .build();
    let mut idt = super::idt::IDT.lock();

    // Set default handlers
    for i in 0..16 {
        idt.set_descriptor(
            crate::config::IRQ_BASE + i,
            Descriptor::new()
                .set_handler_addr(ignore_irq as u64)
                .set_options(flags)
                .build(),
        );
    }

    // Set the clock tick handler
    idt.set_descriptor(
        crate::config::IRQ_BASE,
        Descriptor::new()
            .set_handler_addr(pit_tick as u64)
            .set_options(flags)
            .build(),
    );
}

/// This function is called when the clock tick interrupt is triggered. It will increment the
/// number of ticks and send an EOI to the PIC.
///
/// Currently, due to limitations of the kernel, this function is called by the PIT with using the
/// PIC. Because of this, only the BSP can handle IRQs, and interact with the PIC/PIT is slow.
/// This will be fixed in the future, when the kernel will be more advanced.
pub extern "C" fn pit_tick_handler(state: x86_64::cpu::State) {
    TICKS.fetch_add(1, Ordering::Relaxed);
    unsafe {
        pic::send_eoi(u8::try_from(state.number).unwrap());
    }
}

/// Ignore an IRQ. This is useful to avoid the kernel to crash when an IRQ is triggered (for
/// example, when the keyboard is used), but should not be used in the future.
pub extern "C" fn ignore_irq_handler(_: x86_64::cpu::State) {}

interrupt_handler!(config::IRQ_BASE, pit_tick, pit_tick_handler, 0);
interrupt_handler!(0xFF, ignore_irq, ignore_irq_handler, 0);
