#![no_std]
#![feature(asm, decl_macro, const_fn, global_asm)]

#[cfg(feature = "pmm")]
#[macro_use]
extern crate alloc;

pub mod hw;
pub mod kernel_map;
pub mod logger;
#[cfg(feature = "pmm")]
pub mod memory;
pub mod paging;
pub mod task;

use hal::{boot_info::BootInfo, memory::Size4KiB, Hal};

pub struct HalImpl;

impl Hal for HalImpl {
    type PageTableSize = Size4KiB;
    type PageTable = paging::PageTableImpl;
    type TaskHelper = task::TaskHelperImpl;

    fn new(boot_info: &BootInfo) -> Self {
        HalImpl
    }

    fn init_logger() {
        log::set_logger(&logger::KernelLogger).unwrap();
        log::set_max_level(log::LevelFilter::Trace);
    }

    unsafe fn disable_interrupts() {
        asm!("cli");
    }

    unsafe fn enable_interrupts() {
        asm!("sti");
    }
}
