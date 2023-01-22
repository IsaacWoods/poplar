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
use core::{arch::asm, mem, ptr};
use fdt::Fdt;
use hal::memory::{Flags, FrameAllocator, FrameSize, PAddr, PageTable, Size4KiB, VAddr};
use hal_riscv::{hw::csr::Stvec, paging::PageTableImpl};
use memory::{MemoryManager, MemoryRegions, Region};
use poplar_util::{linker::LinkerSymbol, math::align_up};
use seed::boot_info::BootInfo;
use tracing::{info, trace};

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
    let mut next_available_address_after_kernel = kernel.next_available_address;

    /*
     * Allocate memory for the boot info and start filling it out. We dynamically map the boot info into the
     * address space after the kernel.
     */
    use hal_riscv::kernel_map::PHYSICAL_MAP_BASE;
    let (boot_info_kernel_address, boot_info) = {
        let boot_info_physical_start =
            MEMORY_MANAGER.allocate_n(Size4KiB::frames_needed(mem::size_of::<BootInfo>())).start.start;
        let identity_boot_info_ptr = usize::from(boot_info_physical_start) as *mut BootInfo;
        unsafe {
            ptr::write(identity_boot_info_ptr, BootInfo::default());
        }

        let boot_info_kernel_address = next_available_address_after_kernel;
        next_available_address_after_kernel += align_up(mem::size_of::<BootInfo>(), Size4KiB::SIZE);

        kernel_page_table
            .map_area(
                boot_info_kernel_address,
                boot_info_physical_start,
                align_up(mem::size_of::<BootInfo>(), Size4KiB::SIZE),
                Flags::default(),
                &MEMORY_MANAGER,
            )
            .unwrap();

        (boot_info_kernel_address, unsafe { &mut *identity_boot_info_ptr })
    };
    boot_info.magic = seed::boot_info::BOOT_INFO_MAGIC;
    boot_info.fdt_address = Some(PAddr::new(fdt_ptr as usize).unwrap());

    /*
     * Construct the direct physical memory map.
     * TODO: we should probably do this properly by walking the FDT (you need RAM + devices) but we currently just
     * map 32GiB.
     */
    kernel_page_table
        .map_area(
            PHYSICAL_MAP_BASE,
            PAddr::new(0x0).unwrap(),
            hal::memory::gibibytes(32),
            Flags { writable: true, ..Default::default() },
            &MEMORY_MANAGER,
        )
        .unwrap();

    /*
     * Allocate the kernel heap and dynamically map it into the kernel address space.
     */
    const KERNEL_HEAP_SIZE: hal::memory::Bytes = hal::memory::kibibytes(800);
    boot_info.heap_address = next_available_address_after_kernel;
    next_available_address_after_kernel += KERNEL_HEAP_SIZE;
    let kernel_heap_physical_start =
        MEMORY_MANAGER.allocate_n(Size4KiB::frames_needed(KERNEL_HEAP_SIZE)).start.start;
    kernel_page_table
        .map_area(
            boot_info.heap_address,
            kernel_heap_physical_start,
            KERNEL_HEAP_SIZE,
            Flags { writable: true, ..Default::default() },
            &MEMORY_MANAGER,
        )
        .unwrap();

    /*
     * Now that we've finished allocating memory, we can create the memory map we pass to the kernel. From here, we
     * can't allocate physical memory from the bootloader.
     */
    MEMORY_MANAGER.populate_memory_map(&mut boot_info.memory_map);

    /*
     * Jump into the kernel by setting up the required state, and then moving to the new kernel page tables.
     * Because we don't have Seed's code mapped, this causes a page-fault, and so we trap. We set the trap handler
     * to the kernel's entry point, so this jumps us into the kernel.
     */
    Stvec::set(kernel.entry_point);
    unsafe {
        asm!(
            "
                mv sp, {new_sp}
                mv gp, {new_gp}

                csrw satp, {new_satp}
                sfence.vma
                unimp
            ",
            new_satp = in(reg) kernel_page_table.satp().raw(),
            new_sp = in(reg) usize::from(kernel.stack_top),
            new_gp = in(reg) usize::from(kernel.global_pointer),
            in("a0") usize::from(boot_info_kernel_address),
            options(nostack, noreturn)
        )
    }
}

#[allow(dead_code)]
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
