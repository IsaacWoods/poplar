#![no_std]
#![no_main]
#![feature(
    const_fn,
    lang_items,
    ptr_internals,
    decl_macro,
    panic_info_message,
    asm,
    never_type,
    cell_update,
    global_asm
)]

mod elf;
mod image;
mod kernel;
mod logger;
mod memory;
mod probestack;
mod uefi;

use crate::{
    memory::{BootFrameAllocator, MemoryMap, MemoryType},
    uefi::{
        boot_services::{OpenProtocolAttributes, Protocol, SearchType},
        protocols::{GraphicsOutput, PixelFormat},
        system_table::SystemTable,
        Guid,
        Handle,
    },
};
use core::{mem, panic::PanicInfo};
use log::{error, info, trace, warn};
use uefi::{image_handle, system_table};
use x86_64::{
    boot::{
        BootInfo,
        MemoryEntry,
        MemoryType as BootInfoMemoryType,
        PixelFormat as BootInfoPixelFormat,
        VideoInfo,
    },
    memory::{
        kernel_map,
        EntryFlags,
        Frame,
        FrameAllocator,
        FrameSize,
        Mapper,
        Page,
        PageTable,
        PhysicalAddress,
        Size2MiB,
        Size4KiB,
        VirtualAddress,
    },
};

/// Entry point for the bootloader. This is called from the UEFI firmware.
#[no_mangle]
pub extern "win64" fn efi_main(image_handle: Handle, system_table: &'static SystemTable) -> ! {
    /*
     * The first thing we must do is initialise the UEFI handles.
     */
    unsafe {
        uefi::init(system_table, image_handle);
    }

    logger::init();
    trace!("Pebble bootloader started");

    /*
     * The UEFI installs a set of page tables that identity-maps the entirity of the physical
     * memory.
     */
    const BOOTLOADER_PHYSICAL_BASE: VirtualAddress = unsafe { VirtualAddress::new_unchecked(0x0) };

    let allocator = BootFrameAllocator::new(64);
    let mut kernel_page_table = PageTable::new(allocator.allocate(), BOOTLOADER_PHYSICAL_BASE);
    let mut kernel_mapper = kernel_page_table.mapper();

    /*
     * Construct the initial `BootInfo`.
     */
    let boot_info = construct_boot_info(&mut kernel_mapper, &allocator);

    /*
     * Read and parse bootcmd file.
     */
    let command_file_data = uefi::protocols::read_file("bootcmd", uefi::image_handle()).expect("No bootcmd file");
    let bootcmd = core::str::from_utf8(&command_file_data).expect("bootcmd is not valid UTF-8");

    let mut kernel_info = None;
    for cmd in bootcmd.lines() {
        trace!("Bootcmd: {}", cmd);
        let mut parts = cmd.split(' ');

        match parts.next() {
            Some("kernel") => {
                let kernel_path = parts.next().expect("Expected path after 'kernel' command");
                info!("Loading kernel from '{}'", kernel_path);
                kernel_info = Some(match kernel::load_kernel(&kernel_path, &mut kernel_mapper, &allocator) {
                    Ok(kernel_info) => kernel_info,
                    Err(err) => panic!("Failed to load kernel: {:?}", err),
                });
            }

            Some("image") => {
                let image_path = parts.next().expect("Expected path after 'image' command");
                let task_name = parts.next().expect("Expected name after path in 'image' command");
                info!("Image loaded by bootloader from '{}' for task called '{}'", image_path, task_name);
                let image = match image::load_image(image_path, task_name, true) {
                    Ok(image) => image,
                    Err(err) => panic!("Failed to load image({}): {:?}", image_path, err),
                };
                boot_info.add_image(image);
            }

            Some("video_mode") => {
                let desired_width = parts
                    .next()
                    .and_then(|x| str::parse::<u32>(x).ok())
                    .expect("Expected integer for desired width after 'video_mode' command");
                let desired_height = parts
                    .next()
                    .and_then(|x| str::parse::<u32>(x).ok())
                    .expect("Expected integer for desired height after 'video_mode' command");
                info!("Attempting to set video mode at {}x{}", desired_width, desired_height);
                choose_and_switch_to_video_mode(boot_info, desired_width, desired_height);
            }

            part => panic!("Invalid bootcmd command: {:?}", part),
        }
    }

    /*
     * Allocate physical memory for the kernel heap, and map it into the kernel page tables.
     */
    allocate_and_map_heap(&mut kernel_mapper, &allocator);

    /*
     * Get the final memory map before exiting boot services. We must not allocate between this call and the
     * call to `ExitBootServices` (this also means we can't log anything to the console, as some UEFI
     * implementations allocate when doing so).
     */
    trace!("Getting final memory map and exiting boot services");
    let memory_map = system_table
        .boot_services
        .get_memory_map()
        .map_err(|err| panic!("Failed to get memory map: {:?}", err))
        .unwrap();

    /*
     * We now terminate the boot services. If this is successful, we become responsible for the
     * running of the system and may no longer make use of any boot services.
     */
    system_table.boot_services.exit_boot_services(image_handle, memory_map.key).unwrap();

    /*
     * The console services are not available after we exit Boot Services, so turn off logging to the console if
     * we haven't already (we might already have if we've switched to a new video mode).
     */
    logger::LOGGER.lock().log_to_console = false;

    add_memory_map_to_boot_info(boot_info, &memory_map);
    create_physical_mapping(&mut kernel_mapper, &allocator, &memory_map);

    /*
     * Identity map the bootloader code and data, and UEFI runtime services into the kernel
     * address space. This is needed so we don't page-fault when we switch page tables.
     *
     * TODO: why do we still need boot services code mapped after calling ExitBootServices?!
     */
    for entry in memory_map.iter() {
        match entry.memory_type {
            MemoryType::LoaderCode
            | MemoryType::LoaderData
            | MemoryType::BootServicesCode
            | MemoryType::BootServicesData
            | MemoryType::RuntimeServicesCode
            | MemoryType::RuntimeServicesData => {
                let virtual_start = VirtualAddress::new(usize::from(entry.physical_start)).unwrap();
                let frames = Frame::contains(entry.physical_start)
                    ..(Frame::contains(entry.physical_start) + entry.number_of_pages as usize);
                let pages = Page::contains(virtual_start)
                    ..(Page::contains(virtual_start) + entry.number_of_pages as usize);

                for (frame, page) in frames.zip(pages) {
                    kernel_mapper
                        .map_to(page, frame, EntryFlags::PRESENT | EntryFlags::WRITABLE, &allocator)
                        .unwrap();
                }
            }

            _ => {}
        }
    }

    kernel::jump_into_kernel(kernel_page_table, kernel_info.expect("Failed to load a kernel"));
}

fn allocate_and_map_heap(mapper: &mut Mapper, allocator: &BootFrameAllocator) {
    trace!("Allocating memory for kernel heap");

    assert!(kernel_map::HEAP_START.is_page_aligned::<Size4KiB>());
    assert!((kernel_map::HEAP_END + 1).is_page_aligned::<Size4KiB>());
    let heap_size = (usize::from(kernel_map::HEAP_END) + 1) - usize::from(kernel_map::HEAP_START);
    assert!(heap_size % Size4KiB::SIZE == 0);
    let heap_physical_base = uefi::system_table()
        .boot_services
        .allocate_frames(MemoryType::PebbleKernelHeap, heap_size / Size4KiB::SIZE)
        .map_err(|err| panic!("Failed to allocate memory for kernel heap: {:?}", err))
        .unwrap();

    let heap_frames = Frame::contains(heap_physical_base)..=Frame::contains(heap_physical_base + heap_size);
    let heap_pages = Page::contains(kernel_map::HEAP_START)..=Page::contains(kernel_map::HEAP_END);
    for (frame, page) in heap_frames.zip(heap_pages) {
        mapper
            .map_to(page, frame, EntryFlags::PRESENT | EntryFlags::WRITABLE | EntryFlags::NO_EXECUTE, allocator)
            .unwrap();
    }
}

/// Allocates space and fills out static parts of the `BootInfo`. Passes back a mutable reference
/// so we can fill more out as we load images, find the final memory map etc.
fn construct_boot_info(kernel_mapper: &mut Mapper, allocator: &BootFrameAllocator) -> &'static mut BootInfo {
    use x86_64::boot::{ImageInfo, BOOT_INFO_MAGIC, NUM_IMAGES, NUM_MEMORY_MAP_ENTRIES};
    trace!("Constructing boot info");

    /*
     * Locate the RSDP. The conventional searching method may not work on UEFI systems
     * (because they're free to put the RSDP wherever they please), so we should try to find it
     * in the configuration table first.
     */
    const RSDP_V1_GUID: Guid =
        Guid { a: 0xeb9d2d30, b: 0x2d88, c: 0x11d3, d: [0x9a, 0x16, 0x00, 0x90, 0x27, 0x3f, 0xc1, 0x4d] };
    const RSDP_V2_GUID: Guid =
        Guid { a: 0x8868e871, b: 0xe4f1, c: 0x11d3, d: [0xbc, 0x22, 0x00, 0x80, 0xc7, 0x3c, 0x88, 0x81] };

    let mut rsdp_address = None;
    for config_entry in uefi::system_table().config_table().iter() {
        if config_entry.guid == RSDP_V1_GUID || config_entry.guid == RSDP_V2_GUID {
            rsdp_address = Some(PhysicalAddress::new(config_entry.address).unwrap());
            break;
        }
    }

    /*
     * Allocate space for the `BootInfo`.
     */
    assert!(mem::size_of::<BootInfo>() <= Size4KiB::SIZE * kernel_map::BOOT_INFO_NUM_PAGES);
    let boot_info_address = uefi::system_table()
        .boot_services
        .allocate_frames(MemoryType::PebbleBootInformation, kernel_map::BOOT_INFO_NUM_PAGES)
        .map_err(|err| panic!("Failed to allocate memory for the boot info: {:?}", err))
        .unwrap();
    let boot_info = unsafe { &mut *(usize::from(boot_info_address) as *mut BootInfo) };

    *boot_info = BootInfo {
        magic: BOOT_INFO_MAGIC,
        // TODO: we might be able to replace this with `Default::default()` when const generics
        // land and they sort out the `impl T for [U; 1..=32]` mess
        memory_map: unsafe { mem::transmute([0u8; mem::size_of::<[MemoryEntry; NUM_MEMORY_MAP_ENTRIES]>()]) },
        num_memory_map_entries: 0,
        rsdp_address,
        num_images: 0,
        images: [ImageInfo::default(); NUM_IMAGES],
        video_info: None,
    };

    /*
     * Map the boot info into the kernel address space at the correct virtual address.
     */
    let frames = Frame::contains(boot_info_address)
        ..Frame::contains(boot_info_address + Size4KiB::SIZE * kernel_map::BOOT_INFO_NUM_PAGES);
    let pages = Page::contains(kernel_map::BOOT_INFO)
        ..Page::contains(kernel_map::BOOT_INFO + Size4KiB::SIZE * kernel_map::BOOT_INFO_NUM_PAGES);
    kernel_mapper.map_range_to(pages, frames, EntryFlags::PRESENT | EntryFlags::NO_EXECUTE, allocator).unwrap();

    boot_info
}

fn choose_and_switch_to_video_mode(boot_info: &mut BootInfo, desired_width: u32, desired_height: u32) {
    /*
     * First, we choose a suitable mode on any device that implements the Graphics Output Protocol.
     */
    let chosen_mode = system_table()
        .boot_services
        .locate_handle(SearchType::ByProtocol, Some(GraphicsOutput::guid()), None)
        .unwrap()
        .iter()
        .map(|&protocol_handle| {
            /*
             * For each handle, we open the protocol and query it to find the modes it supports. We
             * record the protocol handle that each mode comes from, so we know which handle to use
             * if we choose that mode.
             */
            system_table()
                .boot_services
                .open_protocol::<GraphicsOutput>(
                    protocol_handle,
                    image_handle(),
                    0,
                    OpenProtocolAttributes::BY_HANDLE_PROTOCOL,
                )
                .unwrap()
                .modes()
                .map(move |(index, mode_info)| (protocol_handle, index, mode_info))
        })
        .flatten()
        .find(|(_proto_handle, _index, mode_info)| {
            /*
             * We can now select the most suitable mode:
             *      - We only want modes that we can create a linear framebuffer from, so we discard modes that
             *        only support the `blt` function (we also don't support custom pixel formats, just the
             *        normal 32-bit RGB and BGR modes)
             *      - At the moment, we only choose modes that exactly match the user's given resolution; we
             *        could relax this in the future and choose a close mode rather than giving up
             *      - At the moment, we don't pay any attention to which handle is supplying the mode.
             */
            (mode_info.format == PixelFormat::RGB || mode_info.format == PixelFormat::BGR)
                && mode_info.x_resolution == desired_width
                && mode_info.y_resolution == desired_height
        });

    if let Some((protocol_handle, mode_index, mode_info)) = chosen_mode {
        /*
         * Switch to the chosen mode.
         */
        let protocol = system_table()
            .boot_services
            .open_protocol::<GraphicsOutput>(
                protocol_handle,
                image_handle(),
                0,
                OpenProtocolAttributes::BY_HANDLE_PROTOCOL,
            )
            .unwrap();
        protocol.set_mode(mode_index).unwrap();
        trace!(
            "Switched to video mode with width {} and height {}",
            mode_info.x_resolution,
            mode_info.y_resolution
        );

        let pixel_format = match mode_info.format {
            PixelFormat::RGB => BootInfoPixelFormat::RGB32,
            PixelFormat::BGR => BootInfoPixelFormat::BGR32,
            _ => panic!("Chosen mode has unsupported pixel format!"),
        };

        /*
         * Record the required information about the video mode we've chosen in the BootInfo.
         */
        boot_info.video_info = Some(VideoInfo {
            framebuffer_address: PhysicalAddress::new(protocol.mode_data().framebuffer_address as usize).unwrap(),
            pixel_format,
            width: mode_info.x_resolution,
            height: mode_info.y_resolution,
            stride: mode_info.stride,
        });
    } else {
        warn!("Failed to find suitable video mode, but one was requested. Continuing, but there may not be any video output.");
    }
}

fn add_memory_map_to_boot_info(boot_info: &mut BootInfo, memory_map: &MemoryMap) {
    for entry in memory_map.iter() {
        let memory_type = match entry.memory_type {
            // Keep the UEFI runtime services stuff around - anything might be using them.
            MemoryType::RuntimeServicesCode | MemoryType::RuntimeServicesData => BootInfoMemoryType::UefiServices,

            /*
             * The bootloader and boot services code and data can be treated like conventional
             * RAM once we're in the kernel.
             */
            MemoryType::LoaderCode
            | MemoryType::LoaderData
            | MemoryType::BootServicesCode
            | MemoryType::BootServicesData
            | MemoryType::ConventionalMemory => BootInfoMemoryType::Conventional,

            /*
             * This memory must not be used until we're done with the ACPI tables, and then can
             * be used as conventional memory.
             */
            MemoryType::ACPIReclaimMemory => BootInfoMemoryType::AcpiReclaimable,

            MemoryType::ACPIMemoryNVS | MemoryType::PersistentMemory => BootInfoMemoryType::SleepPreserve,

            MemoryType::PalCode => BootInfoMemoryType::NonVolatileSleepPreserve,

            /*
             * These types of memory should not be used by the OS, so we don't emit an entry for
             * them.
             */
            MemoryType::ReservedMemoryType
            | MemoryType::MemoryMappedIO
            | MemoryType::MemoryMappedIOPortSpace
            | MemoryType::UnusableMemory => continue,

            /*
             * These are the memory regions we're allocated for the kernel. We just forward them
             * on in the kernel memory map entries.
             */
            MemoryType::PebbleKernelMemory => BootInfoMemoryType::KernelImage,
            MemoryType::PebblePageTables => BootInfoMemoryType::KernelPageTables,
            MemoryType::PebbleBootInformation => BootInfoMemoryType::BootInfo,
            MemoryType::PebbleKernelHeap => BootInfoMemoryType::KernelHeap,
            MemoryType::PebbleImageMemory => BootInfoMemoryType::LoadedImage,

            MemoryType::MaxMemoryType => panic!("Invalid memory type found in UEFI memory map!"),
        };

        let start_frame = Frame::contains(entry.physical_start);
        boot_info.add_memory_map_entry(MemoryEntry {
            area: start_frame..(start_frame + entry.number_of_pages as usize),
            memory_type,
        });
    }
}

fn create_physical_mapping(mapper: &mut Mapper, allocator: &BootFrameAllocator, memory_map: &MemoryMap) {
    let max_physical_address = memory_map
        .iter()
        .map(|entry| entry.physical_start + (entry.number_of_pages as usize * Size4KiB::SIZE))
        .max()
        .unwrap();
    trace!(
        "Mapping physical memory up to physical address {:#x} into the kernel address space",
        max_physical_address
    );

    // We use huge pages (2MiB) here to use less physical memory
    let start_frame = Frame::<Size2MiB>::starts_with(PhysicalAddress::new(0).unwrap());
    let end_frame = Frame::<Size2MiB>::contains(max_physical_address);
    let start_page = Page::<Size2MiB>::starts_with(kernel_map::KERNEL_ADDRESS_SPACE_START);
    let end_page =
        Page::<Size2MiB>::contains(kernel_map::KERNEL_ADDRESS_SPACE_START + usize::from(max_physical_address));

    for (frame, page) in (start_frame..end_frame).zip(start_page..end_page) {
        mapper.map_to_2MiB(page, frame, EntryFlags::WRITABLE | EntryFlags::NO_EXECUTE, allocator).unwrap();
    }
}

#[panic_handler]
#[no_mangle]
pub fn panic(info: &PanicInfo) -> ! {
    /*
     * Write a message that we'll hopefully see on real hardware. Ignore the result because we're already
     * panicking.
     */
    let _ = system_table().console_out.write_str("Bootloader has panicked!");

    let location = info.location().unwrap();
    error!("Panic in {}({}:{}): {}", location.file(), location.line(), location.column(), info.message().unwrap());
    loop {}
}
