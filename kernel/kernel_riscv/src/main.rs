/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

#![no_std]
#![no_main]
#![feature(panic_info_message, const_mut_refs, const_option)]

extern crate alloc;

mod logger;

use alloc::vec::Vec;
use seed::boot_info::BootInfo;
use tracing::info;

#[no_mangle]
pub extern "C" fn kentry(boot_info: &BootInfo) -> ! {
    logger::init();
    info!("Hello from the kernel");

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

    let foo = alloc::vec![1, 2, 4, 5, 6, 7, 9, 14];
    for i in foo {
        info!("Thing in foo: {}", i);
    }

    loop {}
}
