#![no_std]
#![feature(
    decl_macro,
    allocator_api,
    alloc_error_handler,
    trait_alias,
    type_ascription,
    get_mut_unchecked,
    str_from_raw_parts
)]
#[macro_use]
extern crate alloc;

pub mod bootinfo;
pub mod clocksource;
pub mod memory;
pub mod object;
pub mod pci;
pub mod scheduler;
pub mod syscall;
pub mod tasklets;

use alloc::{boxed::Box, string::ToString, sync::Arc, vec::Vec};
use bootinfo::BootInfo;
use clocksource::Clocksource;
use hal::memory::{FrameSize, PAddr, PageTable, Size4KiB, VAddr};
use memory::{vmm::Stack, Pmm, Vmm};
use mulch::InitGuard;
use object::{address_space::AddressSpace, memory_object::MemoryObject, task::Task};
use pci::{PciInfo, PciInterruptConfigurator, PciResolver};
use pci_types::ConfigRegionAccess as PciConfigRegionAccess;
use scheduler::Scheduler;
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
    type Clocksource: Clocksource;
    type TaskContext;

    /// Create a `TaskContext` for a new task with the supplied kernel and user stacks.
    fn new_task_context(kernel_stack: &Stack, user_stack: &Stack, task_entry_point: VAddr) -> Self::TaskContext;

    /// Do the arch-dependent part of the context switch. This should save the context of the
    /// currently running task into `from_context`, and restore `to_context` to start executing.
    unsafe fn context_switch(from_context: *mut Self::TaskContext, to_context: *const Self::TaskContext);

    /// Do the actual drop into usermode. This assumes that the task's page tables have already been installed.
    unsafe fn drop_into_userspace(context: *const Self::TaskContext) -> !;

    // TODO: this should not exist long-term. The common kernel VMM should know about the direct
    // physical mapping and should be able to write to physical memory itself.
    unsafe fn write_to_phys_memory(address: PAddr, data: &[u8]);

    fn rearm_interrupt(interrupt: usize);
}

pub fn load_userspace<P>(scheduler: &Scheduler<P>, boot_info: &BootInfo, kernel_page_table: &mut P::PageTable)
where
    P: Platform,
{
    use hal::memory::Flags;
    use object::{task::Handles, SENTINEL_KERNEL_ID};
    use poplar::manifest::BootstrapManifest;

    if boot_info.num_loaded_images() == 0 {
        return;
    }

    let mut loaded_images = boot_info.loaded_images();

    let pmm = PMM.get();
    let bootstrap_task = loaded_images.next().unwrap();
    let address_space = AddressSpace::new(SENTINEL_KERNEL_ID, kernel_page_table, pmm);
    let handles = Handles::new();

    for segment in &bootstrap_task.segments {
        // TODO: this now uses the wrong task id...
        let memory_object = MemoryObject::from_boot_info(SENTINEL_KERNEL_ID, segment);
        handles.add(memory_object.clone());
        address_space.map_memory_object(memory_object, VAddr::new(segment.virt_addr as usize), pmm).unwrap();
    }

    /*
     * Add other loaded tasks' segments to the bootstrap task and add each task to the manifest.
     */
    let mut manifest = BootstrapManifest { task_name: bootstrap_task.name.to_string(), boot_tasks: Vec::new() };
    for image in loaded_images {
        let mut service = poplar::manifest::BootTask {
            name: image.name.to_string(),
            entry_point: image.entry_point as usize,
            segments: Vec::new(),
        };
        for segment in &image.segments {
            // TODO: this uses the wrong task ID...
            let memory_object = MemoryObject::from_boot_info(SENTINEL_KERNEL_ID, segment);
            let handle = handles.add(memory_object);
            service.segments.push((segment.virt_addr as usize, handle.0));
        }
        manifest.boot_tasks.push(service);
    }
    let mut buffer = Vec::new();
    let bytes_written = ptah::to_wire(&manifest, &mut buffer).unwrap();

    const MANIFEST_ADDRESS: VAddr = VAddr::new(0x2000_0000);
    let mem_object_len = mulch::math::align_up(bytes_written, Size4KiB::SIZE);
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
        VAddr::new(bootstrap_task.entry_point as usize),
        handles,
        pmm,
        kernel_page_table,
    )
    .expect("Failed to load bootstrapping task");
    scheduler.add_task(task);
}

pub fn create_framebuffer(video_info: &seed_bootinfo::VideoModeInfo) {
    use hal::memory::{Flags, Size4KiB};
    use poplar::syscall::{FramebufferInfo, PixelFormat};
    use seed_bootinfo::PixelFormat as BootPixelFormat;

    // We only support RGB32 and BGR32 pixel formats so BPP will always be 4 for now.
    const BPP: u64 = 4;

    let size_in_bytes = (video_info.stride * video_info.height * BPP) as usize;
    let memory_object = MemoryObject::new(
        object::SENTINEL_KERNEL_ID,
        PAddr::new(video_info.framebuffer_address as usize).unwrap(),
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
