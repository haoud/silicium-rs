use core::arch::asm;

/// Waits for an interrupt. If interrupts are disabled, this function will never return, so be
/// careful when using it.
pub fn wait_for() {
    unsafe {
        asm!("hlt");
    }
}

/// Disables interrupts.
pub fn disable() {
    unsafe {
        asm!("cli");
    }
}

/// Enables interrupts.
pub fn enable() {
    unsafe {
        asm!("sti");
    }
}

/// Returns the current interrupt state.
#[must_use]
pub fn enabled() -> bool {
    let flags: u64;
    unsafe {
        asm!("pushfq
              pop {}", out(reg) flags);
    }
    flags & (1 << 9) != 0
}

/// Restores a previous interrupt state.
pub fn restore(state: bool) {
    if state {
        enable();
    } else {
        disable();
    }
}
