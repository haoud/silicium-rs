use limine::LimineSmpInfo;
use x86_64::{pic, pit::Pit};

use crate::{
    config::{self, KERNEL_HZ},
    Spinlock,
};

pub mod acpi;
pub mod address;
pub mod exception;
pub mod gdt;
pub mod idt;
pub mod irq;
pub mod paging;
pub mod smp;
pub mod tss;

pub static PIT: Spinlock<Pit> = Spinlock::new(Pit::new(KERNEL_HZ));

#[no_mangle]
pub unsafe extern "C" fn _start() -> ! {
    core::arch::asm!("xor rbp, rbp"); // Clear the base pointer (useful for backtraces)
    crate::log::init();
    crate::start();
}

/// # Safety
/// This function is unsafe because it dereferences a raw pointer, but is cannot be marked as unsafe
/// because the limine crate expect the start function to be safe (yeah, I know that's weird) to be
/// assign to the `goto_address` field of the `LimineSmpResponse` struct.
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn _ap_start(info: *const LimineSmpInfo) -> ! {
    unsafe {
        crate::arch::smp::ap_start(&*info);
    }
}

/// Initialize the BSP
pub fn init_bsp() {
    PIT.lock().setup();
    smp::bsp_setup();
    paging::setup();
    tss::install(0);
    unsafe {
        pic::remap(config::IRQ_BASE);
    }
}
