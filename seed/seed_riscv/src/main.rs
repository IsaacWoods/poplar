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
use hal_riscv::paging::PageTableImpl;
use memory::{MemoryManager, MemoryRegions, Region};
use poplar_util::{linker::LinkerSymbol, math::align_up};
use seed::boot_info::BootInfo;
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
    boot_info.heap_size = KERNEL_HEAP_SIZE;
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
     * Identity-map all of Seed into the kernel's page tables, so we don't page fault when switching to them.
     * TODO: this could maybe be reduced to just a tiny trampoline, maybe with linker symbols plus a custom section
     * so we don't have to map as much, or removed entirely with the trick we talk about below.
     */
    let seed_size = align_up(unsafe { _seed_end.ptr() as usize - _seed_start.ptr() as usize }, Size4KiB::SIZE);
    info!(
        "Mapping seed: {:#x} to {:#x} ({} bytes)",
        PAddr::new(unsafe { _seed_start.ptr() as usize }).unwrap(),
        PAddr::new(unsafe { _seed_end.ptr() as usize }).unwrap(),
        seed_size,
    );
    kernel_page_table
        .map_area(
            VAddr::new(unsafe { _seed_start.ptr() as usize }),
            PAddr::new(unsafe { _seed_start.ptr() as usize }).unwrap(),
            seed_size,
            Flags { writable: false, executable: true, ..Default::default() },
            &MEMORY_MANAGER,
        )
        .unwrap();

    /*
     * Now that we've finished allocating memory, we can create the memory map we pass to the kernel. From here, we
     * can't allocate physical memory from the bootloader.
     */
    MEMORY_MANAGER.populate_memory_map(&mut boot_info.memory_map);

    /*
     * Jump into the kernel by setting the required state, moving to the new kernel page table, and then jumping to
     * the kernel's entry point.
     * TODO: before, we were trying to do this using a trick where we set the trap handler to the entry point, and
     * then page fault to bounce into the kernel, but this wasn't working for unidentified reasons. Try again?
     */
    info!("Jumping into the kernel!");
    unsafe {
        asm!(
            "
                mv sp, {new_sp}
                mv gp, {new_gp}

                csrw satp, {new_satp}
                sfence.vma
                jr {entry_point}
            ",
            new_satp = in(reg) kernel_page_table.satp().raw(),
            new_sp = in(reg) usize::from(kernel.stack_top),
            new_gp = in(reg) usize::from(kernel.global_pointer),
            entry_point = in(reg) usize::from(kernel.entry_point),
            in("a0") usize::from(boot_info_kernel_address),
            options(nostack, noreturn)
        )
    }
}
