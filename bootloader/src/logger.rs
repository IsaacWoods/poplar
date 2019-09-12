use crate::uefi::system_table;
use core::fmt;
use log::{LevelFilter, Log, Metadata, Record};
use spin::Mutex;
use x86_64::hw::serial::SerialPort;

pub static LOGGER: Mutex<Logger> = Mutex::new(Logger::new());

/// Initialise the serial port and logger. Must be called before any of the `log` macros are used.
pub fn init() {
    unsafe {
        LOGGER.lock().serial_port.initialise();
    }

    log::set_logger(&LogWrapper).unwrap();
    log::set_max_level(LevelFilter::Trace);
}

struct LogWrapper;

impl Log for LogWrapper {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        use core::fmt::Write;

        if self.enabled(record.metadata()) {
            LOGGER.lock().write_fmt(format_args!("[{}] {}\n", record.level(), record.args())).unwrap();
        }
    }

    fn flush(&self) {}
}

pub struct Logger {
    pub log_to_serial: bool,
    pub log_to_console: bool,
    serial_port: SerialPort,
}

impl Logger {
    const fn new() -> Logger {
        Logger {
            /*
             * Some UEFI implementations automatically also output to the serial port with the ConsoleOut
             * device, so this defaults to not.
             */
            log_to_serial: false,
            log_to_console: true,
            serial_port: unsafe { SerialPort::new(x86_64::hw::serial::COM1) },
        }
    }
}

impl fmt::Write for Logger {
    fn write_str(&mut self, s: &str) -> Result<(), fmt::Error> {
        if self.log_to_serial {
            self.serial_port.write_str(s)?;
        }

        if self.log_to_console {
            system_table().console_out.write_str(s).map_err(|_| fmt::Error)?;
        }

        Ok(())
    }
}
