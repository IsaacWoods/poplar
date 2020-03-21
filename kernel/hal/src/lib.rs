#![no_std]
#![feature(const_if_match, decl_macro, step_trait)]

pub mod boot_info;
pub mod memory;

use memory::{FrameAllocator, FrameSize, Mapper};

pub trait Hal {
    type PageTableSize: FrameSize;
    type TableAllocator: FrameAllocator<Self::PageTableSize>;
    type PageTable: Mapper<Self::PageTableSize, Self::TableAllocator>;

    unsafe fn disable_interrupts();
    unsafe fn enable_interrupts();
}
