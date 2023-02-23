use core::arch::asm;

/// Waits for an interrupt. If interrupts are disabled, this function will never return, so be
/// careful when using it.
#[inline]
pub fn enable_and_wait() {
    unsafe {
        crate::cpu::sti();
        crate::cpu::hlt();
    }
}

/// Disables interrupts.
#[inline]
pub fn disable() {
    crate::cpu::cli();
}

/// Enables interrupts.
#[inline]
pub fn enable() {
    unsafe {
        crate::cpu::sti();
    }
}

/// Returns the current interrupt state.
#[inline]
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
#[inline]
pub fn restore(state: bool) {
    if state {
        enable();
    } else {
        disable();
    }
}

/// Raises an interrupt with the given ID.
///
/// # Safety
/// This function is unsafe because it can cause many undefined behaviors when raising an
/// interrupt
#[inline]
pub unsafe fn raise<const T: u8>() {
    asm!("int {id}", id = const T, options(nomem, nostack));
}

/// Executes the given function with interrupts disabled. The previous interrupt state is restored
/// after the function returns, so interrupts will not be re-enabled if they were disabled before
/// calling this function.
pub fn without<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    let irq = enabled();
    if irq {
        disable();
    }
    let ret = f();
    if irq {
        enable();
    }
    ret
}
