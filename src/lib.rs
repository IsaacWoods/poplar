/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

#![no_std]

#![feature(alloc)]
#![feature(core_intrinsics)]

                extern crate volatile;
                extern crate spin;
                extern crate bitflags;
                extern crate bit_field;
                extern crate num_traits;
                extern crate alloc;
#[macro_use]    extern crate log;
                extern crate pebble_syscall_common;

pub mod arch;
pub mod process;
pub mod syscall;
pub mod util;
pub mod vfs;

pub use arch::Architecture;

use vfs::FileManager;

pub fn kernel_main<A>(architecture : A)
    where A : Architecture
{
    trace!("Control passed to kernel crate");
    architecture.clear_screen();

    let file_manager = FileManager::new();
    let test_file = file_manager.open("/ramdisk/test_file");

    loop { }
}
