use sync::spin::Spinlock;
use x86_64::gdt;
use x86_64::segment;
use x86_64::tss::TaskStateSegment;

#[thread_local]
static TSS: Spinlock<TaskStateSegment> = Spinlock::new(TaskStateSegment::new());

#[thread_local]
static GDT: Spinlock<gdt::Table<8>> = Spinlock::new(gdt::Table::new());

static BSP_TSS: Spinlock<TaskStateSegment> = Spinlock::new(TaskStateSegment::new());
static BSP_GDT: Spinlock<gdt::Table<8>> = Spinlock::new(gdt::Table::new());

/// Onlu used during the boot process, this function setup the GDT for the BSP and load it into the
/// CPU. This allow to have early interrupts and exceptions handling, when we not yet have a
/// TLS.
pub fn setup() {
    let mut gdt = BSP_GDT.lock();
    gdt.set_descriptor(0, &gdt::Descriptor::NULL);
    gdt.set_descriptor(1, &gdt::Descriptor::KERNEL_CODE64);
    gdt.set_descriptor(2, &gdt::Descriptor::KERNEL_DATA);
    gdt.set_descriptor(3, &gdt::Descriptor::USER_CODE64);
    gdt.set_descriptor(4, &gdt::Descriptor::USER_DATA);
    gdt.set_descriptor(5, &gdt::Descriptor::tss(&BSP_TSS.lock()));
    gdt.flush();
    unsafe {
        segment::reload(
            &segment::Selector::KERNEL_CODE64,
            &segment::Selector::KERNEL_DATA,
        );
    }
}

/// Setup the GDT for the current CPU and load it into the CPU. The GDT created by this function is
/// a classic `x86_64` GDT, with 6 entries:
/// - NULL descriptor
/// - Kernel 64 bits code descriptor
/// - Kernel data descriptor
/// - User 64 bits code descriptor
/// - User data descriptor
/// - TSS descriptor
pub fn ap_setup() {
    let mut gdt = GDT.lock();
    gdt.set_descriptor(0, &gdt::Descriptor::NULL);
    gdt.set_descriptor(1, &gdt::Descriptor::KERNEL_CODE64);
    gdt.set_descriptor(2, &gdt::Descriptor::KERNEL_DATA);
    gdt.set_descriptor(3, &gdt::Descriptor::USER_CODE64);
    gdt.set_descriptor(4, &gdt::Descriptor::USER_DATA);
    gdt.set_descriptor(5, &gdt::Descriptor::tss(&TSS.lock()));
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
