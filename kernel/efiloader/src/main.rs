#![no_std]
#![no_main]
#![feature(panic_info_message, abi_efiapi, cell_update, never_type, asm)]

extern crate rlibc;

mod allocator;
mod command_line;
mod image;
mod logger;

use allocator::BootFrameAllocator;
use command_line::CommandLine;
use core::{mem, panic::PanicInfo, slice};
use hal::{
    boot_info::{BootInfo, VideoModeInfo},
    memory::{Flags, FrameAllocator, FrameSize, Page, PageTable, PhysicalAddress, Size4KiB, VirtualAddress},
};
use hal_x86_64::paging::PageTableImpl;
use log::{error, info};
use uefi::{
    prelude::*,
    proto::{console::gop::GraphicsOutput, loaded_image::LoadedImage},
    table::boot::{AllocateType, MemoryDescriptor, MemoryType, SearchType},
};

/*
 * These are the custom UEFI memory types we use. They're all collected here so we can easily see which numbers
 * we're using.
 */
pub const KERNEL_MEMORY_TYPE: MemoryType = MemoryType::custom(0x80000000);
pub const IMAGE_MEMORY_TYPE: MemoryType = MemoryType::custom(0x80000001);
pub const PAGE_TABLE_MEMORY_TYPE: MemoryType = MemoryType::custom(0x80000002);
pub const MEMORY_MAP_MEMORY_TYPE: MemoryType = MemoryType::custom(0x80000003);
pub const BOOT_INFO_MEMORY_TYPE: MemoryType = MemoryType::custom(0x80000004);
pub const KERNEL_HEAP_MEMORY_TYPE: MemoryType = MemoryType::custom(0x80000005);

#[derive(Clone, Copy, Debug)]
pub enum LoaderError {
    NoKernelPath,
    NoBootVolume,
    BootVolumeDoesNotExist,
    FailedToLoadKernel,
    FilePathDoesNotExist,
    NoValidVideoMode,
}

#[entry]
fn efi_main(image_handle: Handle, system_table: SystemTable<Boot>) -> Status {
    /*
     * This is the UEFI entry point, which simply wraps the real entry point, `main`, so that is can return a more
     * ergonomic `Result<!, LoaderError>`, instead of a UEFI `Status`. `main` is diverging, so we can be sure the
     * happy path here is unreachable.
     */
    match main(image_handle, system_table) {
        Ok(_) => unsafe { core::hint::unreachable_unchecked() },
        Err(err) => {
            error!("Something went wrong: {:?}", err);
            Status::LOAD_ERROR
        }
    }
}

fn main(image_handle: Handle, system_table: SystemTable<Boot>) -> Result<!, LoaderError> {
    logger::init(system_table.stdout());
    info!("Hello, World!");

    let loaded_image_protocol = unsafe {
        &mut *system_table
            .boot_services()
            .handle_protocol::<LoadedImage>(image_handle)
            .expect_success("Failed to open LoadedImage protocol")
            .get()
    };

    const COMMAND_LINE_MAX_LENGTH: usize = 256;
    let mut buffer = [0u8; COMMAND_LINE_MAX_LENGTH];

    let load_options_str = loaded_image_protocol.load_options(&mut buffer).expect("Failed to load load options");
    let command_line = CommandLine::new(load_options_str);

    /*
     * Switch to a suitable video mode and create a framebuffer, if the user requested us to.
     */
    let video_mode = match command_line.framebuffer {
        Some(framebuffer_info) => Some(create_framebuffer(system_table.boot_services(), framebuffer_info)?),
        None => None,
    };

    /*
     * We create a set of page tables for the kernel. Because memory is identity-mapped in UEFI, we can act as
     * if we've placed the physical mapping at 0x0.
     */
    let allocator = BootFrameAllocator::new(system_table.boot_services(), 64);
    let mut page_table = PageTableImpl::new(allocator.allocate(), VirtualAddress::new(0x0));

    let kernel_info = image::load_kernel(
        system_table.boot_services(),
        loaded_image_protocol.device(),
        command_line.kernel_path,
        &mut page_table,
        &allocator,
    )?;
    let mut next_safe_address = kernel_info.next_safe_address;

    /*
     * Construct boot info to pass to the kernel.
     */
    let boot_info_needed_frames = Size4KiB::frames_needed(mem::size_of::<BootInfo>());
    let boot_info_physical_start = system_table
        .boot_services()
        .allocate_pages(AllocateType::AnyPages, BOOT_INFO_MEMORY_TYPE, boot_info_needed_frames)
        .unwrap_success();
    let identity_boot_info_ptr = VirtualAddress::new(boot_info_physical_start as usize).mut_ptr() as *mut BootInfo;
    unsafe {
        *identity_boot_info_ptr = BootInfo::default();
    }
    let boot_info = unsafe { &mut *identity_boot_info_ptr };
    let boot_info_virtual_address = kernel_info.next_safe_address;
    next_safe_address += boot_info_needed_frames * Size4KiB::SIZE;
    page_table
        .map_area(
            boot_info_virtual_address,
            PhysicalAddress::new(boot_info_physical_start as usize).unwrap(),
            boot_info_needed_frames * Size4KiB::SIZE,
            Flags { ..Default::default() },
            &allocator,
        )
        .unwrap();
    boot_info.magic = hal::boot_info::BOOT_INFO_MAGIC;
    boot_info.video_mode = video_mode;

    /*
     * Find the RSDP address and add it to the boot info.
     */
    boot_info.rsdp_address = system_table.config_table().iter().find_map(|entry| {
        use uefi::table::cfg::{ACPI2_GUID, ACPI_GUID};
        if entry.guid == ACPI_GUID || entry.guid == ACPI2_GUID {
            Some(PhysicalAddress::new(entry.address as usize).unwrap())
        } else {
            None
        }
    });

    /*
     * Allocate the kernel heap.
     */
    allocate_and_map_heap(
        system_table.boot_services(),
        boot_info,
        &mut next_safe_address,
        command_line.kernel_heap_size,
        &mut page_table,
        &allocator,
    )?;

    /*
     * Load all the images we've been asked to.
     */
    for image in command_line.images() {
        let (name, path) = image.unwrap();
        info!("Loading image called '{}' from path '{}'", name, path);
        boot_info
            .loaded_images
            .add_image(image::load_image(
                system_table.boot_services(),
                loaded_image_protocol.device(),
                name,
                path,
            )?)
            .unwrap();
    }

    /*
     * Allocate memory to hold the memory map. This does something pretty janky:
     *      - Some implementations are super broken, so we ask them how much space they need for their memory
     *        map, but they get it wrong. The most sensible reason for this is that by allocating the frames for
     *        the memory map, you make the memory map bigger because it has to add an entry for the allocation
     *        you've just made. Other implementations just seem to calculate it incorrectly.
     *      - They then return `EFI_BUFFER_TOO_SMALL` when we ask for a memory map later
     *      - So we round up to the next frame, since we can only allocate in 4KiB granularity anyway
     *      - This doesn't waste any space on implementations that don't lie, and provides some headroom on ones
     *        that do
     */
    let memory_map_size = system_table.boot_services().memory_map_size();
    let memory_map_frames = Size4KiB::frames_needed(memory_map_size);
    info!(
        "Memory map will apparently be {} bytes. Allocating {}.",
        memory_map_size,
        memory_map_frames * Size4KiB::SIZE
    );
    let memory_map_address = system_table
        .boot_services()
        .allocate_pages(AllocateType::AnyPages, MEMORY_MAP_MEMORY_TYPE, memory_map_frames)
        .unwrap_success();
    let memory_map_buffer =
        unsafe { slice::from_raw_parts_mut(memory_map_address as *mut u8, memory_map_frames * Size4KiB::SIZE) };

    /*
     * After we've exited from the boot services, we are not able to use the ConsoleOut services, so we disable
     * printing to them in the logger.
     */
    logger::LOGGER.lock().disable_console_output(true);
    let (_system_table, memory_map) = system_table
        .exit_boot_services(image_handle, memory_map_buffer)
        .expect_success("Failed to exit boot services");
    process_memory_map(memory_map, boot_info, &mut page_table, &allocator)?;

    /*
     * Jump into the kernel!
     */
    unsafe {
        info!("Switching to new page tables");
        /*
         * We disable interrupts until the kernel has a chance to install its own IDT.
         */
        asm!("cli");
        page_table.switch_to();

        /*
         * We switch to the new kernel stack, making sure to align it down by 8, so that `rsp-8` will be aligned
         * to 16.
         *
         * Because we change the stack pointer, we need to load the entry point into a register, as local
         * variables will no longer be available.
         *
         * We zero `rbp`, so when the kernel creates its first stack frame, it'll terminate at the kernel entry
         * point.
         */
        info!("Jumping into kernel!\n\n\n");
        asm!("xor rbp, rbp
              mov rsp, rax
              jmp rbx",
            in("rax") usize::from(kernel_info.stack_top.align_down(8)),
            in("rbx") usize::from(kernel_info.entry_point),
            in("rdi") usize::from(boot_info_virtual_address),
        );
    }
    unreachable!()
}

/// Process the final UEFI memory map when after we've exited boot services. We need to do three things with it:
///     * We need to identity-map anything that UEFI expects to stay in the same place, including the loader image
///       (the code that's currently running), and the UEFI runtime services. We also map the boot services, as
///       many implementations don't actually stop using them after the call to `ExitBootServices` as they should.
///     * We construct the memory map that will be passed to the kernel, which it uses to initialise its physical
///       memory manager. This is added directly to the already-allocated boot info.
///     * Construct the physical memory mapping - we map the entirity of physical memory into the kernel address
///       space to make it easy for the kernel to access any address it needs to.
fn process_memory_map<'a, A, P>(
    memory_map: impl Iterator<Item = &'a MemoryDescriptor>,
    boot_info: &mut BootInfo,
    mapper: &mut P,
    allocator: &A,
) -> Result<(), LoaderError>
where
    A: FrameAllocator<Size4KiB>,
    P: PageTable<Size4KiB>,
{
    use hal::boot_info::{MemoryMapEntry, MemoryType as BootInfoMemoryType};

    /*
     * To know how much physical memory to map, we keep track of the largest physical address that appears in
     * the memory map.
     */
    let mut max_physical_address = 0x0;

    for entry in memory_map {
        /*
         * If this is the largest physical address we've seen, update it.
         */
        max_physical_address = usize::max(
            max_physical_address,
            entry.phys_start as usize + entry.page_count as usize * Size4KiB::SIZE,
        );

        /*
         * Identity-map the entry in the kernel page tables, if needed.
         */
        match entry.ty {
            MemoryType::LOADER_CODE
            | MemoryType::LOADER_DATA
            | MemoryType::BOOT_SERVICES_CODE
            | MemoryType::BOOT_SERVICES_DATA
            | MemoryType::RUNTIME_SERVICES_CODE
            | MemoryType::RUNTIME_SERVICES_DATA => {
                info!("Identity mapping area: {:?}", entry);
                /*
                 * Implementations don't appear to fill out the `VirtualStart` field, so we use the physical
                 * address for both (as we're identity-mapping anyway).
                 */
                mapper
                    .map_area(
                        VirtualAddress::new(entry.phys_start as usize),
                        PhysicalAddress::new(entry.phys_start as usize).unwrap(),
                        entry.page_count as usize * Size4KiB::SIZE,
                        Flags { writable: true, executable: true, ..Default::default() },
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
        // TODO: move this to a decl_macro when hygiene-opt-out is implemented
        macro_rules! add_entry {
            ($type: expr) => {
                boot_info
                    .memory_map
                    .add_entry(MemoryMapEntry {
                        start: PhysicalAddress::new(entry.phys_start as usize).unwrap(),
                        size: entry.page_count as usize * Size4KiB::SIZE,
                        memory_type: $type,
                    })
                    .unwrap();
            };
        }
        match entry.ty {
            MemoryType::BOOT_SERVICES_CODE
            | MemoryType::BOOT_SERVICES_DATA
            | MemoryType::CONVENTIONAL
            | MemoryType::LOADER_CODE
            | MemoryType::LOADER_DATA
            | MEMORY_MAP_MEMORY_TYPE => add_entry!(BootInfoMemoryType::Conventional),

            MemoryType::ACPI_RECLAIM => add_entry!(BootInfoMemoryType::AcpiReclaimable),
            IMAGE_MEMORY_TYPE => add_entry!(BootInfoMemoryType::LoadedImage),
            PAGE_TABLE_MEMORY_TYPE => add_entry!(BootInfoMemoryType::KernelPageTables),
            BOOT_INFO_MEMORY_TYPE => add_entry!(BootInfoMemoryType::BootInfo),
            KERNEL_HEAP_MEMORY_TYPE => add_entry!(BootInfoMemoryType::KernelHeap),

            // Other regions will never be useable by the kernel, so we don't bother including them
            _ => (),
        }
    }

    /*
     * Construct the physical memory mapping. We find the maximum physical address that the memory map contains,
     * and map that much physical memory.
     */
    info!("Constructing physical mapping from 0x0 to {:#x}", max_physical_address);
    mapper
        .map_area(
            hal_x86_64::kernel_map::PHYSICAL_MAPPING_BASE,
            PhysicalAddress::new(0x0).unwrap(),
            max_physical_address,
            Flags { writable: true, ..Default::default() },
            allocator,
        )
        .unwrap();

    Ok(())
}

/// Allocate and map the kernel heap. This takes the current next safe virtual address, uses it for the heap, and
/// updates it.
fn allocate_and_map_heap<A, P>(
    boot_services: &BootServices,
    boot_info: &mut BootInfo,
    next_safe_address: &mut VirtualAddress,
    heap_size: usize,
    mapper: &mut P,
    allocator: &A,
) -> Result<(), LoaderError>
where
    A: FrameAllocator<Size4KiB>,
    P: PageTable<Size4KiB>,
{
    assert!(heap_size % Size4KiB::SIZE == 0, "Heap will not be page aligned");
    let frames_needed = Size4KiB::frames_needed(heap_size);
    let physical_start = boot_services
        .allocate_pages(AllocateType::AnyPages, KERNEL_HEAP_MEMORY_TYPE, frames_needed)
        .unwrap_success();

    mapper
        .map_area(
            *next_safe_address,
            PhysicalAddress::new(physical_start as usize).unwrap(),
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
    Ok(())
}

fn create_framebuffer(
    boot_services: &BootServices,
    framebuffer_info: command_line::Framebuffer,
) -> Result<VideoModeInfo, LoaderError> {
    use hal::boot_info::PixelFormat;
    use uefi::proto::console::gop::PixelFormat as GopFormat;

    // Make an initial call to find how many handles we need to search
    let num_handles = boot_services
        .locate_handle(SearchType::from_proto::<GraphicsOutput>(), None)
        .expect_success("Failed to get list of GOP devices");

    // Allocate a pool of the needed size
    let pool_addr = boot_services
        .allocate_pool(MemoryType::LOADER_DATA, mem::size_of::<Handle>() * num_handles)
        .expect_success("Failed to allocate pool for GOP handles");
    let handle_slice: &mut [Handle] = unsafe { slice::from_raw_parts_mut(pool_addr as *mut Handle, num_handles) };

    // Actually fetch the handles
    boot_services
        .locate_handle(SearchType::from_proto::<GraphicsOutput>(), Some(handle_slice))
        .expect_success("Failed to get list of graphics output devices");

    for &mut handle in handle_slice {
        let proto = unsafe {
            &mut *boot_services
                .handle_protocol::<GraphicsOutput>(handle)
                .expect_success("Failed to open GraphicsOutput")
                .get()
        };

        let chosen_mode = proto.modes().map(|mode| mode.unwrap()).find(|mode| {
            let (width, height) = mode.info().resolution();
            let pixel_format = mode.info().pixel_format();

            /*
             * TODO: we currently assume that the command line provides both a width and a height. In the future,
             * it would be better to just apply the filters the user actually cares about
             */
            width == framebuffer_info.width.unwrap()
                && height == framebuffer_info.height.unwrap()
                && (pixel_format == GopFormat::RGB || pixel_format == GopFormat::BGR)
        });

        if let Some(mode) = chosen_mode {
            proto.set_mode(&mode).expect_success("Failed to switch to new video mode");

            let framebuffer_address = PhysicalAddress::new(proto.frame_buffer().as_mut_ptr() as usize).unwrap();
            let mode_info = mode.info();
            let (width, height) = mode_info.resolution();
            let pixel_format = match mode_info.pixel_format() {
                GopFormat::RGB => PixelFormat::RGB32,
                GopFormat::BGR => PixelFormat::BGR32,
                _ => panic!("Invalid video mode chosen!"),
            };

            let mode_info =
                VideoModeInfo { framebuffer_address, pixel_format, width, height, stride: mode_info.stride() };
            info!("Switched to video mode: {:?}", mode_info);

            return Ok(mode_info);
        }
    }

    Err(LoaderError::NoValidVideoMode)
}

#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    if let Some(message) = info.message() {
        if let Some(location) = info.location() {
            error!("Panic message: {} ({} - {}:{})", message, location.file(), location.line(), location.column());
        } else {
            error!("Panic message: {} (no location info)", message);
        }
    }
    loop {}
}
