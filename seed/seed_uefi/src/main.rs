#![no_std]
#![no_main]
#![feature(never_type)]

extern crate alloc;

mod allocator;
mod image;
mod logger;

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use allocator::BootFrameAllocator;
use core::{arch::asm, convert::TryFrom, mem, panic::PanicInfo, ptr};
use hal::memory::{Flags, FrameAllocator, FrameSize, PAddr, PageTable, Size4KiB, VAddr};
use hal_x86_64::paging::PageTableImpl;
use log::{error, info};
use logger::Logger;
use seed_bootinfo::VideoModeInfo;
use seed_config::SeedConfig;
use uefi::{
    boot::{AllocateType, MemoryType, SearchType},
    fs::Path,
    mem::memory_map::MemoryMap,
    prelude::*,
    proto::console::gop::GraphicsOutput,
    CString16,
};

/*
 */
/// Records the usage of various memory allocations that need to be tracked in the memory map. Ideally, we would
/// use UEFI custom memory types for this, but unfortunately due to a bug in Tianocore not behaving correctly with
/// custom memory types (see https://wiki.osdev.org/UEFI#My_bootloader_hangs_if_I_use_user_defined_EFI_MEMORY_TYPE_values),
/// we have to track this ourselves.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum MemoryUse {
    Kernel,
    LoadedImage,
    PageTable,
    BootInfo,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct MemoryUsage {
    start: PAddr,
    length: usize,
    usage: MemoryUse,
}

#[entry]
fn main() -> Status {
    let _ = uefi::helpers::init();
    Logger::init();
    info!("Hello, World!");

    let video_mode = create_framebuffer(800, 600);
    Logger::switch_to_graphical(&video_mode);

    /*
     * We create a set of page tables for the kernel. Because memory is identity-mapped in UEFI, we can act as
     * if we've placed the physical mapping at 0x0.
     */
    let allocator = BootFrameAllocator::new(64);
    let mut page_table = PageTableImpl::new(allocator.allocate(), VAddr::new(0x0));

    let mut string_table = BootInfoStringTable::new();
    let mut memory_usage = Vec::new();

    let mut loader_fs =
        uefi::fs::FileSystem::new(uefi::boot::get_image_file_system(uefi::boot::image_handle()).unwrap());
    let config = {
        let config = loader_fs.read_to_string(CString16::try_from("config.toml").unwrap().as_ref()).unwrap();
        picotoml::from_str::<SeedConfig>(&config).unwrap()
    };
    info!("Config: {:?}", config);

    let kernel_path = CString16::try_from("kernel.elf").unwrap();
    let kernel_info = {
        image::load_kernel(&mut loader_fs, Path::new(&kernel_path), &mut page_table, &allocator, &mut memory_usage)
    };
    let mut next_safe_address = kernel_info.next_safe_address;

    /*
     * Create space to assemble boot info into, and map it into kernel space.
     */
    const BOOT_INFO_MAX_SIZE: usize = Size4KiB::SIZE;
    let boot_info_phys = PAddr::new(
        uefi::boot::allocate_pages(
            AllocateType::AnyPages,
            MemoryType::RESERVED,
            Size4KiB::frames_needed(BOOT_INFO_MAX_SIZE),
        )
        .unwrap()
        .addr()
        .get(),
    )
    .unwrap();
    memory_usage.push(MemoryUsage {
        usage: MemoryUse::BootInfo,
        start: boot_info_phys,
        length: BOOT_INFO_MAX_SIZE,
    });
    let boot_info_kernel_address = next_safe_address;
    next_safe_address += BOOT_INFO_MAX_SIZE;
    page_table
        .map_area(
            boot_info_kernel_address,
            boot_info_phys,
            BOOT_INFO_MAX_SIZE,
            Flags { ..Default::default() },
            &allocator,
        )
        .unwrap();
    let mut boot_info_area = unsafe { BootInfoArea::new(boot_info_phys) };

    /*
     * Load the requested images for early tasks.
     */
    let loaded_images_offset = boot_info_area.offset();
    let mut num_loaded_images = 0;
    for name in &config.user_tasks {
        let path = {
            let mut path = CString16::try_from(name.as_str()).unwrap();
            path.push_str(&CString16::try_from(".elf").unwrap());
            path
        };

        let name_offset = string_table.add_string(name);
        let info = image::load_image(&mut loader_fs, name, Path::new(&path), &mut memory_usage);
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

    // Write out the framebuffer descriptor
    let video_mode_offset = boot_info_area.offset();
    unsafe {
        boot_info_area.write(video_mode);
    }

    // Write out the string table
    let string_table_offset = boot_info_area.offset();
    let string_table_length = string_table.table_len;
    unsafe {
        string_table.write(boot_info_area.cursor);
    }
    boot_info_area.advance(string_table_length as usize, 8);

    /*
     * Exit boot services. From this point, we must be careful to not use the allocator.
     */
    let memory_map = unsafe { uefi::boot::exit_boot_services(None) };

    /*
     * Identity-map loader regions into the kernel page tables.
     * TODO: this could be replaced in future by a smaller dedicated trampoline for the kernel jump
     */
    for entry in memory_map.entries() {
        if entry.ty == MemoryType::LOADER_CODE || entry.ty == MemoryType::LOADER_DATA {
            let flags = Flags {
                executable: entry.ty == MemoryType::LOADER_CODE,
                writable: entry.ty == MemoryType::LOADER_DATA,
                ..Default::default()
            };
            page_table
                .map_area(
                    VAddr::new(entry.phys_start as usize),
                    PAddr::new(entry.phys_start as usize).unwrap(),
                    entry.page_count as usize * Size4KiB::SIZE,
                    flags,
                    &allocator,
                )
                .unwrap();
        }
    }

    /*
     * Map the entirity of physical memory into the top of the kernel's address space. We always do this at 1GiB page granularity.
     * TODO: in theory this should respect Reserved regions that may contain us-allocated stuff. I don't think this matters in practice (we'll see) - we'd have to look at our post-processed
     * memory map to do that properly.
     */
    let top_of_phys_mem = {
        let mut top_of_phys_mem = 0;
        for entry in memory_map.entries() {
            match entry.ty {
                MemoryType::CONVENTIONAL
                | MemoryType::LOADER_CODE
                | MemoryType::LOADER_DATA
                | MemoryType::BOOT_SERVICES_CODE
                | MemoryType::BOOT_SERVICES_DATA
                | MemoryType::ACPI_NON_VOLATILE
                | MemoryType::ACPI_RECLAIM => {}
                _ => continue,
            }
            let aligned_top =
                mulch::math::align_up(entry.phys_start + entry.page_count * Size4KiB::SIZE as u64, 0x4000_0000);
            top_of_phys_mem = u64::max(top_of_phys_mem, aligned_top);
        }
        top_of_phys_mem
    };
    info!("Mapping physical memory 0x0..{:#x} into higher-half direct mapping", top_of_phys_mem);
    page_table
        .map_area(
            hal_x86_64::kernel_map::PHYSICAL_MAPPING_BASE,
            PAddr::new(0x0).unwrap(),
            top_of_phys_mem as usize,
            Flags { writable: true, ..Default::default() },
            &allocator,
        )
        .unwrap();

    /*
     * Create the memory map we're going to pass to the kernel.
     */
    let mem_map_offset = boot_info_area.offset();
    let mut mem_map_length = 0;
    for entry in memory_map.entries() {
        info!("Memmap entry: {:x?}", entry);
        let typ = match entry.ty {
            MemoryType::RESERVED => {
                // memory_usage
                //     .iter()
                //     .find_map(|usage| {
                //         if usize::from(usage.start) as u64 == entry.phys_start {
                //             assert!(usage.length == entry.page_count as usize * Size4KiB::SIZE);
                //             Some(match usage.usage {
                //                 MemoryUse::Kernel => seed_bootinfo::MemoryType::Kernel,
                //                 MemoryUse::LoadedImage => seed_bootinfo::MemoryType::LoadedImage,
                //                 MemoryUse::PageTable => seed_bootinfo::MemoryType::Kernel,
                //                 // TODO: separate memory type??
                //                 MemoryUse::BootInfo => seed_bootinfo::MemoryType::Kernel,
                //             })
                //         } else {
                //             None
                //         }
                //     })
                //     .unwrap_or(seed_bootinfo::MemoryType::Reserved)
                seed_bootinfo::MemoryType::Reserved
            }

            MemoryType::CONVENTIONAL
            | MemoryType::LOADER_CODE
            | MemoryType::LOADER_DATA
            | MemoryType::BOOT_SERVICES_CODE
            | MemoryType::BOOT_SERVICES_DATA => seed_bootinfo::MemoryType::Usable,

            MemoryType::RUNTIME_SERVICES_CODE | MemoryType::RUNTIME_SERVICES_DATA => {
                seed_bootinfo::MemoryType::UefiRuntimeServices
            }

            MemoryType::ACPI_RECLAIM => seed_bootinfo::MemoryType::AcpiReclaimable,
            MemoryType::ACPI_NON_VOLATILE => seed_bootinfo::MemoryType::AcpiNvs,
            // TODO: not sure what to do with this. Reserved for now
            MemoryType::PERSISTENT_MEMORY => seed_bootinfo::MemoryType::Reserved,

            MemoryType::UNUSABLE | MemoryType::MMIO | MemoryType::PAL_CODE | MemoryType::UNACCEPTED | _ => {
                seed_bootinfo::MemoryType::Reserved
            }
        };

        unsafe {
            boot_info_area.write(seed_bootinfo::MemoryEntry {
                base: entry.phys_start,
                length: entry.page_count * Size4KiB::SIZE as u64,
                typ,
                _reserved: 0,
            });
        }
        mem_map_length += 1;
    }

    // Push some scratch entries to facilitate early memory allocation by the kernel directly from the memory map
    const NUM_SCRATCH_ENTRIES: usize = 8;
    for _ in 0..NUM_SCRATCH_ENTRIES {
        unsafe {
            boot_info_area.write(seed_bootinfo::MemoryEntry {
                base: 0,
                length: 0,
                typ: seed_bootinfo::MemoryType::Scratch,
                _reserved: 0,
            });
        }
    }

    /*
     * TODO: I think we might need to do various bits of post-processing to make the memory map better for kernel consumption:
     *    - Go through recorded memory usage and split out `Reserved` entries (can't do during processing bc it combines them...)
     *    - Remove 0-sized entries
     *    - Merging contiguous usable entries (v important for clearing up boot_services spattering everywhere)
     *    - Sort by physical address
     * Maybe also sanity-check for overlapping entries. See Limine's version of this here: https://github.com/limine-bootloader/limine/blob/v9.x/common/mm/pmm.s2.c#L124
     *
     * I'm also wondering about adding some fake 'scratch' entries at the end of the memory map. This could facilitate early memory allocation by the kernel directly from the memmap.
     */

    // Write the bootinfo header
    let boot_info_header = seed_bootinfo::Header {
        magic: seed_bootinfo::MAGIC,
        mem_map_offset,
        mem_map_length,
        rsdp_address: usize::from(find_rsdp().unwrap_or(PAddr::new(0).unwrap())) as u64,
        device_tree_address: 0,
        loaded_images_offset,
        num_loaded_images,
        string_table_offset,
        string_table_length,
        video_mode_offset,
        _reserved0: [0; 3],
    };
    unsafe {
        ptr::write(usize::from(boot_info_phys) as *mut seed_bootinfo::Header, boot_info_header);
    }

    /*
     * Jump into the kernel!
     */
    info!("Entering kernel!\n\n\n");
    unsafe {
        let page_table_address = page_table.p4() as *const _ as usize;
        let kernel_rsp = usize::from(kernel_info.stack_top.align_down(8));
        let kernel_entry_point = usize::from(kernel_info.entry_point);
        let boot_info_address = usize::from(boot_info_kernel_address);

        asm!("// Disable interrupts until the kernel has a chance to install an IDT
              cli

              // Switch to the kernel's new page tables
              mov cr3, rax

              // Switch to the kernel's stack, create a new stack frame, and jump!
              xor rbp, rbp
              mov rsp, rcx
              jmp rdx",
          in("rax") page_table_address,
          in("rcx") kernel_rsp,
          in("rdx") kernel_entry_point,
          in("rdi") boot_info_address,
          options(noreturn)
        )
    }
}

fn find_rsdp() -> Option<PAddr> {
    use uefi::table::cfg::{ACPI2_GUID, ACPI_GUID};

    uefi::system::with_config_table(|entries| {
        /*
         * Search the config table for an entry containing the address of the RSDP. First, search the whole table for
         * a v2 RSDP, then if we don't find one, look for a v1 one.
         */
        entries
            .iter()
            .find_map(|entry| {
                if entry.guid == ACPI2_GUID {
                    Some(PAddr::new(entry.address as usize).unwrap())
                } else {
                    None
                }
            })
            .or_else(|| {
                entries.iter().find_map(|entry| {
                    if entry.guid == ACPI_GUID {
                        Some(PAddr::new(entry.address as usize).unwrap())
                    } else {
                        None
                    }
                })
            })
    })
}

fn create_framebuffer(requested_width: usize, requested_height: usize) -> VideoModeInfo {
    use seed_bootinfo::PixelFormat;
    use uefi::proto::console::gop::PixelFormat as GopFormat;

    // Get a list of all the devices that support the `GraphicsOutput` protocol
    let handles = uefi::boot::locate_handle_buffer(SearchType::from_proto::<GraphicsOutput>())
        .expect("Failed to get list of graphics devices");

    for handle in handles.iter() {
        info!("Considering graphics device: {:?}", handle);
        let mut proto = uefi::boot::open_protocol_exclusive::<GraphicsOutput>(*handle).unwrap();

        let chosen_mode = proto.modes().find(|mode| {
            let (width, height) = mode.info().resolution();
            let pixel_format = mode.info().pixel_format();

            (width == requested_width)
                && (height == requested_height)
                && (pixel_format == GopFormat::Rgb || pixel_format == GopFormat::Bgr)
        });

        if let Some(mode) = chosen_mode {
            proto.set_mode(&mode).expect("Failed to switch to new video mode");

            let framebuffer_address = PAddr::new(proto.frame_buffer().as_mut_ptr() as usize).unwrap();
            let mode_info = mode.info();
            let (width, height) = mode_info.resolution();
            let pixel_format = match mode_info.pixel_format() {
                GopFormat::Rgb => PixelFormat::Rgb32,
                GopFormat::Bgr => PixelFormat::Bgr32,
                _ => panic!("Invalid video mode chosen!"),
            };

            let mode_info = VideoModeInfo {
                framebuffer_address: usize::from(framebuffer_address) as u64,
                pixel_format,
                width: width as u64,
                height: height as u64,
                stride: mode_info.stride() as u64,
            };
            info!("Switched to video mode: {:?}", mode_info);

            return mode_info;
        }
    }

    panic!("Could not find valid video mode!")
}

#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    if let Some(location) = info.location() {
        error!("PANIC: {} ({} - {}:{})", info.message(), location.file(), location.line(), location.column());
    } else {
        error!("PANIC: {} (no location info)", info.message());
    }
    loop {}
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
    pub entries: Vec<(u16, String)>,
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
