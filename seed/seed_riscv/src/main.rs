// SPDX-License-Identifier: MPL-2.0
// Copyright 2022, Isaac Woods

#![no_std]
#![no_main]
#![feature(pointer_is_aligned)]

use fdt::Fdt;

/*
 * This is the entry-point jumped to from OpenSBI. It needs to be at the very start of the ELF, so we put it in its
 * own section and then place it manually during linking. On entry, `a0` contains the current HART's ID, and `a1`
 * contains the address of the FDT - these match up with the ABI so we can pass these straight as parameters to
 * `kmain`.
 */
core::arch::global_asm!(
    "
    .section .text.entry
    .global _start
    _start:
        la sp, _stack_top
        mv fp, sp

        j seed_main
"
);

#[no_mangle]
pub fn seed_main(hart_id: usize, fdt: *const u8) -> ! {
    assert!(fdt.is_aligned_to(8));

    let uart = unsafe { &mut *(0x10000000 as *mut hal_riscv::hw::uart16550::Uart16550) };
    use core::fmt::Write;
    writeln!(uart, "Hello, World!").unwrap();
    writeln!(uart, "HART ID: {}", hart_id).unwrap();
    writeln!(uart, "FDT address: {:?}", fdt).unwrap();

    let fdt = unsafe { Fdt::from_ptr(fdt).expect("Failed to parse FDT") };
    for cpu in fdt.cpus() {
        writeln!(uart, "CPU: {:?}", cpu).unwrap();
    }
    writeln!(uart, "Memory: {:?}", fdt.memory()).unwrap();
    for reservation in fdt.memory_reservations() {
        writeln!(uart, "Memory reservation: {:?}", reservation).unwrap();
    }

    loop {}
}

#[panic_handler]
pub fn panic(_info: &core::panic::PanicInfo) -> ! {
    let uart = unsafe { &mut *(0x10000000 as *mut hal_riscv::hw::uart16550::Uart16550) };
    use core::fmt::Write;
    write!(uart, "Panic :(").unwrap();
    loop {}
}
