//! This module defines the kernel entry-point on x86_64.

mod acpi_handler;
mod address_space;
mod cpu;
mod interrupts;
mod logger;
mod memory;
mod task;

use self::{
    acpi_handler::PebbleAcpiHandler,
    address_space::AddressSpace,
    cpu::Cpu,
    interrupts::InterruptController,
    logger::KernelLogger,
    memory::LockedPhysicalMemoryManager,
};
use crate::{
    arch::Architecture,
    object::{map::ObjectMap, KernelObject},
};
use acpi::ProcessorState;
use alloc::vec::Vec;
use aml_parser::AmlContext;
use log::{error, info, warn};
use spin::{Mutex, RwLock};
use x86_64::{
    boot::BootInfo,
    hw::{
        cpu::CpuInfo,
        gdt::{Gdt, TssSegment},
        registers::read_control_reg,
        tss::Tss,
    },
    memory::{kernel_map, Frame, Page, PageTable, PhysicalAddress, VirtualAddress},
};

/// The kernel GDT. This is not thread-safe, and so should only be altered by the bootstrap
/// processor.
static mut GDT: Gdt = Gdt::new();

pub struct Arch {
    pub physical_memory_manager: LockedPhysicalMemoryManager,
    pub kernel_page_table: Mutex<PageTable>,
    pub object_map: RwLock<ObjectMap<Self>>,
}

/// `Arch` contains a bunch of things, like the GDT, that the hardware relies on actually being at
/// the memory addresses we say they're at. We can stop them moving using `Unpin`, but can't stop
/// them from being dropped, so we just panic if the architecture struct is dropped.
impl Drop for Arch {
    fn drop(&mut self) {
        panic!("The `Arch` has been dropped. This should never happen!");
    }
}

impl Architecture for Arch {
    type AddressSpace = AddressSpace;
    type Task = Task;
}

/// This is the entry point for the kernel on x86_64. It is called from the UEFI bootloader and
/// initialises the system, then passes control into the common part of the kernel.
#[no_mangle]
pub fn kmain() -> ! {
    /*
     * Initialise the logger.
     */
    log::set_logger(&KernelLogger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    info!("The Pebble kernel is running");

    let cpu_info = CpuInfo::new();
    info!(
        "We're running on an {:?} processor, model info = {:?}, microarch = {:?}",
        cpu_info.vendor,
        cpu_info.model_info,
        cpu_info.microarch()
    );

    /*
     * Initialise the heap allocator. After this, the kernel is free to use collections etc. that
     * can allocate on the heap through the global allocator.
     */
    #[cfg(not(test))]
    unsafe {
        crate::ALLOCATOR.lock().init(kernel_map::HEAP_START, kernel_map::HEAP_END);
    }

    /*
     * Retrieve the `BootInfo` passed to us from the bootloader and make sure it has the correct
     * magic number.
     */
    let boot_info = unsafe { &mut *(kernel_map::BOOT_INFO.mut_ptr::<BootInfo>()) };
    if boot_info.magic != x86_64::boot::BOOT_INFO_MAGIC {
        panic!("Boot info magic number is not correct!");
    }

    /*
     * Initialise the physical memory manager. From this point, we can allocate physical memory
     * freely.
     *
     * This assumes the bootloader has installed a valid set of page tables, including mapping
     * the entirity of the physical memory at the start of the kernel's P4 entry. Strange
     * things will happen if this assumption does not hold, so this is fairly unsafe.
     */
    let arch = Arch {
        physical_memory_manager: LockedPhysicalMemoryManager::new(boot_info),
        kernel_page_table: Mutex::new(unsafe {
            PageTable::from_frame(Frame::starts_with(
                PhysicalAddress::new(read_control_reg!(cr3) as usize).unwrap(),
            ))
        }),
        object_map: RwLock::new(ObjectMap::new(crate::object::map::INITIAL_OBJECT_CAPACITY)),
    };

    /*
     * Parse the static ACPI tables.
     */
    let acpi_info = match boot_info.rsdp_address {
        Some(rsdp_address) => {
            let mut handler = PebbleAcpiHandler;
            match acpi::parse_rsdp(&mut handler, usize::from(rsdp_address)) {
                Ok(acpi_info) => Some(acpi_info),

                Err(err) => {
                    error!("Failed to parse static ACPI tables: {:?}", err);
                    warn!(
                        "Continuing. Some functionality may not work, or the kernel may
    panic!"
                    );
                    None
                }
            }
        }

        None => None,
    };

    info!("{:#?}", acpi_info);

    /*
     * Register all the CPUs we can find.
     */
    let (mut boot_processor, application_processors) = match acpi_info {
        Some(ref info) => {
            assert!(
                info.boot_processor.is_some()
                    && info.boot_processor.unwrap().state == ProcessorState::Running
            );
            let tss = Tss::new();
            let tss_selector = unsafe { GDT.add_tss(TssSegment::new(&tss)) };
            let boot_processor = Cpu::from_acpi(&info.boot_processor.unwrap(), tss, tss_selector);

            let mut application_processors = Vec::new();
            for application_processor in &info.application_processors {
                if application_processor.state == ProcessorState::Disabled {
                    continue;
                }

                let tss = Tss::new();
                let tss_selector = unsafe { GDT.add_tss(TssSegment::new(&tss)) };
                application_processors.push(Cpu::from_acpi(
                    &application_processor,
                    tss,
                    tss_selector,
                ));
            }

            (boot_processor, application_processors)
        }

        None => {
            /*
             * We couldn't find the number of processors from the ACPI tables. Just create a TSS
             * for this one.
             */
            let tss = Tss::new();
            let tss_selector = unsafe { GDT.add_tss(TssSegment::new(&tss)) };
            let cpu = Cpu { processor_uid: 0, local_apic_id: 0, is_ap: false, tss, tss_selector };
            (cpu, Vec::with_capacity(0))
        }
    };

    unsafe {
        GDT.load();
    }

    // TODO: deal gracefully with a bad ACPI parse
    let interrupt_controller = InterruptController::init(
        &arch,
        match acpi_info {
            Some(ref info) => info.interrupt_model.as_ref().unwrap(),
            None => unimplemented!(),
        },
    );

    /*
     * Parse the DSDT.
     */
    let mut aml_context = AmlContext::new();
    if let Some(dsdt_info) = acpi_info.and_then(|info| info.dsdt) {
        let virtual_address =
            kernel_map::physical_to_virtual(PhysicalAddress::new(dsdt_info.address).unwrap());
        info!(
            "DSDT parse: {:?}",
            aml_context.parse_table(unsafe {
                core::slice::from_raw_parts(virtual_address.ptr(), dsdt_info.length as usize)
            })
        );
    }

    drop_to_userboot(&arch, boot_info, &mut boot_processor.tss)
}

fn drop_to_userboot(arch: &Arch, boot_info: &BootInfo, tss: &mut Tss) -> ! {
    /*
     * Extract userboot's page tables from where the bootloader constructed them, build it an
     * `AddressSpace` and a `Task`, and drop into usermode!
     */
    let address_space = arch.object_map.write().insert(KernelObject::AddressSpace(RwLock::new(
        AddressSpace::from_page_table(&arch, unsafe {
            InactivePageTable::<RecursiveMapping>::new(Frame::contains(
                boot_info.payload.page_table_address,
            ))
        }),
    )));
    let task = KernelObject::Task(RwLock::new(Task::new(
        &arch,
        address_space,
        boot_info.payload.entry_point,
    )));
    let task_id = arch.object_map.write().insert(task);

    info!("Dropping to usermode");
    task::drop_to_usermode(arch, tss, task_id);
}
