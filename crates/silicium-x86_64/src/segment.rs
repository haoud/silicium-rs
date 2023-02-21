use core::arch::asm;

pub struct Selector(u16);

pub const KERNEL_CODE64: Selector = Selector(0x10);
pub const KERNEL_DATA: Selector = Selector(0x20);

pub struct CS;
impl CS {
    /// Read the current code segment selector.
    pub fn read() -> u16 {
        let cs: u16;
        unsafe {
            asm!("mov {0:x}, cs", out(reg) cs, options(nomem, nostack, preserves_flags));
        }
        cs
    }

    /// Write a new code segment selector.
    ///
    /// # Safety
    /// This function is unsafe because it can lead to undefined behavior if the new selector is
    /// invalid.
    pub unsafe fn write(selector: u16) {
        unsafe {
            // Some black magic to load a new code segment selector. This is a bit tricky because
            // we cant directly load the new selector into the CS register, and far jumps are not
            // allowed in 64 bits mode. So we use the 'retfq' instruction to set a new code segment
            // selector
            asm!(
                "push {sel}",
                "lea {tmp}, [1f + rip]",
                "push {tmp}",
                "retfq",
                "1:",
                sel = in(reg) u64::from(selector),
                tmp = lateout(reg) _,
                options(preserves_flags),
            );
        }
    }
}
pub struct DS;
impl DS {
    /// Read the current data segment selector.
    pub fn read() -> u16 {
        let ds: u16;
        unsafe {
            asm!("mov {0:x}, ds", out(reg) ds, options(nomem, nostack, preserves_flags));
        }
        ds
    }

    /// Write a new data segment selector.
    ///
    /// # Safety
    /// This function is unsafe because it can lead to undefined behavior if the new selector is
    /// invalid.
    pub unsafe fn write(selector: u16) {
        unsafe {
            asm!("mov ds, {0:x}", in(reg) selector, options(nomem, nostack, preserves_flags));
        }
    }
}
pub struct ES;
impl ES {
    /// Read the current extra segment selector.
    pub fn read() -> u16 {
        let es: u16;
        unsafe {
            asm!("mov {0:x}, es", out(reg) es, options(nomem, nostack, preserves_flags));
        }
        es
    }

    /// Write a new extra segment selector.
    ///
    /// # Safety
    /// This function is unsafe because it can lead to undefined behavior if the new selector is
    /// invalid.
    pub unsafe fn write(selector: u16) {
        unsafe {
            asm!("mov es, {0:x}", in(reg) selector, options(nomem, nostack, preserves_flags));
        }
    }
}
pub struct FS;
pub struct GS;
impl GS {
    /// Swap the GS segment register between the user and kernel segments. If the GS register
    /// contains the user segment, it will be replaced by the kernel segment, and vice versa.
    ///
    /// # Safety
    /// This function is unsafe because it can lead to undefined behavior if the selector loaded
    /// into the GS register is invalid.
    pub unsafe fn swap() {
        asm!("swapgs", options(nomem, nostack, preserves_flags));
    }
}
pub struct SS;
impl SS {
    /// Read the current stack segment selector.
    pub fn read() -> u16 {
        let ss: u16;
        unsafe {
            asm!("mov {0:x}, ss", out(reg) ss, options(nomem, nostack, preserves_flags));
        }
        ss
    }

    /// Write a new stack segment selector.
    ///
    /// # Safety
    /// This function is unsafe because it can lead to undefined behavior if the new selector is
    /// invalid.
    pub unsafe fn write(selector: u16) {
        unsafe {
            asm!("mov ss, {0:x}", in(reg) selector, options(nomem, nostack, preserves_flags));
        }
    }
}

/// Reload the code, data and stack segment registers with the given selectors. FS and GS are not
/// reloaded because they are used for the TLS and need to be handled separately with MSRs.
pub unsafe fn reload(code: Selector, data: Selector) {
    DS::write(data.0);
    ES::write(data.0);
    SS::write(data.0);
    CS::write(code.0);
}
