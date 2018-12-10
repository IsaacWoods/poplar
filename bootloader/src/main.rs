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

use crate::boot_services::{AllocateType, OpenProtocolAttributes, Pool, Protocol, SearchType};
use crate::memory::{BootFrameAllocator, MemoryType};
use crate::protocols::{FileAttributes, FileInfo, FileMode, FileSystemInfo, SimpleFileSystem};
use crate::system_table::SystemTable;
use crate::types::{Handle, Status};
use core::fmt::Write;
use core::panic::PanicInfo;
use core::slice;
use mer::{
    section::{SectionHeader, SectionType},
    Elf,
};
use x86_64::boot::BootInfo;
use x86_64::hw::registers::{read_control_reg, read_msr, write_control_reg, write_msr};
use x86_64::hw::serial::SerialPort;
use x86_64::memory::kernel_map;
use x86_64::memory::paging::entry::EntryFlags;
use x86_64::memory::paging::table::IdentityMapping;
use x86_64::memory::paging::{Frame, InactivePageTable, Mapper, Page, FRAME_SIZE};
use x86_64::memory::{PhysicalAddress, VirtualAddress};

static mut SYSTEM_TABLE: *const SystemTable = 0 as *const _;
static mut IMAGE_HANDLE: Handle = 0;
static mut SERIAL_PORT: SerialPort = unsafe { SerialPort::new(x86_64::hw::serial::COM1) };

/// Describes the loaded kernel image, including its entry point and where it expects the stack to
/// be.
struct KernelInfo {
    entry_point: VirtualAddress,
    stack_top: VirtualAddress,
}

#[no_mangle]
pub extern "win64" fn uefi_main(image_handle: Handle, system_table: &'static SystemTable) -> ! {
    unsafe {
        /*
         * The first thing we do is set the global references to the system table and image handle.
         * Until we do this, their "safe" getters are not.
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

    let allocator = BootFrameAllocator::new(32);
    let mut page_table = create_page_table();
    let mut mapper = page_table.mapper();

    let kernel_info = match load_kernel(image_handle, &mut mapper, &allocator) {
        Ok(entry_point) => entry_point,
        Err(err) => panic!("Failed to load kernel: {:?}", err),
    };

    /*
     * Allocate physical memory for the kernel heap, and map it into the kernel page tables.
     */
    assert!(kernel_map::HEAP_START.is_page_aligned());
    assert!((kernel_map::HEAP_END + 1).unwrap().is_page_aligned());
    let heap_size = (u64::from(kernel_map::HEAP_END) + 1) - u64::from(kernel_map::HEAP_START);
    assert!(heap_size % FRAME_SIZE == 0);
    let heap_physical_base = match system_table
        .boot_services
        .allocate_frames(MemoryType::PebbleKernelHeap, heap_size / FRAME_SIZE)
    {
        Ok(address) => address,
        Err(err) => panic!("Failed to allocate memory for kernel heap: {:?}", err),
    };

    let heap_frames = Frame::contains(heap_physical_base)
        ..=Frame::contains((heap_physical_base + heap_size).unwrap());
    let heap_pages = Page::contains(kernel_map::HEAP_START)..=Page::contains(kernel_map::HEAP_END);
    for (frame, page) in heap_frames.zip(heap_pages) {
        mapper.map_to(
            page,
            frame,
            EntryFlags::PRESENT | EntryFlags::WRITABLE | EntryFlags::NO_EXECUTE,
            &allocator,
        );
    }

    /*
     * Get the final memory map before exiting boot services. We must not allocate between this
     * call and the call to `ExitBootServices`.
     */
    let memory_map = match system_table.boot_services.get_memory_map() {
        Ok(map) => map,
        Err(err) => panic!("Failed to get memory map: {:?}", err),
    };

    /*
     * Identity-map the bootloader and UEFI runtime stuff into the kernel address space. This is
     * needed so we don't page fault when we switch to the new page tables, and so we can still use
     * runtime services.
     *
     * TODO: why do we need boot services code mapped after we've exited them??
     */
    for entry in memory_map.iter() {
        match entry.memory_type {
            MemoryType::LoaderCode
            | MemoryType::LoaderData
            | MemoryType::BootServicesCode
            | MemoryType::BootServicesData
            | MemoryType::RuntimeServicesCode
            | MemoryType::RuntimeServicesData => {
                let virtual_start = VirtualAddress::new(u64::from(entry.physical_start)).unwrap();
                let frames = Frame::contains(entry.physical_start)
                    ..(Frame::contains(entry.physical_start) + entry.number_of_pages);
                let pages = Page::contains(virtual_start)
                    ..(Page::contains(virtual_start) + entry.number_of_pages);

                for (frame, page) in frames.zip(pages) {
                    mapper.map_to(
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
    system_table
        .boot_services
        .exit_boot_services(image_handle, memory_map.key)
        .unwrap();

    /*
     * We can now setup for kernel entry by switching to the new kernel page tables and enabling
     * the features we want in the kernel.
     */
    setup_for_kernel();
    println!("Switching to kernel page tables");
    unsafe {
        page_table.switch_to::<IdentityMapping>();
    }

    /*
     * TODO: allocate a `BootInfo` somewhere and pass its address to the kernel in the correct way
     * (we might have to change the entry point's ABI to do this safely).
     */

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

fn create_page_table() -> InactivePageTable<IdentityMapping> {
    // Allocate a frame for the P4
    let address = match system_table()
        .boot_services
        .allocate_frames(MemoryType::PebblePageTables, 1)
    {
        Ok(address) => address,
        Err(err) => panic!(
            "Failed to allocate physical memory for page tables: {:?}",
            err
        ),
    };

    // Zero the P4 to mark every entry as non-present
    unsafe {
        system_table()
            .boot_services
            .set_mem(u64::from(address) as *mut _, FRAME_SIZE as usize, 0);
    }

    InactivePageTable::new(Frame::contains(address))
}

/// Load the kernel's sections into memory and return the entry point
fn load_kernel(
    image_handle: Handle,
    mapper: &mut Mapper<IdentityMapping>,
    allocator: &BootFrameAllocator,
) -> Result<KernelInfo, Status> {
    println!("Loading kernel image from boot volume");
    let file_data = match read_file("BOOT", "kernel.elf", image_handle) {
        Ok(data) => data,
        Err(err) => panic!("Failed to read kernel ELF from disk: {:?}", err),
    };

    let elf = match Elf::new(&file_data) {
        Ok(elf) => elf,
        Err(err) => panic!("Failed to parse kernel ELF: {:?}", err),
    };

    // Work out how much space we need for the kernel and check it's a multiple of the page size
    let kernel_size = elf.sections().fold(0, |kernel_size, section| {
        if section.is_allocated() {
            kernel_size + section.size
        } else {
            kernel_size
        }
    });

    if kernel_size % FRAME_SIZE != 0 {
        panic!(
            "Kernel size is not a multiple of frame size: {:#x}!",
            kernel_size
        );
    }

    // Allocate physical memory for the kernel
    let kernel_physical_base = match system_table()
        .boot_services
        .allocate_frames(MemoryType::PebbleKernelMemory, kernel_size / FRAME_SIZE)
    {
        Ok(address) => address,
        Err(err) => panic!("Failed to allocate physical memory for kernel: {:?}", err),
    };

    // We now zero all the kernel memory
    unsafe {
        system_table().boot_services.set_mem(
            u64::from(kernel_physical_base) as *mut _,
            kernel_size as usize,
            0,
        );
    }

    /*
     * We now copy the sections from the ELF image into memory, after which we can free the kernel
     * ELF. We use sections instead of segments (as are traditionally used when loading a program)
     * because sections allow us to define permissions for pages much more accurately. When mapping
     * by program headers, we often end up with an executable `.data`, or a writable `.rodata`,
     * which is less safe.
     */
    let mut physical_address = kernel_physical_base;

    for section in elf.sections() {
        // Skip sections that shouldn't be loaded or ones with no data
        if !section.is_allocated() || section.size == 0 {
            continue;
        }

        println!(
            "Loading section: '{}' from {:#x}-{:#x}",
            section.name(&elf).unwrap(),
            section.address,
            section.address + section.size - 1
        );

        map_section(mapper, physical_address, &section, allocator);

        /*
         * For ProgBits sections, we need to copy the data from the image into the section. For
         * NoBits sections, we can leave it as initialised 0s.
         */
        if let SectionType::ProgBits = section.section_type() {
            unsafe {
                slice::from_raw_parts_mut(
                    u64::from(physical_address) as *mut u8,
                    section.size as usize,
                )
                .copy_from_slice(section.data(&elf).unwrap());
            }
        }

        physical_address = (physical_address + section.size).unwrap();
    }

    /*
     * We now set up the kernel stack. As part of the `.bss` section, it has already had memory
     * allocated for it, and has been mapped into the page tables. However, we need to go back and
     * unmap the guard page, and extract the address of the top of the stack.
     */
    let guard_page_address = match elf
        .symbols()
        .find(|symbol| symbol.name(&elf) == Some("_guard_page"))
    {
        Some(symbol) => VirtualAddress::new(symbol.value).unwrap(),
        None => panic!("Kernel does not have a '_guard_page' symbol!"),
    };
    assert!(
        guard_page_address.is_page_aligned(),
        "Guard page address is not page-aligned"
    );
    println!("Unmapping guard page");
    mapper.unmap(Page::contains(guard_page_address), allocator);

    let stack_top = match elf
        .symbols()
        .find(|symbol| symbol.name(&elf) == Some("_stack_top"))
    {
        Some(symbol) => VirtualAddress::new(symbol.value).unwrap(),
        None => panic!("Kernel does not have a '_stack_top' symbol"),
    };
    assert!(stack_top.is_page_aligned(), "Stack is not page aligned");

    /*
     * Big Scary Transmute™: we turn a virtual address into a function pointer which can be called
     * from Rust. This is safe if:
     *     - The kernel defines the entry point correctly
     *     - We have loaded the kernel ELF correctly
     *     - The correct virtual mappings are installed
     */
    Ok(KernelInfo {
        entry_point: VirtualAddress::new(elf.entry_point() as u64).unwrap(),
        stack_top,
    })
}

fn map_section(
    mapper: &mut Mapper<IdentityMapping>,
    physical_base: PhysicalAddress,
    section: &SectionHeader,
    allocator: &BootFrameAllocator,
) {
    let virtual_address = VirtualAddress::new(section.address).unwrap();
    /*
     * XXX: This is a tad hacky, but because the addresses should be page-aligned, the half-open
     * ranges `[physical_base, physical_base + size)` and `[virtual_address, virtual_address +
     * size)` gives us the correct frame and page ranges.
     */
    let frames =
        Frame::contains(physical_base)..Frame::contains((physical_base + section.size).unwrap());
    let pages =
        Page::contains(virtual_address)..Page::contains((virtual_address + section.size).unwrap());
    assert!(frames.clone().count() == pages.clone().count());

    /*
     * Work out the most restrictive set of permissions this section can be mapped with. If the
     * section needs to be writable, mark the pages as writable. If the section does **not**
     * contain executable instructions, mark it as `NO_EXECUTE`.
     */
    let mut flags = EntryFlags::PRESENT;
    if section.is_writable() {
        flags |= EntryFlags::WRITABLE;
    }
    if !section.is_executable() {
        flags |= EntryFlags::NO_EXECUTE;
    }

    for (frame, page) in frames.zip(pages) {
        mapper.map_to(page, frame, flags, allocator);
    }
}

fn read_file(volume_label: &str, path: &str, image_handle: Handle) -> Result<Pool<[u8]>, Status> {
    let volume_root = system_table()
        .boot_services
        .locate_handle(SearchType::ByProtocol, Some(SimpleFileSystem::guid()), None)?
        .iter()
        .filter_map(|handle| {
            system_table()
                .boot_services
                .open_protocol::<SimpleFileSystem>(
                    *handle,
                    image_handle,
                    0,
                    OpenProtocolAttributes::BY_HANDLE_PROTOCOL,
                )
                .and_then(|volume| volume.open_volume())
                .ok()
        })
        .find(|root| {
            root.get_info::<FileSystemInfo>()
                .and_then(|info| info.volume_label())
                .map(|label| label == volume_label)
                .unwrap_or(false)
        })
        .ok_or(Status::NotFound)?;

    let path = boot_services::str_to_utf16(path)?;
    let file = volume_root.open(&path, FileMode::READ, FileAttributes::empty())?;

    let file_size = file.get_info::<FileInfo>()?.file_size as usize;
    let mut file_buf = system_table()
        .boot_services
        .allocate_slice::<u8>(file_size)?;

    let _ = file.read(&mut file_buf)?;
    Ok(file_buf)
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
