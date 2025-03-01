use core::cell::SyncUnsafeCell;
use fdt::Fdt;
use kernel::clocksource::FractionalFreq;
use tracing::info;

static NS_PER_TICK: SyncUnsafeCell<FractionalFreq> = SyncUnsafeCell::new(FractionalFreq::zero());

pub struct Clocksource;

impl Clocksource {
    pub fn initialize(device_tree: &Fdt) {
        let timebase_freq = device_tree
            .find_node("/cpus")
            .and_then(|cpus| cpus.property("timebase-frequency"))
            .and_then(|freq| freq.as_usize())
            .unwrap();
        info!("Timebase frequency: {:?} Hz", timebase_freq);

        // Find the inverse of the frequency and convert from Hz to nHz in one step.
        let ns_per_tick = FractionalFreq::new(1_000_000_000, timebase_freq as u64);
        unsafe {
            core::ptr::write(NS_PER_TICK.get() as *mut _, ns_per_tick);
        }
    }
}

impl kernel::clocksource::Clocksource for Clocksource {
    fn nanos_since_boot() -> u64 {
        let raw = hal_riscv::hw::csr::Time::read() as u64;
        let ns_per_tick = unsafe { *NS_PER_TICK.get() };
        ns_per_tick * raw
    }
}
