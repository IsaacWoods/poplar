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

/// This address can be used to access the **currently mapped** P4 table, assuming the correct entry
/// is recursively mapped properly.
pub const P4_TABLE_ADDRESS: VirtualAddress = VirtualAddress::from_page_table_offsets(
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

// /// This is the address of the start of the kernel heap.
pub const HEAP_START: VirtualAddress =
    unsafe { VirtualAddress::new_unchecked(0xffff_ffff_c000_0000) };
pub const HEAP_END: VirtualAddress =
    unsafe { VirtualAddress::new_unchecked(0xffff_ffff_cfff_ffff) };

/*
 * Following the heap are a bunch of random memory-mapped configuration spaces and whatnot.
 */
pub const LOCAL_APIC_CONFIG_PAGE: VirtualAddress =
    unsafe { VirtualAddress::new_unchecked(0xffff_ffff_d000_0000) };
