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

mod heap_allocator;
mod memory;
mod object;
mod per_cpu;
mod scheduler;
mod slab_allocator;
mod syscall;

use cfg_if::cfg_if;
use core::panic::PanicInfo;
use hal::{
    boot_info::{BootInfo, LoadedImage},
    memory::{FrameAllocator, VirtualAddress},
    Hal,
};
use heap_allocator::LockedHoleAllocator;
use libpebble::syscall::system_object::FramebufferSystemObjectInfo;
use log::{error, info};
use memory::PhysicalMemoryManager;
use object::{
    address_space::AddressSpace,
    memory_object::MemoryObject,
    task::{KernelStackAllocator, Task},
    KernelObject,
};
use per_cpu::KernelPerCpu;
use scheduler::Scheduler;

cfg_if! {
    if #[cfg(feature = "arch_x86_64")] {
        type HalImpl = hal_x86_64::HalImpl<KernelPerCpu>;
    } else {
        compile_error!("No architecture supplied, or target arch does not have a HAL implementation configured!");
    }
}

#[cfg(not(test))]
#[global_allocator]
pub static ALLOCATOR: LockedHoleAllocator = LockedHoleAllocator::new_uninitialized();

#[no_mangle]
pub extern "C" fn kmain(boot_info: &BootInfo) -> ! {
    HalImpl::init_logger();
    info!("The Pebble kernel is running");

    if boot_info.magic != hal::boot_info::BOOT_INFO_MAGIC {
        panic!("Boot info magic is not correct!");
    }

    /*
     * Initialise the heap allocator. After this, the kernel is free to use collections etc. that
     * can allocate on the heap through the global allocator.
     */
    unsafe {
        #[cfg(not(test))]
        ALLOCATOR.lock().init(boot_info.heap_address, boot_info.heap_size);
    }

    /*
     * We can now initialise the physical memory manager.
     */
    let physical_memory_manager = PhysicalMemoryManager::<HalImpl>::new(boot_info);

    let mut scheduler = Scheduler::new();
    let mut hal = HalImpl::init(boot_info, KernelPerCpu { scheduler });

    // TODO: start doing stuff
    loop {}
}

///// We need to make various bits of data accessible on a system-wide level (all the CPUs access the
///// same data), including from system call and interrupt handlers. I haven't discovered a
///// particularly elegant way of doing that in Rust yet, but this isn't totally awful.
/////
///// This can be accessed from anywhere in the kernel, and from any CPU, and so access to each member
///// must be controlled by a type such as `Mutex` or `RwLock`. This has lower lock contention than
///// locking the entire structure.
//pub static COMMON: InitGuard<Common> = InitGuard::uninit();

// /// This is a collection of stuff we need to access from around the kernel, shared between all
// /// CPUs. This has the potential to end up as a bit of a "God struct", so we need to be careful.
// pub struct Common {
//     pub object_map: RwLock<ObjectMap<arch_impl::Arch>>,

//     /// If the bootloader switched to a graphics mode that enables the use of a linear framebuffer,
//     /// this kernel object will be a MemoryObject that maps the backing memory into a userspace
//     /// driver. This is provided to userspace through the `request_system_object` system call.
//     pub backup_framebuffer: Mutex<Option<(KernelObjectId, FramebufferSystemObjectInfo)>>,
// }

// impl Common {
//     pub fn new() -> Common {
//         Common {
//             object_map: RwLock::new(ObjectMap::new(crate::object::map::INITIAL_OBJECT_CAPACITY)),
//             backup_framebuffer: Mutex::new(None),
//         }
//     }
// }

#[cfg(not(test))]
#[panic_handler]
#[no_mangle]
fn panic(info: &PanicInfo) -> ! {
    error!("KERNEL PANIC: {}", info);
    <HalImpl as Hal<KernelPerCpu>>::cpu_halt()
}
