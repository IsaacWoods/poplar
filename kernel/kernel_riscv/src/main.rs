/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

#![no_std]
#![no_main]

use hal::memory::PAddr;
use hal_riscv::{hw::uart16550::Uart16550, kernel_map::physical_to_virtual};
use seed::boot_info::BootInfo;

#[no_mangle]
pub extern "C" fn kentry(boot_info: &BootInfo) -> ! {
    use core::fmt::Write;

    let uart: &mut Uart16550 = unsafe { &mut *physical_to_virtual(PAddr::new(0x1000_0000).unwrap()).mut_ptr() };
    writeln!(uart, "Hello from the kernel!").unwrap();

    writeln!(uart, "Boot info pointer: {:#x}", boot_info as *const _ as usize).unwrap();
    if boot_info.magic != seed::boot_info::BOOT_INFO_MAGIC {
        panic!("Boot info has incorrect magic!");
    }
    writeln!(uart, "Boot info: {:#?}", boot_info).unwrap();

    loop {}
}

#[panic_handler]
pub fn panic(_info: &core::panic::PanicInfo) -> ! {
    use core::fmt::Write;

    let uart: &mut Uart16550 = unsafe { &mut *physical_to_virtual(PAddr::new(0x1000_0000).unwrap()).mut_ptr() };
    write!(uart, "Panic :(").unwrap();
    loop {}
}
