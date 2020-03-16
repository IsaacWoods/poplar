use core::{fmt, ptr::NonNull};
use hal_x86_64::hw::serial::SerialPort;
use log::{LevelFilter, Log, Metadata, Record};
use spin::Mutex;
use uefi::proto::console::text::Output;

pub static LOGGER: Mutex<Logger> = Mutex::new(Logger::new());

/// Initialise the serial port and logger. Must be called before any of the `log` macros are used.
pub fn init(console_writer: &mut Output) {
    unsafe {
        let mut logger = LOGGER.lock();
        logger.serial_port.initialise();
        logger.console_writer = NonNull::new(console_writer as *const _ as *mut _);
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
    serial_port: SerialPort,
    console_writer: Option<NonNull<Output<'static>>>,
}

unsafe impl Sync for Logger {}
unsafe impl Send for Logger {}

impl Logger {
    const fn new() -> Logger {
        Logger {
            /*
             * Some UEFI implementations automatically also output to the serial port with the ConsoleOut
             * device, so this defaults to not.
             */
            log_to_serial: false,
            serial_port: unsafe { SerialPort::new(hal_x86_64::hw::serial::COM1) },
            console_writer: None,
        }
    }

    /// Disable logging to the console. This must be called before calling `ExitBootServices`.
    pub fn disable_console_output(&mut self, switch_to_serial: bool) {
        self.console_writer = None;
        self.log_to_serial = switch_to_serial;
    }
}

impl fmt::Write for Logger {
    fn write_str(&mut self, s: &str) -> Result<(), fmt::Error> {
        if self.log_to_serial {
            self.serial_port.write_str(s)?;
        }

        if self.console_writer.is_some() {
            unsafe { self.console_writer.unwrap().as_mut() }.write_str(s)?;
        }

        Ok(())
    }
}
