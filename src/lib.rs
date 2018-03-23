/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

#![no_std]

#![feature(alloc)]
#![feature(core_intrinsics)]
#![feature(type_ascription)]
#![feature(string_retain)]

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
pub mod vfs;
pub mod ramdisk;

pub use arch::Architecture;

use alloc::boxed::Box;
use vfs::FileManager;
use ramdisk::Ramdisk;

pub fn kernel_main<A>(architecture : A) -> !
    where A : Architecture
{
    trace!("Control passed to kernel crate");
    architecture.clear_screen();

    let mut file_manager = FileManager::new();
    
    // Register ramdisk
    let (ramdisk_start, ramdisk_end) = architecture.get_module_address("ramdisk").expect("Couldn't load ramdisk");
    file_manager.add_filesystem("/ramdisk", Box::new(Ramdisk::new(ramdisk_start, ramdisk_end)));

    let test_file = file_manager.open("/ramdisk/test_file").unwrap();
    let other_test = file_manager.open("/ramdisk/other_test_file").unwrap();
    info!("Test file contents: {}", core::str::from_utf8(&*test_file.read().unwrap()).unwrap());
    info!("Other test file contents: {}", core::str::from_utf8(&*other_test.read().unwrap()).unwrap());

    loop { }
}
