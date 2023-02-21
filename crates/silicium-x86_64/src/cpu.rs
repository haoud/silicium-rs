use core::arch::asm;

/// Halts definitely the current CPU.
///
/// # Warning
/// This function only halts the current CPU and does not stop other CPUs.
#[inline(always)]
pub fn freeze() -> ! {
    unsafe {
        loop {
            asm!("cli");
            asm!("hlt");
        }
    }
}

/// Load the given GDT register into the CPU. The parameter is a pointer to the
/// GDT register.
///
/// # Safety
/// This function is unsafe because it can cause undefined behavior if the given
/// gdtr is not a valid GDT register.
pub unsafe fn lgdt(gdtr: *const u64) {
    asm!("lgdt [{}]", in(reg) gdtr, options(readonly, nostack, preserves_flags));
}
