/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

use core::{
    fmt,
    fmt::Write,
    sync::atomic::{AtomicU64, Ordering},
};
use fdt::Fdt;
use hal::memory::VAddr;
use hal_riscv::hw::uart16550::Uart16550;
use mulch::InitGuard;
use spinning_top::Spinlock;
use tracing::{span, Collect, Event, Level, Metadata};
use tracing_core::span::Current as CurrentSpan;

static LOGGER: Logger = Logger::new();

pub fn init(fdt: &Fdt) {
    let Some(stdout) = fdt.chosen().stdout() else {
        // TODO: not sure the point of this as we won't be able to print the message? Can we report
        // the error through an SBI call or something instead?
        panic!("FDT must contain a chosen stdout node!");
    };
    // TODO: check the compatible to make sure it's something we support
    // TODO: technically reg-shift could place the registers further apart than their width. Maybe
    // need to support this at some point?
    let addr = stdout.node().reg().unwrap().next().unwrap().starting_address as usize;
    let reg_width = match stdout.node().property("reg-io-width") {
        Some(property) => property.as_usize().unwrap_or(1),
        None => 1,
    };

    LOGGER.serial.lock().init(addr, reg_width);
    tracing::dispatch::set_global_default(tracing::dispatch::Dispatch::from_static(&LOGGER))
        .expect("Failed to set default tracing dispatch");
}

struct SerialWriter {
    serial: InitGuard<Uart16550<'static>>,
}

impl SerialWriter {
    const fn new() -> SerialWriter {
        SerialWriter { serial: InitGuard::uninit() }
    }

    fn init(&mut self, addr: usize, reg_width: usize) {
        let serial = unsafe { Uart16550::new(VAddr::new(addr), reg_width) };
        self.serial.initialize(serial);
    }
}

impl fmt::Write for SerialWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let serial = self.serial.get_mut();
        for byte in s.bytes() {
            serial.write(byte);
        }

        Ok(())
    }
}

struct Logger {
    next_id: AtomicU64,
    serial: Spinlock<SerialWriter>,
}

impl Logger {
    const fn new() -> Logger {
        Logger { next_id: AtomicU64::new(1), serial: Spinlock::new(SerialWriter::new()) }
    }
}

impl Collect for Logger {
    fn current_span(&self) -> CurrentSpan {
        todo!()
    }

    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn enter(&self, _span: &span::Id) {
        todo!()
    }

    fn event(&self, event: &Event) {
        use core::ops::DerefMut;

        if self.enabled(event.metadata()) {
            let level = event.metadata().level();
            let color = match *level {
                Level::TRACE => "\x1b[36m",
                Level::DEBUG => "\x1b[34m",
                Level::INFO => "\x1b[32m",
                Level::WARN => "\x1b[33m",
                Level::ERROR => "\x1b[31m",
            };
            let mut serial = self.serial.lock();
            write!(serial, "[{}{:5}\x1b[0m] {}: ", color, level, event.metadata().target()).unwrap();
            event.record(&mut Visitor::new(serial.deref_mut()));
            write!(serial, "\n").unwrap();
        }
    }

    fn exit(&self, _span: &span::Id) {
        todo!()
    }

    fn new_span(&self, _span: &span::Attributes) -> span::Id {
        let id = self.next_id.fetch_add(1, Ordering::Acquire);
        span::Id::from_u64(id)
    }

    fn record(&self, _span: &span::Id, _values: &span::Record) {
        todo!()
    }

    fn record_follows_from(&self, _span: &span::Id, _follows: &span::Id) {
        todo!()
    }
}

struct Visitor<'w, W>
where
    W: Write,
{
    writer: &'w mut W,
}

impl<'w, W> Visitor<'w, W>
where
    W: Write,
{
    fn new(writer: &'w mut W) -> Visitor<'w, W> {
        Visitor { writer }
    }

    fn record(&mut self, field: &tracing::field::Field, value: &dyn core::fmt::Debug) {
        // Handle the `message` field explicitly to declutter the output
        if field.name() == "message" {
            write!(self.writer, "{:?}", value).unwrap();
        } else {
            write!(self.writer, "{}={:?}", field, value).unwrap();
        }
    }
}

impl<'w, W> tracing::field::Visit for Visitor<'w, W>
where
    W: Write,
{
    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.record(field, &value);
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.record(field, &value);
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.record(field, &value);
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.record(field, &value);
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn core::fmt::Debug) {
        self.record(field, &value);
    }
}

#[panic_handler]
pub fn panic(info: &core::panic::PanicInfo) -> ! {
    if let Some(location) = info.location() {
        let _ = writeln!(
            LOGGER.serial.lock(),
            "PANIC: {} ({} - {}:{})",
            info.message(),
            location.file(),
            location.line(),
            location.column()
        );
    } else {
        let _ = writeln!(LOGGER.serial.lock(), "PANIC: {} (no location info)", info.message());
    }
    loop {}
}
