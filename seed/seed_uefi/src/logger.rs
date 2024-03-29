use core::fmt;
use gfxconsole::{Bgr32, Format, Framebuffer, GfxConsole, Pixel, Rgb32};
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
    /*
     * To avoid putting a generic on `Logger` (which is difficult to work with as it's stored in a static), we
     * enumerate each possible pixel format here. Kinda sucks, but it's the best way I've found to do this.
     */
    Rgb32 { serial_port: SerialPort, console: GfxConsole<gfxconsole::Rgb32> },
    Bgr32 { serial_port: SerialPort, console: GfxConsole<gfxconsole::Bgr32> },
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
        match pixel_format {
            seed::boot_info::PixelFormat::Rgb32 => {
                let framebuffer = Framebuffer {
                    ptr: usize::from(*framebuffer_address) as *mut Pixel<Rgb32>,
                    width: *width,
                    height: *height,
                    stride: *stride,
                };
                *LOGGER.lock() = Logger::Rgb32 {
                    serial_port: unsafe { SerialPort::new(hal_x86_64::hw::serial::COM1) },
                    console: GfxConsole::new(
                        framebuffer,
                        Rgb32::pixel(0x00, 0x00, 0xaa, 0xff),
                        Rgb32::pixel(0xff, 0xff, 0xff, 0xff),
                    ),
                };
            }
            seed::boot_info::PixelFormat::Bgr32 => {
                let framebuffer = Framebuffer {
                    ptr: usize::from(*framebuffer_address) as *mut Pixel<Bgr32>,
                    width: *width,
                    height: *height,
                    stride: *stride,
                };
                *LOGGER.lock() = Logger::Bgr32 {
                    serial_port: unsafe { SerialPort::new(hal_x86_64::hw::serial::COM1) },
                    console: GfxConsole::new(
                        framebuffer,
                        Bgr32::pixel(0x00, 0x00, 0xaa, 0xff),
                        Bgr32::pixel(0xff, 0xff, 0xff, 0xff),
                    ),
                };
            }
        }
    }
}

impl fmt::Write for Logger {
    fn write_str(&mut self, s: &str) -> Result<(), fmt::Error> {
        match self {
            Logger::Uninit => panic!("Tried to log before it was initialized!"),
            Logger::Serial(serial_port) => serial_port.write_str(s),
            Logger::Rgb32 { serial_port, console } => {
                serial_port.write_str(s)?;
                console.write_str(s)
            }
            Logger::Bgr32 { serial_port, console } => {
                serial_port.write_str(s)?;
                console.write_str(s)
            }
        }
    }
}

unsafe impl Sync for Logger {}
unsafe impl Send for Logger {}
