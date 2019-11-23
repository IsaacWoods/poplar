//! These constants centralize the layout of the virtual address space on x86_64. The 511th P4
//! entry (covering addresses `0xffff_ff80_0000_0000` through to `0xffff_ffff_ffff_ffff`) is always
//! mapped to the kernel P3 (which includes the physical memory mappings). The rest of the address
//! space (addresses `0x0000_0000_0000_0000` through to `0xffff_ff7f_ffff_ffff`) are free for
//! userspace to use.
//!
//! This gives us 512 GiB of kernel space. The kernel itself is build with the `kernel` mc-model
//! and so must run in the -2 GiB of the address space (this is the top two entries of the P3).
//! The remaining 510 GiB are used for mapping the entirity of physical memory into the virtual
//! address space, and for each task's kernel stack.
//!
//! 128 GiB are reserved for kernel stacks for tasks, with a maximum of a total of 65536 tasks
//! 1024 address spaces. This lies directly below `KERNEL_BASE`.
//!
//! This leaves 382 GiB for the physical memory map, which should be more than enough for any
//! reasonable system we can imagine running on.

use super::{PhysicalAddress, VirtualAddress, MEBIBYTES_TO_BYTES};

/// The size of a single kernel stack.
pub const STACK_SLOT_SIZE: usize = 2 * MEBIBYTES_TO_BYTES;
pub const MAX_TASKS_PER_ADDRESS_SPACE: usize = 64;
pub const MAX_ADDRESS_SPACES: usize = 1024;

/// The size of a slot of stacks that is allocated for a single address space to use.
pub const ADDRESS_SPACE_STACK_SLOT_SIZE: usize = STACK_SLOT_SIZE * MAX_TASKS_PER_ADDRESS_SPACE;

pub fn kernel_stack_area_base(index: usize) -> VirtualAddress {
    KERNEL_STACKS_BASE + index * ADDRESS_SPACE_STACK_SLOT_SIZE
}

/// Access a given `PhysicalAddress` using the physical memory mapping in the kernel address space.
/// Only works within the kernel - cannot be used by the bootloader, and the addresses can't be
/// given to userspace.
pub fn physical_to_virtual(address: PhysicalAddress) -> VirtualAddress {
    PHYSICAL_MAPPING_BASE + usize::from(address)
}

/// The kernel is mapped into the 511th entry of the P4.
pub const KERNEL_P4_ENTRY: usize = 511;

pub const KERNEL_ADDRESS_SPACE_START: VirtualAddress = VirtualAddress::new_canonicalise(0xffff_ff80_0000_0000);

/// The base virtual address of the physical memory mapping. This is equal to
/// `KERNEL_ADDRESS_SPACE_START` because we map the physical memory at the start of the kernel's P4
/// entry.
pub const PHYSICAL_MAPPING_BASE: VirtualAddress = KERNEL_ADDRESS_SPACE_START;

pub const KERNEL_STACKS_BASE: VirtualAddress = VirtualAddress::new_canonicalise(0xffff_ffdf_8000_0000);

/// This is the base of the kernel address space. It starts at -2GB. We don't know how much memory
/// the kernel image will take up when loaded into memory, so we leave quite a lot of space until
/// the next statically mapped thing.
pub const KERNEL_BASE: VirtualAddress = VirtualAddress::new_canonicalise(0xffff_ffff_8000_0000);

/// The start of the heap. The heap is 200 KiB.
pub const HEAP_START: VirtualAddress = VirtualAddress::new_canonicalise(0xffff_ffff_c00_00000);
pub const HEAP_END: VirtualAddress = VirtualAddress::new_canonicalise(0xffff_ffff_c00_31fff);

/*
 * From here, we place a bunch of hard-coded pages for various things.
 */
/// While we could access the local APIC from the physical mapping, it's easier to just map it to a
/// fixed virtual address, so we can always access its config space. This allows us to use
/// `LocalApic` as a singleton, so we can easily access it from interrupt handlers.
pub const LOCAL_APIC_CONFIG: VirtualAddress = VirtualAddress::new_canonicalise(0xffff_ffff_d000_0000);

/// This is the number of 4KiB pages we need to allocate for the boot info. We also need to respect this
/// in the virtual address space allocation.
pub const BOOT_INFO_NUM_PAGES: usize = 2;
pub const BOOT_INFO: VirtualAddress = unsafe { VirtualAddress::new_unchecked(0xffff_ffff_d000_1000) };
// XXX: the next item must not start at 0xffff_ffff_d000_2000 - make sure to leave BOOT_INFO_NUM_PAGES pages
