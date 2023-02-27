#![no_std]
#![no_main]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(dead_code)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_safety_doc)]
#![feature(asm_const)]
#![feature(fn_traits)]
#![feature(naked_functions)]
#![feature(core_intrinsics)]

use ::log::info;
use limine::{LimineHhdmRequest, LimineMemmapRequest};
pub use silicium_internal::*;

// Limine memory map request
static mut LIMINE_MEMMAP: LimineMemmapRequest = LimineMemmapRequest::new(0);
static mut LIMINE_HHDM: LimineHhdmRequest = LimineHhdmRequest::new(0);

pub mod arch;
pub mod glue;
pub mod log;
pub mod mm;

#[no_mangle]
pub unsafe extern "C" fn start() -> ! {
    log::init();
    info!("Booting Silicium...");
    assert!(
        LIMINE_MEMMAP.get_response().get().is_some(),
        "No memory map provided by Limine!"
    );
    assert!(
        LIMINE_HHDM.get_response().get().is_some(),
        "No high-half direct mapping provided by Limine!"
    );

    arch::init_bsp();
    info!("Silicium booted successfully!");

    // Print the memory map
    info!("Memory map:");
    for entry in LIMINE_MEMMAP.get_response().get().unwrap().memmap().iter() {
        info!(
            "  {:016X} - {:016X} {:?}",
            entry.base,
            entry.base + entry.len,
            entry.typ
        );
    }
    x86_64::cpu::freeze();
}
