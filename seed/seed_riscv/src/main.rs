/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

#![no_std]
#![no_main]
#![feature(pointer_is_aligned, panic_info_message, const_mut_refs, strict_provenance)]

mod logger;
mod memory;

use bit_field::BitField;
use fdt::Fdt;
use hal::memory::{FrameSize, PhysicalAddress, Size4KiB};
use memory::{MemoryManager, Region};
use mer::Elf;
use poplar_util::{linker::LinkerSymbol, math::align_up};
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

extern "C" {
    static _seed_start: LinkerSymbol;
    static _bss_start: LinkerSymbol;
    static _stack_bottom: LinkerSymbol;
    static _stack_top: LinkerSymbol;
    static _bss_end: LinkerSymbol;
    static _seed_end: LinkerSymbol;
}

#[no_mangle]
pub fn seed_main(hart_id: u64, fdt_ptr: *const u8) -> ! {
    assert!(fdt_ptr.is_aligned_to(8));

    logger::init();
    info!("Hello, World!");
    info!("HART ID: {}", hart_id);
    info!("FDT address: {:?}", fdt_ptr);

    let fdt = unsafe { Fdt::from_ptr(fdt_ptr).expect("Failed to parse FDT") };
    // print_fdt(&fdt);

    let mut memory_manager = MemoryManager::new();

    for region in fdt.memory().regions() {
        info!("Memory region: {:?}", region);
        memory_manager.add_region(Region::usable(
            PhysicalAddress::new(region.starting_address as usize).unwrap(),
            region.size.unwrap(),
        ));
    }
    if let Some(reservations) = fdt.find_node("/reserved-memory") {
        for reservation in reservations.children() {
            let reg = reservation.reg().unwrap().next().unwrap();
            info!("Memory reservation with name {}. Reg = {:?}", reservation.name, reg);
            let usage = if reservation.name.starts_with("mmode_resv") {
                memory::Usage::Firmware
            } else {
                memory::Usage::Unknown
            };
            memory_manager.add_region(Region::reserved(
                usage,
                PhysicalAddress::new(reg.starting_address as usize).unwrap(),
                reg.size.unwrap(),
            ));
        }
    } else {
        info!("No memory reservations :(");
    }
    let seed_start = unsafe { _seed_start.ptr() as usize };
    let seed_end = unsafe { _seed_end.ptr() as usize };
    memory_manager.add_region(Region::reserved(
        memory::Usage::Seed,
        PhysicalAddress::new(unsafe { _seed_start.ptr() as usize }).unwrap(),
        align_up(seed_end - seed_start, Size4KiB::SIZE),
    ));
    memory_manager.add_region(Region::reserved(
        memory::Usage::DeviceTree,
        PhysicalAddress::new(fdt_ptr.addr()).unwrap(),
        align_up(fdt.total_size(), Size4KiB::SIZE),
    ));

    let kernel_elf = extract_kernel(&mut memory_manager);
    memory_manager.init_usable_regions();

    for section in kernel_elf.sections() {
        info!("Section: called {:?} at {:#x}", section.name(&kernel_elf), section.address);
    }

    info!("Memory regions: {:#?}", memory_manager);
    memory_manager.walk_usable_memory();
    info!("Looping");
    loop {}
}

fn extract_kernel(memory_manager: &mut MemoryManager) -> Elf<'static> {
    const LOADER_DEVICE_BASE: usize = 0xb000_0000;

    // TODO: loader devices are not added to the FDT - this is kind of gross so maybe use fw_cfg or something else instead?
    let kernel_elf_size = unsafe { *(LOADER_DEVICE_BASE as *const u32) } as usize;
    info!("Kernel elf size: {}", kernel_elf_size);

    // Reserve the kernel ELF in the memory manager, so we don't trample over it
    memory_manager.add_region(Region::reserved(
        memory::Usage::KernelImage,
        PhysicalAddress::new(LOADER_DEVICE_BASE).unwrap(),
        align_up(kernel_elf_size + 4, Size4KiB::SIZE),
    ));

    assert_eq!(
        unsafe { &*((LOADER_DEVICE_BASE + 4) as *const [u8; 4]) },
        b"\x7fELF",
        "Kernel ELF magic isn't correct"
    );
    Elf::new(unsafe { core::slice::from_raw_parts((LOADER_DEVICE_BASE + 4) as *const u8, kernel_elf_size) })
        .expect("Failed to read kernel ELF :(")
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
