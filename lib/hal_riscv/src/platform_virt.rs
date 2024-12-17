use crate::paging::Level4;

pub mod memory {
    use hal::memory::PAddr;

    pub const DRAM_START: PAddr = PAddr::new(0x8000_0000).unwrap();
    pub const OPENSBI_ADDR: PAddr = DRAM_START;
    // TODO: when const traits are implemented, this should be rewritten in terms of DRAM_START
    pub const SEED_ADDR: PAddr = PAddr::new(0x8020_0000).unwrap();
    pub const RAMDISK_ADDR: PAddr = PAddr::new(0xb000_0000).unwrap();
}

pub const VIRTUAL_ADDRESS_BITS: usize = 48;
pub type PageTableImpl = crate::paging::PageTableImpl<Level4>;

/// This module contains constants that define how the kernel address space is laid out on RISC-V
/// using the Sv48 paging model. It is very similar to the layout on `x86_64`, as the structure of
/// the page tables are almost identical on the two architectures.
///
/// The 511th P4 entry (virtual addresses `0xffff_ff80_0000_0000` through `0xffff_ffff_ffff_ffff`)
/// is always mapped to the kernel P3. The rest of the virtual address space (virtual addresses
/// `0x0000_0000_0000_0000` through `0xffff_ff7f_ffff_ffff`) are free for userspace to use.
///
/// This gives us 512 GiB of kernel space. The kernel itself lies within the top 2GiB of the
/// address space (the top two entries of the kernel P3). The remaining 510 GiB of the kernel P3 is
/// used to map the entirety of physical memory into the kernel address space, and for task kernel
/// stacks.
///
/// Directly below the base of the kernel, we reserve 128GiB for task kernel stacks, which gives us
/// a maximum of
/// 65536 tasks if each one has the default stack size.
///
/// This leaves us 382GiB for the physical memory map, which should be sufficient for any system I
/// can imagine us running on (famous last words).
pub mod kernel_map {
    use hal::memory::{mebibytes, Bytes, PAddr, VAddr};

    pub const KERNEL_TABLE_ENTRY: usize = 511;
    pub const KERNEL_ADDRESS_SPACE_START: VAddr = VAddr::new(0xffff_ff80_0000_0000);

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

    pub const KERNEL_STACKS_BASE: VAddr = VAddr::new(0xffff_ffdf_8000_0000);
    /*
     * There is an imposed maximum number of tasks because of the simple way we're allocating task kernel stacks.
     * This is currently 65536 with a task kernel stack size of 2MiB.
     */
    pub const STACK_SLOT_SIZE: Bytes = mebibytes(2);
    pub const MAX_TASKS: usize = 65536;

    /// The kernel starts at -2GiB. The kernel image is loaded directly at this address, and the following space until
    /// the top of memory is managed dynamically and contains the boot info structures, memory map, and kernel heap.
    pub const KERNEL_BASE: VAddr = VAddr::new(0xffff_ffff_8000_0000);
}

pub fn hart_to_plic_context_id(hart_id: usize) -> usize {
    return 1 + 2 * hart_id;
}
