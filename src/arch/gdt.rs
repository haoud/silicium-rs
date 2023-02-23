use crate::sync::spin::Spinlock;
use crate::x86_64::gdt;
use crate::x86_64::segment;
use crate::x86_64::tss::TaskStateSegment;

static TSS: Spinlock<TaskStateSegment> = Spinlock::new(TaskStateSegment::new());
static GDT: Spinlock<gdt::Table<6>> = Spinlock::new(gdt::Table::new());

/// Setup the GDT and load it into the CPU. The GDT created by this function is a classic `x86_64`
/// GDT, with 6 entries:
/// - NULL descriptor
/// - Kernel 64 bits code descriptor
/// - Kernel data descriptor
/// - User 64 bits code descriptor
/// - User data descriptor
/// - TSS descriptor
pub fn setup() {
    let mut gdt = GDT.lock();
    gdt.set(0, gdt::Descriptor::NULL);
    gdt.set(1, gdt::Descriptor::KERNEL_CODE64);
    gdt.set(2, gdt::Descriptor::KERNEL_DATA);
    gdt.set(3, gdt::Descriptor::USER_CODE64);
    gdt.set(4, gdt::Descriptor::USER_DATA);
    gdt.set(5, gdt::Descriptor::tss(&TSS.lock()));
    gdt.flush();
    unsafe {
        segment::reload(
            &segment::Selector::KERNEL_CODE64,
            &segment::Selector::KERNEL_DATA,
        );
    }
}

/// Reload the GDT and all the segment registers, except the FS and GS registers which are used for
/// the TLS and not loaded here.
pub fn reload() {
    GDT.lock().flush();
    unsafe {
        segment::reload(
            &segment::Selector::KERNEL_CODE64,
            &segment::Selector::KERNEL_DATA,
        );
    }
}
