#![no_std]
#![cfg_attr(not(test), no_main)]
#![feature(str_from_raw_parts)]

extern crate alloc;
#[cfg(test)]
extern crate std;

mod bootinfo;
mod heap;
mod kacpi;
mod trace;

use crate::bootinfo::BootInfo;
use hal::mem::PageTable;
use hal::{cmdline::Cmdline, mem::VAddr};
use tracing::info;

#[unsafe(no_mangle)]
pub fn kentry(boot_info_ptr: VAddr) -> ! {
    tracing::dispatch::set_global_default(tracing::Dispatch::from_static(&trace::SUBSCRIBER))
        .unwrap();
    info!("Hello from the kernel!");

    let mut boot_info = BootInfo::new(boot_info_ptr);
    info!("Kernel cmdline: \"{}\"", boot_info.cmdline());
    let cmdline = Cmdline::new(boot_info.cmdline());
    trace::SUBSCRIBER.configure(&cmdline);

    let mut kernel_page_table =
        unsafe { PageTable::current(hal::mem::kernel_map::PHYSICAL_MAPPING_BASE) };
    heap::bootstrap(&mut kernel_page_table, &mut boot_info);

    let _acpi_tables = kacpi::find_tables(&boot_info);

    loop {}
}
