#![no_std]
#![feature(
    decl_macro,
    allocator_api,
    alloc_error_handler,
    trait_alias,
    type_ascription,
    naked_functions,
    get_mut_unchecked
)]
#[macro_use]
extern crate alloc;

pub mod memory;
pub mod object;
pub mod pci;
pub mod scheduler;
pub mod syscall;
pub mod tasklets;

use crate::memory::vmm::Stack;
use alloc::{boxed::Box, string::ToString, sync::Arc, vec::Vec};
use hal::memory::{FrameSize, PAddr, PageTable, Size4KiB, VAddr};
use memory::{Pmm, Vmm};
use mulch::InitGuard;
use object::{address_space::AddressSpace, memory_object::MemoryObject, task::Task};
use pci::{PciInfo, PciInterruptConfigurator, PciResolver};
use pci_types::ConfigRegionAccess as PciConfigRegionAccess;
use scheduler::Scheduler;
use seed::boot_info::BootInfo;
use spinning_top::{RwSpinlock, Spinlock};

#[cfg(not(test))]
#[global_allocator]
pub static ALLOCATOR: linked_list_allocator::LockedHeap = linked_list_allocator::LockedHeap::empty();

pub static PMM: InitGuard<Pmm> = InitGuard::uninit();
pub static VMM: InitGuard<Vmm> = InitGuard::uninit();
pub static FRAMEBUFFER: InitGuard<(poplar::syscall::FramebufferInfo, Arc<MemoryObject>)> = InitGuard::uninit();
pub static PCI_INFO: RwSpinlock<Option<PciInfo>> = RwSpinlock::new(None);
pub static PCI_ACCESS: InitGuard<Option<Spinlock<Box<dyn PciConfigRegionAccess + Send>>>> = InitGuard::uninit();

pub trait Platform: Sized + 'static {
    type PageTableSize: FrameSize;
    type PageTable: PageTable<Self::PageTableSize> + Send;
    type TaskContext;

    /// Often, the platform will need to put stuff on either the kernel or the user stack before a task is run for
    /// the first time. `task_entry_point` is the virtual address that should be jumped to in usermode when the
    /// task is run for the first time.
    ///
    /// The return value is of the form `(kernel_stack_pointer, user_stack_pointer)`.
    unsafe fn initialize_task_stacks(
        kernel_stack: &Stack,
        user_stack: &Stack,
        task_entry_point: VAddr,
    ) -> (VAddr, VAddr);

    fn new_task_context(
        kernel_stack_pointer: VAddr,
        user_stack_pointer: VAddr,
        task_entry_point: VAddr,
    ) -> Self::TaskContext;

    unsafe fn switch_user_stack_pointer(new_user_stack_pointer: VAddr) -> VAddr;

    /// Do the final part of a context switch: save all the state that needs to be for the
    /// currently running task, switch to the new kernel stack, and restore the state of the next
    /// task.
    ///
    /// This function takes both kernel stacks for the current and new tasks, and also the
    /// platform-specific task context held in the task. This is because we use various methods of
    /// doing context switches on different platforms, according to the easiest / most performant
    /// for the architecture. A pointer to the current kernel stack is provided so that it can be
    /// updated if state is pushed onto it.
    unsafe fn context_switch(
        current_kernel_stack_pointer: *mut VAddr,
        new_kernel_stack_pointer: VAddr,
        from_context: *mut Self::TaskContext,
        to_context: *const Self::TaskContext,
    );

    /// Do the actual drop into usermode. This assumes that the task's page tables have already been installed,
    /// and that an initial frame has been put into the task's kernel stack that this will use to enter userspace.
    unsafe fn drop_into_userspace(
        context: *const Self::TaskContext,
        kernel_stack_pointer: VAddr,
        user_stack_pointer: VAddr,
    ) -> !;

    // TODO: this should not exist long-term. I think the common-kernel PMM should know how to fill
    // regions of physical memory using the direct-physical-memory-map, but this can be done with
    // the revamp of the PMM.
    unsafe fn write_to_phys_memory(address: PAddr, data: &[u8]);
}

pub fn load_userspace<P>(scheduler: &Scheduler<P>, boot_info: &BootInfo, kernel_page_table: &mut P::PageTable)
where
    P: Platform,
{
    use hal::memory::Flags;
    use object::{task::Handles, SENTINEL_KERNEL_ID};
    use poplar::manifest::BootstrapManifest;

    if boot_info.loaded_images.is_empty() {
        return;
    }

    let pmm = PMM.get();
    let bootstrap_task = boot_info.loaded_images.first().unwrap();
    let address_space = AddressSpace::new(SENTINEL_KERNEL_ID, kernel_page_table, pmm);
    let handles = Handles::new();

    for segment in &bootstrap_task.segments {
        // TODO: this now uses the wrong task id...
        let memory_object = MemoryObject::from_boot_info(SENTINEL_KERNEL_ID, segment);
        handles.add(memory_object.clone());
        address_space.map_memory_object(memory_object, segment.virtual_address, pmm).unwrap();
    }

    /*
     * Add other loaded tasks' segments to the bootstrap task and add each task to the manifest.
     */
    let mut manifest =
        BootstrapManifest { task_name: bootstrap_task.name.as_str().to_string(), boot_tasks: Vec::new() };
    for image in &boot_info.loaded_images[1..] {
        let mut service = poplar::manifest::BootTask {
            name: image.name.as_str().to_string(),
            entry_point: usize::from(image.entry_point),
            segments: Vec::new(),
        };
        for segment in &image.segments {
            // TODO: this uses the wrong task ID...
            let memory_object = MemoryObject::from_boot_info(SENTINEL_KERNEL_ID, segment);
            let handle = handles.add(memory_object);
            service.segments.push((usize::from(segment.virtual_address), handle.0));
        }
        manifest.boot_tasks.push(service);
    }
    let mut buffer = Vec::new();
    let bytes_written = ptah::to_wire(&manifest, &mut buffer).unwrap();

    const MANIFEST_ADDRESS: VAddr = VAddr::new(0x20000000);
    let mem_object_len = mulch::math::align_up(bytes_written, 0x1000);
    let manifest_object = {
        let phys = pmm.alloc(mem_object_len / Size4KiB::SIZE);
        unsafe {
            P::write_to_phys_memory(phys, &(bytes_written as u32).to_le_bytes());
            P::write_to_phys_memory(phys + 4, &buffer);
        }
        MemoryObject::new(
            SENTINEL_KERNEL_ID,
            phys,
            mem_object_len,
            Flags { user_accessible: true, ..Default::default() },
        )
    };
    address_space.map_memory_object(manifest_object, MANIFEST_ADDRESS, pmm).unwrap();

    let task = Task::new(
        SENTINEL_KERNEL_ID,
        address_space.clone(),
        bootstrap_task.name.to_string(),
        bootstrap_task.entry_point,
        handles,
        pmm,
        kernel_page_table,
    )
    .expect("Failed to load bootstrapping task");
    scheduler.add_task(task);
}

pub fn create_framebuffer(video_info: &seed::boot_info::VideoModeInfo) {
    use hal::memory::{Flags, Size4KiB};
    use poplar::syscall::{FramebufferInfo, PixelFormat};
    use seed::boot_info::PixelFormat as BootPixelFormat;

    // We only support RGB32 and BGR32 pixel formats so BPP will always be 4 for now.
    const BPP: usize = 4;

    let size_in_bytes = video_info.stride * video_info.height * BPP;
    let memory_object = MemoryObject::new(
        object::SENTINEL_KERNEL_ID,
        video_info.framebuffer_address,
        mulch::math::align_up(size_in_bytes, Size4KiB::SIZE),
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

pub fn initialize_pci<A>(access: A)
where
    A: PciConfigRegionAccess + PciInterruptConfigurator + Send + 'static,
{
    let (access, info) = PciResolver::resolve(access);
    *PCI_INFO.write() = Some(info);
    PCI_ACCESS.initialize(Some(Spinlock::new(Box::new(access))));
}

#[cfg(not(test))]
#[alloc_error_handler]
fn handle_alloc_error(layout: core::alloc::Layout) -> ! {
    panic!("Alloc error: {:?}", layout);
}
