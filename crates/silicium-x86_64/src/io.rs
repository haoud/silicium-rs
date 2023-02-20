use core::arch::asm;
use core::marker::PhantomData;

pub trait IO {
    /// Write a value to a port.
    /// 
    /// # Safety
    /// This function is unsafe because writing to a port can have side effects, including
    /// causing the hardware to do something unexpected and possibly violating memory safety.
    unsafe fn write(port: u16, value: Self);

    /// Read a value from a port.
    /// 
    /// # Safety
    /// This function is unsafe because reading from a port can have side effects, including
    /// causing the hardware to do something unexpected and possibly violating memory safety.
    unsafe fn read(port: u16) -> Self;

    /// Write a value to a port, then pause for a short time. This is useful for
    /// writing to ports that require a short delay after writing in order to let
    /// enough time pass for the hardware to process the write.
    /// 
    /// # Safety
    /// This function is unsafe because writing to a port can have side effects, including
    /// causing the hardware to do something unexpected and possibly violating memory safety.
    unsafe fn write_and_pause(port: u16, value: Self) where Self: Sized {
        Self::write(port, value);
        pause();
    }
}

impl IO for u8 {
    unsafe fn write(port: u16, value: u8) {
        outb(port, value);
    }

    unsafe fn read(port: u16) -> u8 {
        inb(port)
    }
}

impl IO for u16 {
    unsafe fn write(port: u16, value: u16) {
        outw(port, value);
    }

    unsafe fn read(port: u16) -> u16 {
        inw(port)
    }
}

impl IO for u32 {
    unsafe fn write(port: u16, value: u32) {
        outd(port, value);
    }

    unsafe fn read(port: u16) -> u32 {
        ind(port)
    }
}

pub struct Port<T> {
    port: u16,
    _phantom: PhantomData<T>,
}

impl<T: IO> Port<T> {
    #[must_use]
    pub const unsafe fn new(port: u16) -> Port<T> {
        Port {
            port,
            _phantom: PhantomData,
        }
    }

    pub fn write_and_pause(&self, value: T) {
        unsafe {
            T::write_and_pause(self.port, value);
        }
    }

    pub fn write(&self, value: T) {
        unsafe {
            T::write(self.port, value);
        }
    }

    #[must_use]
    pub fn read(&self) -> T {
        unsafe { T::read(self.port) }
    }
}

pub struct UnsafePort<T> {
    port: u16,
    _phantom: PhantomData<T>,
}

impl<T: IO> UnsafePort<T> {
    #[must_use]
    pub const unsafe fn new(port: u16) -> UnsafePort<T> {
        UnsafePort {
            port,
            _phantom: PhantomData,
        }
    }

    pub unsafe fn write_and_pause(&self, value: T) {
        T::write_and_pause(self.port, value);
    }

    pub unsafe fn write(&self, value: T) {
        T::write(self.port, value);
    }

    #[must_use]
    pub unsafe fn read(&self) -> T {
        T::read(self.port)
    }
}

pub unsafe fn outb(port: u16, value: u8) {
    asm!("out dx, al", in("dx") port, in("al") value);
}

pub unsafe fn outw(port: u16, value: u16) {
    asm!("out dx, ax", in("dx") port, in("ax") value);
}

pub unsafe fn outd(port: u16, value: u32) {
    asm!("out dx, eax", in("dx") port, in("eax") value);
}

#[must_use]
pub unsafe fn inb(port: u16) -> u8 {
    let mut value: u8;
    asm!("in al, dx", in("dx") port, out("al") value);
    value
}

#[must_use]
pub unsafe fn inw(port: u16) -> u16 {
    let mut value: u16;
    asm!("in ax, dx", in("dx") port, out("ax") value);
    value
}

#[must_use]
pub unsafe fn ind(port: u16) -> u32 {
    let mut value: u32;
    asm!("in eax, dx", in("dx") port, out("eax") value);
    value
}

pub unsafe fn pause() {
    outb(0x80, 0); // Used by linux, may be fragile
}
