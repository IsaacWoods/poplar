//! This module defines the kernel entry-point on x86_64.

mod acpi_handler;
mod address_space;
mod cpu;
mod interrupts;
mod logger;
mod memory;
mod memory_object;
mod per_cpu;
mod task;

// Export the items that every architecture module is expected to provide to the rest of the
// kernel.
pub use self::{
    per_cpu::{common_per_cpu_data, common_per_cpu_data_mut},
    task::context_switch,
};

use self::{
    acpi_handler::PebbleAcpiHandler,
    address_space::AddressSpace,
    interrupts::InterruptController,
    logger::KernelLogger,
    memory::LockedPhysicalMemoryManager,
    memory_object::MemoryObject,
    task::Task,
};
use crate::{
    arch::Architecture,
    mailbox::Mailbox,
    object::{KernelObject, WrappedKernelObject},
    scheduler::Scheduler,
    x86_64::per_cpu::per_cpu_data_mut,
};
use acpi::Acpi;
use aml::AmlContext;
use core::time::Duration;
use hal::{
    boot_info::{BootInfo, LoadedImage},
    memory::{Frame, PhysicalAddress},
};
use hal_x86_64::{
    hw::{cpu::CpuInfo, gdt::Gdt, registers::read_control_reg},
    kernel_map,
    paging::PageTable,
};
use log::{error, info, warn};
use pebble_util::InitGuard;
use spin::{Mutex, RwLock};

pub(self) static GDT: Mutex<Gdt> = Mutex::new(Gdt::new());
pub static ARCH: InitGuard<Arch> = InitGuard::uninit();

pub struct Arch {
    pub cpu_info: CpuInfo,
    pub acpi_info: Option<Acpi>,
    pub aml_context: Mutex<AmlContext>,
    pub physical_memory_manager: LockedPhysicalMemoryManager,
    /// Each bit in this bitmap corresponds to a slot for an address space worth of kernel stacks
    /// in the kernel address space. We can have up 1024 address spaces, so need 128 bytes.
    pub kernel_stack_bitmap: Mutex<[u8; 128]>,
    pub kernel_page_table: Mutex<PageTable>,
}

/// `Arch` contains a bunch of things, like the GDT, that the hardware relies on actually being at
/// the memory addresses we say they're at. We can stop them moving using `Unpin`, but can't stop
/// them from being dropped, so we just panic if the architecture struct is dropped.
impl Drop for Arch {
    fn drop(&mut self) {
        panic!("The `Arch` has been dropped. This should never happen!");
    }
}

impl Architecture for Arch {
    type AddressSpace = AddressSpace;
    type Task = Task;
    type MemoryObject = MemoryObject;
    type Mailbox = Mailbox;

    fn drop_to_userspace(&self, task: WrappedKernelObject<Arch>) -> ! {
        task::drop_to_usermode(task);
    }
}

/// This is the entry point for the kernel on x86_64. It is called from `efiloader`.
#[no_mangle]
pub extern "C" fn kmain(boot_info: &BootInfo) -> ! {
    /*
     * Initialise the logger.
     */
    log::set_logger(&KernelLogger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    info!("The Pebble kernel is running");

    if boot_info.magic != hal::boot_info::BOOT_INFO_MAGIC {
        panic!("Boot info magic is not correct!");
    }

    let cpu_info = CpuInfo::new();
    info!(
        "We're running on an {:?} processor, model info = {:?}, microarch = {:?}",
        cpu_info.vendor,
        cpu_info.model_info,
        cpu_info.microarch()
    );
    if let Some(ref hypervisor_info) = cpu_info.hypervisor_info {
        info!("We're running under a hypervisor ({:?})", hypervisor_info.vendor);
    }
    check_support_and_enable_features(&cpu_info);

    /*
     * Initialise the heap allocator. After this, the kernel is free to use collections etc. that
     * can allocate on the heap through the global allocator.
     */
    #[cfg(not(test))]
    unsafe {
        crate::ALLOCATOR.lock().init(boot_info.heap_address, boot_info.heap_size);
    }

    /*
     * Parse the static ACPI tables.
     */
    let acpi_info = match boot_info.rsdp_address {
        Some(rsdp_address) => {
            let mut handler = PebbleAcpiHandler;
            match acpi::parse_rsdp(&mut handler, usize::from(rsdp_address)) {
                Ok(acpi_info) => Some(acpi_info),

                Err(err) => {
                    error!("Failed to parse static ACPI tables: {:?}", err);
                    warn!("Continuing. Some functionality may not work, or the kernel may panic!");
                    None
                }
            }
        }

        None => None,
    };

    info!("{:#?}", acpi_info);

    /*
     * Set up the main kernel data structure, which also initializes the physical memory manager.
     * From this point, we can freely allocate physical memory from any point in the kernel.
     *
     * This assumes that the bootloader has correctly installed a set of page tables, including a
     * full physical mapping in the correct location. Strange things will happen if this is not
     * true, so this process is a tad unsafe.
     */
    ARCH.initialize(Arch {
        cpu_info,
        acpi_info,
        aml_context: Mutex::new(AmlContext::new()),
        physical_memory_manager: LockedPhysicalMemoryManager::new(boot_info),
        kernel_page_table: Mutex::new(unsafe {
            PageTable::from_frame(
                Frame::starts_with(PhysicalAddress::new(read_control_reg!(cr3) as usize).unwrap()),
                kernel_map::PHYSICAL_MAPPING_BASE,
            )
        }),
        kernel_stack_bitmap: Mutex::new([0; 128]),
    });

    /*
     * Initialize the common kernel data structures too.
     */
    crate::COMMON.initialize(crate::Common::new());

    /*
     * Create the per-cpu data, then load the GDT, then install the per-cpu data. This has to be
     * done in this specific order because loading the GDT after setting GS_BASE will override it.
     */
    let (guarded_per_cpu, tss_selector) = per_cpu::GuardedPerCpu::new();
    unsafe {
        // TODO: having to lock it prevents `load` from taking a pinned reference, reference with
        // 'static, which we should probably deal with.
        GDT.lock().load(tss_selector);
    }
    guarded_per_cpu.install();

    /*
     * Parse the DSDT.
     */
    if let Some(dsdt_info) = ARCH.get().acpi_info.as_ref().and_then(|info| info.dsdt.as_ref()) {
        let virtual_address = kernel_map::physical_to_virtual(PhysicalAddress::new(dsdt_info.address).unwrap());
        info!(
            "DSDT parse: {:?}",
            ARCH.get().aml_context.lock().parse_table(unsafe {
                core::slice::from_raw_parts(virtual_address.ptr(), dsdt_info.length as usize)
            })
        );

        // TODO: we should parse the SSDTs here. If we can't find the DSDT, should we even bother?
    }

    let mut interrupt_controller = InterruptController::init(&ARCH.get());
    interrupt_controller.enable_local_timer(&ARCH.get(), Duration::from_secs(3));

    // info!("----- Printing AML namespace -----");
    // info!("{:#?}", ARCH.get().aml_context.lock().namespace);
    // info!("----- Finished AML namespace -----");

    /*
     * Create the backup framebuffer if the bootloader switched to a graphics mode.
     */
    if let Some(ref mode) = boot_info.video_mode {
        create_framebuffer(mode);
    }

    /*
     * Load all the images as initial tasks, and add them to the scheduler's ready list.
     */
    let scheduler = &mut unsafe { per_cpu_data_mut() }.common_mut().scheduler;
    info!("Adding {} initial tasks to the ready queue", boot_info.loaded_images.num_images);
    for image in boot_info.loaded_images.images() {
        load_task(&ARCH.get(), scheduler, image);
    }

    info!("Dropping to usermode");
    scheduler.drop_to_userspace(&ARCH.get())
}

fn create_framebuffer(video_info: &hal::boot_info::VideoModeInfo) {
    use hal::{
        boot_info::PixelFormat,
        memory::{Flags, FrameSize, Size4KiB, VirtualAddress},
    };

    /*
     * For now, we just put the framebuffer at the start of the region where we map MemoryObjects
     * into userspace address spaces. We might run into issues with this in the future.
     */
    const VIRTUAL_ADDRESS: VirtualAddress = self::memory::userspace_map::MEMORY_OBJECTS_START;
    /*
     * We only support RGB32 and BGR32 pixel formats, so there will always be 4 bytes per pixel.
     */
    const BPP: usize = 4;

    let size_in_bytes = video_info.stride * video_info.height * BPP;
    let memory_object = KernelObject::MemoryObject(RwLock::new(box MemoryObject::new(
        VIRTUAL_ADDRESS,
        video_info.framebuffer_address,
        pebble_util::math::align_up(size_in_bytes, Size4KiB::SIZE),
        Flags { writable: true, executable: false, user_accessible: true, cached: false },
    )))
    .add_to_map(&mut crate::COMMON.get().object_map.write());

    let info = libpebble::syscall::system_object::FramebufferSystemObjectInfo {
        address: usize::from(VIRTUAL_ADDRESS),
        width: video_info.width as u16,
        stride: video_info.stride as u16,
        height: video_info.height as u16,
        pixel_format: match video_info.pixel_format {
            // TODO: maybe define these constants in libpebble and use both here and in userspace
            PixelFormat::RGB32 => 0,
            PixelFormat::BGR32 => 1,
        },
    };

    *crate::COMMON.get().backup_framebuffer.lock() = Some((memory_object.id, info));
}

fn load_task(arch: &Arch, scheduler: &mut Scheduler, image: &LoadedImage) {
    let object_map = &mut crate::COMMON.get().object_map.write();

    // Make an AddressSpace for the image
    let address_space: WrappedKernelObject<Arch> =
        KernelObject::AddressSpace(RwLock::new(box AddressSpace::new(&arch))).add_to_map(object_map);

    // Make a MemoryObject for each segment and map it into the AddressSpace
    for segment in image.segments() {
        let memory_object =
            KernelObject::MemoryObject(RwLock::new(box MemoryObject::from_boot_info(&segment, true)))
                .add_to_map(object_map);
        address_space
            .object
            .address_space()
            .unwrap()
            .write()
            .map_memory_object(memory_object: WrappedKernelObject<Arch>)
            .unwrap();
    }

    // Create a Task for the image and add it to the scheduler's ready queue
    let task = KernelObject::Task(RwLock::new(
        box Task::from_boot_info_image(&arch, address_space.clone(), image).unwrap(),
    ))
    .add_to_map(object_map);
    scheduler.add_task(task).unwrap();
}

/// We rely on certain processor features to be present for simplicity and sanity-retention. This
/// function checks that we support everything we need to, and enable features that we need.
fn check_support_and_enable_features(cpu_info: &CpuInfo) {
    use bit_field::BitField;
    use hal_x86_64::hw::registers::{
        read_control_reg,
        read_msr,
        write_control_reg,
        write_msr,
        CR4_ENABLE_GLOBAL_PAGES,
        CR4_RESTRICT_RDTSC,
        CR4_XSAVE_ENABLE_BIT,
        EFER,
        EFER_ENABLE_NX_BIT,
        EFER_ENABLE_SYSCALL,
    };

    if !cpu_info.supported_features.xsave {
        panic!("Processor does not support xsave instruction!");
    }

    let mut cr4 = read_control_reg!(CR4);
    cr4.set_bit(CR4_XSAVE_ENABLE_BIT, true);
    cr4.set_bit(CR4_ENABLE_GLOBAL_PAGES, true);
    cr4.set_bit(CR4_RESTRICT_RDTSC, true);
    unsafe {
        write_control_reg!(CR4, cr4);
    }

    let mut efer = read_msr(EFER);
    efer.set_bit(EFER_ENABLE_SYSCALL, true);
    efer.set_bit(EFER_ENABLE_NX_BIT, true);
    unsafe {
        write_msr(EFER, efer);
    }
}
