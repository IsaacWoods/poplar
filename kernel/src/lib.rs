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

use alloc::sync::Arc;
use cfg_if::cfg_if;
use core::panic::PanicInfo;
use hal::{
    boot_info::{BootInfo, LoadedImage},
    memory::VirtualAddress,
    Hal,
};
use heap_allocator::LockedHoleAllocator;
use log::{error, info};
use memory::PhysicalMemoryManager;
use object::{
    address_space::AddressSpace,
    memory_object::MemoryObject,
    task::{KernelStackAllocator, Task},
    KernelObject,
};
use pebble_util::InitGuard;
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

pub static PHYSICAL_MEMORY_MANAGER: InitGuard<PhysicalMemoryManager> = InitGuard::uninit();
pub static FRAMEBUFFER: InitGuard<(libpebble::syscall::FramebufferInfo, Arc<MemoryObject>)> = InitGuard::uninit();

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
    PHYSICAL_MEMORY_MANAGER.initialize(PhysicalMemoryManager::new(boot_info));

    let mut hal = HalImpl::init(boot_info, KernelPerCpu { scheduler: Scheduler::new() });

    // TODO: this is x86_64 specific
    const KERNEL_STACKS_BOTTOM: VirtualAddress = VirtualAddress::new(0xffff_ffdf_8000_0000);
    const KERNEL_STACKS_TOP: VirtualAddress = VirtualAddress::new(0xffff_ffff_8000_0000);
    let mut kernel_stack_allocator = KernelStackAllocator::<HalImpl>::new(
        KERNEL_STACKS_BOTTOM,
        KERNEL_STACKS_TOP,
        2 * hal::memory::MEBIBYTES_TO_BYTES,
    );

    /*
     * Create kernel objects from loaded images and schedule them.
     */
    info!("Loading {} initial tasks to the ready queue", boot_info.loaded_images.num_images);
    for image in boot_info.loaded_images.images() {
        load_task(
            unsafe { HalImpl::per_cpu() }.kernel_data().scheduler(),
            image,
            hal.kernel_page_table(),
            &PHYSICAL_MEMORY_MANAGER.get(),
            &mut kernel_stack_allocator,
        );
    }
    if let Some(ref video_info) = boot_info.video_mode {
        create_framebuffer(video_info);
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
    allocator: &PhysicalMemoryManager,
    kernel_stack_allocator: &mut KernelStackAllocator<HalImpl>,
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
        let memory_object = MemoryObject::from_boot_info(task.id(), segment);
        address_space.map_memory_object(memory_object, allocator).unwrap();
    }

    scheduler.add_task(task).unwrap();
}

fn create_framebuffer(video_info: &hal::boot_info::VideoModeInfo) {
    use hal::{
        boot_info::PixelFormat as BootPixelFormat,
        memory::{Flags, FrameSize, Size4KiB},
    };
    use libpebble::syscall::{FramebufferInfo, PixelFormat};

    // TODO: this is not the way to do it - remove the set virtual address when we've implemented the virtual range
    // manager
    const VIRTUAL_START: VirtualAddress = VirtualAddress::new(0x00000005_00000000);
    // We only support RGB32 and BGR32 pixel formats so BPP will always be 4 for now.
    const BPP: usize = 4;

    let size_in_bytes = video_info.stride * video_info.height * BPP;
    let memory_object = MemoryObject::new(
        object::SENTINEL_KERNEL_ID,
        VIRTUAL_START,
        video_info.framebuffer_address,
        pebble_util::math::align_up(size_in_bytes, Size4KiB::SIZE),
        Flags { writable: true, user_accessible: true, cached: false, ..Default::default() },
    );

    let info = FramebufferInfo {
        width: video_info.width as u16,
        height: video_info.height as u16,
        stride: video_info.stride as u16,
        pixel_format: match video_info.pixel_format {
            BootPixelFormat::RGB32 => PixelFormat::RGB32,
            BootPixelFormat::BGR32 => PixelFormat::BGR32,
        },
    };

    FRAMEBUFFER.initialize((info, memory_object));
}

#[cfg(not(test))]
#[panic_handler]
#[no_mangle]
fn panic(info: &PanicInfo) -> ! {
    error!("KERNEL PANIC: {}", info);
    <HalImpl as Hal<KernelPerCpu>>::cpu_halt()
}
