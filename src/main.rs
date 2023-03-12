#![no_std]
#![no_main]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(dead_code)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_safety_doc)]
#![feature(asm_const)]
#![feature(fn_traits)]
#![feature(thread_local)]
#![feature(const_mut_refs)]
#![feature(naked_functions)]
#![feature(core_intrinsics)]
#![feature(alloc_error_handler)]

extern crate alloc;

use core::sync::atomic::{AtomicBool, Ordering};

use ::log::info;
use limine::{
    LimineHhdmRequest, LimineMemmapRequest, LimineRsdpRequest, LimineSmpRequest,
    LimineStackSizeRequest,
};

/// Request a 128 kio stack for the kernel and the APs. This is absolutely humongous, but it may
/// be required for unoptimized debug builds and to avoid any stack overflow and strange behaviour.
pub static LIMINE_STACK_REQUEST: LimineStackSizeRequest =
    LimineStackSizeRequest::new(0).stack_size(1024 * 128);
pub static LIMINE_MEMMAP: LimineMemmapRequest = LimineMemmapRequest::new(0);
pub static LIMINE_HHDM: LimineHhdmRequest = LimineHhdmRequest::new(0);
pub static LIMINE_RSDP: LimineRsdpRequest = LimineRsdpRequest::new(0);
pub static LIMINE_SMP: LimineSmpRequest = LimineSmpRequest::new(0);

/// This is used to determine if the kernel is running in early mode or not. This is absolutely
/// required to avoid any undefined behaviour during the initialization of the kernel, when some
/// structures are not initialized yet, and trying to access them could lead to a panic.
///
/// The best example is the per-cpu variables, which are not initialized when the kernel is
/// booting and are only initialized after the initialization of the memory manager.
/// If a per-cpu variable is accessed before it is initialized, the kernel will panic when trying
/// to dereference a null pointer.
/// To avoid this, the `EARLY` variable is used to determine if the kernel is running in early mode
/// and if yes, the per-cpu variables are not accessed and an other method is used instead. For an
/// concrete example, see the [`crate::arch::paging`] module.
pub static EARLY: AtomicBool = AtomicBool::new(true);

/// A spinlock type alias. This is used to avoid the confusion between a spinlock (which does not
/// sleep or yield) and a mutex (which does).
type Spinlock<T> = spin::Mutex<T>;

pub mod config;

pub mod arch;
pub mod glue;
pub mod log;
pub mod mm;
pub mod sys;

/// This function performs some checks to ensure that the kernel is running in a valid environment.
/// This function is called before any other initialization function (except for the logging) and
/// should do the most checks possible, to avoid any undefined behaviour later on.
pub fn check_around() {
    assert!(
        LIMINE_STACK_REQUEST.get_response().get().is_some(),
        "No stack size provided by Limine!"
    );
    assert!(
        LIMINE_MEMMAP.get_response().get().is_some(),
        "No memory map provided by Limine!"
    );
    assert!(
        LIMINE_HHDM.get_response().get().is_some(),
        "No high-half direct mapping provided by Limine!"
    );
    assert!(
        LIMINE_RSDP.get_response().get().is_some(),
        "No RSDP provided by Limine!"
    );
    assert!(
        LIMINE_SMP.get_response().get().is_some(),
        "No SMP information provided by Limine!"
    );
    assert!(
        LIMINE_HHDM.get_response().get().unwrap().offset == mm::HHDM_START,
        "High-half direct mapping provided by Limine is not at the expected address!"
    );
}

pub unsafe fn start() -> ! {
    info!("Booting Silicium...");
    check_around();

    // Install GDT, IDT, IRQs, exceptions... as soon as possible to be able to handle interrupts
    arch::gdt::setup();
    arch::idt::setup();
    arch::irq::setup();
    arch::exception::setup();

    // Initialise the memory subsystem
    mm::setup();

    // Initialise the BSP and external devices (PIT, PIC, etc.)
    arch::init_bsp();

    // Setup ACPI and everything related to it (LAPIC, HPET, etc.)
    arch::acpi::setup();

    // Initialise the APs
    arch::smp::start_cpus();

    // Disable early mode and unlock all features of the kernel
    EARLY.store(false, Ordering::Relaxed);

    // Enable interrupts and loop forever
    info!("Silicium booted successfully!");
    loop {
        x86_64::irq::enable();
        x86_64::cpu::hlt();
    }
}
