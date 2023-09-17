/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

#![no_std]
#![no_main]
#![feature(decl_macro, naked_functions, allocator_api, panic_info_message)]

extern crate alloc;

mod acpi_handler;
mod interrupts;
mod logger;
mod pci;
mod per_cpu;
mod task;
mod topo;

use acpi::{platform::ProcessorState, AcpiTables, PciConfigRegions};
use acpi_handler::{AmlHandler, PoplarAcpiHandler};
use alloc::{alloc::Global, boxed::Box};
use aml::AmlContext;
use core::time::Duration;
use hal::memory::{Frame, PAddr, VAddr};
use hal_x86_64::{
    hw::{registers::read_control_reg, tss::Tss},
    kernel_map,
    paging::PageTableImpl,
};
use interrupts::InterruptController;
use kernel::{
    memory::{KernelStackAllocator, PhysicalMemoryManager, Stack},
    per_cpu::PerCpu,
    scheduler::Scheduler,
    Platform,
};
use per_cpu::PerCpuImpl;
use seed::boot_info::BootInfo;
use topo::Topology;
use tracing::info;

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

    unsafe fn per_cpu<'a>() -> &'a mut Self::PerCpu {
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
    logger::init();
    info!("Poplar kernel is running");

    if boot_info.magic != seed::boot_info::BOOT_INFO_MAGIC {
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
     * Install a TSS for this processor. This then allows us to set up the per-CPU data structures.
     */
    let tss = Box::new(Tss::new());
    let tss_selector = hal_x86_64::hw::gdt::GDT.lock().add_tss(0, tss.as_ref() as *const Tss);
    unsafe {
        core::arch::asm!("ltr ax", in("ax") tss_selector.0);
    }
    PerCpuImpl::install(tss, Scheduler::new());

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
    let acpi_platform_info = acpi_tables.platform_info_in(Global).unwrap();

    /*
     * Create a topology and add the boot processor to it.
     */
    let mut topology = Topology::new();
    {
        let acpi_info = acpi_platform_info.processor_info.as_ref().unwrap().boot_processor;
        assert_eq!(acpi_info.state, ProcessorState::Running);
        assert!(!acpi_info.is_ap);

        topology.add_boot_processor(topo::Cpu { id: topo::BOOT_CPU_ID, local_apic_id: acpi_info.local_apic_id });
    }

    let pci_access = pci::EcamAccess::new(PciConfigRegions::new_in(&acpi_tables, Global).unwrap());

    /*
     * Parse the DSDT.
     */
    let mut aml_context = AmlContext::new(Box::new(AmlHandler::new(pci_access)), aml::DebugVerbosity::None);
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
    info!("Loading {} initial tasks to the ready queue", boot_info.loaded_images.len());
    for image in &boot_info.loaded_images {
        kernel::load_task(
            &mut unsafe { PlatformImpl::per_cpu() }.scheduler(),
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
    unsafe { PlatformImpl::per_cpu() }.scheduler().drop_to_userspace()
}
