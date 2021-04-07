#![no_std]
#![no_main]
#![feature(panic_info_message, abi_efiapi, cell_update, never_type, asm)]

mod allocator;
mod image;
mod logger;

use allocator::BootFrameAllocator;
use core::{fmt::Write, mem, panic::PanicInfo, ptr, slice};
use hal::{
    boot_info::{BootInfo, VideoModeInfo},
    memory::{
        kibibytes,
        Bytes,
        Flags,
        FrameAllocator,
        FrameSize,
        Page,
        PageTable,
        PhysicalAddress,
        Size4KiB,
        VirtualAddress,
    },
};
use hal_x86_64::paging::PageTableImpl;
use log::{error, info};
use logger::Logger;
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

const KERNEL_HEAP_SIZE: Bytes = kibibytes(800);

#[entry]
fn efi_main(image_handle: Handle, system_table: SystemTable<Boot>) -> Status {
    writeln!(system_table.stdout(), "Hello, World!").unwrap();

    let video_mode = create_framebuffer(system_table.boot_services(), 800, 600);
    Logger::initialize(&video_mode);
    info!("Hello, World!");

    let loaded_image_protocol = unsafe {
        &mut *system_table
            .boot_services()
            .handle_protocol::<LoadedImage>(image_handle)
            .expect_success("Failed to open LoadedImage protocol")
            .get()
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
        "kernel.elf",
        &mut page_table,
        &allocator,
    );
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
            .unwrap_success();
        let identity_boot_info_ptr =
            VirtualAddress::new(boot_info_physical_start as usize).mut_ptr() as *mut BootInfo;
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
                PhysicalAddress::new(boot_info_physical_start as usize).unwrap(),
                boot_info_needed_frames * Size4KiB::SIZE,
                Flags { ..Default::default() },
                &allocator,
            )
            .unwrap();
        (boot_info_virtual_address, unsafe { &mut *identity_boot_info_ptr })
    };
    boot_info.magic = hal::boot_info::BOOT_INFO_MAGIC;
    boot_info.video_mode = Some(video_mode);

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
        KERNEL_HEAP_SIZE,
        &mut page_table,
        &allocator,
    );

    /*
     * Load all the images we've been asked to.
     * TODO
     */
    boot_info
        .loaded_images
        .add_image(image::load_image(
            system_table.boot_services(),
            loaded_image_protocol.device(),
            "test_pf",
            "test_pf.elf",
        ))
        .unwrap();
    boot_info
        .loaded_images
        .add_image(image::load_image(
            system_table.boot_services(),
            loaded_image_protocol.device(),
            "test1",
            "test1.elf",
        ))
        .unwrap();
    boot_info
        .loaded_images
        .add_image(image::load_image(
            system_table.boot_services(),
            loaded_image_protocol.device(),
            "test2",
            "test2.elf",
        ))
        .unwrap();

    // TEMP XXX: pause until key pressed before switching to graphics mode
    // info!("Waiting for key press. Will switch to graphics mode next.");
    // system_table.boot_services().wait_for_event(&mut [system_table.stdin().wait_for_key_event()]);

    /*
     * Allocate memory to hold the memory map. We ask UEFI how much it thinks it needs, and then add a bit, as the
     * allocation for the memory map can itself change how much space the memory map will take.
     */
    let memory_map_size = system_table.boot_services().memory_map_size() + 4 * mem::size_of::<MemoryDescriptor>();
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
    unsafe {
        system_table.boot_services().memset(memory_map_address as *mut u8, memory_map_frames * Size4KiB::SIZE, 0);
    }
    let memory_map_buffer =
        unsafe { slice::from_raw_parts_mut(memory_map_address as *mut u8, memory_map_frames * Size4KiB::SIZE) };

    let (_system_table, memory_map) = system_table
        .exit_boot_services(image_handle, memory_map_buffer)
        .expect_success("Failed to exit boot services");
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
              mov rsp, rbx
              jmp rcx",
          in("rax") page_table_address,
          in("rbx") kernel_rsp,
          in("rcx") kernel_entry_point,
          in("rdi") boot_info_address,
          options(noreturn)
        )
    }
}

/// Process the final UEFI memory map when after we've exited boot services. We need to do three things with it:
///     * We need to identity-map anything that UEFI expects to stay in the same place, including the loader image
///       (the code that's currently running), and the UEFI runtime services. We also map the boot services, as
///       many implementations don't actually stop using them after the call to `ExitBootServices` as they should.
///     * We construct the memory map that will be passed to the kernel, which it uses to initialize its physical
///       memory manager. This is added directly to the already-allocated boot info.
///     * Construct the physical memory mapping - we map the entirity of physical memory into the kernel address
///       space to make it easy for the kernel to access any address it needs to.
fn process_memory_map<'a, A, P>(
    memory_map: impl Iterator<Item = &'a MemoryDescriptor>,
    boot_info: &mut BootInfo,
    mapper: &mut P,
    allocator: &A,
) where
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
         * Keep the loader identity-mapped, or it will disappear out from under us when we switch to the kernel's
         * page tables. We won't be using the runtime services, so don't bother mapping them.
         *
         * XXX: the virtual address isn't filled out when physically mapped, so use the `phys_start` for both fields
         */
        match entry.ty {
            MemoryType::LOADER_CODE => {
                mapper
                    .map_area(
                        VirtualAddress::new(entry.phys_start as usize),
                        PhysicalAddress::new(entry.phys_start as usize).unwrap(),
                        entry.page_count as usize * Size4KiB::SIZE,
                        Flags { executable: true, ..Default::default() },
                        allocator,
                    )
                    .unwrap();
            }
            MemoryType::LOADER_DATA => {
                mapper
                    .map_area(
                        VirtualAddress::new(entry.phys_start as usize),
                        PhysicalAddress::new(entry.phys_start as usize).unwrap(),
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
                    .expect("Run out of memory entry slots in boot info");
            };
        }
        match entry.ty {
            MemoryType::CONVENTIONAL
            | MemoryType::LOADER_CODE
            | MemoryType::LOADER_DATA
            // | MemoryType::BOOT_SERVICES_CODE
            // | MemoryType::BOOT_SERVICES_DATA
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
    info!(
        "Constructing physical mapping 0x0..{:#x} at {:#x}",
        max_physical_address,
        hal_x86_64::kernel_map::PHYSICAL_MAPPING_BASE
    );
    mapper
        .map_area(
            hal_x86_64::kernel_map::PHYSICAL_MAPPING_BASE,
            PhysicalAddress::new(0x0).unwrap(),
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
    next_safe_address: &mut VirtualAddress,
    heap_size: usize,
    mapper: &mut P,
    allocator: &A,
) where
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
}

fn create_framebuffer(
    boot_services: &BootServices,
    requested_width: usize,
    requested_height: usize,
) -> VideoModeInfo {
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
            (width == requested_width)
                && (height == requested_height)
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

            return mode_info;
        }
    }

    panic!("Could not find valid video mode!")
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
