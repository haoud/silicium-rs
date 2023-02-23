use core::arch::asm;

#[repr(C)]
pub struct State {
    // FS are saved because both the kernel and the user use it for TLS. Normally, the kernel should
    // uses GS, but there is no way to change it without recompiling the rust compiler (and I don't
    // know how to do it).
    pub fs: u64,

    // Preserved registers
    pub rbp: u64,
    pub rbx: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,

    // Scratch registers
    pub rax: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,

    // Used to return from "interrupt_enter"
    address: u64,

    // Error code (if any) and interrupt number
    pub number: u64,
    pub code: u64,

    // Pushed by the CPU automatically
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

pub enum Privilege {
    Ring0 = 0,
    Ring1 = 1,
    Ring2 = 2,
    Ring3 = 3,
}

impl Privilege {
    pub const KERNEL: Self = Self::Ring0;
    pub const USER: Self = Self::Ring3;
}

/// Halts definitely the current CPU.
///
/// # Warning
/// This function only halts the current CPU and does not stop other CPUs.
#[inline]
pub fn freeze() -> ! {
    unsafe {
        loop {
            cli();
            hlt();
        }
    }
}

/// Disables interrupts on the current CPU. If an interrupt occurs while interrupts are disabled, it
/// will be queued and executed when interrupts are re-enabled (for example, with [`sti`])
#[inline]
pub fn cli() {
    // SAFETY: Disabling interrupts should not cause any undefined behavior
    unsafe {
        asm!("cli");
    }
}

/// Enables interrupts on the current CPU. If an interrupt was queued while interrupts were disabled,
/// it will be executed after this function returns.
///
/// # Safety
/// This function is unsafe because it can cause undefined behavior if the IDT or an interrupt
/// handler is not properly written.
#[inline]
pub unsafe fn sti() {
    asm!("sti");
}

/// Stop the current CPU core until the next interrupt occurs.
///
/// # Safety
/// This function is unsafe because it can cause unexpected behavior if interrupts are not enabled
/// when this function is called.
#[inline]
pub unsafe fn hlt() {
    asm!("hlt");
}

/// Load the given GDT register into the CPU. The parameter is a pointer to the
/// GDT register.
///
/// # Safety
/// This function is unsafe because it can cause undefined behavior if the given
/// gdtr is not a valid GDT register.
#[inline]
pub unsafe fn lgdt(gdtr: u64) {
    asm!("lgdt [{}]", in(reg) gdtr, options(readonly, nostack, preserves_flags));
}

/// Load the given IDT register into the CPU. The parameter is a pointer to the
/// IDT register.
///
/// # Safety
/// This function is unsafe because it can cause undefined behavior if the given
/// idtr is not a valid IDT register.
#[inline]
pub unsafe fn lidt(idtr: u64) {
    asm!("lidt [{}]", in(reg) idtr, options(readonly, nostack, preserves_flags));
}

/// Load a new task state segment (TSS) into the CPU. The parameter is the selector of the TSS.
///
/// # Safety
/// This function is unsafe because it can cause undefined behavior if the given selector is not a
/// valid TSS selector, if the TSS is not loaded or not properly configured or if the GDT is not
/// loaded or not properly configured.
#[inline]
pub unsafe fn ltr(selector: u16) {
    asm!("ltr ax", in("ax") selector, options(readonly, nostack, preserves_flags));
}
