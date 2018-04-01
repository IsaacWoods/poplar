/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

#![no_std]

#![feature(alloc)]
#![feature(core_intrinsics)]
#![feature(type_ascription)]
#![feature(string_retain)]
#![feature(pattern)]

#![cfg_attr(feature = "clippy", feature(plugin))]
#![cfg_attr(feature = "clippy", plugin(clippy))]

                extern crate volatile;
                extern crate spin;
                extern crate bitflags;
                extern crate bit_field;
                extern crate num_traits;
                extern crate alloc;
#[macro_use]    extern crate log;
                extern crate libpebble;

pub mod arch;
pub mod process;
pub mod syscall;
pub mod util;
pub mod fs;

pub use arch::Architecture;

use alloc::rc::Rc;
use fs::{FileManager,ramdisk::Ramdisk};

pub fn kernel_main<A>(architecture : A) -> !
    where A : Architecture
{
    trace!("Control passed to kernel crate");
    architecture.clear_screen();

    let mut file_manager = FileManager::new();
    
    // Register ramdisk
    let ramdisk_mapping = architecture.get_module_mapping("ramdisk").expect("Couldn't load ramdisk");
    file_manager.mount("/ramdisk", Rc::new(Ramdisk::new(&ramdisk_mapping)));

    let test_file = file_manager.open("/ramdisk/test_file").unwrap();
    info!("Test file contents: {}", core::str::from_utf8(&file_manager.read(&test_file).unwrap()).unwrap());

    let test_process = file_manager.open("/ramdisk/test_process.elf").unwrap();
    info!("Test process is mapped to physical memory: {:?}", unsafe { file_manager.get_physical_mapping(test_process).unwrap() });

    loop { }
}
