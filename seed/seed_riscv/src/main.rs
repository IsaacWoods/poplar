/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

#![no_std]
#![no_main]
#![feature(pointer_is_aligned_to, fn_align)]

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
use alloc::{string::ToString, vec::Vec};
use core::{arch::asm, mem, ptr};
use fdt::Fdt;
use hal::memory::{Flags, FrameAllocator, FrameSize, PAddr, PageTable, Size4KiB, VAddr};
use hal_riscv::hw::csr::Stvec;
use linked_list_allocator::LockedHeap;
use memory::{MemoryManager, MemoryRegions};
use mulch::{linker::LinkerSymbol, math::align_up};
use pci::PciResolver;
use seed_config::SeedConfig;
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

#[cfg(feature = "platform_rv64_virt")]
pub type PageTableImpl = hal_riscv::paging::PageTableImpl<hal_riscv::paging::Level4>;
#[cfg(feature = "platform_mq_pro")]
pub type PageTableImpl = hal_riscv::paging::PageTableImpl<hal_riscv::paging::Level3>;

/// This module contains constants that define how the kernel address space is laid out on RISC-V
/// using the Sv48 paging model. It closely resembles the layout used by x86_64.
///
/// The higher-half starts at `0xffff_8000_0000_0000`. We dedicate the first half of the
/// higher-half (64 TiB) to the direct physical map. Following this is an area the kernel can use
/// for dynamic virtual allocations (starting at `0xffff_c000_0000_0000`).
///
/// The actual kernel image is loaded at `-2GiB` (`0xffff_ffff_8000_0000`), and is followed by boot
/// information constructed by Seed. This allows best utilisation of the `kernel` code model, which
/// optimises for encoding offsets in signed 32-bit immediates, which are common in x86_64 instruction
/// encodings.
#[cfg(feature = "platform_rv64_virt")]
pub mod kernel_map {
    use hal::memory::{PAddr, VAddr};

    pub const DRAM_START: PAddr = PAddr::new(0x8000_0000).unwrap();
    pub const OPENSBI_ADDR: PAddr = DRAM_START;
    // TODO: when const traits are implemented, this should be rewritten in terms of DRAM_START
    pub const SEED_ADDR: PAddr = PAddr::new(0x8020_0000).unwrap();
    pub const RAMDISK_ADDR: PAddr = PAddr::new(0xb000_0000).unwrap();

    pub const HIGHER_HALF_START: VAddr = VAddr::new(0xffff_8000_0000_0000);
    pub const PHYSICAL_MAPPING_BASE: VAddr = HIGHER_HALF_START;
    pub const KERNEL_DYNAMIC_AREA_BASE: VAddr = VAddr::new(0xffff_c000_0000_0000);
    pub const KERNEL_IMAGE_BASE: VAddr = VAddr::new(0xffff_ffff_8000_0000);
}

/// On platforms that only support Sv39, we follow a similar layout, but obviously with a smaller
/// higher-half.
///
/// The higher-half and therefore physical mapping starts at `0xffff_ffc0_0000_0000`. We dedicate
/// the first half of the higher-half to the physical mapping, and the dynamic kernel area
/// therefore starts at `0xffff_ffe0_0000_0000`.
///
/// The kernel image is again loaded at `-2GiB`, so is at `0xffff_ffff_8000_0000`.
#[cfg(feature = "platform_mq_pro")]
pub mod kernel_map {
    use hal::memory::{kibibytes, mebibytes, PAddr, VAddr};

    pub const DRAM_START: PAddr = PAddr::new(0x4000_0000).unwrap();
    pub const OPENSBI_ADDR: PAddr = DRAM_START;
    // TODO: when const traits are implemented, this should be rewritten in terms of DRAM_START
    pub const SEED_ADDR: PAddr = PAddr::new(0x4000_0000 + kibibytes(512)).unwrap();
    pub const RAMDISK_ADDR: PAddr = PAddr::new(0x4000_0000 + mebibytes(1)).unwrap();

    pub const HIGHER_HALF_START: VAddr = VAddr::new(0xffff_ffc0_0000_0000);
    pub const PHYSICAL_MAPPING_BASE: VAddr = HIGHER_HALF_START;
    pub const KERNEL_DYNAMIC_AREA_BASE: VAddr = VAddr::new(0xffff_ffe0_0000_0000);
    pub const KERNEL_IMAGE_BASE: VAddr = VAddr::new(0xffff_ffff_8000_0000);
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
    info!("Made memory regions");

    /*
     * Find the loaded ramdisk (if there is one) and mark it as a reserved region before we
     * initialize the physical memory manager (it is not otherwise described as a not-usable region).
     */
    let mut ramdisk = unsafe { Ramdisk::new(usize::from(kernel_map::RAMDISK_ADDR)) };
    if let Some(ref ramdisk) = ramdisk {
        let (address, size) = ramdisk.memory_region();
        memory_regions.add_region(Region::reserved(
            memory::Usage::Ramdisk,
            address,
            align_up(size, Size4KiB::SIZE),
        ));
    }
    info!("Found ramdisk");

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

    let config = if let Some(ref mut ramdisk) = ramdisk {
        let config = ramdisk.load("config").expect("No config file found!");
        picotoml::from_str::<SeedConfig>(core::str::from_utf8(config.data).unwrap()).unwrap()
    } else {
        panic!("No config file found!");
    };
    info!("Config: {:?}", config);

    let mut kernel_page_table = PageTableImpl::new(MEMORY_MANAGER.allocate(), VAddr::new(0x0));
    let kernel_file = if let Some(ref mut ramdisk) = ramdisk {
        ramdisk.load("kernel_riscv").unwrap()
    } else {
        panic!("No kernel source is present!");
    };
    let kernel = image::load_kernel(&kernel_file, &mut kernel_page_table, &MEMORY_MANAGER);
    let mut next_available_kernel_address = kernel.next_available_address;

    /*
     * Enumerate and initialize PCI devices, if present. Even if Seed doesn't end up using them,
     * we're responsible for allocating BAR memory etc.
     */
    PciResolver::initialize(&fdt);

    /*
     * Create space to assemble boot info into, and map it into kernel space.
     */
    const BOOT_INFO_MAX_SIZE: usize = Size4KiB::SIZE;
    let boot_info_phys = MEMORY_MANAGER.allocate_n(Size4KiB::frames_needed(BOOT_INFO_MAX_SIZE)).start.start;
    let boot_info_kernel_address = next_available_kernel_address;
    next_available_kernel_address += BOOT_INFO_MAX_SIZE;
    kernel_page_table
        .map_area(
            boot_info_kernel_address,
            boot_info_phys,
            BOOT_INFO_MAX_SIZE,
            Flags { writable: true, ..Default::default() },
            &MEMORY_MANAGER,
        )
        .unwrap();
    let mut boot_info_area = unsafe { BootInfoArea::new(boot_info_phys) };
    let mut string_table = BootInfoStringTable::new();

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
     * Load desired early tasks.
     */
    let loaded_images_offset = boot_info_area.offset();
    let mut num_loaded_images = 0;
    for name in &config.user_tasks {
        let file = if let Some(ref mut ramdisk) = ramdisk {
            ramdisk.load(&name).unwrap()
        } else {
            panic!("No user tasks source is present!");
        };
        let info = image::load_image(&file, name, &MEMORY_MANAGER);

        let name_offset = string_table.add_string(name);
        let info = seed_bootinfo::LoadedImage {
            name_offset,
            name_len: name.len() as u16,
            num_segments: info.num_segments as u16,
            _reserved0: 0,
            segments: info.segments,
            entry_point: usize::from(info.entry_point) as u64,
        };
        unsafe {
            boot_info_area.write(info);
        }
        num_loaded_images += 1;
    }

    /*
     * Construct the direct physical memory map.
     * TODO: we should probably do this properly by walking the FDT (you need RAM + devices) but we currently just
     * map 32GiB.
     */
    const PHYSICAL_MAP_SIZE: usize = hal::memory::gibibytes(16);
    kernel_page_table
        .map_area(
            kernel_map::PHYSICAL_MAPPING_BASE,
            PAddr::new(0x0).unwrap(),
            PHYSICAL_MAP_SIZE,
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

    // Write out the string table
    let string_table_offset = boot_info_area.offset();
    let string_table_length = string_table.table_len;
    unsafe {
        string_table.write(boot_info_area.cursor);
    }
    boot_info_area.advance(string_table_length as usize, 8);

    /*
     * Now that we've finished allocating memory, we can create the memory map we pass to the kernel. From here, we
     * can't allocate physical memory from the bootloader.
     */
    let (mem_map_offset, mem_map_length) = MEMORY_MANAGER.populate_memory_map(&mut boot_info_area);

    // Write the bootinfo header
    let boot_info_header = seed_bootinfo::Header {
        magic: seed_bootinfo::MAGIC,
        mem_map_offset,
        mem_map_length,

        higher_half_base: usize::from(kernel_map::HIGHER_HALF_START) as u64,
        physical_mapping_base: usize::from(kernel_map::PHYSICAL_MAPPING_BASE) as u64,
        kernel_dynamic_area_base: usize::from(kernel_map::KERNEL_DYNAMIC_AREA_BASE) as u64,
        kernel_image_base: usize::from(kernel_map::KERNEL_IMAGE_BASE) as u64,
        kernel_free_start: usize::from(next_available_kernel_address) as u64,

        rsdp_address: 0,
        device_tree_address: usize::from(fdt_address) as u64,

        loaded_images_offset,
        num_loaded_images,

        string_table_offset,
        string_table_length,

        video_mode_offset: 0,
        _reserved0: [0; 3],
    };
    unsafe {
        ptr::write(usize::from(boot_info_phys) as *mut seed_bootinfo::Header, boot_info_header);
    }

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

#[align(4)]
pub extern "C" fn trap_handler() {
    use hal_riscv::hw::csr::{Scause, Sepc};
    let scause = Scause::read();
    let sepc = Sepc::read();
    panic!("Trap! Scause = {:?}, sepc = {:?}", scause, sepc);
}

pub struct BootInfoArea {
    boot_info_ptr: *mut u8,
    cursor: *mut u8,
}

impl BootInfoArea {
    pub unsafe fn new(base_addr: PAddr) -> BootInfoArea {
        let ptr = usize::from(base_addr) as *mut u8;
        let cursor = unsafe { ptr.byte_add(mem::size_of::<seed_bootinfo::Header>()) };
        BootInfoArea { boot_info_ptr: ptr, cursor }
    }

    /// Get the offset of the current `cursor` into the bootinfo area
    pub fn offset(&self) -> u16 {
        (self.cursor.addr() - self.boot_info_ptr.addr()) as u16
    }

    /// Reserve `count` bytes of boot info space, returning a pointer to the start of the reserved space.
    /// Will ensure the final `cursor` has an alignment of at-least `align`.
    pub fn advance(&mut self, count: usize, align: usize) -> *mut u8 {
        let ptr = self.cursor;
        // TODO: bounds checking
        self.cursor = unsafe { self.cursor.byte_add(count) };
        self.cursor = unsafe { self.cursor.byte_add(self.cursor.align_offset(align)) };
        ptr
    }

    /// Write `value` at `cursor`, and advance `cursor` by the size of `T`.
    pub unsafe fn write<T>(&mut self, value: T) {
        // TODO: bounds checking
        unsafe {
            ptr::write(self.cursor as *mut T, value);
            self.cursor = self.cursor.byte_add(mem::size_of::<T>());
        }
    }
}

pub struct BootInfoStringTable {
    pub entries: Vec<(u16, alloc::string::String)>,
    pub table_len: u16,
}

impl BootInfoStringTable {
    pub fn new() -> BootInfoStringTable {
        BootInfoStringTable { entries: Vec::new(), table_len: 0 }
    }

    pub fn add_string(&mut self, s: &str) -> u16 {
        let offset = self.table_len;
        self.table_len += s.len() as u16;
        self.entries.push((offset, s.to_string()));
        offset
    }

    /// Write the string table out to the given `ptr`. Returns a pointer to the next byte after the string table.
    pub unsafe fn write(self, mut ptr: *mut u8) -> *mut u8 {
        for (_offset, s) in self.entries {
            unsafe {
                ptr::copy(s.as_ptr(), ptr, s.len());
            }
            ptr = ptr.byte_add(s.len());
        }

        ptr.byte_add(1)
    }
}
