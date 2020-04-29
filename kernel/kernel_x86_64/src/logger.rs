use hal_x86_64::hw::serial::SerialPort;
use log::{Log, Metadata, Record};
use spin::Mutex;

/// The COM1 serial port, accessed through the UART 16550 controller found in many old platforms,
/// and emulated by most emulators. There is no need to initialise it in the kernel, as efiloader already does that
/// for us.
static COM1: Mutex<SerialPort> = Mutex::new(unsafe { SerialPort::new(hal_x86_64::hw::serial::COM1) });

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
            COM1.lock()
                .write_fmt(format_args!("[{}][{}] {}\n", record.level(), record.target(), record.args()))
                .unwrap();
        }
    }

    fn flush(&self) {}
}
