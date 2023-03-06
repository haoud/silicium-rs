use sync::spin::Spinlock;
use x86_64::{cpu::Privilege, segment, tss::TaskStateSegment};

const SELECTOR_BASE: usize = 6;

#[thread_local]
static TSS: Spinlock<TaskStateSegment> = Spinlock::new(TaskStateSegment::new());

/// Loads the TSS into the current CPU. This function must be called after the TSS
/// is installed in the GDT.
pub fn install(id: usize) {
    unsafe {
        let index = SELECTOR_BASE + id * 2;
        super::gdt::GDT
            .lock()
            .set_descriptor(index, &x86_64::gdt::Descriptor::tss(&TSS.lock()));
        x86_64::cpu::ltr(segment::Selector::new(index as u16, Privilege::Ring0).value());
    }
}
