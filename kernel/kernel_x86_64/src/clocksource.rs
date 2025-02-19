//! As Poplar only targets relatively modern x86 systems, we can always rely on the TSC to be a
//! reasonable clocksource. All Intel CPUs since Nehalem and AMD since Bulldozer have supported
//! invariant TSCs, which makes it a nice timer for wall-clock measurements.
//!
//! Where supported, we use information from `cpuid` to calculate the TSC frequency, either
//! reported directly, or derived from the frequency of the ART (Always-Running Timer). Where this
//! is not possible, we use another clock to calibrate the frequency of the TSC.

use core::{arch::asm, cell::SyncUnsafeCell};
use hal_x86_64::hw::cpu::CpuInfo;
use kernel::clocksource::{Clocksource, FractionalFreq};
use tracing::info;

static TSC_NS_PER_TICK: SyncUnsafeCell<FractionalFreq> = SyncUnsafeCell::new(FractionalFreq::zero());

pub struct TscClocksource;

impl TscClocksource {
    pub fn init(cpu_info: &CpuInfo) {
        let tsc_freq = cpu_info
            .tsc_frequency()
            .expect("No TSC frequency in CPUID; need to implement alternative calibration")
            as u64;
        info!("TSC frequency: {} Hz", tsc_freq);

        // TODO: if cpuid doesn't report the TSC frequency, we'll need to calibrate it ourselves
        // with another timer

        // Find the inverse of the frequency and convert from Hz to nHz in one step.
        let tsc_ns_per_tick = FractionalFreq::new(1_000_000_000, tsc_freq);
        unsafe {
            core::ptr::write(TSC_NS_PER_TICK.get() as *mut _, tsc_ns_per_tick);
        }
    }
}

impl Clocksource for TscClocksource {
    fn nanos_since_boot() -> u64 {
        let raw = read_tsc();
        let tsc_ns_per_tick = unsafe { *TSC_NS_PER_TICK.get() };
        tsc_ns_per_tick * raw
    }
}

pub fn read_tsc() -> u64 {
    let (high, low): (u32, u32);
    unsafe {
        /*
         * The `rdtsc` instruction is not serializing, and may be speculatively executed before
         * preceding loads. This can, in theory, produce non-monotonically-increasing TSC values
         * when reads are performed between CPUs. To avoid having to consider whether this could be
         * a problem for us, we perform a load fence before to serialize all loads before the
         * `rdtsc`.
         *
         * This may not be necessary / not necessary all the time.
         *
         * TODO: `rdtscp` is also not serializing, but seems to do the equivalent of a load fence
         * before it by default. If we're happy to assume it exists, we could do `rdtscp` instead
         * here (this does clobber `ecx`)?
         */
        asm!("lfence; rdtsc",
            out("eax") low,
            out("edx") high
        );
    }
    (high as u64) << 32 | (low as u64)
}
