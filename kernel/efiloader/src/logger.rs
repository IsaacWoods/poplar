use core::{fmt, ptr::NonNull};
use hal_x86_64::hw::serial::SerialPort;
use log::{LevelFilter, Log, Metadata, Record};
use spin::Mutex;
use uefi::proto::console::text::Output;

pub static LOGGER: Mutex<Logger> = Mutex::new(Logger::Nop);

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

pub enum Logger {
    Nop,
    Console(NonNull<Output<'static>>),
    Serial(SerialPort),
}

impl Logger {
    pub fn init_console(console_writer: &mut Output) {
        *LOGGER.lock() = Logger::Console(NonNull::new(console_writer as *const _ as *mut _).unwrap());

        log::set_logger(&LogWrapper).unwrap();
        log::set_max_level(LevelFilter::Trace);
    }

    pub fn switch_to_serial() {
        *LOGGER.lock() = Logger::Serial(unsafe { SerialPort::new(hal_x86_64::hw::serial::COM1) });
    }
}

impl fmt::Write for Logger {
    fn write_str(&mut self, s: &str) -> Result<(), fmt::Error> {
        match self {
            Logger::Nop => Ok(()),
            Logger::Console(output) => unsafe { output.as_mut() }.write_str(s),
            Logger::Serial(serial) => serial.write_str(s),
        }
    }
}

unsafe impl Sync for Logger {}
unsafe impl Send for Logger {}
