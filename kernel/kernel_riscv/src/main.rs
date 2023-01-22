/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

#![no_std]
#![no_main]
#![feature(panic_info_message, const_mut_refs, const_option)]

mod logger;

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

    loop {}
}
