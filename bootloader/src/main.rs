#![feature(
    const_fn,
    lang_items,
    ptr_internals,
    decl_macro,
    panic_info_message,
    asm,
    never_type
)]
#![no_std]
#![no_main]

#[macro_use]
extern crate bitflags;

mod boot_services;
mod memory;
mod protocols;
mod runtime_services;
mod system_table;
mod types;

use core::fmt::Write;
use core::mem;
use core::panic::PanicInfo;
use core::slice;
use crate::boot_services::{AllocateType, OpenProtocolAttributes, Pool, Protocol, SearchType};
use crate::memory::{BootFrameAllocator, MemoryType};
use crate::protocols::{FileAttributes, FileInfo, FileMode, FileSystemInfo, SimpleFileSystem};
use crate::system_table::SystemTable;
use crate::types::{Handle, Status};
use x86_64::boot::BootInformation;
use x86_64::memory::paging::entry::EntryFlags;
use x86_64::memory::paging::table::IdentityMapping;
use x86_64::memory::paging::{Frame, InactivePageTable, Mapper, Page, FRAME_SIZE};
use x86_64::memory::{PhysicalAddress, VirtualAddress};
use xmas_elf::{
    sections::{ShType, SHF_ALLOC, SHF_EXECINSTR, SHF_WRITE},
    ElfFile,
};

static mut SYSTEM_TABLE: *const SystemTable = 0 as *const _;
static mut IMAGE_HANDLE: Handle = 0;

/// Returns a reference to the `SystemTable`. This is safe to call after the global has been
/// initialised, which we do straight after control is passed to us.
pub fn system_table() -> &'static SystemTable {
    unsafe { &*SYSTEM_TABLE }
}

pub fn image_handle() -> Handle {
    unsafe { IMAGE_HANDLE }
}

macro print {
    ($($arg: tt)*) => {
        // TODO: for some reason, this still doesn't make the trait visible. Maybe an issue with
        // this nightly?
        // use core::fmt::Write;
        (&*system_table().console_out).write_fmt(format_args!($($arg)*)).expect("Failed to write to console");
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

#[no_mangle]
pub extern "win64" fn uefi_main(image_handle: Handle, system_table: &'static SystemTable) -> ! {
    /*
     * The first thing we do is set the global references to the system table and image handle. Until we do this,
     * their "safe" getters are not.
     */
    unsafe {
        SYSTEM_TABLE = system_table;
        IMAGE_HANDLE = image_handle;
    }

    println!("Hello UEFI!");

    let mut page_table = create_page_table();
    let mut mapper = page_table.mapper();

    let kernel_entry = match load_kernel(image_handle, &mut mapper) {
        Ok(entry_point) => entry_point,
        Err(err) => panic!("Failed to load kernel: {:?}", err),
    };

    let memory_map = match system_table.boot_services.get_memory_map() {
        Ok(map) => map,
        Err(err) => panic!("Failed to get memory map: {:?}", err),
    };

    /*
     * We now terminate the boot services. If this is successful, we become responsible for the
     * running of the system and may no longer make use of any boot services, including the console
     * protocols.
     */
    system_table
        .boot_services
        .exit_boot_services(image_handle, memory_map.key)
        .unwrap();
    loop {}
}

fn create_page_table() -> InactivePageTable<IdentityMapping> {
    // Allocate a frame for the P4
    let mut address = PhysicalAddress::default();
    match system_table().boot_services.allocate_pages(
        AllocateType::AllocateAnyPages,
        MemoryType::PebblePageTables,
        1,
        &mut address,
    ) {
        Ok(()) => {}
        Err(err) => panic!(
            "Failed to allocate physical memory for page tables: {:?}",
            err
        ),
    }

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
) -> Result<fn(&BootInformation) -> !, Status> {
    let file_data = match read_file("BOOT", "kernel.elf", image_handle) {
        Ok(data) => data,
        Err(err) => panic!("Failed to read kernel ELF from disk: {:?}", err),
    };

    let elf = match ElfFile::new(&file_data) {
        Ok(elf) => elf,
        Err(err) => panic!("Failed to parse kernel ELF: {}", err),
    };

    // Work out how much space we need for the kernel and check it's a multiple of the page size
    let kernel_size = elf.section_iter().fold(0, |kernel_size, section| {
        // If the section should be allocated, include its size
        if section.flags() & SHF_ALLOC != 0 {
            kernel_size + section.size()
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
    let mut kernel_physical_base = PhysicalAddress::default();
    match system_table().boot_services.allocate_pages(
        AllocateType::AllocateAnyPages,
        MemoryType::PebbleKernelMemory,
        (kernel_size / FRAME_SIZE) as usize,
        &mut kernel_physical_base,
    ) {
        Ok(()) => {}
        Err(err) => panic!("Failed to allocate physical memory for kernel: {:?}", err),
    }
    println!(
        "Allocated physical memory for kernel at {:?}, kernel_size = {:#x}",
        kernel_physical_base, kernel_size
    );

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

    for section in elf.section_iter() {
        // Skip sections that shouldn't be loaded or ones with no data
        if section.flags() & SHF_ALLOC == 0 || section.size() == 0 {
            continue;
        }

        println!(
            "Loading section: '{}' from {:#x}-{:#x}",
            section.get_name(&elf).unwrap(),
            section.address(),
            section.address() + section.size() - 1
        );

        match section.get_type() {
            Ok(ShType::ProgBits) => {
                // Map the section to its correct virtual address in the kernel page tables
                map_section(
                    mapper,
                    physical_address,
                    VirtualAddress::new(section.address()).unwrap(),
                    section.size(),
                    section.flags(),
                );

                // Copy the section from the image into its new home
                unsafe {
                    slice::from_raw_parts_mut(
                        u64::from(physical_address) as *mut u8,
                        section.size() as usize,
                    )
                }
                .copy_from_slice(section.raw_data(&elf));

                physical_address = (physical_address + section.size()).unwrap();
            }

            Ok(ShType::NoBits) => {
                // Map the section into the kernel page tables
                map_section(
                    mapper,
                    physical_address,
                    VirtualAddress::new(section.address()).unwrap(),
                    section.size(),
                    section.flags(),
                );

                /*
                 * For `NoBits` sections such as `.bss`, we need to map the pages into the kernel page
                 * tables, but don't need to actually copy any data into memory, as its already zerod
                 * from above.
                 */
                physical_address = (physical_address + section.size()).unwrap();
            }

            Ok(_) => (),
            Err(err) => panic!("Failed to parse section header type: {}", err),
        }
    }

    // Big Scary Transmute™: we turn a virtual address into a function pointer which can be called
    // from Rust. This is safe if:
    //     * The kernel defines the entry point correctly
    //     * We have loaded the kernel ELF correctly
    //     * The correct virtual mappings are installed
    Ok(unsafe { mem::transmute(elf.header.pt2.entry_point()) })
}

fn map_section(
    mapper: &mut Mapper<IdentityMapping>,
    physical_base: PhysicalAddress,
    virtual_address: VirtualAddress,
    section_size: u64,
    elf_flags: u64,
) {
    /*
     * XXX: This is a tad hacky, but because the addresses should be page-aligned, the half-open
     * ranges `[physical_base, physical_base + size)` and `[virtual_address, virtual_address +
     * size)` gives us the correct frame and page ranges.
     */
    let frames =
        Frame::contains(physical_base)..Frame::contains((physical_base + section_size).unwrap());
    let pages =
        Page::contains(virtual_address)..Page::contains((virtual_address + section_size).unwrap());
    assert!(frames.clone().count() == pages.clone().count());

    /*
     * Work out the most restrictive set of permissions this section can be mapped with. If the
     * section needs to be writable, mark the pages as writable. If the section does **not**
     * contain executable instructions, mark it as `NO_EXECUTE`.
     */
    let mut flags = EntryFlags::PRESENT;
    if elf_flags & SHF_WRITE != 0 {
        flags |= EntryFlags::WRITABLE;
    }
    if elf_flags & SHF_EXECINSTR == 0 {
        flags |= EntryFlags::NO_EXECUTE;
    }

    for (frame, page) in frames.zip(pages) {
        mapper.map_to(page, frame, flags, &BootFrameAllocator);
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
