use core::fmt::Write;
use x86_64::serial::{Port, Serial};

use crate::Spinlock;

pub struct SiliciumLogger;

pub static LOGGER: SiliciumLogger = SiliciumLogger;
static SERIAL: Spinlock<Serial> = Spinlock::new(Serial::new(Port::COM1));

impl log::Log for SiliciumLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let level = match record.level() {
                log::Level::Error => "\x1b[1m\x1b[31m[!]\x1b[0m",
                log::Level::Warn => "\x1b[1m\x1b[33m[-]\x1b[0m",
                log::Level::Info => "\x1b[1m\x1b[32m[*]\x1b[0m",
                log::Level::Debug => "\x1b[1m\x1b[34m[#]\x1b[0m",
                log::Level::Trace => "\x1b[1m[~]\x1b[0m",
            };

            x86_64::irq::without(|| {
                SERIAL
                    .lock()
                    .write_fmt(format_args!("{} {}\n", level, record.args()))
                    .unwrap();
            });
        }
    }

    fn flush(&self) {}
}

#[cold]
pub fn init() {
    log::set_logger(&LOGGER).unwrap(); // Fail only if a logger was already set
    log::set_max_level(log::LevelFilter::Trace);
    SERIAL.lock().init_com();
}
