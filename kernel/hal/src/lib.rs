#![no_std]
#![feature(const_if_match, decl_macro, step_trait)]

pub mod boot_info;
pub mod memory;

use boot_info::BootInfo;
use memory::{FrameAllocator, FrameSize, PageTable};

pub trait Hal: Sized {
    type PageTableSize: FrameSize;
    type TableAllocator: FrameAllocator<Self::PageTableSize>;
    type PageTable: PageTable<Self::PageTableSize, Self::TableAllocator>;

    fn init_logger();
    fn new(boot_info: &BootInfo) -> Self;

    unsafe fn disable_interrupts();
    unsafe fn enable_interrupts();
}
