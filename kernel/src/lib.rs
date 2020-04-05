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

    // TODO: this is x86_64 specific
    const KERNEL_STACKS_BOTTOM: VirtualAddress = VirtualAddress::new(0xffff_ffdf_8000_0000);
    const KERNEL_STACKS_TOP: VirtualAddress = VirtualAddress::new(0xffff_ffff_8000_0000);
    let mut kernel_stack_allocator =
        KernelStackAllocator::new(KERNEL_STACKS_BOTTOM, KERNEL_STACKS_TOP, 2 * hal::memory::MEBIBYTES_TO_BYTES);

    /*
     * Create kernel objects from loaded images and schedule them.
     */
    info!("Loading {} initial tasks to the ready queue", boot_info.loaded_images.num_images);
    for image in boot_info.loaded_images.images() {
        load_task(
            unsafe { HalImpl::per_cpu() }.kernel_data().scheduler(),
            image,
            hal.kernel_page_table(),
            &physical_memory_manager,
            &mut kernel_stack_allocator,
        );
    }

    /*
     * Drop into userspace!
     */
    unsafe { HalImpl::per_cpu() }.kernel_data().scheduler().drop_to_userspace()
}

fn load_task(
    scheduler: &mut Scheduler<HalImpl>,
    image: &LoadedImage,
    kernel_page_table: &mut <HalImpl as Hal<KernelPerCpu>>::PageTable,
    allocator: &PhysicalMemoryManager<HalImpl>,
    kernel_stack_allocator: &mut KernelStackAllocator,
) {
    use object::SENTINEL_KERNEL_ID;

    let address_space = AddressSpace::new(SENTINEL_KERNEL_ID, kernel_page_table, allocator);
    let task = Task::from_boot_info(
        SENTINEL_KERNEL_ID,
        address_space.clone(),
        image,
        allocator,
        kernel_page_table,
        kernel_stack_allocator,
    )
    .expect("Failed to load initial task");

    for segment in image.segments() {
        let memory_object = MemoryObject::from_boot_info(task.id(), segment, true);
        address_space.map_memory_object(memory_object, allocator);
    }

    scheduler.add_task(task).unwrap();
}

#[cfg(not(test))]
#[panic_handler]
#[no_mangle]
fn panic(info: &PanicInfo) -> ! {
    error!("KERNEL PANIC: {}", info);
    <HalImpl as Hal<KernelPerCpu>>::cpu_halt()
}
