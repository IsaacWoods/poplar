use crate::per_cpu::PerCpuImpl;
use alloc::{boxed::Box, vec, vec::Vec};
use core::{fmt, pin::Pin};
use hal_x86_64::hw::{cpu::CpuInfo, gdt::SegmentSelector};
use kernel::scheduler::Scheduler;
use log::{info, warn};

pub type CpuId = u32;
pub const BOOT_CPU_ID: CpuId = 0;

pub struct Cpu {
    pub id: CpuId,
    pub local_apic_id: u32,
    pub per_cpu: Pin<Box<PerCpuImpl>>,
}

pub struct Topology {
    pub cpu_info: CpuInfo,
    pub boot_cpu: Option<Cpu>,
    pub application_cpus: Vec<Cpu>,
}

impl Topology {
    pub fn new() -> Topology {
        let cpu_info = CpuInfo::new();
        info!(
            "We're running on an {:?} processor. The microarchitecture is: {:?}",
            cpu_info.vendor,
            cpu_info.microarch()
        );
        if let Some(ref hypervisor_info) = cpu_info.hypervisor_info {
            info!("We're running under a hypervisor: {:?}", hypervisor_info.vendor);
        }

        check_support_and_enable_features(&cpu_info);

        Topology { cpu_info, boot_cpu: None, application_cpus: vec![] }
    }

    pub fn add_boot_processor(&mut self, cpu: Cpu) {
        assert!(self.boot_cpu.is_none());
        self.boot_cpu = Some(cpu);
    }
}

/// We rely on certain processor features to be present for simplicity and sanity-retention. This
/// function checks that we support everything we need to, and enable features that we need.
fn check_support_and_enable_features(cpu_info: &CpuInfo) {
    use bit_field::BitField;
    use hal_x86_64::hw::registers::{
        read_control_reg,
        read_msr,
        write_control_reg,
        write_msr,
        CR4_ENABLE_GLOBAL_PAGES,
        CR4_RESTRICT_RDTSC,
        CR4_XSAVE_ENABLE_BIT,
        EFER,
        EFER_ENABLE_NX_BIT,
        EFER_ENABLE_SYSCALL,
    };

    if !cpu_info.supported_features.xsave {
        panic!("Processor does not support xsave instruction!");
    }

    let mut cr4 = read_control_reg!(CR4);
    cr4.set_bit(CR4_XSAVE_ENABLE_BIT, true);
    cr4.set_bit(CR4_ENABLE_GLOBAL_PAGES, true);
    cr4.set_bit(CR4_RESTRICT_RDTSC, true);
    unsafe {
        write_control_reg!(CR4, cr4);
    }

    let mut efer = read_msr(EFER);
    efer.set_bit(EFER_ENABLE_SYSCALL, true);
    efer.set_bit(EFER_ENABLE_NX_BIT, true);
    unsafe {
        write_msr(EFER, efer);
    }
}
