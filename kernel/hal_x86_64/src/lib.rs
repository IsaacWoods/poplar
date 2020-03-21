#![no_std]
#![feature(asm, decl_macro, const_fn)]

#[cfg(feature = "pmm")]
#[macro_use]
extern crate alloc;

pub mod hw;
pub mod kernel_map;
#[cfg(feature = "pmm")]
pub mod memory;
pub mod paging;

use hal::{memory::Size4KiB, Hal};

pub struct HalImpl;

impl Hal for HalImpl {
    type PageTableSize = Size4KiB;
    #[cfg(feature = "pmm")]
    type TableAllocator = memory::LockedPhysicalMemoryManager;
    #[cfg(not(feature = "pmm"))]
    type TableAllocator = hal::memory::PlaceholderFrameAllocator;
    type PageTable = paging::PageTableImpl;

    unsafe fn disable_interrupts() {
        asm!("cli");
    }

    unsafe fn enable_interrupts() {
        asm!("sti");
    }
}
