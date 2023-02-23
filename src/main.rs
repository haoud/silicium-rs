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
pub use silicium_internal::*;

pub mod arch;
pub mod glue;
pub mod log;

#[no_mangle]
pub unsafe extern "C" fn start() -> ! {
    log::init();
    info!("Booting Silicium...");
    arch::init_bsp();
    info!("Silicium booted successfully!");
    x86_64::cpu::freeze();
}
