/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

#![no_std]
#![no_main]

use seed::boot_info::BootInfo;

#[no_mangle]
pub extern "C" fn kentry(boot_info: &BootInfo) -> ! {
    use core::fmt::Write;

    let uart =
        unsafe { &mut *((0xffff_ff80_0000_0000usize + 0x1000_0000) as *mut hal_riscv::hw::uart16550::Uart16550) };
    writeln!(uart, "Hello from the kernel!").unwrap();

    writeln!(uart, "Boot info pointer: {:#x}", boot_info as *const _ as usize).unwrap();
    if boot_info.magic != seed::boot_info::BOOT_INFO_MAGIC {
        panic!("Boot info has incorrect magic!");
    }
    writeln!(uart, "Boot info: {:?}", boot_info).unwrap();

    loop {}
}

#[panic_handler]
pub fn panic(_info: &core::panic::PanicInfo) -> ! {
    use core::fmt::Write;

    let uart =
        unsafe { &mut *((0xffff_ff80_0000_0000usize + 0x1000_0000) as *mut hal_riscv::hw::uart16550::Uart16550) };
    write!(uart, "Panic :(").unwrap();
    loop {}
}
