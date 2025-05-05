/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

#![no_std]
#![no_main]
#![feature(fn_align, naked_functions, sync_unsafe_cell)]

extern crate alloc;

mod clocksource;
mod interrupts;
mod pci;
mod serial;
mod task;
mod trap;

use alloc::string::String;
use hal::memory::{Frame, PAddr, VAddr};
use hal_riscv::{
    hw::csr::Satp,
    platform::{kernel_map, PageTableImpl},
};
use kernel::{
    memory::{Pmm, Vmm},
    scheduler::Scheduler,
    Platform,
};
use mulch::InitGuard;
use seed::boot_info::BootInfo;
use spinning_top::RwSpinlock;
use tracing::info;

pub struct PlatformImpl;

impl Platform for PlatformImpl {
    type PageTableSize = hal::memory::Size4KiB;
    type PageTable = hal_riscv::platform::PageTableImpl;
    type Clocksource = clocksource::Clocksource;
    type TaskContext = task::TaskContext;

    fn new_task_context(
        kernel_stack: &kernel::memory::vmm::Stack,
        user_stack: &kernel::memory::vmm::Stack,
        task_entry_point: VAddr,
    ) -> Self::TaskContext {
        task::new_task_context(kernel_stack, user_stack, task_entry_point)
    }

    unsafe fn context_switch(from_context: *mut Self::TaskContext, to_context: *const Self::TaskContext) {
        task::context_switch(from_context, to_context);
    }

    unsafe fn drop_into_userspace(context: *const Self::TaskContext) -> ! {
        task::drop_into_userspace(context)
    }

    unsafe fn write_to_phys_memory(address: PAddr, data: &[u8]) {
        let virt: *mut u8 = hal_riscv::platform::kernel_map::physical_to_virtual(address).mut_ptr();
        unsafe {
            core::ptr::copy(data.as_ptr(), virt, data.len());
        }
    }

    fn rearm_interrupt(_interrupt: usize) {}
}

pub static SCHEDULER: InitGuard<Scheduler<PlatformImpl>> = InitGuard::uninit();
pub static KERNEL_PAGE_TABLES: InitGuard<RwSpinlock<hal_riscv::platform::PageTableImpl>> = InitGuard::uninit();

#[no_mangle]
pub extern "C" fn kentry(boot_info: &BootInfo) -> ! {
    /*
     * TODO: bringup on the D1 has made me realise this early-boot is not super tenable. We need a
     * system for early logging that happens before initialising all this stuff etc. The real UART
     * needs to then be hidden behind a trait and hooked up into a centralised `tracing` system.
     *
     * Options for early logging should also be customisable and include something set up by Seed
     * (e.g. a UEFI service, SBI, etc.) or a UART impl. Kernels we've seen such as FreeBSD take a
     * more pragmatic approach than us and literally hard-code addresses of the serial devices, for
     * example - we should accept that atm we literally are developing for a specific device in
     * the D1, for example.
     */

    let fdt = {
        let address = hal_riscv::platform::kernel_map::physical_to_virtual(boot_info.fdt_address.unwrap());
        unsafe { fdt::Fdt::from_ptr(address.ptr()).unwrap() }
    };
    serial::init(&fdt);
    info!("Hello from the kernel");

    trap::install_early_handler();

    if boot_info.magic != seed::boot_info::BOOT_INFO_MAGIC {
        panic!("Boot info has incorrect magic!");
    }

    // info!("Boot info: {:#?}", boot_info);
    // info!("FDT: {:#?}", fdt);

    clocksource::Clocksource::initialize(&fdt);

    /*
     * Initialise the heap allocator. After this, the kernel is free to use collections etc. that
     * can allocate on the heap through the global allocator.
     */
    info!("Initializing heap at {:#x} of size {} bytes", boot_info.heap_address, boot_info.heap_size);
    unsafe {
        kernel::ALLOCATOR.lock().init(boot_info.heap_address.mut_ptr(), boot_info.heap_size);
    }

    let kernel_page_table = unsafe {
        match Satp::read() {
            Satp::Sv39 { root, .. } => {
                assert!(hal_riscv::platform::VIRTUAL_ADDRESS_BITS == 39);
                PageTableImpl::from_frame(Frame::starts_with(root), kernel_map::PHYSICAL_MAP_BASE)
            }
            Satp::Sv48 { root, .. } => {
                assert!(hal_riscv::platform::VIRTUAL_ADDRESS_BITS == 48);
                PageTableImpl::from_frame(Frame::starts_with(root), kernel_map::PHYSICAL_MAP_BASE)
            }
            _ => {
                panic!("Kernel booted in an unexpected paging mode! Have we been built for the correct platform?");
            }
        }
    };
    KERNEL_PAGE_TABLES.initialize(RwSpinlock::new(kernel_page_table));

    kernel::PMM.initialize(Pmm::new(boot_info));
    kernel::VMM.initialize(Vmm::new(
        kernel_map::KERNEL_STACKS_BASE,
        kernel_map::KERNEL_STACKS_BASE + kernel_map::STACK_SLOT_SIZE * kernel_map::MAX_TASKS,
        kernel_map::STACK_SLOT_SIZE,
    ));

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

    // TODO: this is broken on the D1 because the device tree is being a cunt
    // let (uart_prod, uart_cons) = kernel::tasklets::queue::SpscQueue::new();
    // serial::enable_input(&fdt, uart_prod);
    // SCHEDULER.get().tasklet_scheduler.spawn(async move {
    //     loop {
    //         let line = {
    //             let mut line = String::new();
    //             loop {
    //                 let bytes = uart_cons.read().await;
    //                 let as_str = core::str::from_utf8(&bytes).unwrap();
    //                 if let Some(index) = as_str.find('\r') {
    //                     let (before, _after) = as_str.split_at(index);
    //                     line += before;
    //                     // Only release up to (and including) the newline so the next pass can consume any bytes
    //                     // after it
    //                     bytes.release(index + 1);
    //                     break;
    //                 } else {
    //                     line += as_str;
    //                     let num_bytes = bytes.len();
    //                     bytes.release(num_bytes);
    //                 }
    //             }
    //             line
    //         };
    //         info!("Line from UART: {}", line);
    //     }
    // });

    /*
     * Create kernel objects from loaded images and schedule them.
     */
    kernel::load_userspace(SCHEDULER.get(), &boot_info, &mut KERNEL_PAGE_TABLES.get().write());

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
