use core::{
    fmt::{self, Write},
    str::FromStr,
    sync::atomic::{AtomicU64, Ordering},
};
use hal::{cmdline::Cmdline, io::IoPort, sync::Spinlock};
use tracing::warn;
use tracing_core::{Event, Level, span};

pub static SUBSCRIBER: Subscriber = Subscriber::new();

pub struct Subscriber {
    next_id: AtomicU64,
    debug: Spinlock<DebugWriter>,
}

impl Subscriber {
    pub const fn new() -> Subscriber {
        Subscriber {
            next_id: AtomicU64::new(0),
            debug: Spinlock::new(DebugWriter::new()),
        }
    }

    pub fn configure(&self, cmdline: &Cmdline) {
        if let Some(Some(level)) = cmdline.get("trace.level") {
            if let Ok(level) = Level::from_str(level) {
                self.debug.lock().min_level = level;
            } else {
                warn!(
                    "Invalid option for `trace.level` ({:?}). Valid options are `error`, `warn`, `info`, `debug`, or `trace`",
                    level
                );
            }
        }
    }
}

impl tracing_core::Collect for Subscriber {
    fn enabled(&self, metadata: &tracing_core::Metadata<'_>) -> bool {
        *metadata.level() <= self.debug.lock().min_level
    }

    fn new_span(&self, _span: &span::Attributes<'_>) -> span::Id {
        let id = self.next_id.fetch_add(1, Ordering::Acquire);
        span::Id::from_u64(id)
    }

    fn record(&self, span: &span::Id, values: &span::Record<'_>) {
        let _ = (span, values);
        todo!()
    }

    fn record_follows_from(&self, span: &span::Id, follows: &span::Id) {
        let _ = (span, follows);
        todo!()
    }

    fn event(&self, event: &Event<'_>) {
        use core::ops::DerefMut;

        if self.enabled(event.metadata()) {
            let (color, label) = match *event.metadata().level() {
                Level::TRACE => ("\x1b[36m", "T"),
                Level::DEBUG => ("\x1b[34m", "D"),
                Level::INFO => ("\x1b[32m", "I"),
                Level::WARN => ("\x1b[33m", "W"),
                Level::ERROR => ("\x1b[31m", "E"),
            };
            // TODO: clocksource
            let (time_secs, time_subsec) = (0, 0);
            let mut writer = self.debug.lock();
            write!(
                writer,
                "[{:>6}.{:06}] {}{}\x1b[0m {}: ",
                time_secs,
                time_subsec,
                color,
                label,
                event.metadata().target()
            )
            .unwrap();
            event.record(&mut Visitor::new(writer.deref_mut()));
            write!(writer, "\n").unwrap();
        }
    }

    fn enter(&self, span: &span::Id) {
        let _ = span;
        todo!()
    }

    fn exit(&self, span: &span::Id) {
        let _ = span;
        todo!()
    }

    fn current_span(&self) -> tracing_core::span::Current {
        todo!()
    }
}

pub struct Visitor<'a, W>
where
    W: fmt::Write,
{
    writer: &'a mut W,
}

impl<'a, W> Visitor<'a, W>
where
    W: fmt::Write,
{
    pub fn new(writer: &'a mut W) -> Visitor<'a, W> {
        Visitor { writer }
    }

    fn record(&mut self, field: &tracing_core::field::Field, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            write!(self.writer, "{:?}", value).unwrap();
        } else {
            write!(self.writer, "{}={:?}", field, value).unwrap()
        }
    }
}

impl<'a, W> tracing_core::field::Visit for Visitor<'a, W>
where
    W: fmt::Write,
{
    #[inline]
    fn record_f64(&mut self, field: &tracing_core::Field, value: f64) {
        self.record(field, &value)
    }

    #[inline]
    fn record_i64(&mut self, field: &tracing_core::Field, value: i64) {
        self.record(field, &value)
    }

    #[inline]
    fn record_u64(&mut self, field: &tracing_core::Field, value: u64) {
        self.record(field, &value)
    }

    #[inline]
    fn record_i128(&mut self, field: &tracing_core::Field, value: i128) {
        self.record(field, &value)
    }

    #[inline]
    fn record_u128(&mut self, field: &tracing_core::Field, value: u128) {
        self.record(field, &value)
    }

    #[inline]
    fn record_bool(&mut self, field: &tracing_core::Field, value: bool) {
        self.record(field, &value)
    }

    #[inline]
    fn record_str(&mut self, field: &tracing_core::Field, value: &str) {
        self.record(field, &value)
    }

    #[inline]
    fn record_bytes(&mut self, field: &tracing_core::Field, value: &[u8]) {
        self.record(field, &value)
    }

    #[inline]
    fn record_debug(&mut self, field: &tracing_core::Field, value: &dyn fmt::Debug) {
        self.record(field, value)
    }
}

pub struct DebugWriter {
    port: IoPort<u8>,
    min_level: Level,
}

impl DebugWriter {
    pub const fn new() -> DebugWriter {
        DebugWriter {
            port: unsafe { IoPort::new(0xe9) },
            min_level: Level::TRACE,
        }
    }
}

impl fmt::Write for DebugWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.as_bytes() {
            unsafe {
                self.port.write(*b);
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
