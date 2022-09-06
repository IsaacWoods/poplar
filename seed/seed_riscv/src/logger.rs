use core::{fmt, fmt::Write};
use log::{Level, LevelFilter, Metadata, Record};

pub struct Logger;

static LOGGER: Logger = Logger;

impl Logger {
    pub fn init() {
        log::set_logger(&LOGGER).map(|_| log::set_max_level(LevelFilter::Trace)).unwrap();
    }
}

impl fmt::Write for Logger {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            unsafe {
                (0x1000_0000 as *mut u8).write_volatile(byte);
            }
        }

        Ok(())
    }
}

impl log::Log for Logger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let color = match record.metadata().level() {
                Level::Trace => "\x1b[36m",
                Level::Debug => "\x1b[34m",
                Level::Info => "\x1b[32m",
                Level::Warn => "\x1b[33m",
                Level::Error => "\x1b[31m",
            };
            writeln!(Logger, "[{}{:5}\x1b[0m] {}: {}", color, record.level(), record.target(), record.args())
                .unwrap();
        }
    }

    fn flush(&self) {}
}
