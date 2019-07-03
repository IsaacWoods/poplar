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
    cell_update
)]

mod elf;
mod kernel;
mod logger;
mod memory;
mod uefi;

use crate::{
    memory::{BootFrameAllocator, MemoryMap, MemoryType},
    uefi::{system_table::SystemTable, Guid, Handle},
};
use core::{mem, panic::PanicInfo};
use log::{error, trace};
use x86_64::{
    boot::{BootInfo, MemoryEntry, MemoryType as BootInfoMemoryType},
    memory::{
        kernel_map,
        paging::{
            entry::EntryFlags,
            table::IdentityMapping,
            Frame,
            FrameAllocator,
            InactivePageTable,
            Mapper,
            Page,
            FRAME_SIZE,
        },
        PhysicalAddress,
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

    let allocator = BootFrameAllocator::new(64);
    let mut kernel_page_table = unsafe {
        InactivePageTable::<IdentityMapping>::new_with_recursive_mapping(
            allocator.allocate(),
            kernel_map::RECURSIVE_ENTRY,
        )
    };
    let kernel_p4_frame = kernel_page_table.p4_frame;
    let mut kernel_mapper = kernel_page_table.mapper();

    /*
     * We permanently map the kernel's P4 frame to a virtual address so the kernel can always
     * access it without using the recursive mapping.
     */
    kernel_mapper.map_to(
        Page::contains(kernel_map::KERNEL_P4_START),
        kernel_p4_frame,
        EntryFlags::PRESENT | EntryFlags::WRITABLE | EntryFlags::NO_EXECUTE,
        &allocator,
    );

    /*
     * Construct the initial `BootInfo`.
     */
    let boot_info = construct_boot_info(&mut kernel_mapper, &allocator);

    /*
     * Read and parse bootcmd file.
     */
    let command_file_data =
        uefi::protocols::read_file("bootcmd", uefi::image_handle()).expect("No bootcmd file");
    let bootcmd = core::str::from_utf8(&command_file_data).expect("bootcmd is not valid UTF-8");

    let mut kernel_info = None;
    for cmd in bootcmd.lines() {
        trace!("Bootcmd: {}", cmd);
        let mut parts = cmd.split(' ');

        match parts.next() {
            Some("kernel") => {
                let kernel_path = parts.next().expect("Expected path after 'kernel' command");
                kernel_info =
                    Some(match kernel::load_kernel(&kernel_path, &mut kernel_mapper, &allocator) {
                        Ok(kernel_info) => kernel_info,
                        Err(err) => panic!("Failed to load kernel: {:?}", err),
                    });
            }

            part => panic!("Invalid bootcmd command: {:?}", part),
        }
    }

    /*
     * Allocate physical memory for the kernel heap, and map it into the kernel page tables.
     */
    allocate_and_map_heap(&mut kernel_mapper, &allocator);

    /*
     * Get the final memory map before exiting boot services. We must not allocate between this
     * call and the call to `ExitBootServices`.
     */
    let memory_map = system_table
        .boot_services
        .get_memory_map()
        .map_err(|err| panic!("Failed to get memory map: {:?}", err))
        .unwrap();

    /*
     * We now terminate the boot services. If this is successful, we become responsible for the
     * running of the system and may no longer make use of any boot services.
     */
    trace!("Exiting boot services");
    system_table.boot_services.exit_boot_services(image_handle, memory_map.key).unwrap();
    add_memory_map_to_boot_info(boot_info, &memory_map);

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
                    kernel_mapper.map_to(
                        page,
                        frame,
                        EntryFlags::PRESENT | EntryFlags::WRITABLE,
                        &allocator,
                    );
                }
            }

            _ => {}
        }
    }

    kernel::jump_into_kernel(kernel_page_table, kernel_info.expect("Failed to load a kernel"));
}

fn allocate_and_map_heap(mapper: &mut Mapper<IdentityMapping>, allocator: &BootFrameAllocator) {
    trace!("Allocating memory for kernel heap");

    assert!(kernel_map::HEAP_START.is_page_aligned());
    assert!((kernel_map::HEAP_END + 1).is_page_aligned());
    let heap_size = (usize::from(kernel_map::HEAP_END) + 1) - usize::from(kernel_map::HEAP_START);
    assert!(heap_size % FRAME_SIZE == 0);
    let heap_physical_base = uefi::system_table()
        .boot_services
        .allocate_frames(MemoryType::PebbleKernelHeap, heap_size / FRAME_SIZE)
        .map_err(|err| panic!("Failed to allocate memory for kernel heap: {:?}", err))
        .unwrap();

    let heap_frames =
        Frame::contains(heap_physical_base)..=Frame::contains(heap_physical_base + heap_size);
    let heap_pages = Page::contains(kernel_map::HEAP_START)..=Page::contains(kernel_map::HEAP_END);
    for (frame, page) in heap_frames.zip(heap_pages) {
        mapper.map_to(
            page,
            frame,
            EntryFlags::PRESENT | EntryFlags::WRITABLE | EntryFlags::NO_EXECUTE,
            allocator,
        );
    }
}

/// Allocates space and fills out static parts of the `BootInfo`. Passes back a mutable reference
/// so we can fill more out as we load images, find the final memory map etc.
fn construct_boot_info(
    kernel_mapper: &mut Mapper<IdentityMapping>,
    allocator: &BootFrameAllocator,
) -> &'static mut BootInfo {
    use x86_64::boot::{ImageInfo, BOOT_INFO_MAGIC, NUM_IMAGES, NUM_MEMORY_MAP_ENTRIES};
    trace!("Constructing boot info");

    /*
     * Locate the RSDP. The conventional searching method may not work on UEFI systems
     * (because they're free to put the RSDP wherever they please), so we should try to find it
     * in the configuration table first.
     */
    const RSDP_V1_GUID: Guid = Guid {
        a: 0xeb9d2d30,
        b: 0x2d88,
        c: 0x11d3,
        d: [0x9a, 0x16, 0x00, 0x90, 0x27, 0x3f, 0xc1, 0x4d],
    };
    const RSDP_V2_GUID: Guid = Guid {
        a: 0x8868e871,
        b: 0xe4f1,
        c: 0x11d3,
        d: [0xbc, 0x22, 0x00, 0x80, 0xc7, 0x3c, 0x88, 0x81],
    };

    let mut rsdp_address = None;
    for config_entry in uefi::system_table().config_table().iter() {
        if config_entry.guid == RSDP_V1_GUID || config_entry.guid == RSDP_V2_GUID {
            rsdp_address = Some(PhysicalAddress::new(config_entry.address).unwrap());
            break;
        }
    }

    /*
     * Allocate space for the `BootInfo`. We allocate a single frame for it, so make sure it'll
     * fit.
     */
    assert!(mem::size_of::<BootInfo>() <= FRAME_SIZE);
    let boot_info_address = uefi::system_table()
        .boot_services
        .allocate_frames(MemoryType::PebbleBootInformation, 1)
        .map_err(|err| panic!("Failed to allocate memory for the boot info: {:?}", err))
        .unwrap();
    let boot_info = unsafe { &mut *(usize::from(boot_info_address) as *mut BootInfo) };

    *boot_info = BootInfo {
        magic: BOOT_INFO_MAGIC,
        // TODO: we might be able to replace this with `Default::default()` when const generics
        // land and they sort out the `impl T for [U; 1..=32]` mess
        memory_map: unsafe {
            mem::transmute([0u8; mem::size_of::<[MemoryEntry; NUM_MEMORY_MAP_ENTRIES]>()])
        },
        num_memory_map_entries: 0,
        rsdp_address,
        num_images: 0,
        images: [ImageInfo::default(); NUM_IMAGES],
    };

    /*
     * Map the boot info into the kernel address space at the correct virtual address.
     */
    kernel_mapper.map_to(
        Page::contains(kernel_map::BOOT_INFO),
        Frame::contains(boot_info_address),
        EntryFlags::PRESENT | EntryFlags::NO_EXECUTE,
        allocator,
    );

    boot_info
}

fn add_memory_map_to_boot_info(boot_info: &mut BootInfo, memory_map: &MemoryMap) {
    for entry in memory_map.iter() {
        let memory_type = match entry.memory_type {
            // Keep the UEFI runtime services stuff around - anything might be using them.
            MemoryType::RuntimeServicesCode | MemoryType::RuntimeServicesData => {
                BootInfoMemoryType::UefiServices
            }

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

            MemoryType::ACPIMemoryNVS | MemoryType::PersistentMemory => {
                BootInfoMemoryType::SleepPreserve
            }

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

            MemoryType::MaxMemoryType => panic!("Invalid memory type found in UEFI memory map!"),
        };

        let start_frame = Frame::contains(entry.physical_start);
        boot_info.add_memory_map_entry(MemoryEntry {
            area: start_frame..(start_frame + entry.number_of_pages as usize),
            memory_type,
        });
    }
}

#[panic_handler]
#[no_mangle]
pub fn panic(info: &PanicInfo) -> ! {
    let location = info.location().unwrap();
    error!(
        "Panic in {}({}:{}): {}",
        location.file(),
        location.line(),
        location.column(),
        info.message().unwrap()
    );
    loop {}
}
