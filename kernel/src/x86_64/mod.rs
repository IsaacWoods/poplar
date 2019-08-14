//! This module defines the kernel entry-point on x86_64.

mod acpi_handler;
mod address_space;
mod cpu;
mod interrupts;
mod logger;
mod memory;
mod memory_object;
mod per_cpu;
mod task;

// Export the common per-CPU data accessors, so they can be used from the rest of the kernel.
pub use self::per_cpu::{common_per_cpu_data, common_per_cpu_data_mut};

use self::{
    acpi_handler::PebbleAcpiHandler,
    address_space::AddressSpace,
    interrupts::InterruptController,
    logger::KernelLogger,
    memory::LockedPhysicalMemoryManager,
    memory_object::MemoryObject,
    task::Task,
};
use crate::{
    arch::Architecture,
    object::{map::ObjectMap, KernelObject, WrappedKernelObject},
    scheduler::Scheduler,
    x86_64::per_cpu::per_cpu_data_mut,
};
use aml_parser::AmlContext;
use core::time::Duration;
use log::{error, info, warn};
use spin::{Mutex, RwLock};
use x86_64::{
    boot::{BootInfo, ImageInfo},
    hw::{cpu::CpuInfo, gdt::Gdt, registers::read_control_reg},
    memory::{kernel_map, Frame, PageTable, PhysicalAddress},
};

pub(self) static GDT: Mutex<Gdt> = Mutex::new(Gdt::new());

pub struct Arch {
    pub cpu_info: CpuInfo,
    pub physical_memory_manager: LockedPhysicalMemoryManager,
    pub kernel_page_table: Mutex<PageTable>,
    pub object_map: RwLock<ObjectMap<Self>>,
    /* pub boot_processor: Mutex<Cpu>,
     * pub application_processors: Mutex<Vec<Cpu>>, */
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
    type MemoryObject = MemoryObject;

    fn drop_to_userspace(&self, task: WrappedKernelObject<Arch>) -> ! {
        task::drop_to_usermode(self, task);
    }
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
     * Parse the static ACPI tables.
     */
    let acpi_info = match boot_info.rsdp_address {
        Some(rsdp_address) => {
            let mut handler = PebbleAcpiHandler;
            match acpi::parse_rsdp(&mut handler, usize::from(rsdp_address)) {
                Ok(acpi_info) => Some(acpi_info),

                Err(err) => {
                    error!("Failed to parse static ACPI tables: {:?}", err);
                    warn!("Continuing. Some functionality may not work, or the kernel may panic!");
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
    // let (mut boot_processor, application_processors) = match acpi_info {
    //     Some(ref info) => {
    //         assert!(
    //             info.boot_processor.is_some()
    //                 && info.boot_processor.unwrap().state == ProcessorState::Running
    //         );
    //         // TODO: Cpu shouldn't manage the TSS anymore - that should be the job of the per-cpu
    //         // data
    //         let tss = Tss::new();
    //         let tss_selector = unsafe { GDT.lock().add_tss(TssSegment::new(&tss)) };
    //         let boot_processor = Cpu::from_acpi(&info.boot_processor.unwrap(), tss, tss_selector);

    //         let mut application_processors = Vec::new();
    //         for application_processor in &info.application_processors {
    //             if application_processor.state == ProcessorState::Disabled {
    //                 continue;
    //             }

    //             let tss = Tss::new();
    //             let tss_selector = unsafe { GDT.lock().add_tss(TssSegment::new(&tss)) };
    //             application_processors.push(Cpu::from_acpi(&application_processor, tss, tss_selector));
    //         }

    //         (boot_processor, application_processors)
    //     }

    //     None => {
    //         /*
    //          * We couldn't find the number of processors from the ACPI tables. Just create a TSS
    //          * for this one.
    //          */
    //         let tss = Tss::new();
    //         let tss_selector = unsafe { GDT.lock().add_tss(TssSegment::new(Pin::new(&tss))) };
    //         let cpu = Cpu { processor_uid: 0, local_apic_id: 0, is_ap: false, tss, tss_selector };
    //         (cpu, Vec::with_capacity(0))
    //     }
    // };

    /*
     * Initialise the physical memory manager. From this point, we can allocate physical memory
     * freely.
     *
     * This assumes the bootloader has installed a valid set of page tables, including mapping
     * the entirity of the physical memory at the start of the kernel's P4 entry. Strange
     * things will happen if this assumption does not hold, so this is fairly unsafe.
     */
    let arch = Arch {
        cpu_info,
        physical_memory_manager: LockedPhysicalMemoryManager::new(boot_info),
        kernel_page_table: Mutex::new(unsafe {
            PageTable::from_frame(
                Frame::starts_with(PhysicalAddress::new(read_control_reg!(cr3) as usize).unwrap()),
                kernel_map::PHYSICAL_MAPPING_BASE,
            )
        }),
        object_map: RwLock::new(ObjectMap::new(crate::object::map::INITIAL_OBJECT_CAPACITY)),
        /* boot_processor: Mutex::new(boot_processor),
         * application_processors: Mutex::new(application_processors), */
    };

    /*
     * Create the per-cpu data, then load the GDT, then install the per-cpu data. This has to be
     * done in this specific order because loading the GDT after setting GS_BASE will override it.
     */
    let (guarded_per_cpu, tss_selector) = per_cpu::GuardedPerCpu::new();
    unsafe {
        // TODO: having to lock it prevents `load` from taking a pinned reference, reference with
        // 'static, which we should probably deal with.
        GDT.lock().load(tss_selector);
    }
    guarded_per_cpu.install();

    // TODO: deal gracefully with a bad ACPI parse
    let mut interrupt_controller = InterruptController::init(
        &arch,
        match acpi_info {
            Some(ref info) => info.interrupt_model.as_ref().unwrap(),
            None => unimplemented!(),
        },
    );
    interrupt_controller.enable_local_timer(&arch, Duration::from_secs(3));

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

    /*
     * Load all the images as initial tasks, and add them to the scheduler's ready list.
     */
    use core::ops::DerefMut;
    let mut scheduler = unsafe { per_cpu_data_mut() }.scheduler();
    for image in boot_info.images() {
        load_task(&arch, &mut scheduler.deref_mut(), image);
    }

    info!("Dropping to usermode");
    scheduler.drop_to_userspace(&arch)
}

fn load_task(arch: &Arch, scheduler: &mut Scheduler<Arch>, image: &ImageInfo) {
    // Make an AddressSpace for the image
    let address_space: WrappedKernelObject<Arch> =
        KernelObject::AddressSpace(RwLock::new(box AddressSpace::new(&arch)))
            .add_to_map(&mut arch.object_map.write());

    // Make a MemoryObject for each segment and map it into the AddressSpace
    for segment in image.segments() {
        let memory_object = KernelObject::MemoryObject(RwLock::new(box MemoryObject::new(&segment)))
            .add_to_map(&mut arch.object_map.write());
        address_space
            .object
            .address_space()
            .unwrap()
            .write()
            .map_memory_object(&arch, memory_object: WrappedKernelObject<Arch>);
    }

    // Create a Task for the image and add it to the scheduler's ready queue
    let task = KernelObject::Task(RwLock::new(
        box Task::from_image_info(&arch, address_space.clone(), image).unwrap(),
    ))
    .add_to_map(&mut arch.object_map.write());
    scheduler.add_task(task).unwrap();
}
