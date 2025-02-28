use crate::paging::Level3;

pub mod memory {
    use hal::memory::{kibibytes, mebibytes, PAddr};

    pub const DRAM_START: PAddr = PAddr::new(0x4000_0000).unwrap();
    pub const M_FIRMWARE_ADDR: PAddr = DRAM_START;
    // TODO: when const traits are implemented, this should be rewritten in terms of DRAM_START
    pub const SEED_ADDR: PAddr = PAddr::new(0x4000_0000 + kibibytes(512)).unwrap();
    pub const RAMDISK_ADDR: PAddr = PAddr::new(0x4000_0000 + mebibytes(1)).unwrap();
}

pub const VIRTUAL_ADDRESS_BITS: usize = 39;
pub type PageTableImpl = crate::paging::PageTableImpl<Level3>;

/// This module contains constants that define how the kernel address space is laid out on RISC-V,
/// using the Sv39 paging model. The Sv39 model provides us with a 512GiB address space, which is a
/// little more compact than the layout we have on other architectures or with Sv48, but still more
/// than sufficient for the vast majority of platforms.
///
/// For simplicity, we reserve the top half of the address space, from `0xffff_ffc0_0000_0000` to
/// `0xffff_ffff_ffff_ffff`, for the kernel - this makes it easy to distinguish kernel addresses
/// from userspace ones (both visually, and in code by testing a single sign-extended bit).
///
/// All of physical memory is mapped at the base of kernel-space.
///
/// The top 1GiB is reserved for the kernel itself, starting at `0xffff_ffff_c000_0000`.
pub mod kernel_map {
    use hal::memory::{PAddr, VAddr};

    pub const KERNEL_ADDRESS_SPACE_START: VAddr = VAddr::new(0xffff_ffc0_0000_0000);
    pub const PHYSICAL_MAP_BASE: VAddr = KERNEL_ADDRESS_SPACE_START;

    /// Access a given physical address through the physical mapping. This cannot be used until the kernel page tables
    /// have been switched to.
    ///
    /// # Safety
    /// This itself is safe, because to cause memory unsafety a raw pointer must be created and accessed from the
    /// `VAddr`, which is unsafe.
    pub fn physical_to_virtual(address: PAddr) -> VAddr {
        PHYSICAL_MAP_BASE + usize::from(address)
    }

    /// The kernel starts at -1GiB. The kernel image is loaded directly at this address, and the following space until
    /// the top of memory is managed dynamically and contains the boot info structures, memory map, and kernel heap.
    pub const KERNEL_BASE: VAddr = VAddr::new(0xffff_ffff_c000_0000);
}
