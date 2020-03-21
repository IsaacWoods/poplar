//! This module probably looks rather sparse! Check the root of one of the architecture modules for
//! an entry point.

#![cfg_attr(not(test), no_std)]
#![feature(
    asm,
    decl_macro,
    allocator_api,
    const_fn,
    alloc_error_handler,
    core_intrinsics,
    trait_alias,
    type_ascription,
    naked_functions,
    box_syntax,
    const_generics,
    global_asm
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
        use crate::x86_64 as arch_impl;
        pub use crate::x86_64::kmain;
    } else {
        compile_error!("Tried to build kernel without specifying an architecture!");
    }
}

mod heap_allocator;
mod mailbox;
mod object;
mod per_cpu;
mod scheduler;
mod syscall;

use crate::{heap_allocator::LockedHoleAllocator, object::map::ObjectMap};
use cfg_if::cfg_if;
use core::panic::PanicInfo;
use libpebble::{syscall::system_object::FramebufferSystemObjectInfo, KernelObjectId};
use log::error;
use pebble_util::InitGuard;
use spin::{Mutex, RwLock};

#[cfg(not(test))]
#[global_allocator]
pub static ALLOCATOR: LockedHoleAllocator = LockedHoleAllocator::new_uninitialized();

/// We need to make various bits of data accessible on a system-wide level (all the CPUs access the
/// same data), including from system call and interrupt handlers. I haven't discovered a
/// particularly elegant way of doing that in Rust yet, but this isn't totally awful.
///
/// This can be accessed from anywhere in the kernel, and from any CPU, and so access to each member
/// must be controlled by a type such as `Mutex` or `RwLock`. This has lower lock contention than
/// locking the entire structure.
pub static COMMON: InitGuard<Common> = InitGuard::uninit();

/// This is a collection of stuff we need to access from around the kernel, shared between all
/// CPUs. This has the potential to end up as a bit of a "God struct", so we need to be careful.
pub struct Common {
    pub object_map: RwLock<ObjectMap<arch_impl::Arch>>,

    /// If the bootloader switched to a graphics mode that enables the use of a linear framebuffer,
    /// this kernel object will be a MemoryObject that maps the backing memory into a userspace
    /// driver. This is provided to userspace through the `request_system_object` system call.
    pub backup_framebuffer: Mutex<Option<(KernelObjectId, FramebufferSystemObjectInfo)>>,
}

impl Common {
    pub fn new() -> Common {
        Common {
            object_map: RwLock::new(ObjectMap::new(crate::object::map::INITIAL_OBJECT_CAPACITY)),
            backup_framebuffer: Mutex::new(None),
        }
    }
}

#[cfg(not(test))]
#[panic_handler]
#[no_mangle]
fn panic(info: &PanicInfo) -> ! {
    error!("KERNEL PANIC: {}", info);
    loop {
        // TODO: arch-independent cpu halt?
    }
}
