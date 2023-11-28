#![no_std]
#![feature(const_option)]

pub mod hw;
pub mod paging;

pub mod platform_d1;
pub mod platform_virt;

cfg_if::cfg_if! {
    if #[cfg(feature = "platform_rv64_virt")] {
        pub use platform_virt as platform;
    } else if #[cfg(feature = "platform_mq_pro")] {
        pub use platform_d1 as platform;
    } else {
        pub mod platform {
            /*
             * If a platform feature hasn't been selected, we define what's effectively a fake platform
             * module. This documents the desired API a platform should provide, but also provides
             * information to tools such as `rust-analyzer` such as to allow completions without
             * faffing about with fake platform features.
             */
            pub mod memory {
                use hal::memory::PAddr;

                pub const DRAM_START: PAddr = PAddr::new(0x0).unwrap();
                pub const SEED_START: PAddr = PAddr::new(0x0).unwrap();
                pub const RAMDISK_ADDR: PAddr = PAddr::new(0x0).unwrap();
            }

            pub const VIRTUAL_ADDRESS_BITS: usize = 39;
            pub type PageTableImpl = crate::paging::PageTableImpl<crate::paging::Level3>;

            pub mod kernel_map {
                use hal::memory::{PAddr, VAddr};

                /// Platforms using `Level4` paging schemes define this to set which P4 entry is
                /// duplicated across all page tables as the kernel's entry.
                pub const KERNEL_P4_ENTRY: usize = 0;

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

                /// The kernel starts at -2GiB. The kernel image is loaded directly at this address, and the following space until
                /// the top of memory is managed dynamically and contains the boot info structures, memory map, and kernel heap.
                pub const KERNEL_BASE: VAddr = VAddr::new(0xffff_ffff_8000_0000);
            }
        }
    }
}
