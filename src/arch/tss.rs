use sync::spin::Spinlock;
use x86_64::{cpu::Privilege, segment::Selector, tss::TaskStateSegment};

const SELECTOR_BASE: usize = 6;

#[thread_local]
static TSS: Spinlock<TaskStateSegment> = Spinlock::new(TaskStateSegment::new());

/// Loads the TSS into the current CPU. This function must be called after the TSS
/// is installed in the GDT.
pub fn install(id: usize) {
    unsafe {
        let index = SELECTOR_BASE + id * 2;
        let selector = Selector::new(u16::try_from(index).unwrap(), Privilege::Ring0);
        super::gdt::GDT
            .lock()
            .set_descriptor(index, &x86_64::gdt::Descriptor::tss(&TSS.lock()));
        x86_64::cpu::ltr(selector.value());
    }
}
