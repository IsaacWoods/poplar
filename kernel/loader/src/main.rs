#![no_std]
#![no_main]

extern crate alloc;

#[macro_use]
mod util;
mod elf;

use crate::elf::{Elf, ProgramHeaderFlags};
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use core::{
    arch::naked_asm,
    mem,
    num::NonZero,
    ptr::{self, NonNull},
    slice,
};
use hal::{
    bootinfo,
    cmdline::Cmdline,
    mem::{MemFlags, PAddr, PageTable, PageTableAllocator, VAddr},
};
use uefi::{
    CString16,
    boot::{AllocateType, MemoryType},
    fs::Path,
    mem::memory_map::{MemoryMap, MemoryMapMut, MemoryMapOwned},
    prelude::*,
    table::cfg::ConfigTableEntry,
};

#[unsafe(naked)]
unsafe extern "C" fn trampoline(
    boot_info: VAddr,
    page_tables: PAddr,
    stack_ptr: VAddr,
    entry_point: VAddr,
) -> ! {
    /*
     * XXX: Parameters are passed following the UEFI x86_64 calling convention.
     * Integer params are in `rcx`, `rdx`, `r8`, `r9`.
     */
    unsafe {
        naked_asm!(
            "cli",
            "mov cr3, rdx",
            "xor rbp, rbp",
            "mov rsp, r8",
            "mov rdi, rcx",
            "jmp r9",
        )
    }
}

#[entry]
fn main() -> Status {
    println!("Booting Poplar - loader v{}", env!("CARGO_PKG_VERSION"));
    println!(
        "Firmware: {} (revision {}; implementing UEFI {})",
        uefi::system::firmware_vendor(),
        uefi::system::firmware_revision(),
        uefi::system::uefi_revision()
    );

    let mut loader_fs =
        uefi::fs::FileSystem::new(boot::get_image_file_system(boot::image_handle()).unwrap());

    let cmdline_str =
        match loader_fs.read_to_string(Path::new(&CString16::try_from("cmdline").unwrap())) {
            Ok(str) => str,
            Err(err) => {
                println!("Error reading cmdline from disk: {}. Using default.", err);
                "".to_string()
            }
        };
    let cmdline_str = cmdline_str.trim();
    let cmdline = Cmdline::new(&cmdline_str);

    let mut kernel_page_table = PageTable::new(&BSPageTableAllocator, VAddr::new(0x0));
    let (kernel, mut next_available_address) = load_kernel(&mut loader_fs, &mut kernel_page_table);

    let mut boot_info_area =
        unsafe { BootInfoArea::new(next_available_address, &mut kernel_page_table) };
    let boot_info_kernel_addr = next_available_address;
    next_available_address += BootInfoArea::MAX_SIZE;

    let mut string_table = BootInfoStringTable::new();
    let cmdline_offset = string_table.add_string(&cmdline_str);

    let (string_table_offset, string_table_length) =
        boot_info_area.write_string_table(string_table);

    /*
     * Identity map the trampoline.
     */
    let trampoline_ptr = trampoline as *const extern "C" fn(VAddr, PAddr, VAddr, VAddr) -> !;
    kernel_page_table
        .map_one(
            VAddr::new(trampoline_ptr as usize).align_down(PageTable::PAGE_SIZE_4KIB),
            PAddr::new(trampoline_ptr as usize).align_down(PageTable::PAGE_SIZE_4KIB),
            PageTable::PAGE_SIZE_4KIB,
            MemFlags {
                executable: true,
                ..Default::default()
            },
            &BSPageTableAllocator,
        )
        .unwrap();

    /*
     * Map the higher-half physical map.
     * TODO: currently, this just maps the first 4GiB of physical memory. We could map all usable
     * RAM + ACPI structures by iterating the memory-map, but we don't track memory allocated by us
     * (which will appear as `Reserved`) well enough to do this yet.
     */
    println!(
        "Constructing HHDM from 0x0..{:#x} at {:#x}..{:#x}",
        hal::mem::gib(4),
        hal::mem::kernel_map::PHYSICAL_MAPPING_BASE,
        hal::mem::kernel_map::PHYSICAL_MAPPING_BASE + hal::mem::gib(4)
    );
    for i in 0..4 {
        let addr = i * hal::mem::gib(1);
        kernel_page_table
            .map(
                hal::mem::kernel_map::PHYSICAL_MAPPING_BASE + addr,
                PAddr::new(addr),
                PageTable::PAGE_SIZE_1GIB,
                MemFlags {
                    writable: true,
                    ..Default::default()
                },
                &BSPageTableAllocator,
            )
            .unwrap();
    }

    /*
     * Exit boot services. From this point, we must be careful not to allocate.
     */
    let mut memory_map = unsafe { boot::exit_boot_services(None) };
    memory_map.sort();
    let (mem_map_offset, mem_map_length) =
        process_memory_map(&mut boot_info_area, memory_map, &cmdline);

    boot_info_area.write_header(bootinfo::Header {
        magic: bootinfo::MAGIC,
        mem_map_offset,
        mem_map_length,
        kernel_free_start: usize::from(next_available_address) as u64,
        rsdp_address: find_rsdp()
            .map(|addr| usize::from(addr) as u64)
            .unwrap_or(0),
        cmdline_offset,
        cmdline_len: cmdline_str.len() as u16,
        loaded_images_offset: 0,
        num_loaded_images: 0,
        string_table_offset,
        string_table_length,
        video_mode_offset: 0,
        _reserved0: [0; _],
    });

    unsafe {
        trampoline(
            boot_info_kernel_addr,
            kernel_page_table.p4,
            kernel.stack_ptr,
            kernel.entry_point,
        )
    }
}

/// Takes the final UEFI memory map and processes it into a form suitable to pass to the kernel.
/// Returns the `(offset, length)` of the emitted memory map in the `BootInfoArea`.
fn process_memory_map(
    boot_info_area: &mut BootInfoArea,
    memory_map: MemoryMapOwned,
    cmdline: &Cmdline,
) -> (u16, u16) {
    let offset = boot_info_area.offset();
    let mut length = 0;

    let debug = cmdline.get("loader.debug_memmap").is_some();

    if debug {
        println!("UEFI memory map:");
    }
    for entry in memory_map.entries() {
        if debug {
            let ty_str = match entry.ty {
                MemoryType::RESERVED => "RESERVED",
                MemoryType::LOADER_CODE => "LOADER_CODE",
                MemoryType::LOADER_DATA => "LOADER_DATA",
                MemoryType::BOOT_SERVICES_CODE => "BOOT_SERVICES_CODE",
                MemoryType::BOOT_SERVICES_DATA => "BOOT_SERVICES_DATA",
                MemoryType::RUNTIME_SERVICES_CODE => "RUNTIME_SERVICES_CODE",
                MemoryType::RUNTIME_SERVICES_DATA => "RUNTIME_SERVICES_DATA",
                MemoryType::CONVENTIONAL => "CONVENTIONAL",
                MemoryType::UNUSABLE => "UNUSABLE",
                MemoryType::ACPI_RECLAIM => "ACPI_RECLAIM",
                MemoryType::ACPI_NON_VOLATILE => "ACPI_NON_VOLATILE",
                MemoryType::MMIO => "MMIO",
                MemoryType::MMIO_PORT_SPACE => "MMIO_PORT_SPACE",
                MemoryType::PAL_CODE => "PAL_CODE",
                MemoryType::PERSISTENT_MEMORY => "PERSISTENT_MEMORY",
                MemoryType::UNACCEPTED => "UNACCEPTED",
                _ => "????",
            };
            println!(
                "    {:<30} {:016x} .. {:016x}",
                ty_str,
                entry.phys_start,
                entry.phys_start + entry.page_count * PageTable::PAGE_SIZE_4KIB as u64
            );
        }

        let typ = match entry.ty {
            MemoryType::RESERVED => bootinfo::MemoryType::Reserved,

            MemoryType::CONVENTIONAL
            | MemoryType::LOADER_CODE
            | MemoryType::LOADER_DATA
            | MemoryType::BOOT_SERVICES_CODE
            | MemoryType::BOOT_SERVICES_DATA
            | MemoryType::PERSISTENT_MEMORY => bootinfo::MemoryType::Usable,

            MemoryType::RUNTIME_SERVICES_CODE | MemoryType::RUNTIME_SERVICES_DATA => {
                bootinfo::MemoryType::UefiRuntimeServices
            }

            MemoryType::ACPI_RECLAIM => bootinfo::MemoryType::AcpiReclaimable,
            MemoryType::ACPI_NON_VOLATILE => bootinfo::MemoryType::AcpiNvs,

            _ => bootinfo::MemoryType::Reserved,
        };

        unsafe {
            boot_info_area.write(bootinfo::MemoryEntry {
                base: entry.phys_start,
                length: entry.page_count * PageTable::PAGE_SIZE_4KIB as u64,
                typ,
                _reserved: 0,
            });
        }
        length += 1;
    }

    let memory_map = unsafe {
        slice::from_raw_parts_mut(
            boot_info_area.boot_info_ptr.byte_add(offset as usize) as *mut bootinfo::MemoryEntry,
            length,
        )
    };

    const SCRATCH: bootinfo::MemoryEntry = bootinfo::MemoryEntry {
        base: 0,
        length: 0,
        typ: bootinfo::MemoryType::Scratch,
        _reserved: 0,
    };

    // Remove zero-length entries
    for i in 0..length {
        if memory_map[i].length == 0 {
            memory_map[i] = SCRATCH;
            continue;
        }
    }

    // Check for overlapping regions (never trust UEFI firmwares)
    for i in 0..(length - 1) {
        let entry = memory_map[i];
        if (entry.base + entry.length) > memory_map[i + 1].base {
            println!(
                "Error: memory map contains overlapping regions! Entry of type {:?} at {:#x}..{:#x} overlaps entry at {:#x}",
                entry.typ,
                entry.base,
                entry.base + entry.length,
                memory_map[i + 1].base
            );
            // TODO: if one region is reserved/runtime services/etc, we could trim the overlapping region out of the other entry to make it safer?
        }
    }

    /*
     * Merge contiguous entries of the same time. This is common as many of the entries in the UEFI
     * map can be collapsed into a single usable entry.
     * XXX: We merge into the *second* entry of the memory map, so that the new entry will be
     * considered for further merging on the next iteration. If the next entry is a scratch entry,
     * we swap them, to allow the same.
     */
    for i in 0..(length - 1) {
        let a = memory_map[i];
        let b = memory_map[i + 1];

        if b.typ == bootinfo::MemoryType::Scratch {
            memory_map[i + i] = a;
            memory_map[i] = SCRATCH;
            continue;
        }

        if a.typ == b.typ && (a.base + a.length) == b.base {
            memory_map[i + 1] = bootinfo::MemoryEntry {
                base: a.base,
                length: a.length + b.length,
                typ: a.typ,
                _reserved: 0,
            };
            memory_map[i] = SCRATCH;
        }
    }

    /*
     * TODO: it may be useful for the kernel to know where things we've allocated are (to e.g.
     * deallocate the memory used by a loaded module that is no longer needed). At the
     * moment these allocations will be `Reserved` memory. We could keep a separate list and
     * extract them from the memory map to mark them separately - at worst, this would split a
     * `Reserved` entry into 3 new entries, requiring 2 free `Scratch` entries and a re-sort by
     * address at the end (if necessary).
     */

    if debug {
        println!("Final memory map:");
        for entry in memory_map {
            if entry.typ == bootinfo::MemoryType::Scratch {
                continue;
            }
            println!(
                "    {:<30?} {:016x} .. {:016x}",
                entry.typ,
                entry.base,
                entry.base + entry.length
            );
        }
    }

    (offset, length as u16)
}

struct LoadedKernel {
    entry_point: VAddr,
    stack_ptr: VAddr,
}

fn load_kernel(
    loader_fs: &mut uefi::fs::FileSystem,
    page_table: &mut PageTable,
) -> (LoadedKernel, VAddr) {
    use elf::SegmentType;

    let mut next_available_address = hal::mem::kernel_map::KERNEL_IMAGE_BASE;

    let image = loader_fs
        .read(Path::new(&CString16::try_from("kernel.elf").unwrap()))
        .unwrap();
    let image = Elf::new(&image);
    image.validate().unwrap();

    for ph in image.segments() {
        if ph.typ() == SegmentType::Load && ph.mem_size > 0 {
            let file_size = ph.file_size as usize;
            let mem_size = (ph.mem_size as usize).next_multiple_of(PageTable::PAGE_SIZE_4KIB);
            let paddr = boot::allocate_pages(
                AllocateType::AnyPages,
                MemoryType::RESERVED,
                mem_size / PageTable::PAGE_SIZE_4KIB,
            )
            .unwrap()
            .as_ptr() as *mut u8;

            unsafe {
                ptr::copy_nonoverlapping(
                    image.bytes.as_ptr().byte_add(ph.offset as usize),
                    paddr,
                    file_size,
                );
                ptr::write_bytes(paddr.byte_add(file_size), 0, mem_size - file_size);
            }

            page_table
                .map(
                    VAddr::new(ph.vaddr as usize),
                    PAddr::new(paddr as usize),
                    mem_size,
                    MemFlags {
                        writable: ph.flags.get(ProgramHeaderFlags::WRITABLE),
                        executable: ph.flags.get(ProgramHeaderFlags::EXECUTABLE),
                        user_accessible: false,
                        cached: true,
                    },
                    &BSPageTableAllocator,
                )
                .unwrap();
            next_available_address = VAddr::new(usize::max(
                usize::from(next_available_address),
                ph.vaddr as usize + mem_size,
            ));
        }
    }

    // Create a bootstrap stack for the kernel to start with, directly after the kernel image
    const BOOTSTRAP_STACK_SIZE: usize = hal::mem::kib(128);
    let bootstrap_stack_start = next_available_address;
    next_available_address += BOOTSTRAP_STACK_SIZE;
    let bootstrap_stack_ptr = (next_available_address - 1).align_down(16);
    next_available_address += PageTable::PAGE_SIZE_4KIB; // Guard page
    let stack_paddr = PAddr::new(
        boot::allocate_pages(
            AllocateType::AnyPages,
            MemoryType::RESERVED,
            BOOTSTRAP_STACK_SIZE / PageTable::PAGE_SIZE_4KIB,
        )
        .unwrap()
        .as_ptr() as usize,
    );
    page_table
        .map(
            bootstrap_stack_start,
            stack_paddr,
            BOOTSTRAP_STACK_SIZE,
            MemFlags {
                writable: true,
                ..Default::default()
            },
            &BSPageTableAllocator,
        )
        .unwrap();

    (
        LoadedKernel {
            entry_point: VAddr::new(image.header().entry_point as usize),
            stack_ptr: bootstrap_stack_ptr,
        },
        next_available_address,
    )
}

struct BSPageTableAllocator;

impl PageTableAllocator for BSPageTableAllocator {
    fn alloc(&self) -> PAddr {
        let paddr = boot::allocate_pages(AllocateType::AnyPages, MemoryType::RESERVED, 1).unwrap();
        unsafe {
            ptr::write_bytes(paddr.as_ptr(), 0, PageTable::PAGE_SIZE_4KIB);
        }
        PAddr::new(paddr.as_ptr() as usize)
    }

    fn free(&self, frame: PAddr) {
        unsafe {
            boot::free_pages(
                NonNull::without_provenance(NonZero::new(usize::from(frame)).unwrap()),
                1,
            )
            .unwrap();
        }
    }
}

pub struct BootInfoArea {
    boot_info_ptr: *mut u8,
    cursor: *mut u8,
}

impl BootInfoArea {
    const MAX_SIZE: usize = 4 * PageTable::PAGE_SIZE_4KIB;

    pub unsafe fn new(map_at: VAddr, kernel_page_table: &mut PageTable) -> BootInfoArea {
        let boot_info_phys = PAddr::new(
            boot::allocate_pages(
                AllocateType::AnyPages,
                MemoryType::RESERVED,
                Self::MAX_SIZE / PageTable::PAGE_SIZE_4KIB,
            )
            .unwrap()
            .as_ptr() as usize,
        );
        kernel_page_table
            .map(
                map_at,
                boot_info_phys,
                Self::MAX_SIZE,
                MemFlags {
                    writable: true,
                    ..Default::default()
                },
                &BSPageTableAllocator,
            )
            .unwrap();
        let ptr = usize::from(boot_info_phys) as *mut u8;
        let cursor = unsafe { ptr.byte_add(mem::size_of::<bootinfo::Header>()) };
        BootInfoArea {
            boot_info_ptr: ptr,
            cursor,
        }
    }

    /// Get the offset of the current `cursor` into the bootinfo area
    pub fn offset(&self) -> u16 {
        (self.cursor.addr() - self.boot_info_ptr.addr()) as u16
    }

    /// Reserve `count` bytes of boot info space, returning a pointer to the start of the reserved space.
    /// Will ensure the final `cursor` has an alignment of at-least `align`.
    pub fn advance(&mut self, count: usize, align: usize) -> *mut u8 {
        let ptr = self.cursor;
        assert!(unsafe {
            self.cursor.byte_add(count) < self.boot_info_ptr.byte_add(Self::MAX_SIZE)
        });

        self.cursor = unsafe { self.cursor.byte_add(count) };
        self.cursor = unsafe { self.cursor.byte_add(self.cursor.align_offset(align)) };

        ptr
    }

    /// Write `value` at `cursor`, and advance `cursor` by the size of `T`.
    pub unsafe fn write<T>(&mut self, value: T) {
        assert!(unsafe {
            self.cursor.byte_add(mem::size_of::<T>()) < self.boot_info_ptr.byte_add(Self::MAX_SIZE)
        });
        unsafe {
            ptr::write(self.cursor as *mut T, value);
            self.cursor = self.cursor.byte_add(mem::size_of::<T>());
        }
    }

    pub fn write_header(&mut self, header: bootinfo::Header) {
        unsafe {
            ptr::write(self.boot_info_ptr as *mut bootinfo::Header, header);
        }
    }

    /// Writes out the string table. Returns the `(offset, length)` of the table.
    pub fn write_string_table(&mut self, string_table: BootInfoStringTable) -> (u16, u16) {
        let offset = self.offset();
        let mut ptr = self.advance(string_table.table_len as usize, 8);
        unsafe {
            for (_offset, s) in string_table.entries {
                ptr::copy(s.as_ptr(), ptr, s.len());
                ptr = ptr.byte_add(s.len());
            }
        }

        (offset, string_table.table_len)
    }
}

pub struct BootInfoStringTable {
    pub entries: Vec<(u16, String)>,
    pub table_len: u16,
}

impl BootInfoStringTable {
    pub fn new() -> BootInfoStringTable {
        BootInfoStringTable {
            entries: Vec::new(),
            table_len: 0,
        }
    }

    pub fn add_string(&mut self, s: &str) -> u16 {
        let offset = self.table_len;
        self.table_len += s.len() as u16;
        self.entries.push((offset, s.to_string()));
        offset
    }
}

fn find_rsdp() -> Option<PAddr> {
    system::with_config_table(|entries| {
        /*
         * Search the config table for an entry containing the address of the RSDP. First, search the whole table for
         * a v2 RSDP, then if we don't find one, look for a v1 one.
         */
        entries
            .iter()
            .find(|entry| matches!(entry.guid, ConfigTableEntry::ACPI2_GUID))
            .or_else(|| {
                entries
                    .iter()
                    .find(|entry| matches!(entry.guid, ConfigTableEntry::ACPI_GUID))
            })
            .map(|entry| PAddr::new(entry.address as usize))
    })
}
