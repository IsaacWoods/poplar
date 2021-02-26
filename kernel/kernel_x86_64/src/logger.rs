use hal_x86_64::hw::serial::SerialPort;
use log::{Log, Metadata, Record};

/// This handles calls to the log macros throughout the kernel, and writes logging to the COM1
/// serial port.
pub struct KernelLogger;

impl Log for KernelLogger {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        use core::fmt::Write;

        if self.enabled(record.metadata()) {
            let mut serial_port = unsafe { SerialPort::new(hal_x86_64::hw::serial::COM1) };
            serial_port
                .write_fmt(format_args!("[{}][{}] {}\n", record.level(), record.target(), record.args()))
                .unwrap();
        }
    }

    fn flush(&self) {}
}
