//! These constants define the layout of the memory map when the bootloader passes control to the
//! kernel. We split the virtual address space into two regions - the kernel address space between
//! `0xffff_ffff_8000_0000` and `0xffff_ffff_ffff_ffff`, and the userspace address space between
//! `0x0000_0000_0000_0000` and `0xffff_efff_ffff_ffff`. These are non-contiguous because the 510th
//! entry of the PML4 is recursively mapped so we can access the page tables.

use super::VirtualAddress;

/// We use the 510th entry of the PML4 (P4) to access the page tables easily using the recursive
/// paging trick. Any address that would use this entry can therefore not be used. This entry was
/// picked because it places the unusable portion of the virtual address space between the
/// userspace and kernel portions, which is less inconvienient than it being a hole.
pub const RECURSIVE_ENTRY: u16 = 510;

/// The kernel is mapped into the 511th entry of the P4.
pub const KERNEL_P4_ENTRY: u16 = 511;

/// This address can be used to access the **currently mapped** P4 table, assuming the correct entry
/// is recursively mapped properly.
pub const P4_TABLE_RECURSIVE_ADDRESS: VirtualAddress = VirtualAddress::from_page_table_offsets(
    RECURSIVE_ENTRY,
    RECURSIVE_ENTRY,
    RECURSIVE_ENTRY,
    RECURSIVE_ENTRY,
    0,
);

/// This is the base of the kernel address space. It starts at -2GB. We don't know how much memory
/// the kernel image will take up when loaded into memory, so we leave quite a lot of space until
/// the next statically mapped thing.
pub const KERNEL_BASE: VirtualAddress =
    unsafe { VirtualAddress::new_unchecked(0xffff_ffff_8000_0000) };

/// This is the address of the start of the area in the kernel address space for random physical
/// mappings. We reserve 32 frames.
pub const PHYSICAL_MAPPING_START: VirtualAddress =
    unsafe { VirtualAddress::new_unchecked(0xffff_ffff_c00_00000) };
pub const PHYSICAL_MAPPING_END: VirtualAddress =
    unsafe { VirtualAddress::new_unchecked(0xffff_ffff_c00_1f000) };

/// The start of the heap. The heap is 200 KiB.
pub const HEAP_START: VirtualAddress =
    unsafe { VirtualAddress::new_unchecked(0xffff_ffff_c00_20000) };
pub const HEAP_END: VirtualAddress =
    unsafe { VirtualAddress::new_unchecked(0xffff_ffff_c00_51fff) };

/*
 * From here, we place a bunch of hard-coded pages for various things, such as the `BootInfo`
 * struct and memory-mapped configuration pages and stuff.
 */
pub const BOOT_INFO: VirtualAddress =
    unsafe { VirtualAddress::new_unchecked(0xffff_ffff_d000_0000) };

/// This address can be used to access the kernel page table's P4 table **all the time**. It does
/// not make use of the recursive mapping, so can be used when we're modifying another set of
/// tables by installing them into the kernel's recursive entry. This mapping is set up by the
/// bootloader.
pub const KERNEL_P4_START: VirtualAddress =
    unsafe { VirtualAddress::new_unchecked(0xffff_ffff_d000_1000) };

/// The virtual address that the configuration page of the local APIC is mapped to. We don't manage
/// this using a simple `PhysicalMapping` because we need to be able to access the local APIC from
/// interrupt handlers, which can't easily access owned `PhysicalMapping`s.
pub const LOCAL_APIC_CONFIG: VirtualAddress =
    unsafe { VirtualAddress::new_unchecked(0xffff_ffff_d000_2000) };
