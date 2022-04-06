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

// pub fn build_topology(acpi_platform_info: &acpi::PlatformInfo) -> Topology {
//     let processor_info = acpi_platform_info.processor_info.as_ref().unwrap();
//     let mut boot_cpu = {
//         assert_eq!(processor_info.boot_processor.state, ProcessorState::Running);
//         assert!(!processor_info.boot_processor.is_ap);
//         Cpu::new(0, processor_info.boot_processor.local_apic_id)
//     };

//     /*
//      * Create a `Cpu` for each application processor that can be brought up. This maintains the order that the
//      * processors appear in the MADT, which is the order that they should be brought up in.
//      */
//     let application_cpus = Vec::new();
//     // let mut id = 1;
//     // let application_cpus = processor_info
//     //     .application_processors
//     //     .iter()
//     //     .filter_map(|processor| match processor.state {
//     //         ProcessorState::WaitingForSipi => {
//     //             assert!(processor.is_ap);
//     //             let cpu = Cpu::new(id, processor.local_apic_id);
//     //             id += 1;
//     //             Some(cpu)
//     //         }
//     //         ProcessorState::Disabled => {
//     //             warn!(
//     //                 "Processor with local APIC id {} is disabled by firmware. Ignoring.",
//     //                 processor.local_apic_id
//     //             );
//     //             None
//     //         }
//     //         ProcessorState::Running => {
//     //             panic!("Application processor is already running; how have you managed that?")
//     //         }
//     //     })
//     //     .collect::<Vec<_>>();
//     // info!("Located {} application processors to attempt bring-up on", application_cpus.len());

//     /*
//      * This code runs on the boot processor, so we can load the GDT with the boot processor's TSS and the boot
//      * processor's per-CPU data here.
//      * XXX: per-CPU data must be installed after the GDT, as we zero `gs` when the GDT is loaded.
//      */
//     // TODO: we no longer need to load the GDT on the boot processor, but we do need to create a TSS descriptor in
//     // the correct slot, and then call `ltr`, as this is no longer done as part of GDT creation.
//     unsafe {
//         // hal_x86_64::hw::gdt::GDT.lock().load(boot_cpu.tss_selector);
//     }
//     boot_cpu.per_cpu.as_mut().install();

//     Topology { cpu_info, boot_cpu, application_cpus }
// }

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
