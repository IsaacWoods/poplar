// SPDX-License-Identifier: MPL-2.0
// Copyright 2022, Isaac Woods

#![no_std]
#![no_main]
#![feature(pointer_is_aligned, panic_info_message, const_mut_refs)]

mod logger;

use fdt::Fdt;
use log::info;

/*
 * This is the entry-point jumped to from OpenSBI. It needs to be at the very start of the ELF, so we put it in its
 * own section and then place it manually during linking. On entry, `a0` contains the current HART's ID, and `a1`
 * contains the address of the FDT - these match up with the ABI so we can pass these straight as parameters to
 * `kmain`.
 */
core::arch::global_asm!(
    "
    .section .text.start
    .global _start
    _start:
        // Zero the BSS
        la t0, __bss_start
        la t1, __bss_end
        bgeu t0, t1, .bss_zero_loop_end
    .bss_zero_loop:
        sd zero, (t0)
        addi t0, t0, 8
        bltu t0, t1, .bss_zero_loop
    .bss_zero_loop_end:

        la sp, _stack_top

        jal seed_main
        unimp
"
);

#[no_mangle]
pub fn seed_main(hart_id: u64, fdt: *const u8) -> ! {
    assert!(fdt.is_aligned_to(8));

    logger::init();
    info!("Hello, World!");
    info!("HART ID: {}", hart_id);
    info!("FDT address: {:?}", fdt);

    let fdt = unsafe { Fdt::from_ptr(fdt).expect("Failed to parse FDT") };
    for region in fdt.memory().regions() {
        info!("Memory region: {:?}", region);
    }
    if let Some(reservations) = fdt.find_node("/reserved-memory") {
        for child in reservations.children() {
            info!("Memory reservation with name {}. Reg = {:?}", child.name, child.reg().unwrap().next().unwrap());
        }
    } else {
        info!("No memory reservations :(");
    }

    info!("Looping");
    loop {}
}
