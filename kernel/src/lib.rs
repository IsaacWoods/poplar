//! This module probably looks rather sparse! Check the root of one of the architecture modules for
//! an entry point.

#![cfg_attr(not(test), no_std)]
#![feature(
    asm,
    decl_macro,
    allocator_api,
    const_fn,
    alloc,
    alloc_error_handler,
    core_intrinsics,
    trait_alias,
    type_ascription,
    naked_functions,
    bind_by_move_pattern_guards
)]
#[macro_use]
extern crate alloc;

/*
 * This selects the correct module to include depending on the architecture we're compiling the
 * kernel for. These architecture modules contain the kernel entry point and any
 * platform-specific code.
 */
cfg_if! {
    if #[cfg(feature = "arch_x86_64")] {
        mod x86_64;
        pub use crate::x86_64::kmain;
    } else {
        compile_error!("Tried to build kernel without specifying an architecture!");
    }
}

mod arch;
mod heap_allocator;
mod object;
mod syscall;
mod util;

use crate::{arch::Architecture, heap_allocator::LockedHoleAllocator};
use cfg_if::cfg_if;
use core::panic::PanicInfo;
use log::{error, info};

#[cfg(not(test))]
#[global_allocator]
pub static ALLOCATOR: LockedHoleAllocator = LockedHoleAllocator::new_uninitialized();

#[cfg(not(test))]
#[panic_handler]
#[no_mangle]
fn panic(info: &PanicInfo) -> ! {
    error!("KERNEL PANIC: {}", info);
    loop {
        // TODO: arch-independent cpu halt?
    }
}
