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

mod boot_services;
mod memory;
mod protocols;
mod runtime_services;
mod system_table;
mod types;

use crate::{
    memory::{BootFrameAllocator, MemoryMap, MemoryType},
    system_table::SystemTable,
    types::{Guid, Handle, Status},
};
use core::{fmt::Write, mem, panic::PanicInfo, slice};
use mer::{
    section::{SectionHeader, SectionType},
    Elf,
};
use x86_64::{
    boot::{BootInfo, MemoryEntry, MemoryType as BootInfoMemoryType, PayloadInfo},
    hw::{
        registers::{read_control_reg, read_msr, write_control_reg, write_msr},
        serial::SerialPort,
    },
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

/// Describes the loaded kernel image, including its entry point and where it expects the stack to
/// be.
struct KernelInfo {
    entry_point: VirtualAddress,
    stack_top: VirtualAddress,
}

/// Entry point for the bootloader. This is called from the UEFI firmware.
#[no_mangle]
pub extern "win64" fn efi_main(image_handle: Handle, system_table: &'static SystemTable) -> ! {
    unsafe {
        /*
         * The first thing we do is set the global references to the system table and image
         * handle. Until we do this, their "safe" getters are not.
         */
        SYSTEM_TABLE = system_table;
        IMAGE_HANDLE = image_handle;

        /*
         * Initialise the COM1 serial port for debug output.
         */
        SERIAL_PORT.initialise();
    }

    println!("┌─┐┌─┐┌┐ ┌┐ ┬  ┌─┐");
    println!("├─┘├┤ ├┴┐├┴┐│  ├┤ ");
    println!("┴  └─┘└─┘└─┘┴─┘└─┘");

    let allocator = BootFrameAllocator::new(64);
    let mut kernel_page_table = unsafe {
        InactivePageTable::<IdentityMapping>::new_with_recursive_mapping(
            allocator.allocate(),
            kernel_map::RECURSIVE_ENTRY,
        )
    };
    let mut kernel_mapper = kernel_page_table.mapper();

    let kernel_info = match load_kernel(&mut kernel_mapper, &allocator) {
        Ok(kernel_info) => kernel_info,
        Err(err) => panic!("Failed to load kernel: {:?}", err),
    };

    let payload_info = match load_payload(&allocator) {
        Ok(payload_info) => payload_info,
        Err(err) => panic!("Failed to load payload: {:?}", err),
    };

    /*
     * Allocate physical memory for the kernel heap, and map it into the kernel page tables.
     */
    allocate_and_map_heap(&mut kernel_mapper, &allocator);

    /*
     * Allocate space for the `BootInfo`. We allocate a single frame for it, so make sure it'll
     * fit.
     */
    assert!(mem::size_of::<BootInfo>() <= FRAME_SIZE);
    let boot_info_address = system_table
        .boot_services
        .allocate_frames(MemoryType::PebbleBootInformation, 1)
        .map_err(|err| panic!("Failed to allocate memory for the boot info: {:?}", err))
        .unwrap();
    let boot_info = unsafe { &mut *(usize::from(boot_info_address) as *mut BootInfo) };
    boot_info.magic = x86_64::boot::BOOT_INFO_MAGIC;
    boot_info.num_memory_map_entries = 0;

    /*
     * Map the `BootInfo` into the kernel address space at the correct location.
     */
    kernel_mapper.map_to(
        Page::contains(kernel_map::BOOT_INFO),
        Frame::contains(boot_info_address),
        EntryFlags::PRESENT | EntryFlags::NO_EXECUTE,
        &allocator,
    );

    /*
     * Get the final memory map before exiting boot services. We must not allocate between this
     * call and the call to `ExitBootServices`.
     */
    let memory_map = system_table
        .boot_services
        .get_memory_map()
        .map_err(|err| panic!("Failed to get memory map: {:?}", err))
        .unwrap();

    mem::replace(boot_info, construct_boot_info(&memory_map, payload_info));

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

    /*
     * We now terminate the boot services. If this is successful, we become responsible for the
     * running of the system and may no longer make use of any boot services.
     */
    println!("Exiting boot services");
    system_table.boot_services.exit_boot_services(image_handle, memory_map.key).unwrap();

    /*
     * We can now setup for kernel entry by switching to the new kernel page tables and enabling
     * the features we want in the kernel.
     */
    setup_for_kernel();
    println!("Switching to kernel page tables");
    unsafe {
        kernel_page_table.switch_to::<IdentityMapping>();
    }

    /*
     * Jump into the kernel!
     *
     * Because we change the stack pointer, we need to pre-load the kernel entry point into a
     * register, as local variables will no longer be available. We also disable interrupts until
     * the kernel has a chance to install its own IDT and configure the interrupt controller.
     */
    println!("Jumping into kernel\n\n");
    unsafe {
        asm!("cli
              mov rsp, rax
              jmp rbx"
         :
         : "{rax}"(kernel_info.stack_top), "{rbx}"(kernel_info.entry_point)
         : "rax", "rbx", "rsp"
         : "intel"
        );
    }
    unreachable!();
}

/// Set up a common kernel environment. Some of this stuff will already be true for everything we'll
/// successfully boot on realistically, but it doesn't hurt to explicitly set it up.
fn setup_for_kernel() {
    let mut cr4 = read_control_reg!(CR4);
    cr4 |= 1 << 7; // Enable global pages
    cr4 |= 1 << 5; // Enable PAE
    cr4 |= 1 << 2; // Only allow use of the RDTSC instruction in ring 0
    unsafe {
        write_control_reg!(CR4, cr4);
    }

    let mut efer = read_msr!(x86_64::hw::registers::EFER);
    efer |= 1 << 8; // Enable long mode
    efer |= 1 << 11; // Enable use of the NX bit in the page tables
    unsafe {
        write_msr!(x86_64::hw::registers::EFER, efer);
    }
}

fn allocate_and_map_heap(mapper: &mut Mapper<IdentityMapping>, allocator: &BootFrameAllocator) {
    println!("Allocating memory for kernel heap");

    assert!(kernel_map::HEAP_START.is_page_aligned());
    assert!((kernel_map::HEAP_END + 1).unwrap().is_page_aligned());
    let heap_size = (usize::from(kernel_map::HEAP_END) + 1) - usize::from(kernel_map::HEAP_START);
    assert!(heap_size % FRAME_SIZE == 0);
    let heap_physical_base = system_table()
        .boot_services
        .allocate_frames(MemoryType::PebbleKernelHeap, heap_size / FRAME_SIZE)
        .map_err(|err| panic!("Failed to allocate memory for kernel heap: {:?}", err))
        .unwrap();

    let heap_frames = Frame::contains(heap_physical_base)
        ..=Frame::contains((heap_physical_base + heap_size).unwrap());
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

fn construct_boot_info(memory_map: &MemoryMap, payload_info: PayloadInfo) -> BootInfo {
    use x86_64::boot::{BOOT_INFO_MAGIC, MEMORY_MAP_NUM_ENTRIES};
    println!("Constructing boot info to pass to kernel");

    let mut boot_info = BootInfo {
        magic: BOOT_INFO_MAGIC,
        // TODO: we might be able to replace this with `Default::default()` when const generics
        // land and they sort out the `impl T for [U; 1..=32]` mess
        memory_map: unsafe {
            mem::transmute([0u8; mem::size_of::<[MemoryEntry; MEMORY_MAP_NUM_ENTRIES]>()])
        },
        num_memory_map_entries: 0,
        rsdp_address: None,
        payload: payload_info,
    };

    /*
     * First, we construct the memory map. This is used by the OS to initialise the physical
     * memory manager, so it can allocate RAM to things that need it.
     */
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
            MemoryType::PebblePayloadMemory => BootInfoMemoryType::PayloadImage,

            MemoryType::MaxMemoryType => panic!("Invalid memory type found in UEFI memory map!"),
        };

        let start_frame = Frame::contains(entry.physical_start);
        let bootinfo_entry = MemoryEntry {
            area: start_frame..(start_frame + entry.number_of_pages as usize),
            memory_type,
        };

        if boot_info.num_memory_map_entries == x86_64::boot::MEMORY_MAP_NUM_ENTRIES {
            panic!("Run out of space for memory map entries in the BootInfo!");
        }

        boot_info.memory_map[boot_info.num_memory_map_entries] = bootinfo_entry;
        boot_info.num_memory_map_entries += 1;
    }

    /*
     * Next, we locate the RSDP. The conventional searching method may not work on UEFI systems
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

    for config_entry in system_table().config_table().iter() {
        if config_entry.guid == RSDP_V1_GUID || config_entry.guid == RSDP_V2_GUID {
            boot_info.rsdp_address = Some(PhysicalAddress::new(config_entry.address).unwrap());
            break;
        }
    }

    boot_info
}

struct ImageInfo<'a> {
    pub physical_base: PhysicalAddress,
    pub elf: Elf<'a>,
}

/// Loads an ELF from the given path on the boot volume, allocates physical memory for it, and
/// copies its sections into the new memory. Also maps each allocated section into the given set of
/// page tables.
///
/// This borrows the file data, instead of reading the file itself, so that it can return the
/// loaded `Elf` back to the caller.
fn load_image<'a>(
    path: &str,
    image_data: &'a [u8],
    memory_type: MemoryType,
    mapper: &mut Mapper<IdentityMapping>,
    allocator: &BootFrameAllocator,
) -> Result<ImageInfo<'a>, Status> {
    let elf = Elf::new(&image_data)
        .map_err(|err| panic!("Failed to parse ELF({}): {:?}", path, err))
        .unwrap();

    /*
     * Work out how much space we need and check it's a multiple of the page size.
     */
    let image_size =
        elf.sections().fold(
            0,
            |size, section| {
                if section.is_allocated() {
                    size + section.size
                } else {
                    size
                }
            },
        ) as usize;

    if image_size % FRAME_SIZE != 0 {
        panic!("Image size is not a multiple of the frame size: {}", path);
    }

    /*
     * Allocate enough memory and zero it.
     */
    let physical_base = system_table()
        .boot_services
        .allocate_frames(memory_type, image_size / FRAME_SIZE)
        .map_err(|err| panic!("Failed to allocate memory for image({}): {:?}", path, err))
        .unwrap();

    unsafe {
        system_table().boot_services.set_mem(
            usize::from(physical_base) as *mut _,
            image_size as usize,
            0,
        );
    }

    /*
     * Load the sections of the ELF into memory, after which we can free the ELF. We use sections
     * instead of segments because it allows us to define permissions on a per-section basis.
     */
    let mut section_physical_address = physical_base;

    for section in elf.sections() {
        // Skip sections that shouln't be loaded or ones with no data
        if !section.is_allocated() || section.size == 0 {
            continue;
        }

        println!(
            "Loading section of '{}': '{}' at {:#x}-{:#x} at physical address {:#x}",
            path,
            section.name(&elf).unwrap(),
            section.address,
            section.address + section.size - 1,
            section_physical_address,
        );

        map_section(mapper, section_physical_address, &section, allocator);

        /*
         * For `ProgBits` sections, we copy the data from the image into the section's new home.
         * For `NoBits` sections, we leave it zeroed.
         */
        if let SectionType::ProgBits = section.section_type() {
            unsafe {
                slice::from_raw_parts_mut(
                    usize::from(section_physical_address) as *mut u8,
                    section.size as usize,
                )
                .copy_from_slice(section.data(&elf).unwrap());
            }
        }

        section_physical_address = (section_physical_address + section.size as usize).unwrap();
    }

    Ok(ImageInfo { physical_base, elf })
}

fn load_kernel(
    mapper: &mut Mapper<IdentityMapping>,
    allocator: &BootFrameAllocator,
) -> Result<KernelInfo, Status> {
    const KERNEL_PATH: &str = "kernel.elf";

    /*
     * Load the kernel ELF and map it into the page tables.
     */
    let file_data = protocols::read_file(KERNEL_PATH, image_handle())?;
    let image =
        load_image(KERNEL_PATH, &file_data, MemoryType::PebbleKernelMemory, mapper, allocator)?;

    /*
     * We now set up the kernel stack. As part of the `.bss` section, it has already had memory
     * allocated for it, and has been mapped into the page tables. However, we need to go back
     * and unmap the guard page, and extract the address of the top of the stack.
     */
    let guard_page_address =
        match image.elf.symbols().find(|symbol| symbol.name(&image.elf) == Some("_guard_page")) {
            Some(symbol) => VirtualAddress::new(symbol.value as usize).unwrap(),
            None => panic!("Kernel does not have a '_guard_page' symbol!"),
        };
    assert!(guard_page_address.is_page_aligned());
    println!("Unmapping guard page");
    mapper.unmap(Page::contains(guard_page_address), allocator);

    let stack_top =
        match image.elf.symbols().find(|symbol| symbol.name(&image.elf) == Some("_stack_top")) {
            Some(symbol) => VirtualAddress::new(symbol.value as usize).unwrap(),
            None => panic!("Kernel does not have a '_stack_top' symbol"),
        };
    assert!(stack_top.is_page_aligned(), "Stack is not page aligned");

    Ok(KernelInfo { entry_point: VirtualAddress::new(image.elf.entry_point()).unwrap(), stack_top })
}

fn load_payload(allocator: &BootFrameAllocator) -> Result<PayloadInfo, Status> {
    let mut page_table = unsafe {
        InactivePageTable::<IdentityMapping>::new_with_recursive_mapping(
            allocator.allocate(),
            kernel_map::RECURSIVE_ENTRY,
        )
    };

    /*
     * Load and map the ELF.
     */
    const PAYLOAD_PATH: &str = "payload.elf";
    let file_data = protocols::read_file(PAYLOAD_PATH, image_handle())?;
    let image = load_image(
        PAYLOAD_PATH,
        &file_data,
        MemoryType::PebblePayloadMemory,
        &mut page_table.mapper(),
        allocator,
    )?;

    Ok(PayloadInfo {
        entry_point: VirtualAddress::new(image.elf.entry_point()).unwrap(),
        page_table_address: page_table.p4_frame.start_address(),
    })
}

fn map_section(
    mapper: &mut Mapper<IdentityMapping>,
    physical_base: PhysicalAddress,
    section: &SectionHeader,
    allocator: &BootFrameAllocator,
) {
    let virtual_address = VirtualAddress::new(section.address as usize).unwrap();
    /*
     * Because the addresses should be page-aligned, the half-open ranges `[physical_base,
     * physical_base + size)` and `[virtual_address, virtual_address + size)` gives us the
     * correct frame and page ranges.
     */
    let frames = Frame::contains(physical_base)
        ..Frame::contains((physical_base + section.size as usize).unwrap());
    let pages = Page::contains(virtual_address)
        ..Page::contains((virtual_address + section.size as usize).unwrap());
    assert!(frames.clone().count() == pages.clone().count());

    /*
     * Work out the most restrictive set of permissions this section can be mapped with. If the
     * section needs to be writable, mark the pages as writable. If the section does **not**
     * contain executable instructions, mark it as `NO_EXECUTE`.
     */
    let flags = EntryFlags::PRESENT
        | if section.is_writable() { EntryFlags::WRITABLE } else { EntryFlags::empty() }
        | if !section.is_executable() { EntryFlags::NO_EXECUTE } else { EntryFlags::empty() };

    for (frame, page) in frames.zip(pages) {
        mapper.map_to(page, frame, flags, allocator);
    }
}

#[panic_handler]
#[no_mangle]
pub fn panic(info: &PanicInfo) -> ! {
    let location = info.location().unwrap();
    println!(
        "Panic in {}({}:{}): {}",
        location.file(),
        location.line(),
        location.column(),
        info.message().unwrap()
    );
    loop {}
}

macro print {
    ($($arg: tt)*) => {
        unsafe {
            SERIAL_PORT.write_fmt(format_args!($($arg)*)).expect("Failed to write to COM1");
        }
    }
}

macro println {
    ($fmt: expr) => {
        print!(concat!($fmt, "\r\n"));
    },

    ($fmt: expr, $($arg: tt)*) => {
        print!(concat!($fmt, "\r\n"), $($arg)*);
    }
}

/// Returns a reference to the `SystemTable`. This is safe to call after the global has been
/// initialised, which we do straight after control is passed to us.
pub fn system_table() -> &'static SystemTable {
    unsafe { &*SYSTEM_TABLE }
}

pub fn image_handle() -> Handle {
    unsafe { IMAGE_HANDLE }
}

/*
 * It's only safe to have these `static mut`s because we know the bootloader will only have one
 * thread of execution and is completely non-reentrant.
 */
static mut SYSTEM_TABLE: *const SystemTable = 0 as *const _;
static mut IMAGE_HANDLE: Handle = 0;
static mut SERIAL_PORT: SerialPort = unsafe { SerialPort::new(x86_64::hw::serial::COM1) };
