#![no_std]
#![feature(
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
    const_btree_new,
    get_mut_unchecked
)]
#[macro_use]
extern crate alloc;

mod heap_allocator;
pub mod memory;
pub mod object;
pub mod pci;
pub mod per_cpu;
pub mod scheduler;
pub mod syscall;

use crate::memory::Stack;
use alloc::{boxed::Box, sync::Arc};
use core::pin::Pin;
use hal::{
    boot_info::LoadedImage,
    memory::{FrameSize, PageTable, VirtualAddress},
};
use heap_allocator::LockedHoleAllocator;
use memory::{KernelStackAllocator, PhysicalMemoryManager};
use object::{address_space::AddressSpace, memory_object::MemoryObject, task::Task, KernelObject, KernelObjectId};
use pci::PciInfo;
use pci_types::ConfigRegionAccess as PciConfigRegionAccess;
use pebble_util::InitGuard;
use per_cpu::PerCpu;
use scheduler::Scheduler;
use spin::{Mutex, RwLock};

#[cfg(not(test))]
#[global_allocator]
pub static ALLOCATOR: LockedHoleAllocator = LockedHoleAllocator::new_uninitialized();

pub static PHYSICAL_MEMORY_MANAGER: InitGuard<PhysicalMemoryManager> = InitGuard::uninit();
pub static FRAMEBUFFER: InitGuard<(libpebble::syscall::FramebufferInfo, Arc<MemoryObject>)> = InitGuard::uninit();
pub static PCI_INFO: RwLock<Option<PciInfo>> = RwLock::new(None);
pub static PCI_ACCESS: InitGuard<Option<Mutex<Box<dyn PciConfigRegionAccess>>>> = InitGuard::uninit();

pub trait Platform: Sized + 'static {
    type PageTableSize: FrameSize;
    type PageTable: PageTable<Self::PageTableSize> + Send;
    type PerCpu: PerCpu<Self>;

    fn kernel_page_table(&mut self) -> &mut Self::PageTable;

    /// Get the per-CPU info for the current CPU. To make this safe, the per-CPU info must be installed before the
    /// `Platform` implementation is created.
    fn per_cpu<'a>() -> Pin<&'a mut Self::PerCpu>;

    /// Often, the platform will need to put stuff on either the kernel or the user stack before a task is run for
    /// the first time. `task_entry_point` is the virtual address that should be jumped to in usermode when the
    /// task is run for the first time.
    ///
    /// The return value is of the form `(kernel_stack_pointer, user_stack_pointer)`.
    unsafe fn initialize_task_stacks(
        kernel_stack: &Stack,
        user_stack: &Stack,
        task_entry_point: VirtualAddress,
    ) -> (VirtualAddress, VirtualAddress);

    /// Do the final part of a context switch: save all the state that needs to be to the current kernel stack,
    /// switch to a new kernel stack, and restore all the state from that stack.
    unsafe fn context_switch(current_kernel_stack: *mut VirtualAddress, new_kernel_stack: VirtualAddress);

    /// Do the actual drop into usermode. This assumes that the task's page tables have already been installed,
    /// and that an initial frame has been put into the task's kernel stack that this will use to enter userspace.
    unsafe fn drop_into_userspace() -> !;
}

pub fn load_task<P>(
    scheduler: &mut Scheduler<P>,
    image: &LoadedImage,
    kernel_page_table: &mut P::PageTable,
    allocator: &PhysicalMemoryManager,
    kernel_stack_allocator: &mut KernelStackAllocator<P>,
) where
    P: Platform,
{
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
        address_space.map_memory_object(memory_object, None, allocator).unwrap();
    }

    scheduler.add_task(task);
}

pub fn create_framebuffer(video_info: &hal::boot_info::VideoModeInfo) {
    use hal::{
        boot_info::PixelFormat as BootPixelFormat,
        memory::{Flags, Size4KiB},
    };
    use libpebble::syscall::{FramebufferInfo, PixelFormat};

    // We only support RGB32 and BGR32 pixel formats so BPP will always be 4 for now.
    const BPP: usize = 4;

    let size_in_bytes = video_info.stride * video_info.height * BPP;
    let memory_object = MemoryObject::new(
        object::SENTINEL_KERNEL_ID,
        None,
        video_info.framebuffer_address,
        pebble_util::math::align_up(size_in_bytes, Size4KiB::SIZE),
        Flags { writable: true, user_accessible: true, cached: false, ..Default::default() },
    );

    let info = FramebufferInfo {
        width: video_info.width as u16,
        height: video_info.height as u16,
        stride: video_info.stride as u16,
        pixel_format: match video_info.pixel_format {
            BootPixelFormat::Rgb32 => PixelFormat::Rgb32,
            BootPixelFormat::Bgr32 => PixelFormat::Bgr32,
        },
    };

    FRAMEBUFFER.initialize((info, memory_object));
}
