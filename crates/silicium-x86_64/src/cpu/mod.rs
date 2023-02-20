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
