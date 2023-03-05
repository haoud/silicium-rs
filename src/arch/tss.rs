use x86_64::{cpu::Privilege, segment};

/// Loads the TSS into the current CPU. This function must be called after the TSS
/// is installed in the GDT.
pub fn install() {
    unsafe {
        x86_64::cpu::ltr(segment::Selector::new(5, Privilege::Ring0).value());
    }
}
