//! As Poplar only targets relatively modern x86 systems, we can always rely on the TSC to be a
//! reasonable clocksource. All Intel CPUs since Nehalem and AMD since Bulldozer have supported
//! invariant TSCs, which makes it a nice timer for wall-clock measurements.
//!
//! Where supported, we use information from `cpuid` to calculate the TSC frequency, either
//! reported directly, or derived from the frequency of the ART (Always-Running Timer). Where this
//! is not possible, we use another clock to calibrate the frequency of the TSC.

use crate::{kacpi::PoplarHandler, pci::EcamAccess};
use acpi::{AcpiTables, HpetInfo};
use core::{arch::asm, cell::SyncUnsafeCell};
use hal::memory::{Flags, PAddr};
use hal_x86_64::hw::{
    cpu::CpuInfo,
    hpet::{GeneralCapsAndId, HpetRegBlock},
};
use kernel::clocksource::{Clocksource, FractionalFreq};
use mulch::InitGuard;
use tracing::info;

struct Hpet {
    regs: HpetRegBlock,
    timer_period: u64,
}

unsafe impl Send for Hpet {}
unsafe impl Sync for Hpet {}

impl Hpet {
    const FEMTOS_PER_NANO: u64 = 1_000_000;

    pub fn new(info: HpetInfo) -> Hpet {
        let regs_ptr: *mut u64 = crate::VMM
            .get()
            .map_kernel(
                PAddr::new(info.base_address as usize).unwrap(),
                0x1000,
                Flags { writable: true, cached: false, ..Default::default() },
            )
            .unwrap()
            .mut_ptr();
        let regs = unsafe { HpetRegBlock::new(regs_ptr) };
        let general_caps = regs.general_caps();
        info!("HPET capabilities: {:?}", general_caps);

        regs.enable_counter();

        Hpet { regs, timer_period: general_caps.get(GeneralCapsAndId::COUNTER_CLK_PERIOD) }
    }

    pub fn stall_nanos(&self, nanos: u64) {
        let current_timer = self.regs.main_counter_value();
        let stop_at = current_timer + nanos * Self::FEMTOS_PER_NANO / self.timer_period;

        while self.regs.main_counter_value() < stop_at {
            core::hint::spin_loop();
        }
    }
}

static TSC_NS_PER_TICK: SyncUnsafeCell<FractionalFreq> = SyncUnsafeCell::new(FractionalFreq::zero());
static HPET: InitGuard<Hpet> = InitGuard::uninit();

pub struct TscClocksource;

impl TscClocksource {
    pub fn init(cpu_info: &CpuInfo, acpi_tables: &AcpiTables<PoplarHandler<EcamAccess>>) {
        /*
         * If the TSC frequency is reported by `cpuid`, we just use that, and don't even try to initialize the HPET.
         */
        if let Some(tsc_freq) = cpu_info.tsc_frequency() {
            info!("TSC frequency: {} Hz", tsc_freq);
            // Find the inverse of the frequency and convert from Hz to nHz in one step.
            let tsc_ns_per_tick = FractionalFreq::new(1_000_000_000, tsc_freq as u64);
            unsafe {
                core::ptr::write(TSC_NS_PER_TICK.get() as *mut _, tsc_ns_per_tick);
            }
        } else {
            /*
             * Otherwise, use the HPET to calibrate the TSC.
             */
            let hpet_info = HpetInfo::new(&acpi_tables).expect("No HPET ACPI table");
            HPET.initialize(Hpet::new(hpet_info));

            const MS100_IN_NS: u64 = 100_000_000;
            let tsc_start = read_tsc();
            HPET.get().stall_nanos(MS100_IN_NS);
            let tsc_elapsed = read_tsc() - tsc_start;

            let tsc_ns_per_tick = FractionalFreq::new(MS100_IN_NS, tsc_elapsed as u64);
            unsafe {
                core::ptr::write(TSC_NS_PER_TICK.get() as *mut _, tsc_ns_per_tick);
            }
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
