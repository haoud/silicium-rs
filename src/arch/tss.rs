use crate::x86_64::{cpu::Privilege, segment};

/// Loads the TSS into the CPU. This function must be called after the TSS 
/// is installed in the GDT. 
pub fn install() {
    unsafe {
        crate::x86_64::cpu::ltr(segment::Selector::new(5, Privilege::Ring0).value());
    }
}
