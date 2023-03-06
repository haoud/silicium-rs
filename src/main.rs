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

use ::log::info;
use limine::{LimineHhdmRequest, LimineMemmapRequest, LimineRsdpRequest, LimineSmpRequest};

pub static LIMINE_MEMMAP: LimineMemmapRequest = LimineMemmapRequest::new(0);
pub static LIMINE_HHDM: LimineHhdmRequest = LimineHhdmRequest::new(0);
pub static LIMINE_RSDP: LimineRsdpRequest = LimineRsdpRequest::new(0);
pub static LIMINE_SMP: LimineSmpRequest = LimineSmpRequest::new(0);

pub mod config;

pub mod arch;
pub mod glue;
pub mod log;
pub mod mm;

pub unsafe fn start() -> ! {
    info!("Booting Silicium...");
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

    // Install GDT, IDT and exceptions as soon as possible to be able to handle interrupts
    arch::gdt::setup();
    arch::idt::setup();
    arch::exception::setup();

    // Initialise the memory subsystem
    mm::setup();

    // Initialise the BSP, and start the other CPUs
    arch::init_bsp();
    arch::smp::start_cpus();

    info!("Silicium booted successfully!");
    x86_64::cpu::freeze();
}
