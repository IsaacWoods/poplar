use log::{LevelFilter, Log, Metadata, Record};
use x86_64::hw::serial::SerialPort;

static mut SERIAL_PORT: SerialPort = unsafe { SerialPort::new(x86_64::hw::serial::COM1) };

/// Initialise the serial port and logger. Must be called before any of the `log` macros are used.
pub fn init() {
    unsafe {
        SERIAL_PORT.initialise();
    }

    log::set_logger(&BootLogger).unwrap();
    log::set_max_level(LevelFilter::Trace);
}

struct BootLogger;

impl Log for BootLogger {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        use core::fmt::Write;

        if self.enabled(record.metadata()) {
            /*
             * The bootloader is non-reentrant and only has one thread, so this is safe.
             */
            unsafe {
                SERIAL_PORT
                    .write_fmt(format_args!("[{}] {}\n", record.level(), record.args()))
                    .unwrap();
            }
        }
    }

    fn flush(&self) {}
}
