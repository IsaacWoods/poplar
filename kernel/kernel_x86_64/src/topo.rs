use acpi::platform::ProcessorState;
use alloc::vec::Vec;
use hal_x86_64::hw::cpu::CpuInfo;
use tracing::{info, warn};

pub type ProcessorId = u32;
pub const BOOT_PROCESSOR_ID: ProcessorId = 0;

#[derive(Clone, Copy, Debug)]
pub struct Processor {
    pub id: ProcessorId,
    pub local_apic_id: u32,
}

#[derive(Clone, Debug)]
pub struct Topology {
    pub cpu_info: CpuInfo,
    pub boot_processor: Processor,
    pub application_processors: Vec<Processor>,
}

impl Topology {
    pub fn new(cpu_info: CpuInfo, acpi_info: &acpi::PlatformInfo<alloc::alloc::Global>) -> Topology {
        info!(
            "We're running on an {:?} processor. The microarchitecture is: {:?}",
            cpu_info.vendor,
            cpu_info.microarch()
        );
        if let Some(ref hypervisor_info) = cpu_info.hypervisor_info {
            info!("We're running under a hypervisor: {:?}", hypervisor_info.vendor);
        }

        check_support_and_enable_features(&cpu_info);

        let processor_info = acpi_info.processor_info.as_ref().unwrap();
        let boot_processor = {
            let acpi = processor_info.boot_processor;
            assert_eq!(acpi.state, ProcessorState::Running);
            assert!(!acpi.is_ap);
            Processor { id: BOOT_PROCESSOR_ID, local_apic_id: acpi.local_apic_id }
        };

        let mut id = 1;
        let application_processors = processor_info
            .application_processors
            .iter()
            .filter_map(|info| match info.state {
                ProcessorState::WaitingForSipi => {
                    assert!(info.is_ap);
                    let processor = Processor { id, local_apic_id: info.local_apic_id };
                    id += 1;
                    Some(processor)
                }
                ProcessorState::Disabled => {
                    warn!(
                        "Processor with local APIC id {} is disabled by firmware. Ignoring.",
                        info.local_apic_id
                    );
                    None
                }
                ProcessorState::Running => {
                    panic!("Application processor is already running; how have you managed that?")
                }
            })
            .collect::<Vec<_>>();
        info!("Located {} application processors to attempt bring-up on", application_processors.len());

        Topology { cpu_info, boot_processor, application_processors }
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
