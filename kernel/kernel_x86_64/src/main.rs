#![no_std]
#![no_main]
#![feature(asm, global_asm, decl_macro, naked_functions)]

extern crate alloc;
extern crate rlibc;

mod acpi_handler;
mod interrupts;
mod logger;
mod pci;
mod per_cpu;
mod task;
mod topo;

use acpi::{AcpiTables, PciConfigRegions};
use acpi_handler::{AmlHandler, PebbleAcpiHandler};
use alloc::{boxed::Box, sync::Arc};
use aml::AmlContext;
use core::{panic::PanicInfo, pin::Pin, time::Duration};
use hal::{
    boot_info::BootInfo,
    memory::{Frame, PhysicalAddress, VirtualAddress},
};
use hal_x86_64::{hw::registers::read_control_reg, kernel_map, paging::PageTableImpl};
use interrupts::InterruptController;
use kernel::{
    memory::{KernelStackAllocator, PhysicalMemoryManager, Stack},
    object::{memory_object::MemoryObject, KernelObjectId},
    Platform,
};
use log::{error, info};
use pci::PciResolver;
use spin::Mutex;
use topo::Topology;

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
        task_entry_point: VirtualAddress,
    ) -> (VirtualAddress, VirtualAddress) {
        task::initialize_stacks(kernel_stack, user_stack, task_entry_point)
    }

    unsafe fn context_switch(current_kernel_stack: *mut VirtualAddress, new_kernel_stack: VirtualAddress) {
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
    info!("Pebble kernel is running");

    if boot_info.magic != hal::boot_info::BOOT_INFO_MAGIC {
        panic!("Boot info magic is not correct!");
    }

    /*
     * Get the kernel page tables set up by the loader. We have to assume that the loader has set up a correct set
     * of page tables, including a full physical mapping at the correct location, and so this is very unsafe.
     */
    let kernel_page_table = unsafe {
        PageTableImpl::from_frame(
            Frame::starts_with(PhysicalAddress::new(read_control_reg!(cr3) as usize).unwrap()),
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
     * Install the exception handlers. Where we do this is a compromise between as-early-as-possible (we don't
     * catch exceptions properly before this), and having enough infrastructure to install nice handlers (e.g. with
     * IST entries etc.). This seems like a good point to do it.
     */
    InterruptController::install_exception_handlers();

    /*
     * Parse the static ACPI tables.
     */
    if boot_info.rsdp_address.is_none() {
        panic!("Bootloader did not pass RSDP address. Booting without ACPI is not supported.");
    }
    let acpi_tables =
        match unsafe { AcpiTables::from_rsdp(PebbleAcpiHandler, usize::from(boot_info.rsdp_address.unwrap())) } {
            Ok(acpi_tables) => acpi_tables,
            Err(err) => panic!("Failed to discover ACPI tables: {:?}", err),
        };
    let acpi_platform_info = acpi_tables.platform_info().unwrap();

    /*
     * Create the topology, which also creates a TSS and per-CPU data for each processor, and loads them for the
     * boot processor.
     */
    let topology = topo::build_topology(&acpi_platform_info);
    let pci_access = pci::EcamAccess::new(PciConfigRegions::new(&acpi_tables).unwrap());

    /*
     * Parse the DSDT.
     */
    // TODO: if we're on ACPI 1.0 - pass true as legacy mode.
    let mut aml_context =
        AmlContext::new(Box::new(AmlHandler::new(pci_access.clone())), aml::DebugVerbosity::None);
    if let Some(ref dsdt) = acpi_tables.dsdt {
        let virtual_address = kernel_map::physical_to_virtual(PhysicalAddress::new(dsdt.address).unwrap());
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
    *kernel::PCI_INFO.write() = Some(PciResolver::resolve(pci_access.clone()));
    kernel::PCI_ACCESS.initialize(Some(Mutex::new(Box::new(pci_access))));


    /*
     * Initialize devices defined in AML.
     * TODO: We should probably call `_REG` on all the op-regions we allow access to at this point before this.
     */
    aml_context.initialize_objects().expect("Failed to initialize AML objects");

    /*
     * Initialise the interrupt controller, which enables interrupts, and start the per-cpu timer.
     */
    let mut interrupt_controller =
        InterruptController::init(&acpi_platform_info.interrupt_model, &mut aml_context);
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
            asm!("hlt");
        }
    }
}
