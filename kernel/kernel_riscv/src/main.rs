/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

#![no_std]
#![no_main]
#![feature(panic_info_message, const_mut_refs, const_option, fn_align)]

extern crate alloc;

mod logger;

use core::arch::asm;
use hal::memory::VAddr;
use hal_riscv::hw::csr::Stvec;
use seed::boot_info::BootInfo;
use tracing::info;

#[no_mangle]
pub extern "C" fn kentry(boot_info: &BootInfo) -> ! {
    logger::init();
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

    unsafe {
        asm!(".word 0");
    }
    loop {}
}

#[repr(align(4))]
pub extern "C" fn trap_handler() {
    use hal_riscv::hw::csr::{Scause, Sepc};
    let scause = Scause::read();
    let sepc = Sepc::read();
    panic!("Trap! Scause = {:?}, sepc = {:?}", scause, sepc);
}
