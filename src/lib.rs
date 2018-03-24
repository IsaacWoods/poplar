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
    let (ramdisk_start, ramdisk_end) = architecture.get_module_address("ramdisk").expect("Couldn't load ramdisk");
    file_manager.mount("/ramdisk", Rc::new(Ramdisk::new(ramdisk_start, ramdisk_end)));

    let test_file = file_manager.open("/ramdisk/test_file").unwrap();
    let other_test = file_manager.open("/ramdisk/other_test_file").unwrap();
    info!("Test file contents: {}", core::str::from_utf8(&file_manager.read(&test_file).unwrap()).unwrap());
    info!("Other test file contents: {}", core::str::from_utf8(&file_manager.read(&other_test).unwrap()).unwrap());

    loop { }
}
