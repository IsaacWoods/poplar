/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

#![no_std]
#![no_main]

#[no_mangle]
pub extern "C" fn kentry() -> ! {
    use core::fmt::Write;

    let uart =
        unsafe { &mut *((0xffff_ff80_0000_0000usize + 0x1000_0000) as *mut hal_riscv::hw::uart16550::Uart16550) };
    writeln!(uart, "Hello from the kernel!").unwrap();
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
