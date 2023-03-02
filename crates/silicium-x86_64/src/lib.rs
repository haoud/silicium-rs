//! # Silicium `x86_64` crate
//! Contains x86_64-specific code used by Silicium. This crate is not intended to be used outside of
//! Silicium (for now), and may not be stable, safe or well-documented. Use at your own risk.
//! The code is greatly inspired by [Phil Opp's blog](https://os.phil-opp.com/), and his [crate](
//! https://github.com/rust-osdev/x86_64)
#![cfg_attr(not(test), no_std)]
#![feature(asm_const)]
#![feature(naked_functions)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(dead_code)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_safety_doc)]

pub mod address;
pub mod cpu;
pub mod gdt;
pub mod idt;
pub mod io;
pub mod irq;
pub mod paging;
pub mod segment;
pub mod serial;
pub mod tss;

pub mod prelude {
    pub use crate::*;
}
