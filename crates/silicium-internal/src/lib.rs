#![no_std]

pub mod prelude;
pub mod x86_64 {
    pub use silicium_x86_64::*;
}

pub mod sync {
    pub use silicium_sync::*;
}
