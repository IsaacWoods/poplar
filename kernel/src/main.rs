#![no_std]
#![cfg_attr(not(test), no_main)]
#![feature(str_from_raw_parts)]

// extern crate alloc;
// #[cfg(test)]
// extern crate std;

mod bootinfo;
mod trace;

use crate::bootinfo::BootInfo;
use hal::{cmdline::Cmdline, mem::VAddr};
use tracing::info;
use tracing::trace;

#[unsafe(no_mangle)]
pub fn kentry(boot_info_ptr: VAddr) -> ! {
    tracing::dispatch::set_global_default(tracing::Dispatch::from_static(&trace::SUBSCRIBER))
        .unwrap();
    info!("Hello from the kernel!");

    let boot_info = BootInfo::new(boot_info_ptr);
    info!("Kernel cmdline: \"{}\"", boot_info.cmdline());
    let cmdline = Cmdline::new(boot_info.cmdline());
    trace::SUBSCRIBER.configure(&cmdline);

    for entry in boot_info.memory_map() {
        trace!("Memory map entry: {:?}", entry);
    }

    loop {}
}
