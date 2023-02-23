use crate::sync::spin::Spinlock;
use crate::x86_64::gdt;
use crate::x86_64::segment;

static GDT: Spinlock<gdt::Table<256>> = Spinlock::new(gdt::Table::new());

/// Setup the GDT and load it into the CPU. The GDT created by this function is a classic `x86_64`
/// GDT, with 5 entries:
/// - NULL descriptor
/// - Kernel 64 bits code descriptor
/// - Kernel data descriptor
/// - User 64 bits code descriptor
/// - User data descriptor
/// Later on, other entries will be added to the GDT, especially for the TSS and kernel/user TLS.
pub fn setup() {
    let mut gdt = GDT.lock();
    gdt.set(0, gdt::Descriptor::NULL);
    gdt.set(1, gdt::Descriptor::KERNEL_CODE64);
    gdt.set(2, gdt::Descriptor::KERNEL_DATA);
    gdt.set(3, gdt::Descriptor::USER_CODE64);
    gdt.set(4, gdt::Descriptor::USER_DATA);
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
