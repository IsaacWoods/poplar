/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

#![no_std]
#![no_main]
#![feature(pointer_is_aligned, panic_info_message, const_mut_refs, strict_provenance, fn_align)]

extern crate alloc;

mod block;
mod fs;
mod image;
mod logger;
mod memory;
mod pci;

use crate::{
    fs::{ramdisk::Ramdisk, Filesystem},
    memory::Region,
};
use core::{arch::asm, mem, ptr};
use fdt::Fdt;
use hal::memory::{Flags, FrameAllocator, FrameSize, PAddr, PageTable, Size4KiB, VAddr};
use hal_riscv::{hw::csr::Stvec, platform::PageTableImpl};
use linked_list_allocator::LockedHeap;
use memory::{MemoryManager, MemoryRegions};
use pci::PciAccess;
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

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

#[no_mangle]
pub fn seed_main(hart_id: u64, fdt_ptr: *const u8) -> ! {
    assert!(fdt_ptr.is_aligned_to(8));
    /*
     * We extract the address of the device tree before we do anything with it. Once we've used the pointer, we
     * shouldn't turn it back into an address afaiu due to strict provenance.
     */
    let fdt_address = PAddr::new(fdt_ptr.addr()).unwrap();
    let fdt = unsafe { Fdt::from_ptr(fdt_ptr).expect("Failed to parse FDT") };

    logger::init(&fdt);
    info!("Hello, World!");
    info!("HART ID: {}", hart_id);
    info!("FDT address: {:?}", fdt_ptr);

    Stvec::set(VAddr::new(trap_handler as extern "C" fn() as usize));

    /*
     * Construct an initial map of memory - a series of usable and reserved regions, and what is in
     * each of them.
     */
    let mut memory_regions = MemoryRegions::new(&fdt, fdt_address);

    /*
     * Find the loaded ramdisk (if there is one) and mark it as a reserved region before we
     * initialize the physical memory manager (it is not otherwise described as a not-usable region).
     */
    let mut ramdisk = unsafe { Ramdisk::new(usize::from(hal_riscv::platform::memory::RAMDISK_ADDR)) };
    if let Some(ref ramdisk) = ramdisk {
        let (address, size) = ramdisk.memory_region();
        memory_regions.add_region(Region::reserved(
            memory::Usage::Ramdisk,
            address,
            align_up(size, Size4KiB::SIZE),
        ));
    }

    /*
     * We can then use this mapping of memory regions to initialise the physical memory manager so we can allocate
     * out of the usable regions.
     */
    info!("Memory regions: {:#?}", memory_regions);
    MEMORY_MANAGER.init(memory_regions);
    MEMORY_MANAGER.walk_usable_memory();

    /*
     * Allocate memory for and initialize Seed's heap.
     */
    const HEAP_SIZE: usize = hal::memory::kibibytes(200);
    let heap_memory = MEMORY_MANAGER.allocate_n(Size4KiB::frames_needed(HEAP_SIZE));
    unsafe {
        ALLOCATOR.lock().init(usize::from(heap_memory.start.start) as *mut u8, HEAP_SIZE);
    }

    let mut kernel_page_table = PageTableImpl::new(MEMORY_MANAGER.allocate(), VAddr::new(0x0));
    let kernel_file = if let Some(ref mut ramdisk) = ramdisk {
        ramdisk.load("kernel").unwrap()
    } else {
        panic!("No kernel source is present!");
    };
    let kernel = image::load_kernel(&kernel_file, &mut kernel_page_table, &MEMORY_MANAGER);
    let mut next_available_kernel_address = kernel.next_available_address;

    /*
     * Enumerate PCI devices.
     */
    let pci_access = match PciAccess::new(&fdt) {
        Some(access) => Some(pci::PciResolver::resolve(access)),
        None => None,
    };

    /*
     * Find the initialize a Virtio block device if one is present.
     */
    // let mut virtio_block = VirtioBlockDevice::init(&fdt, &MEMORY_MANAGER);
    // if let Some(mut device) = virtio_block {
    //     use block::BlockDevice;
    //
    //     let gpt_header = unsafe { device.read(1).data.cast::<gpt::GptHeader>().as_ref() };
    //     gpt_header.validate().unwrap();
    //     info!("GPT header: {:#?}", gpt_header);
    //
    //     // TODO: at some point we should iterate all parition entries properly
    //     // (including reading multiple sectors if needed)
    //     let partition_table =
    //         unsafe { device.read(gpt_header.partition_entry_lba).data.cast::<gpt::PartitionEntry>().as_ref() };
    //     info!("First partition entry: {:#?}", partition_table);
    //     assert_eq!(partition_table.partition_type_guid, gpt::Guid::EFI_SYSTEM_PARTITION);
    // }

    /*
     * Allocate memory for the boot info and start filling it out.
     */
    let (boot_info_kernel_address, boot_info) =
        create_boot_info(&mut next_available_kernel_address, &mut kernel_page_table);
    boot_info.magic = seed::boot_info::BOOT_INFO_MAGIC;
    boot_info.fdt_address = Some(PAddr::new(fdt_ptr as usize).unwrap());

    /*
     * Load desired early tasks.
     */
    // TODO: load from config file
    for name in &["hello_world"] {
        let file = if let Some(ref mut ramdisk) = ramdisk {
            ramdisk.load(&name).unwrap()
        } else {
            panic!("No user tasks source is present!");
        };
        let info = image::load_image(&file, name, &MEMORY_MANAGER);
        boot_info.loaded_images.push(info).unwrap();
    }

    /*
     * Construct the direct physical memory map.
     * TODO: we should probably do this properly by walking the FDT (you need RAM + devices) but we currently just
     * map 32GiB.
     */
    use hal_riscv::platform::kernel_map::PHYSICAL_MAP_BASE;
    const PHYSICAL_MAP_SIZE: usize = hal::memory::gibibytes(16);
    kernel_page_table
        .map_area(
            PHYSICAL_MAP_BASE,
            PAddr::new(0x0).unwrap(),
            PHYSICAL_MAP_SIZE,
            Flags { writable: true, ..Default::default() },
            &MEMORY_MANAGER,
        )
        .unwrap();

    alloc_and_map_kernel_heap(&mut next_available_kernel_address, &mut kernel_page_table, boot_info);

    /*
     * Identity-map all of Seed into the kernel's page tables, so we don't page fault when switching to them.
     * TODO: this could maybe be reduced to just a tiny trampoline, maybe with linker symbols plus a custom section
     * so we don't have to map as much, or removed entirely with the trick we talk about below.
     */
    let seed_size = align_up(unsafe { _seed_end.ptr() as usize - _seed_start.ptr() as usize }, Size4KiB::SIZE);
    info!(
        "Mapping Seed: {:#x} to {:#x} ({} bytes)",
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

/// Allocate memory for the boot info, and dynamically map it into the address space after the kernel.
fn create_boot_info<'a>(
    next_available_kernel_address: &mut VAddr,
    kernel_page_table: &mut PageTableImpl,
) -> (VAddr, &'a mut BootInfo) {
    let boot_info_physical_start =
        MEMORY_MANAGER.allocate_n(Size4KiB::frames_needed(mem::size_of::<BootInfo>())).start.start;
    let identity_boot_info_ptr = usize::from(boot_info_physical_start) as *mut BootInfo;
    unsafe {
        ptr::write(identity_boot_info_ptr, BootInfo::default());
    }

    let boot_info_kernel_address = *next_available_kernel_address;
    *next_available_kernel_address += align_up(mem::size_of::<BootInfo>(), Size4KiB::SIZE);

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
}

/// Allocate memory for the kernel heap, and dynamically map it into the address space after the kernel. We tell
/// the kernel where to find it via the boot info.
fn alloc_and_map_kernel_heap(
    next_available_kernel_address: &mut VAddr,
    kernel_page_table: &mut PageTableImpl,
    boot_info: &mut BootInfo,
) {
    const KERNEL_HEAP_SIZE: hal::memory::Bytes = hal::memory::kibibytes(800);

    boot_info.heap_address = *next_available_kernel_address;
    boot_info.heap_size = KERNEL_HEAP_SIZE;
    *next_available_kernel_address += KERNEL_HEAP_SIZE;

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
}

#[repr(align(4))]
pub extern "C" fn trap_handler() {
    use hal_riscv::hw::csr::{Scause, Sepc};
    let scause = Scause::read();
    let sepc = Sepc::read();
    panic!("Trap! Scause = {:?}, sepc = {:?}", scause, sepc);
}
