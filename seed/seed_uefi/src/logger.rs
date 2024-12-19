use core::fmt;
use gfxconsole::{Framebuffer, GfxConsole};
use hal_x86_64::hw::serial::SerialPort;
use log::{LevelFilter, Log, Metadata, Record};
use seed::boot_info::VideoModeInfo;
use spinning_top::Spinlock;

pub static LOGGER: Spinlock<Logger> = Spinlock::new(Logger::Uninit);

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
    Uninit,
    Serial(SerialPort),
    Graphical { serial_port: SerialPort, console: GfxConsole },
}

impl Logger {
    /// Initialize the logger, initially just printing to the serial port. Once a graphics device has been
    /// initialized, `switch_to_graphical` can be called to switch to logging both to serial and the graphical
    /// device.
    pub fn init() {
        *LOGGER.lock() = Logger::Serial(unsafe { SerialPort::new(hal_x86_64::hw::serial::COM1) });
        log::set_logger(&LogWrapper).unwrap();
        log::set_max_level(LevelFilter::Trace);
    }

    pub fn switch_to_graphical(
        VideoModeInfo { framebuffer_address, pixel_format, width, height, stride }: &VideoModeInfo,
    ) {
        let framebuffer = match pixel_format {
            seed::boot_info::PixelFormat::Rgb32 => {
                Framebuffer::new(usize::from(*framebuffer_address) as *mut u32, *width, *height, *stride, 0, 8, 16)
            }
            seed::boot_info::PixelFormat::Bgr32 => {
                Framebuffer::new(usize::from(*framebuffer_address) as *mut u32, *width, *height, *stride, 16, 8, 0)
            }
        };
        *LOGGER.lock() = Logger::Graphical {
            serial_port: unsafe { SerialPort::new(hal_x86_64::hw::serial::COM1) },
            console: GfxConsole::new(framebuffer, 0x0000aaff, 0xffffffff),
        };
    }
}

impl fmt::Write for Logger {
    fn write_str(&mut self, s: &str) -> Result<(), fmt::Error> {
        match self {
            Logger::Uninit => panic!("Tried to log before it was initialized!"),
            Logger::Serial(serial_port) => serial_port.write_str(s),
            Logger::Graphical { serial_port, console } => {
                serial_port.write_str(s)?;
                console.write_str(s)
            }
        }
    }
}

unsafe impl Sync for Logger {}
unsafe impl Send for Logger {}
