/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

#![no_std]
#![no_main]
#![feature(fn_align, sync_unsafe_cell)]

extern crate alloc;

mod clocksource;
mod interrupts;
mod pci;
mod serial;
mod task;
mod trap;

use alloc::string::String;
use hal::memory::{Flags, Frame, FrameSize, Size4KiB, VAddr};
use hal_riscv::hw::csr::Satp;
use kernel::{
    bootinfo::{BootInfo, EarlyFrameAllocator},
    pmm::Pmm,
    scheduler::Scheduler,
    vmm::Vmm,
    Platform,
};
use mulch::InitGuard;
use tracing::info;

pub struct PlatformImpl;

impl Platform for PlatformImpl {
    type PageTableSize = hal::memory::Size4KiB;
    #[cfg(any(feature = "platform_rv64_virt", test))]
    type PageTable = hal_riscv::paging::PageTableImpl<hal_riscv::paging::Level4>;
    #[cfg(feature = "platform_mq_pro")]
    type PageTable = hal_riscv::paging::PageTableImpl<hal_riscv::paging::Level3>;
    type Clocksource = clocksource::Clocksource;
    type TaskContext = task::TaskContext;

    fn new_task_context(
        kernel_stack: &kernel::vmm::Stack,
        user_stack: &kernel::vmm::Stack,
        task_entry_point: VAddr,
    ) -> Self::TaskContext {
        task::new_task_context(kernel_stack, user_stack, task_entry_point)
    }

    fn new_task_page_tables() -> Self::PageTable {
        todo!()
    }

    unsafe fn context_switch(from_context: *mut Self::TaskContext, to_context: *const Self::TaskContext) {
        task::context_switch(from_context, to_context);
    }

    unsafe fn drop_into_userspace(context: *const Self::TaskContext) -> ! {
        task::drop_into_userspace(context)
    }

    fn rearm_interrupt(_interrupt: usize) {}
}

pub static VMM: InitGuard<Vmm<PlatformImpl>> = InitGuard::uninit();
pub static SCHEDULER: InitGuard<Scheduler<PlatformImpl>> = InitGuard::uninit();

#[no_mangle]
pub extern "C" fn kentry(boot_info_ptr: *mut ()) -> ! {
    let mut boot_info = unsafe { BootInfo::new(boot_info_ptr) };

    let fdt = {
        let address = boot_info.physical_mapping_base() + boot_info.device_tree_addr().unwrap() as usize;
        unsafe { fdt::Fdt::from_ptr(address.ptr()).unwrap() }
    };
    serial::init(&fdt, &boot_info);
    info!("Hello from the kernel");

    trap::install_early_handler();

    // info!("Boot info: {:#?}", boot_info);
    // info!("FDT: {:#?}", fdt);

    clocksource::Clocksource::initialize(&fdt);

    let mut kernel_page_table = unsafe {
        match Satp::read() {
            Satp::Sv39 { root, .. } => <PlatformImpl as Platform>::PageTable::from_frame(
                Frame::starts_with(root),
                boot_info.physical_mapping_base(),
            ),
            Satp::Sv48 { root, .. } => <PlatformImpl as Platform>::PageTable::from_frame(
                Frame::starts_with(root),
                boot_info.physical_mapping_base(),
            ),
            _ => {
                panic!("Kernel booted in an unexpected paging mode! Have we been built for the correct platform?");
            }
        }
    };

    /*
     * Initialise the heap allocator. After this, the kernel is free to use collections etc. that
     * can allocate on the heap through the global allocator.
     */
    {
        use hal::memory::{FrameAllocator, PageTable};

        // TODO: reduce initial size probs and add ability to grow heap as needed
        const INITIAL_HEAP_SIZE: usize = 800 * 1024;
        // TODO: we might want to do this in the dynamic area instead of after the kernel
        let heap_start = boot_info.kernel_free_start();
        let early_allocator = EarlyFrameAllocator::new(&mut boot_info);
        let initial_heap = early_allocator.allocate_n(Size4KiB::frames_needed(INITIAL_HEAP_SIZE));

        info!("Initialising early heap of size {:#x} bytes at {:#x}", INITIAL_HEAP_SIZE, heap_start);
        kernel_page_table
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
    VMM.initialize(Vmm::new(kernel_page_table, &boot_info));

    interrupts::init(&fdt);
    unsafe {
        hal_riscv::hw::csr::Sie::enable_all();
        hal_riscv::hw::csr::Sstatus::enable_interrupts();
    }

    if let Some(access) = pci::PciAccess::new(&fdt) {
        kernel::initialize_pci(access);
    }

    SCHEDULER.initialize(Scheduler::new());
    maitake::time::set_global_timer(&SCHEDULER.get().tasklet_scheduler.timer).unwrap();

    let (uart_prod, uart_cons) = kernel::tasklets::queue::SpscQueue::new();
    serial::enable_input(&fdt, uart_prod);
    SCHEDULER.get().tasklet_scheduler.spawn(async move {
        loop {
            let line = {
                let mut line = String::new();
                loop {
                    let bytes = uart_cons.read().await;
                    let as_str = core::str::from_utf8(&bytes).unwrap();
                    if let Some(index) = as_str.find('\r') {
                        let (before, _after) = as_str.split_at(index);
                        line += before;
                        // Only release up to (and including) the newline so the next pass can consume any bytes
                        // after it
                        bytes.release(index + 1);
                        break;
                    } else {
                        line += as_str;
                        let num_bytes = bytes.len();
                        bytes.release(num_bytes);
                    }
                }
                line
            };
            info!("Line from UART: {}", line);
        }
    });

    /*
     * Create kernel objects from loaded images and schedule them.
     */
    kernel::load_userspace(SCHEDULER.get(), &boot_info, &VMM.get());

    /*
     * Kick the timer off. We do this just before installing the full handler because the shim
     * handler doesn't support timer interrupts, so we'll get stuck if we do take too long between
     * this and having the real handler in place.
     */
    // TODO: global function for getting number of ticks per us or whatever from the device tree
    sbi::timer::set_timer(hal_riscv::hw::csr::Time::read() as u64 + 0x989680 / 50).unwrap();

    /*
     * Move to a trap handler that can handle traps from both S-mode and U-mode. We can only do
     * this now because we need a `sscratch` context installed (which hasn't technically happened
     * yet but will very soon, so cross your fingers AND toes).
     */
    trap::install_full_handler();

    SCHEDULER.get().start_scheduling()
}
