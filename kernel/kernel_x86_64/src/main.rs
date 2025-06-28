/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

#![no_std]
#![no_main]
#![feature(decl_macro, allocator_api, iterator_try_collect, unsafe_cell_access, sync_unsafe_cell)]

extern crate alloc;

mod clocksource;
mod interrupts;
mod kacpi;
mod logger;
mod pci;
mod per_cpu;
mod task;
mod topo;

use alloc::boxed::Box;
use clocksource::TscClocksource;
use core::time::Duration;
use hal::memory::{Flags, Frame, FrameSize, PAddr, PageTable, Size4KiB, VAddr};
use hal_x86_64::{
    hw::{cpu::CpuInfo, registers::read_control_reg, tss::Tss},
    paging::PageTableImpl,
};
use interrupts::{InterruptController, INTERRUPT_CONTROLLER};
use kacpi::AcpiManager;
use kernel::{
    bootinfo::{BootInfo, EarlyFrameAllocator},
    pmm::Pmm,
    scheduler::Scheduler,
    vmm::{Stack, Vmm},
    Platform,
};
use mulch::{linker::LinkerSymbol, InitGuard};
use pci::PciConfigurator;
use per_cpu::PerCpuImpl;
use topo::Topology;
use tracing::info;

extern "C" {
    static _kernel_start: LinkerSymbol;
    static _kernel_end: LinkerSymbol;
}

pub struct PlatformImpl {
    topology: Topology,
}

impl Platform for PlatformImpl {
    type PageTableSize = hal::memory::Size4KiB;
    type PageTable = PageTableImpl;
    type Clocksource = TscClocksource;
    type TaskContext = task::TaskContext;

    const HIGHER_HALF_START: VAddr = seed_bootinfo::kernel_map::HIGHER_HALF_START;
    const PHYSICAL_MAPPING_BASE: VAddr = seed_bootinfo::kernel_map::PHYSICAL_MAPPING_BASE;
    const KERNEL_DYNAMIC_AREA_BASE: VAddr = seed_bootinfo::kernel_map::KERNEL_DYNAMIC_AREA_BASE;
    const KERNEL_IMAGE_BASE: VAddr = seed_bootinfo::kernel_map::KERNEL_IMAGE_BASE;

    fn new_task_context(kernel_stack: &Stack, user_stack: &Stack, task_entry_point: VAddr) -> Self::TaskContext {
        task::new_task_context(kernel_stack, user_stack, task_entry_point)
    }

    fn new_task_page_tables() -> Self::PageTable {
        use hal::memory::FrameAllocator;
        use hal_x86_64::paging::ENTRY_COUNT;

        let mut page_table = PageTableImpl::new(kernel::PMM.get().allocate(), Self::PHYSICAL_MAPPING_BASE);
        let kernel_tables = &VMM.get().kernel_page_table.lock();

        for i in (ENTRY_COUNT / 2)..ENTRY_COUNT {
            page_table.p4_mut()[i] = kernel_tables.p4()[i];
        }

        page_table
    }

    unsafe fn context_switch(from_context: *mut Self::TaskContext, to_context: *const Self::TaskContext) {
        task::context_switch(from_context, to_context)
    }

    /// Do the actual drop into usermode. This assumes that the task's page tables have already been installed,
    /// and that an initial frame has been put into the task's kernel stack that this will use to enter userspace.
    unsafe fn drop_into_userspace(context: *const Self::TaskContext) -> ! {
        task::drop_into_userspace(context)
    }

    fn rearm_interrupt(interrupt: usize) {
        // TODO: this should be replaced by a spinlock that actually disables interrupts...
        unsafe { core::arch::asm!("cli") };
        interrupts::INTERRUPT_CONTROLLER.get().try_lock().unwrap().rearm_interrupt(interrupt as u32);
        unsafe { core::arch::asm!("sti") };
    }
}

pub static VMM: InitGuard<Vmm<PlatformImpl>> = InitGuard::uninit();
pub static SCHEDULER: InitGuard<Scheduler<PlatformImpl>> = InitGuard::uninit();

#[no_mangle]
pub extern "C" fn kentry(boot_info_ptr: *const ()) -> ! {
    logger::init();
    info!("Poplar kernel is running");

    /*
     * TODO: I want to redo how we do initialization here in a few ways given our increased experience:
     * - Take a memory map from Seed and have a bootstrapping phys allocator in the kernel
     * - Do not take a heap from Seed. Instead alloc out of the bootstrapping allocator for the initial heap.
     * - Have an early logging system for initial debug printing and then a proper common logging framework
     * - Collect CPU initialization into one place. This is inspired by Managarm and will be useful for SMP support.
     * - PCI init should theoretically be done in line with ACPI namespace iteration
     * - I wonder if we can make the GDT a const structure and remove its locking?? (need to think about per-CPU TSS mangling)
     */

    let mut boot_info = unsafe { BootInfo::new(boot_info_ptr) };

    /*
     * Get the kernel page tables set up by the loader. We have to assume that the loader has set up a correct set
     * of page tables, including a full physical mapping at the correct location, and so this is very unsafe.
     */
    let mut kernel_page_tables = unsafe {
        PageTableImpl::from_frame(
            Frame::starts_with(PAddr::new(read_control_reg!(cr3) as usize).unwrap()),
            seed_bootinfo::kernel_map::PHYSICAL_MAPPING_BASE,
        )
    };

    /*
     * Set up an initial heap at the start of the kernelspace dynamic area. This is required to
     * initialise the PMM and VMM as they both utilise allocating collections.
     */
    {
        use hal::memory::FrameAllocator;

        // TODO: reduce initial size probs and add ability to grow heap as needed
        const INITIAL_HEAP_SIZE: usize = 800 * 1024;
        // TODO: we might want to do this in the dynamic area instead of after the kernel
        let heap_start = boot_info.kernel_free_start();
        let early_allocator = EarlyFrameAllocator::new(&mut boot_info);
        let initial_heap = early_allocator.allocate_n(Size4KiB::frames_needed(INITIAL_HEAP_SIZE));

        info!("Initialising early heap of size {:#x} bytes at {:#x}", INITIAL_HEAP_SIZE, heap_start);
        kernel_page_tables
            .map_area(
                heap_start,
                initial_heap.start.start,
                INITIAL_HEAP_SIZE,
                Flags { writable: true, ..Default::default() },
                &early_allocator,
            )
            .unwrap();

        unsafe {
            kernel::ALLOCATOR.lock().init(heap_start.mut_ptr(), INITIAL_HEAP_SIZE);
        }
    }

    kernel::PMM.initialize(Pmm::new(boot_info.memory_map()));
    VMM.initialize(Vmm::new(kernel_page_tables));

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
    PerCpuImpl::install(tss);

    // TODO: go back and set the #PF handler to use a separate kernel stack via the TSS
    // TODO: various of the other handlers need separate IST entries (at least NMI, MCE(?), DF too)

    let acpi_tables = kacpi::find_tables(&boot_info);

    /*
     * Initialize the clocksource.
     */
    let cpu_info = CpuInfo::new();
    TscClocksource::init(&cpu_info, &acpi_tables);

    /*
     * Initialize ACPI. This also gives us access to the PCI configuration space and topology
     * information.
     */
    let (acpi_manager, pci_access) = AcpiManager::initialize(acpi_tables);
    let topology = Topology::new(cpu_info, &acpi_manager.platform);

    /*
     * Initialise the interrupt controller, which enables interrupts, and start the per-cpu timer.
     */
    InterruptController::init(&acpi_manager);
    unsafe {
        core::arch::asm!("sti");
    }
    INTERRUPT_CONTROLLER.get().lock().enable_local_timer(&topology.cpu_info, Duration::from_millis(10));

    let pci_configurator = PciConfigurator::new(pci_access, acpi_manager.clone());
    kernel::initialize_pci(pci_configurator);

    task::install_syscall_handler();

    let platform = PlatformImpl { topology };

    SCHEDULER.initialize(Scheduler::new());
    maitake::time::set_global_timer(&SCHEDULER.get().tasklet_scheduler.timer).unwrap();

    /*
     * Create kernel objects from loaded images and schedule them.
     */
    kernel::load_userspace(SCHEDULER.get(), &boot_info, VMM.get());
    if let Some(video_info) = boot_info.video_mode_info() {
        kernel::create_framebuffer(&video_info);
    }

    SCHEDULER.get().start_scheduling();
}
