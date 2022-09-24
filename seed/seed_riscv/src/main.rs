/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

#![no_std]
#![no_main]
#![feature(pointer_is_aligned, panic_info_message, const_mut_refs)]

mod logger;

use bit_field::BitField;
use fdt::Fdt;
use tracing::info;

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
        la t0, _bss_start
        la t1, _bss_end
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
    print_fdt(&fdt);

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

fn print_fdt(fdt: &Fdt) {
    use fdt::node::FdtNode;

    const INDENT_PER_DEPTH: usize = 4;

    fn print_node(node: &FdtNode, mut depth: usize) {
        info!("{:indent$}{} {{", "", node.name, indent = depth * INDENT_PER_DEPTH);
        depth += 1;

        for prop in node.properties() {
            match prop.name {
                "stdout-path" | "riscv,isa" | "status" | "mmu-type" | "model" | "device_type" => {
                    info!(
                        "{:indent$}{} = {}",
                        "",
                        prop.name,
                        prop.as_str().unwrap(),
                        indent = depth * INDENT_PER_DEPTH
                    );
                }
                "compatible" => {
                    info!("{:indent$}{} = [", "", prop.name, indent = depth * INDENT_PER_DEPTH);
                    for compatible in node.compatible().unwrap().all() {
                        info!("{:indent$}{:?}", "", compatible, indent = (depth + 1) * INDENT_PER_DEPTH);
                    }
                    info!("{:indent$}]", "", indent = depth * INDENT_PER_DEPTH);
                }
                "interrupt-map" if node.compatible().unwrap().all().any(|c| c == "pci-host-ecam-generic") => {
                    let mut chunks = prop.value.chunks_exact(4).map(|c| u32::from_be_bytes(c.try_into().unwrap()));
                    info!("{:indent$}{} = [", "", prop.name, indent = depth * INDENT_PER_DEPTH);
                    while let Some(entry) = chunks.next() {
                        let _ = chunks.next().unwrap();
                        let _ = chunks.next().unwrap();
                        let intn = chunks.next().unwrap();
                        let ctrl = chunks.next().unwrap();
                        let cintr = chunks.next().unwrap();

                        let bus = entry.get_bits(16..24);
                        let device = entry.get_bits(11..16);
                        let function = entry.get_bits(8..11);

                        info!(
                            "{:indent$}  {bus:02x}:{device:02x}:{function:02x} INT{} on controller {ctrl:#x}, vector {cintr}",
                            "",
                            (b'A' - 1 + intn as u8) as char,
                            indent = depth * INDENT_PER_DEPTH
                        );
                    }
                }
                _ => {
                    info!("{:indent$}{} = [", "", prop.name, indent = depth * INDENT_PER_DEPTH);
                    let mut first = true;
                    prop.value.chunks_exact(4).for_each(|c| {
                        first = false;
                        info!(
                            "{:indent$}{:#010x}",
                            "",
                            u32::from_be_bytes(<[u8; 4]>::try_from(c).unwrap()),
                            indent = (depth + 1) * INDENT_PER_DEPTH
                        );
                    });
                    info!("{:indent$}]", "", indent = depth * INDENT_PER_DEPTH);
                }
            }
        }

        for node in node.children() {
            print_node(&node, depth);
        }

        depth -= 1;
        info!("{:indent$}}};", "", indent = depth * INDENT_PER_DEPTH);
    }

    let root = fdt.all_nodes().next().unwrap();
    print_node(&root, 0);
}
