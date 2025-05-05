/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

use core::{
    fmt,
    fmt::Write,
    panic::PanicInfo,
    sync::atomic::{AtomicU64, Ordering},
};
use fdt::Fdt;
use hal::memory::PAddr;
use hal_riscv::{hw::uart16550::Uart16550, platform::kernel_map::physical_to_virtual};
use kernel::tasklets::queue::QueueProducer;
use mulch::InitGuard;
use spinning_top::Spinlock;
use tracing::{span, Collect, Event, Level, Metadata};
use tracing_core::span::Current as CurrentSpan;

static SERIAL: InitGuard<Uart16550<'static>> = InitGuard::uninit();
static SERIAL_PRODUCER: InitGuard<kernel::tasklets::queue::QueueProducer> = InitGuard::uninit();
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

    /*
     * TODO XXX: re-initializing the UART seems to break things on the D1. I'm not yet sure why -
     * the IP is apparently a Synopsys DesignWare 8250. Apparently this impl will raise an
     * interrupt when you write to its LCR when it's busy - I wonder if that might be happening
     * here, and we're not handling it correctly and so get the UART stuck somehow?
     *
     * See: https://patchwork.kernel.org/project/linux-arm-kernel/patch/1354640699-6066-1-git-send-email-gregory.clement@free-electrons.com/
     *
     * We'll need to dig into whether we should be re-initing the UART anyways here, and if we want
     * to how to poke the D1's UART in the correct way.
     */
    let serial_mapped_address = physical_to_virtual(PAddr::new(addr).unwrap());
    let serial = unsafe { Uart16550::new(serial_mapped_address, reg_width) };
    // serial.init();
    SERIAL.initialize(serial);

    tracing::dispatch::set_global_default(tracing::dispatch::Dispatch::from_static(&LOGGER))
        .expect("Failed to set default tracing dispatch");
}

pub fn enable_input(fdt: &Fdt, producer: QueueProducer) {
    // TODO: on the D1 this doesn't seem to produce a node with an `interrupts` property? The DT
    // does have one on `uart0`
    let stdout = fdt.chosen().stdout().unwrap().node();
    crate::interrupts::handle_wired_fdt_device_interrupt(stdout, interrupt_handler);
    SERIAL_PRODUCER.initialize(producer);
}

fn interrupt_handler(_: u16) {
    let serial = SERIAL.get();
    if let Some(producer) = SERIAL_PRODUCER.try_get() {
        while let Some(byte) = serial.read() {
            // TODO: with more stuff running and higher baud we might end up with multiple
            // chars - would be more efficient to use a bigger grant.
            let mut write = producer.grant_sync(1).unwrap();
            write[0] = byte;
            write.commit(1);
        }
    } else {
        /*
         * Nothing's interested in the serial input, so just blackhole it to avoid repeat
         * interrupts.
         */
        while let Some(_) = serial.read() {}
    }
}

struct SerialWriter;

impl fmt::Write for SerialWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let serial = SERIAL.get();
        for byte in s.bytes() {
            serial.write(byte);
        }

        Ok(())
    }
}

// TODO: abstract out serial writer and centralise `tracing` logging infra into `kernel`

struct Logger {
    next_id: AtomicU64,
    pub serial: Spinlock<SerialWriter>,
}

impl Logger {
    const fn new() -> Logger {
        Logger { next_id: AtomicU64::new(1), serial: Spinlock::new(SerialWriter) }
    }
}

impl Collect for Logger {
    fn current_span(&self) -> CurrentSpan {
        todo!()
    }

    fn enabled(&self, metadata: &Metadata) -> bool {
        // TODO: support more extensive + customizable filtering
        *metadata.level() <= Level::INFO
    }

    fn enter(&self, _span: &span::Id) {}

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

    fn exit(&self, _span: &span::Id) {}

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

    fn record(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
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

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        self.record(field, &value);
    }
}

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    if let Some(location) = info.location() {
        let _ = writeln!(
            SerialWriter,
            "PANIC: {} ({} - {}:{})",
            info.message(),
            location.file(),
            location.line(),
            location.column()
        );
    } else {
        let _ = writeln!(SerialWriter, "PANIC: {} (no location info)", info.message());
    }
    loop {}
}
