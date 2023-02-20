use crate::io;

#[derive(Copy, Clone, Debug)]
pub enum Port {
    COM1 = 0x3F8,
    COM2 = 0x2F8,
    COM3 = 0x3E8,
    COM4 = 0x2E8,
}

pub struct Serial {
    data: io::Port<u8>,
    interrupt_enable: io::Port<u8>,
    fifo_control: io::Port<u8>,
    line_control: io::Port<u8>,
    modem_control: io::Port<u8>,
    line_status: io::Port<u8>,
    modem_status: io::Port<u8>,
    scratch: io::Port<u8>,
}

impl Serial {
    #[must_use]
    pub const fn new(com: Port) -> Serial {
        unsafe {
            Serial {
                data: io::Port::new(com as u16),
                interrupt_enable: io::Port::new(com as u16 + 1),
                fifo_control: io::Port::new(com as u16 + 2),
                line_control: io::Port::new(com as u16 + 3),
                modem_control: io::Port::new(com as u16 + 4),
                line_status: io::Port::new(com as u16 + 5),
                modem_status: io::Port::new(com as u16 + 6),
                scratch: io::Port::new(com as u16 + 7),
            }
        }
    }

    /// Initialize the serial port. Currently, serial port are only used for debugging using QEMU's
    /// serial port, and this function even required to print anything to the QEMU console, so this
    /// function probably doesn't work on real hardware.
    pub fn init_com(&self) {
        self.interrupt_enable.write(0x00);
        self.line_control.write(0x80);
        self.data.write(0x03);
        self.interrupt_enable.write(0x00);
        self.line_control.write(0x03);
        self.fifo_control.write(0xC7);
        self.modem_control.write(0x0B);
        // We don't test if the line is ready to be written to here (I'm lazy)
    }

    /// Check if the serial port is ready to be written to.
    #[must_use]
    pub fn is_transmit_empty(&self) -> bool {
        self.line_status.read() & 0x20 != 0
    }

    /// Check if the serial port has data to be read.
    #[must_use]
    pub fn data_pending(&self) -> bool {
        self.line_status.read() & 0x01 != 0
    }

    /// Write a byte to the serial port.
    pub fn write(&self, byte: u8) {
        while !self.is_transmit_empty() {
            core::hint::spin_loop();
        }
        self.data.write(byte);
    }

    /// Read a byte from the serial port.
    #[must_use]
    pub fn read(&self) -> u8 {
        while !self.data_pending() {
            core::hint::spin_loop();
        }
        self.data.read()
    }
}

impl core::fmt::Write for Serial {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for byte in s.bytes() {
            self.write(byte);
        }
        Ok(())
    }
}
