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
use hal::memory::{Frame, PAddr, VAddr};
use hal_x86_64::{
    hw::{cpu::CpuInfo, registers::read_control_reg, tss::Tss},
    kernel_map,
    paging::PageTableImpl,
};
use interrupts::{InterruptController, INTERRUPT_CONTROLLER};
use kacpi::AcpiManager;
use kernel::{
    memory::{vmm::Stack, Pmm, Vmm},
    scheduler::Scheduler,
    Platform,
};
use mulch::InitGuard;
use pci::PciConfigurator;
use per_cpu::PerCpuImpl;
use seed::boot_info::BootInfo;
use spinning_top::RwSpinlock;
use topo::Topology;
use tracing::info;

pub struct PlatformImpl {
    topology: Topology,
}

impl Platform for PlatformImpl {
    type PageTableSize = hal::memory::Size4KiB;
    type PageTable = PageTableImpl;
    type Clocksource = TscClocksource;
    type TaskContext = task::TaskContext;

    fn new_task_context(kernel_stack: &Stack, user_stack: &Stack, task_entry_point: VAddr) -> Self::TaskContext {
        task::new_task_context(kernel_stack, user_stack, task_entry_point)
    }

    unsafe fn context_switch(from_context: *mut Self::TaskContext, to_context: *const Self::TaskContext) {
        task::context_switch(from_context, to_context)
    }

    /// Do the actual drop into usermode. This assumes that the task's page tables have already been installed,
    /// and that an initial frame has been put into the task's kernel stack that this will use to enter userspace.
    unsafe fn drop_into_userspace(context: *const Self::TaskContext) -> ! {
        task::drop_into_userspace(context)
    }

    unsafe fn write_to_phys_memory(address: PAddr, data: &[u8]) {
        let virt: *mut u8 = hal_x86_64::kernel_map::physical_to_virtual(address).mut_ptr();
        unsafe {
            core::ptr::copy(data.as_ptr(), virt, data.len());
        }
    }

    fn rearm_interrupt(interrupt: usize) {
        // TODO: this should be replaced by a spinlock that actually disables interrupts...
        unsafe { core::arch::asm!("cli") };
        interrupts::INTERRUPT_CONTROLLER.get().try_lock().unwrap().rearm_interrupt(interrupt as u32);
        unsafe { core::arch::asm!("sti") };
    }
}

pub static SCHEDULER: InitGuard<Scheduler<PlatformImpl>> = InitGuard::uninit();
pub static KERNEL_PAGE_TABLES: InitGuard<RwSpinlock<hal_x86_64::paging::PageTableImpl>> = InitGuard::uninit();

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
    let kernel_page_tables = unsafe {
        PageTableImpl::from_frame(
            Frame::starts_with(PAddr::new(read_control_reg!(cr3) as usize).unwrap()),
            kernel_map::PHYSICAL_MAPPING_BASE,
        )
    };
    KERNEL_PAGE_TABLES.initialize(RwSpinlock::new(kernel_page_tables));

    /*
     * Initialise the heap allocator. After this, the kernel is free to use collections etc. that
     * can allocate on the heap through the global allocator.
     */
    unsafe {
        kernel::ALLOCATOR.lock().init(boot_info.heap_address.mut_ptr(), boot_info.heap_size);
    }

    kernel::PMM.initialize(Pmm::new(boot_info));
    kernel::VMM.initialize(Vmm::new(
        kernel_map::KERNEL_STACKS_BASE,
        kernel_map::KERNEL_STACKS_BASE + kernel_map::STACK_SLOT_SIZE * kernel_map::MAX_TASKS,
        hal::memory::mebibytes(2),
    ));

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
    kernel::load_userspace(SCHEDULER.get(), &boot_info, &mut KERNEL_PAGE_TABLES.get().write());
    if let Some(ref video_info) = boot_info.video_mode {
        kernel::create_framebuffer(video_info);
    }

    SCHEDULER.get().start_scheduling();
}
