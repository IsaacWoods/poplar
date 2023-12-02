/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

#![no_std]
#![no_main]
#![feature(panic_info_message, const_mut_refs, const_option, fn_align, naked_functions)]

extern crate alloc;

mod logger;
mod task;

use hal::memory::{Frame, VAddr};
use hal_riscv::{
    hw::csr::{Satp, Stvec},
    platform::{kernel_map, PageTableImpl},
};
use kernel::{
    memory::{KernelStackAllocator, PhysicalMemoryManager},
    scheduler::Scheduler,
    Platform,
};
use poplar_util::InitGuard;
use seed::boot_info::BootInfo;
use tracing::info;

pub struct PlatformImpl {
    kernel_page_table: <Self as Platform>::PageTable,
}

impl Platform for PlatformImpl {
    type PageTableSize = hal::memory::Size4KiB;
    type PageTable = hal_riscv::platform::PageTableImpl;

    fn kernel_page_table(&mut self) -> &mut Self::PageTable {
        &mut self.kernel_page_table
    }

    unsafe fn initialize_task_stacks(
        kernel_stack: &kernel::memory::Stack,
        user_stack: &kernel::memory::Stack,
        task_entry_point: VAddr,
    ) -> (VAddr, VAddr) {
        task::initialize_stacks(kernel_stack, user_stack, task_entry_point)
    }

    unsafe fn switch_user_stack_pointer(new_user_stack_pointer: VAddr) -> VAddr {
        todo!()
    }

    unsafe fn context_switch(current_kernel_stack: *mut VAddr, new_kernel_stack: VAddr) {
        task::context_switch(current_kernel_stack, new_kernel_stack)
    }

    unsafe fn drop_into_userspace(kernel_stack_pointer: VAddr, user_stack_pointer: VAddr) -> ! {
        task::drop_into_userspace(kernel_stack_pointer)
    }
}

pub static SCHEDULER: InitGuard<Scheduler<PlatformImpl>> = InitGuard::uninit();

#[no_mangle]
pub extern "C" fn kentry(boot_info: &BootInfo) -> ! {
    let fdt = {
        let address = hal_riscv::platform::kernel_map::physical_to_virtual(boot_info.fdt_address.unwrap());
        unsafe { fdt::Fdt::from_ptr(address.ptr()).unwrap() }
    };
    logger::init(&fdt);
    info!("Hello from the kernel");

    Stvec::set(VAddr::new(trap_handler as extern "C" fn() as usize));

    if boot_info.magic != seed::boot_info::BOOT_INFO_MAGIC {
        panic!("Boot info has incorrect magic!");
    }
    info!("Boot info: {:#?}", boot_info);

    /*
     * Initialise the heap allocator. After this, the kernel is free to use collections etc. that
     * can allocate on the heap through the global allocator.
     */
    info!("Initializing heap at {:#x} of size {} bytes", boot_info.heap_address, boot_info.heap_size);
    unsafe {
        kernel::ALLOCATOR.lock().init(boot_info.heap_address, boot_info.heap_size);
    }

    kernel::PHYSICAL_MEMORY_MANAGER.initialize(PhysicalMemoryManager::new(boot_info));

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

    let mut platform = PlatformImpl { kernel_page_table };

    let mut kernel_stack_allocator = KernelStackAllocator::<PlatformImpl>::new(
        kernel_map::KERNEL_STACKS_BASE,
        kernel_map::KERNEL_STACKS_BASE + kernel_map::STACK_SLOT_SIZE * kernel_map::MAX_TASKS,
        kernel_map::STACK_SLOT_SIZE,
    );

    SCHEDULER.initialize(Scheduler::new());

    /*
     * Create kernel objects from loaded images and schedule them.
     */
    info!("Loading {} initial tasks to the ready queue", boot_info.loaded_images.len());
    for image in &boot_info.loaded_images {
        kernel::load_task(
            SCHEDULER.get(),
            image,
            platform.kernel_page_table(),
            &kernel::PHYSICAL_MEMORY_MANAGER.get(),
            &mut kernel_stack_allocator,
        );
    }

#[repr(align(4))]
pub extern "C" fn trap_handler() {
    use hal_riscv::hw::csr::{Scause, Sepc};
    let scause = Scause::read();
    let sepc = Sepc::read();
    panic!("Trap! Scause = {:?}, sepc = {:?}", scause, sepc);
    /*
     * Drop into userspace!
     */
    info!("Dropping into usermode");
    SCHEDULER.get().drop_to_userspace()
}
