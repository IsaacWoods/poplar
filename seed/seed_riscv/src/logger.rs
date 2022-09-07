use core::{fmt, fmt::Write};
use hal_riscv::hw::uart16550::Uart16550;
use log::{Level, LevelFilter, Metadata, Record};
use poplar_util::InitGuard;
use spinning_top::Spinlock;

static LOGGER: LockedLogger = LockedLogger(Spinlock::new(Logger::new()));

pub fn init() {
    LOGGER.0.lock().init();
    log::set_logger(&LOGGER).map(|_| log::set_max_level(LevelFilter::Trace)).unwrap();
}

struct Logger {
    serial: InitGuard<&'static mut Uart16550>,
}

impl Logger {
    const fn new() -> Logger {
        Logger { serial: InitGuard::uninit() }
    }

    fn init(&mut self) {
        let serial = unsafe { &mut *(0x1000_0000 as *mut Uart16550) };
        self.serial.initialize(serial);
    }
}

impl fmt::Write for Logger {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let serial = self.serial.get_mut();
        for byte in s.bytes() {
            serial.write(byte);
        }

        Ok(())
    }
}

struct LockedLogger(Spinlock<Logger>);

impl log::Log for LockedLogger {
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
            writeln!(
                self.0.lock(),
                "[{}{:5}\x1b[0m] {}: {}",
                color,
                record.level(),
                record.target(),
                record.args()
            )
            .unwrap();
        }
    }

    fn flush(&self) {}
}

#[panic_handler]
pub fn panic(info: &core::panic::PanicInfo) -> ! {
    if let Some(message) = info.message() {
        if let Some(location) = info.location() {
            let _ = writeln!(
                LOGGER.0.lock(),
                "Panic message: {} ({} - {}:{})",
                message,
                location.file(),
                location.line(),
                location.column()
            );
        } else {
            let _ = writeln!(LOGGER.0.lock(), "Panic message: {} (no location info)", message);
        }
    }
    loop {}
}
