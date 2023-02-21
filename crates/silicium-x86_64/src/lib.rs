#![no_std]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(dead_code)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_safety_doc)]

pub mod cpu;
pub mod gdt;
pub mod interrupts;
pub mod io;
pub mod segment;
pub mod serial;

pub mod prelude {
    pub use crate::*;
}
