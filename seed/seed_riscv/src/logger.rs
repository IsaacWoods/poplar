use core::sync::atomic::{AtomicU64, Ordering};
use hal_riscv::hw::uart16550::Uart16550;
use poplar_util::InitGuard;
use tracing::{span, Collect, Event, Metadata};
use tracing_core::span::Current as CurrentSpan;

static LOGGER: Logger = Logger::new();

pub struct Logger {
    next_id: AtomicU64,
    serial_port: InitGuard<&'static mut Uart16550>,
}

impl Logger {
    const fn new() -> Logger {
        Logger { next_id: AtomicU64::new(0), serial_port: InitGuard::uninit() }
    }

    pub fn init() {
        let serial_port = unsafe { &mut *(0x10000000 as *mut hal_riscv::hw::uart16550::Uart16550) };
        LOGGER.serial_port.initialize(serial_port);
        tracing::dispatch::set_global_default(tracing::dispatch::Dispatch::from_static(&LOGGER))
            .expect("Failed to set default tracing dispatch");
    }
}

impl Collect for Logger {
    fn current_span(&self) -> CurrentSpan {
        todo!()
    }

    fn enabled(&self, _: &Metadata) -> bool {
        true
    }

    fn enter(&self, span: &span::Id) {
        todo!()
    }

    fn event(&self, event: &Event) {
        todo!()
    }

    fn exit(&self, span: &span::Id) {
        todo!()
    }

    fn new_span(&self, span: &span::Attributes) -> span::Id {
        let mut id = self.next_id.fetch_add(1, Ordering::Acquire);
        span::Id::from_u64(id)
    }

    fn record(&self, _span: &span::Id, _values: &span::Record) {
        todo!()
    }

    fn record_follows_from(&self, _span: &span::Id, _follows: &span::Id) {
        todo!()
    }
}
