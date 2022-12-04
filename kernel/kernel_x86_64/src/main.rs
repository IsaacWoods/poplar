#![no_std]
#![no_main]
#![feature(asm_sym, decl_macro, naked_functions, allocator_api)]

extern crate alloc;
extern crate rlibc;

mod acpi_handler;
mod interrupts;
mod logger;
mod pci;
mod per_cpu;
mod task;
mod topo;

use acpi::{platform::ProcessorState, AcpiTables, PciConfigRegions};
use acpi_handler::{AmlHandler, PoplarAcpiHandler};
use alloc::{alloc::Global, boxed::Box, sync::Arc};
use aml::AmlContext;
use core::{panic::PanicInfo, pin::Pin, time::Duration};
use hal::{
    boot_info::BootInfo,
    memory::{Frame, PAddr, VAddr},
};
use hal_x86_64::{hw::registers::read_control_reg, kernel_map, paging::PageTableImpl};
use interrupts::InterruptController;
use kernel::{
    memory::{KernelStackAllocator, PhysicalMemoryManager, Stack},
    object::{memory_object::MemoryObject, KernelObjectId},
    scheduler::Scheduler,
    Platform,
};
use log::{error, info};
use pci::PciResolver;
use per_cpu::PerCpuImpl;
use spin::Mutex;
use topo::Topology;

// TODO: store the PciInfo in here and allow access from the common kernel
pub struct PlatformImpl {
    kernel_page_table: PageTableImpl,
    topology: Topology,
}

impl Platform for PlatformImpl {
    type PageTableSize = hal::memory::Size4KiB;
    type PageTable = PageTableImpl;
    type PerCpu = per_cpu::PerCpuImpl;

    fn kernel_page_table(&mut self) -> &mut Self::PageTable {
        &mut self.kernel_page_table
    }

    fn per_cpu<'a>() -> Pin<&'a mut Self::PerCpu> {
        unsafe { per_cpu::get_per_cpu_data() }
    }

    unsafe fn initialize_task_stacks(
        kernel_stack: &Stack,
        user_stack: &Stack,
        task_entry_point: VAddr,
    ) -> (VAddr, VAddr) {
        task::initialize_stacks(kernel_stack, user_stack, task_entry_point)
    }

    unsafe fn context_switch(current_kernel_stack: *mut VAddr, new_kernel_stack: VAddr) {
        task::context_switch(current_kernel_stack, new_kernel_stack)
    }

    unsafe fn drop_into_userspace() -> ! {
        task::drop_into_userspace()
    }
}

#[no_mangle]
pub extern "C" fn kentry(boot_info: &BootInfo) -> ! {
    log::set_logger(&logger::KernelLogger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    info!("Poplar kernel is running");

    if boot_info.magic != hal::boot_info::BOOT_INFO_MAGIC {
        panic!("Boot info magic is not correct!");
    }

    /*
     * Get the kernel page tables set up by the loader. We have to assume that the loader has set up a correct set
     * of page tables, including a full physical mapping at the correct location, and so this is very unsafe.
     */
    let kernel_page_table = unsafe {
        PageTableImpl::from_frame(
            Frame::starts_with(PAddr::new(read_control_reg!(cr3) as usize).unwrap()),
            kernel_map::PHYSICAL_MAPPING_BASE,
        )
    };

    /*
     * Initialise the heap allocator. After this, the kernel is free to use collections etc. that
     * can allocate on the heap through the global allocator.
     */
    unsafe {
        kernel::ALLOCATOR.lock().init(boot_info.heap_address, boot_info.heap_size);
    }

    kernel::PHYSICAL_MEMORY_MANAGER.initialize(PhysicalMemoryManager::new(boot_info));

    let mut kernel_stack_allocator = KernelStackAllocator::<PlatformImpl>::new(
        kernel_map::KERNEL_STACKS_BASE,
        kernel_map::KERNEL_STACKS_BASE + kernel_map::STACK_SLOT_SIZE * kernel_map::MAX_TASKS,
        hal::memory::mebibytes(2),
    );

    /*
     * We want to replace the GDT and IDT as soon as we can, as we're currently relying on the ones installed by
     * UEFI. This is required for us to install exception handlers, which allows us to gracefully catch and report
     * exceptions.
     */
    unsafe {
        hal_x86_64::hw::gdt::GDT.lock().load();
    }

    /*
     * Install exception handlers early, so we can catch and report exceptions if they occur during initialization.
     * We don't have much infrastructure up yet, so we can't do anything fancy like set up IST stacks, but we can
     * always come back when more of the kernel is set up and add them.
     */
    InterruptController::install_exception_handlers();

    /*
     * Parse the static ACPI tables.
     */
    if boot_info.rsdp_address.is_none() {
        panic!("Bootloader did not pass RSDP address. Booting without ACPI is not supported.");
    }
    let acpi_tables =
        match unsafe { AcpiTables::from_rsdp(PoplarAcpiHandler, usize::from(boot_info.rsdp_address.unwrap())) } {
            Ok(acpi_tables) => acpi_tables,
            Err(err) => panic!("Failed to discover ACPI tables: {:?}", err),
        };
    let acpi_platform_info = acpi_tables.platform_info_in(&Global).unwrap();

    /*
     * Create a topology and add the boot processor to it.
     */
    let mut topology = Topology::new();
    let boot_cpu = {
        use hal_x86_64::hw::tss::Tss;

        let acpi_info = acpi_platform_info.processor_info.as_ref().unwrap().boot_processor;
        assert_eq!(acpi_info.state, ProcessorState::Running);
        assert!(!acpi_info.is_ap);

        let tss = Box::pin(Tss::new());
        let tss_selector = hal_x86_64::hw::gdt::GDT.lock().add_tss(0, tss.as_ref());
        unsafe {
            core::arch::asm!("ltr ax", in("ax") tss_selector.0);
        }

        let mut per_cpu = PerCpuImpl::new(tss, Scheduler::new());
        per_cpu.as_mut().install();
        topology.add_boot_processor(topo::Cpu {
            id: topo::BOOT_CPU_ID,
            local_apic_id: acpi_info.local_apic_id,
            per_cpu,
        });
    };

    let pci_access = pci::EcamAccess::new(PciConfigRegions::new_in(&acpi_tables, &Global).unwrap());

    /*
     * Parse the DSDT.
     */
    // TODO: if we're on ACPI 1.0 - pass true as legacy mode.
    let mut aml_context = AmlContext::new(Box::new(AmlHandler::new(pci_access)), aml::DebugVerbosity::None);
    // TODO: match on this to differentiate between there being no DSDT vs real error
    if let Ok(ref dsdt) = acpi_tables.dsdt() {
        let virtual_address = kernel_map::physical_to_virtual(PAddr::new(dsdt.address).unwrap());
        info!(
            "DSDT parse: {:?}",
            aml_context
                .parse_table(unsafe { core::slice::from_raw_parts(virtual_address.ptr(), dsdt.length as usize) })
        );

        // TODO: we should parse the SSDTs here. Only bother if we've managed to parse the DSDT.

        // info!("----- Printing AML namespace -----");
        // info!("{:#?}", aml_context.namespace);
        // info!("----- Finished AML namespace -----");
    }

    /*
     * Resolve all the PCI info.
     * XXX: not sure this is the right place to do this just yet.
     */
    // TODO: this whole situation is a bit gross and needs more thought I think
    // FIXME: this is broken by the new version of `acpi` for now
    // *kernel::PCI_INFO.write() = Some(PciResolver::resolve(pci_access.clone()));
    // kernel::PCI_ACCESS.initialize(Some(Mutex::new(Box::new(pci_access))));

    // TODO: if we need to route PCI interrupts, this might be useful at some point?
    // let routing_table =
    //     PciRoutingTable::from_prt_path(&AmlName::from_str("\\_SB.PCI0._PRT").unwrap(), aml_context)
    //         .expect("Failed to parse _PRT");

    /*
     * Initialize devices defined in AML.
     * TODO: We should probably call `_REG` on all the op-regions we allow access to at this point before this.
     */
    // aml_context.initialize_objects().expect("Failed to initialize AML objects");

    /*
     * Initialise the interrupt controller, which enables interrupts, and start the per-cpu timer.
     */
    let mut interrupt_controller =
        InterruptController::init(&acpi_platform_info.interrupt_model, &mut aml_context);
    unsafe {
        core::arch::asm!("sti");
    }
    interrupt_controller.enable_local_timer(&topology.cpu_info, Duration::from_millis(10));

    task::install_syscall_handler();

    let mut platform = PlatformImpl { kernel_page_table, topology };

    /*
     * Create kernel objects from loaded images and schedule them.
     */
    info!("Loading {} initial tasks to the ready queue", boot_info.loaded_images.num_images);
    for image in boot_info.loaded_images.images() {
        kernel::load_task(
            &mut PlatformImpl::per_cpu().scheduler(),
            image,
            platform.kernel_page_table(),
            &kernel::PHYSICAL_MEMORY_MANAGER.get(),
            &mut kernel_stack_allocator,
        );
    }
    if let Some(ref video_info) = boot_info.video_mode {
        kernel::create_framebuffer(video_info);
    }

    /*
     * Drop into userspace!
     */
    info!("Dropping into usermode");
    PlatformImpl::per_cpu().scheduler().drop_to_userspace()
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    error!("KERNEL PANIC: {}", info);

    /*
     * If the `qemu_exit` feature is set, we use the debug port to exit.
     */
    #[cfg(feature = "qemu_exit")]
    {
        use hal_x86_64::hw::qemu::{ExitCode, ExitPort};
        unsafe { ExitPort::new() }.exit(ExitCode::Failed)
    }

    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
