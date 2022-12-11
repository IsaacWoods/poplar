/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

#![no_std]
#![no_main]
#![feature(pointer_is_aligned, panic_info_message, const_mut_refs, strict_provenance)]

mod image;
mod logger;
mod memory;

use bit_field::BitField;
use core::arch::asm;
use fdt::Fdt;
use hal::memory::{Flags, FrameAllocator, FrameSize, PAddr, PageTable, Size4KiB, VAddr};
use hal_riscv::{hw::csr::Stvec, paging::PageTableImpl};
use memory::{MemoryManager, MemoryRegions, Region};
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

static MEMORY_MANAGER: MemoryManager = MemoryManager::new();

#[no_mangle]
pub fn seed_main(hart_id: u64, fdt_ptr: *const u8) -> ! {
    assert!(fdt_ptr.is_aligned_to(8));

    logger::init();
    info!("Hello, World!");
    info!("HART ID: {}", hart_id);
    info!("FDT address: {:?}", fdt_ptr);

    let fdt = unsafe { Fdt::from_ptr(fdt_ptr).expect("Failed to parse FDT") };
    // print_fdt(&fdt);

    let mut memory_regions = MemoryRegions::new();

    for region in fdt.memory().regions() {
        info!("Memory region: {:?}", region);
        memory_regions.add_region(Region::usable(
            PAddr::new(region.starting_address as usize).unwrap(),
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
            memory_regions.add_region(Region::reserved(
                usage,
                PAddr::new(reg.starting_address as usize).unwrap(),
                reg.size.unwrap(),
            ));
        }
    } else {
        info!("No memory reservations :(");
    }
    let seed_start = unsafe { _seed_start.ptr() as usize };
    let seed_end = unsafe { _seed_end.ptr() as usize };
    memory_regions.add_region(Region::reserved(
        memory::Usage::Seed,
        PAddr::new(unsafe { _seed_start.ptr() as usize }).unwrap(),
        align_up(seed_end - seed_start, Size4KiB::SIZE),
    ));
    memory_regions.add_region(Region::reserved(
        memory::Usage::DeviceTree,
        PAddr::new(fdt_ptr.addr()).unwrap(),
        align_up(fdt.total_size(), Size4KiB::SIZE),
    ));

    let kernel_elf = image::extract_kernel(&mut memory_regions);
    info!("Memory regions: {:#?}", memory_regions);
    MEMORY_MANAGER.init(memory_regions);
    MEMORY_MANAGER.walk_usable_memory();

    let mut kernel_page_table = PageTableImpl::new(MEMORY_MANAGER.allocate(), VAddr::new(0x0));
    let kernel = image::load_kernel(kernel_elf, &mut kernel_page_table, &MEMORY_MANAGER);

    /*
     * Construct the direct physical memory map.
     * TODO: we should probably do this properly by walking the FDT (you need RAM + devices) but we currently just
     * map 32GiB.
     */
    // This is the start of the 511th P4 entry - the very start of kernel space
    // TODO: put this constant somewhere better once we've laid everything out properly
    const PHYSICAL_MAP_BASE: VAddr = VAddr::new(0xffff_ff80_0000_0000);
    kernel_page_table
        .map_area(
            PHYSICAL_MAP_BASE,
            PAddr::new(0x0).unwrap(),
            hal::memory::gibibytes(32),
            Flags { writable: true, ..Default::default() },
            &MEMORY_MANAGER,
        )
        .unwrap();

    kernel_page_table.walk();
    Stvec::set(kernel.entry_point);

    MEMORY_MANAGER.walk_usable_memory();

    // Enter the kernel by trapping on a fetch (definitely safe)
    /*
     * Jump into the kernel by setting up the required state, and then moving page tables. Because we don't have
     * Seed's code mapped, this causes a page-fault
     */
    unsafe {
        asm!(
            "
                mv sp, {new_sp}

                csrw satp, {new_satp}
                sfence.vma
                unimp
            ",
            new_satp = in(reg) kernel_page_table.satp().raw(),
            new_sp = in(reg) usize::from(kernel.stack_top),
            options(nostack, noreturn)
        )
    }
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
