#![no_std]
#![no_main]
#![feature(cell_update, never_type)]

extern crate alloc;

mod allocator;
mod image;
mod logger;

use allocator::BootFrameAllocator;
use core::{arch::asm, convert::TryFrom, mem, panic::PanicInfo, ptr};
use hal::memory::{kibibytes, Bytes, Flags, FrameAllocator, FrameSize, PAddr, Page, PageTable, Size4KiB, VAddr};
use hal_x86_64::paging::PageTableImpl;
use log::{error, info};
use logger::Logger;
use seed::{
    boot_info::{BootInfo, VideoModeInfo},
    SeedConfig,
};
use uefi::{
    fs::Path,
    prelude::*,
    proto::{console::gop::GraphicsOutput, loaded_image::LoadedImage},
    table::boot::{AllocateType, MemoryType, SearchType},
    CString16,
};

/*
 * These are the custom UEFI memory types we use. They're all collected here so we can easily see which numbers
 * we're using.
 * TODO: I think some UEFI impls don't like this so get rid of it at some point.
 */
pub const KERNEL_MEMORY_TYPE: MemoryType = MemoryType::custom(0x80000000);
pub const IMAGE_MEMORY_TYPE: MemoryType = MemoryType::custom(0x80000001);
pub const PAGE_TABLE_MEMORY_TYPE: MemoryType = MemoryType::custom(0x80000002);
pub const MEMORY_MAP_MEMORY_TYPE: MemoryType = MemoryType::custom(0x80000003);
pub const BOOT_INFO_MEMORY_TYPE: MemoryType = MemoryType::custom(0x80000004);
pub const KERNEL_HEAP_MEMORY_TYPE: MemoryType = MemoryType::custom(0x80000005);

const KERNEL_HEAP_SIZE: Bytes = kibibytes(800);

#[entry]
fn efi_main(image_handle: Handle, system_table: SystemTable<Boot>) -> Status {
    Logger::init();
    info!("Hello, World!");

    unsafe {
        uefi::allocator::init(system_table.boot_services());
    }

    let video_mode = create_framebuffer(system_table.boot_services(), 800, 600);
    Logger::switch_to_graphical(&video_mode);

    /*
     * We create a set of page tables for the kernel. Because memory is identity-mapped in UEFI, we can act as
     * if we've placed the physical mapping at 0x0.
     */
    let allocator = BootFrameAllocator::new(system_table.boot_services(), 64);
    let mut page_table = PageTableImpl::new(allocator.allocate(), VAddr::new(0x0));

    /*
     * Get the handle of the volume that the loader's image was loaded off. This will allow us to get access to the
     * filesystem that contains the kernel and other files.
     */
    let loader_image_device =
        system_table.boot_services().open_protocol_exclusive::<LoadedImage>(image_handle).unwrap().device();

    let config = {
        use core::convert::TryFrom;
        use uefi::{data_types::CString16, fs::Path, proto::media::fs::SimpleFileSystem};
        let root_file_protocol = system_table
            .boot_services()
            .open_protocol_exclusive::<SimpleFileSystem>(loader_image_device)
            .expect("Failed to get volume");
        let mut filesystem = uefi::fs::FileSystem::new(root_file_protocol);
        let config = filesystem.read(Path::new(&CString16::try_from("config.toml").unwrap())).unwrap();
        picotoml::from_str::<SeedConfig>(core::str::from_utf8(&config).unwrap()).unwrap()
    };
    info!("Config: {:?}", config);

    let kernel_path = CString16::try_from("kernel.elf").unwrap();
    let kernel_info = {
        image::load_kernel(
            system_table.boot_services(),
            loader_image_device,
            Path::new(&kernel_path),
            &mut page_table,
            &allocator,
        )
    };
    let mut next_safe_address = kernel_info.next_safe_address;

    /*
     * Construct boot info to pass to the kernel.
     */
    let (boot_info_kernel_address, boot_info) = {
        /*
         * First, allocate physical memory to store the boot info in. We can access it directly during construction
         * due to the identity mapping.
         */
        let boot_info_needed_frames = Size4KiB::frames_needed(mem::size_of::<BootInfo>());
        let boot_info_physical_start = system_table
            .boot_services()
            .allocate_pages(AllocateType::AnyPages, BOOT_INFO_MEMORY_TYPE, boot_info_needed_frames)
            .unwrap();
        let identity_boot_info_ptr = boot_info_physical_start as *mut BootInfo;
        unsafe {
            ptr::write(identity_boot_info_ptr, BootInfo::default());
        }

        /*
         * But we need to map it into the kernel's part of the address space for when we switch to the new set of
         * page tables. Choose the next free address and map it there.
         */
        let boot_info_virtual_address = kernel_info.next_safe_address;
        next_safe_address += boot_info_needed_frames * Size4KiB::SIZE;
        page_table
            .map_area(
                boot_info_virtual_address,
                PAddr::new(boot_info_physical_start as usize).unwrap(),
                boot_info_needed_frames * Size4KiB::SIZE,
                Flags { ..Default::default() },
                &allocator,
            )
            .unwrap();
        (boot_info_virtual_address, unsafe { &mut *identity_boot_info_ptr })
    };
    boot_info.magic = seed::boot_info::BOOT_INFO_MAGIC;
    boot_info.video_mode = Some(video_mode);
    boot_info.rsdp_address = find_rsdp(&system_table);

    /*
     * Allocate the kernel heap.
     */
    allocate_and_map_heap(
        system_table.boot_services(),
        boot_info,
        &mut next_safe_address,
        KERNEL_HEAP_SIZE,
        &mut page_table,
        &allocator,
    );

    /*
     * Load the requested images for early tasks.
     */
    for name in &config.user_tasks {
        let path = {
            let mut path = CString16::try_from(name.as_str()).unwrap();
            path.push_str(&CString16::try_from(".elf").unwrap());
            path
        };
        let info = image::load_image(system_table.boot_services(), loader_image_device, name, Path::new(&path));
        boot_info.loaded_images.push(info).unwrap();
    }

    uefi::allocator::exit_boot_services();
    let (_system_table, memory_map) = system_table.exit_boot_services();
    process_memory_map(memory_map, boot_info, &mut page_table, &allocator);

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

fn find_rsdp(system_table: &SystemTable<Boot>) -> Option<PAddr> {
    use uefi::table::cfg::{ACPI2_GUID, ACPI_GUID};

    /*
     * Search the config table for an entry containing the address of the RSDP. First, search the whole table for
     * a v2 RSDP, then if we don't find one, look for a v1 one.
     */
    system_table
        .config_table()
        .iter()
        .find_map(
            |entry| {
                if entry.guid == ACPI2_GUID {
                    Some(PAddr::new(entry.address as usize).unwrap())
                } else {
                    None
                }
            },
        )
        .or_else(|| {
            system_table.config_table().iter().find_map(|entry| {
                if entry.guid == ACPI_GUID {
                    Some(PAddr::new(entry.address as usize).unwrap())
                } else {
                    None
                }
            })
        })
}

/// Process the final UEFI memory map when after we've exited boot services:
///    * Identity-map the loader, so it doesn't disappear from under us.
///    * Construct the memory map passed to the kernel, and add it to the boot info.
///    * Construct the physical memory mapping by mapping all of physical memory into the kernel address space.
fn process_memory_map<'a, A, P>(
    memory_map: uefi::table::boot::MemoryMap<'a>,
    boot_info: &mut BootInfo,
    mapper: &mut P,
    allocator: &A,
) where
    A: FrameAllocator<Size4KiB>,
    P: PageTable<Size4KiB>,
{
    use seed::boot_info::{MemoryMapEntry, MemoryType as BootInfoMemoryType};

    for entry in memory_map.entries() {
        /*
         * Keep the loader identity-mapped, or it will disappear out from under us when we switch to the kernel's
         * page tables. We won't be using the runtime services, so don't bother mapping them.
         *
         * XXX: the virtual address isn't filled out when physically mapped, so use the `phys_start` for both fields
         */
        match entry.ty {
            MemoryType::LOADER_CODE => {
                mapper
                    .map_area(
                        VAddr::new(entry.phys_start as usize),
                        PAddr::new(entry.phys_start as usize).unwrap(),
                        entry.page_count as usize * Size4KiB::SIZE,
                        Flags { executable: true, ..Default::default() },
                        allocator,
                    )
                    .unwrap();
            }
            MemoryType::LOADER_DATA => {
                mapper
                    .map_area(
                        VAddr::new(entry.phys_start as usize),
                        PAddr::new(entry.phys_start as usize).unwrap(),
                        entry.page_count as usize * Size4KiB::SIZE,
                        Flags { writable: true, ..Default::default() },
                        allocator,
                    )
                    .unwrap();
            }
            _ => (),
        }

        /*
         * Add the entry to the boot info memory map, if it can be used by the kernel. This memory map will only
         * be processed after we've left the loader, so we can include memory currently used by the
         * loader as free.
         */
        let mut add_entry = |typ| {
            let start = PAddr::new(entry.phys_start as usize).unwrap();
            let size = entry.page_count as usize * Size4KiB::SIZE;
            boot_info
                .memory_map
                .push(MemoryMapEntry::new(typ, start, size))
                .expect("Run out of memory entry slots in boot info!");
        };
        match entry.ty {
            MemoryType::CONVENTIONAL
            | MemoryType::BOOT_SERVICES_CODE
            | MemoryType::BOOT_SERVICES_DATA
            | MEMORY_MAP_MEMORY_TYPE => add_entry(BootInfoMemoryType::Conventional),

            MemoryType::ACPI_RECLAIM => add_entry(BootInfoMemoryType::AcpiReclaimable),

            BOOT_INFO_MEMORY_TYPE => add_entry(BootInfoMemoryType::BootInfo),
            MemoryType::LOADER_CODE | MemoryType::LOADER_DATA => add_entry(BootInfoMemoryType::Loader),

            // IMAGE_MEMORY_TYPE => add_entry!(BootInfoMemoryType::LoadedImage),
            // PAGE_TABLE_MEMORY_TYPE => add_entry!(BootInfoMemoryType::KernelPageTables),
            // KERNEL_HEAP_MEMORY_TYPE => add_entry!(BootInfoMemoryType::KernelHeap),

            // Other regions will never be usable by the kernel, so we don't bother including them
            _ => (),
        }
    }

    /*
     * Construct the physical memory mapping. We find the maximum physical address that the memory map contains,
     * and map that much physical memory.
     *
     * TODO: this can't handle the newer OVMF memory map, so we're just hardcoding 4GiB for now.
     * This is bad. Replace with something more robust, even maybe with split physical mapping if
     * needed?
     */
    // let max_physical_address = memory_map
    //     .entries()
    //     .filter(|entry| entry.ty == MemoryType::RESERVED)
    //     .map(|entry| entry.phys_start as usize + entry.page_count as usize * Size4KiB::SIZE)
    //     .max()
    //     .unwrap();
    let max_physical_address = 0x10_0000000; // 4GiB
    info!(
        "Constructing physical mapping 0x0..{:#x} at {:#x}",
        max_physical_address,
        hal_x86_64::kernel_map::PHYSICAL_MAPPING_BASE
    );
    mapper
        .map_area(
            hal_x86_64::kernel_map::PHYSICAL_MAPPING_BASE,
            PAddr::new(0x0).unwrap(),
            max_physical_address,
            Flags { writable: true, ..Default::default() },
            allocator,
        )
        .unwrap();
}

/// Allocate and map the kernel heap. This takes the current next safe virtual address, uses it for the heap, and
/// updates it.
fn allocate_and_map_heap<A, P>(
    boot_services: &BootServices,
    boot_info: &mut BootInfo,
    next_safe_address: &mut VAddr,
    heap_size: usize,
    mapper: &mut P,
    allocator: &A,
) where
    A: FrameAllocator<Size4KiB>,
    P: PageTable<Size4KiB>,
{
    assert!(heap_size % Size4KiB::SIZE == 0, "Heap will not be page aligned");
    let frames_needed = Size4KiB::frames_needed(heap_size);
    let physical_start =
        boot_services.allocate_pages(AllocateType::AnyPages, KERNEL_HEAP_MEMORY_TYPE, frames_needed).unwrap();

    mapper
        .map_area(
            *next_safe_address,
            PAddr::new(physical_start as usize).unwrap(),
            heap_size,
            Flags { writable: true, ..Default::default() },
            allocator,
        )
        .unwrap();

    boot_info.heap_address = *next_safe_address;
    boot_info.heap_size = heap_size;
    info!(
        "Mapping heap between {:#x} and {:#x}",
        boot_info.heap_address,
        boot_info.heap_address + boot_info.heap_size - 1
    );

    *next_safe_address = (Page::<Size4KiB>::contains(*next_safe_address + heap_size) + 1).start;
}

fn create_framebuffer(
    boot_services: &BootServices,
    requested_width: usize,
    requested_height: usize,
) -> VideoModeInfo {
    use seed::boot_info::PixelFormat;
    use uefi::proto::console::gop::PixelFormat as GopFormat;

    // Get a list of all the devices that support the `GraphicsOutput` protocol
    let handles = boot_services
        .locate_handle_buffer(SearchType::from_proto::<GraphicsOutput>())
        .expect("Failed to get list of graphics devices");

    for handle in handles.iter() {
        info!("Considering graphics device: {:?}", handle);
        let mut proto = boot_services.open_protocol_exclusive::<GraphicsOutput>(*handle).unwrap();

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

            let mode_info =
                VideoModeInfo { framebuffer_address, pixel_format, width, height, stride: mode_info.stride() };
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
