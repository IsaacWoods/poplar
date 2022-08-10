// SPDX-License-Identifier: MPL-2.0
// Copyright 2022, Isaac Woods

#![no_std]
#![no_main]
#![feature(pointer_is_aligned, panic_info_message, const_mut_refs)]

mod logger;

use fdt::Fdt;
use logger::Logger;
use tracing::info;

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

    Logger::init();
    info!("Hello, World!");
    let uart = unsafe { &mut *(0x10000000 as *mut hal_riscv::hw::uart16550::Uart16550) };
    use core::fmt::Write;
    writeln!(uart, "Hello, World!").unwrap();
    writeln!(uart, "HART ID: {}", hart_id).unwrap();
    writeln!(uart, "FDT address: {:?}", fdt).unwrap();

    let fdt = unsafe { Fdt::from_ptr(fdt).expect("Failed to parse FDT") };
    for region in fdt.memory().regions() {
        writeln!(uart, "Memory region: {:?}", region).unwrap();
    }
    // for reservation in fdt.memory_reservations() {
    //     writeln!(uart, "Memory reservation: {:?}", reservation).unwrap();
    // }
    if let Some(reservations) = fdt.find_node("/reserved-memory") {
        for child in reservations.children() {
            writeln!(
                uart,
                "Memory reservation with name {}. Reg = {:?}",
                child.name,
                child.reg().unwrap().next().unwrap()
            )
            .unwrap();
        }
    } else {
        writeln!(uart, "No memory reservations :(").unwrap();
    }

    writeln!(uart, "Looping :)").unwrap();
    loop {}
}

#[panic_handler]
pub fn panic(info: &core::panic::PanicInfo) -> ! {
    let uart = unsafe { &mut *(0x10000000 as *mut hal_riscv::hw::uart16550::Uart16550) };
    use core::fmt::Write;

    if let Some(message) = info.message() {
        if let Some(location) = info.location() {
            let _ = writeln!(
                uart,
                "Panic message: {} ({} - {}:{})",
                message,
                location.file(),
                location.line(),
                location.column()
            );
        } else {
            let _ = writeln!(uart, "Panic message: {} (no location info)", message);
        }
    }
    loop {}
}
