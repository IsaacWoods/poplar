use core::fmt;
use hal::{io::IoPort, sync::Spinlock};

pub const DEBUG_WRITER: Spinlock<DebugWriter> = Spinlock::new(DebugWriter::new());

macro_rules! println {
    () => {
        core::write!($crate::util::DEBUG_WRITER.lock(), "\n").expect("Failed to write to debug writer!");
    };
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        $crate::util::DEBUG_WRITER
            .lock()
            .write_fmt(core::format_args!(
                "{}{}",
                core::format_args!($($arg)*),
                "\n"
            ))
            .expect("Failed to write to debug writer!");
    }};
}

pub struct DebugWriter(IoPort<u8>);

impl DebugWriter {
    pub const fn new() -> DebugWriter {
        DebugWriter(unsafe { IoPort::new(0xe9) })
    }
}

impl fmt::Write for DebugWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.as_bytes() {
            unsafe {
                self.0.write(*b);
            }
        }
        Ok(())
    }
}

#[cfg(not(test))]
#[panic_handler]
fn panic_handler(info: &core::panic::PanicInfo) -> ! {
    use core::fmt::Write;

    let mut debug_writer = DebugWriter::new();

    if let Some(location) = info.location() {
        let _ = write!(
            debug_writer,
            "PANIC: {} ({} - {}:{})",
            info.message(),
            location.file(),
            location.line(),
            location.column()
        );
    } else {
        let _ = write!(debug_writer, "PANIC: {} (no location info)", info.message(),);
    }

    loop {}
}
