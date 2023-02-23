#![no_std]
#![feature(asm_const)]
#![feature(naked_functions)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(dead_code)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_safety_doc)]

pub mod cpu;
pub mod gdt;
pub mod idt;
pub mod io;
pub mod irq;
pub mod segment;
pub mod serial;

pub mod prelude {
    pub use crate::*;
}
