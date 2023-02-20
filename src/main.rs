#![no_std]
#![no_main]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(dead_code)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_safety_doc)]
#![feature(fn_traits)]
#![feature(core_intrinsics)]

use ::log::info;
pub use silicium_internal::x86_64;

pub mod glue;
pub mod log;

#[no_mangle]
pub unsafe extern "C" fn start() -> ! {
    log::init();
    info!("Booting Silicium...");
    info!("Silicium booted successfully!");
    x86_64::cpu::freeze();
}
