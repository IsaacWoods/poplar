#![no_std]
#![no_main]
#![feature(asm, global_asm, decl_macro, naked_functions)]

extern crate alloc;

mod acpi_handler;
mod interrupts;
mod logger;
mod per_cpu;
mod task;
mod topo;

use acpi_handler::PebbleAcpiHandler;
use alloc::boxed::Box;
use aml::AmlContext;
use core::{panic::PanicInfo, pin::Pin};
use hal::{
    boot_info::BootInfo,
    memory::{Frame, PhysicalAddress, VirtualAddress},
};
use hal_x86_64::{
    hw::{cpu::CpuInfo, registers::read_control_reg},
    kernel_map,
    paging::PageTableImpl,
};
use interrupts::InterruptController;
use kernel::{
    memory::{KernelStackAllocator, PhysicalMemoryManager},
    Platform,
};
use log::{error, info};
use topo::Topology;

pub struct PlatformImpl {
    kernel_page_table: PageTableImpl,
    topology: Topology,
}

impl Platform for PlatformImpl {
    type PageTableSize = hal::memory::Size4KiB;
    type PageTable = PageTableImpl;
    type PerCpu = per_cpu::PerCpuImpl;

    fn kernel_page_table(&mut self) -> &mut Self::PageTable {
        &mut self.kernel_page_table
    }

    fn per_cpu<'a>() -> Pin<&'a mut Self::PerCpu> {
        unsafe { per_cpu::get_per_cpu_data() }
    }

    unsafe fn initialize_task_kernel_stack(
        kernel_stack_top: &mut VirtualAddress,
        task_entry_point: VirtualAddress,
        user_stack_top: &mut VirtualAddress,
    ) {
        task::initialize_kernel_stack(kernel_stack_top, task_entry_point, user_stack_top);
    }

    unsafe fn context_switch(current_kernel_stack: *mut VirtualAddress, new_kernel_stack: VirtualAddress) {
        task::context_switch(current_kernel_stack, new_kernel_stack)
    }

    unsafe fn drop_into_userspace(kernel_stack_pointer: VirtualAddress) -> ! {
        task::drop_into_userspace(kernel_stack_pointer)
    }
}

#[no_mangle]
pub extern "C" fn kentry(boot_info: &BootInfo) -> ! {
    log::set_logger(&logger::KernelLogger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    info!("Pebble kernel is running");

    if boot_info.magic != hal::boot_info::BOOT_INFO_MAGIC {
        panic!("Boot info magic is not correct!");
    }

    /*
     * Initialise the heap allocator. After this, the kernel is free to use collections etc. that
     * can allocate on the heap through the global allocator.
     */
    unsafe {
        kernel::ALLOCATOR.lock().init(boot_info.heap_address, boot_info.heap_size);
    }

    kernel::PHYSICAL_MEMORY_MANAGER.initialize(PhysicalMemoryManager::new(boot_info));

    /*
     * Create our version of the kernel page table. This assumes that the loader has correctly installed a
     * set of page tables, including a full physical mapping at the correct location. Strange things will happen
     * if this is not the case, so this is a tad unsafe.
     */
    let kernel_page_table = unsafe {
        PageTableImpl::from_frame(
            Frame::starts_with(PhysicalAddress::new(read_control_reg!(cr3) as usize).unwrap()),
            kernel_map::PHYSICAL_MAPPING_BASE,
        )
    };

    let mut kernel_stack_allocator = KernelStackAllocator::<PlatformImpl>::new(
        kernel_map::KERNEL_STACKS_BASE,
        kernel_map::KERNEL_STACKS_BASE + kernel_map::STACK_SLOT_SIZE * kernel_map::MAX_TASKS,
        hal::memory::mebibytes(2),
    );

    /*
     * Install the exception handlers. Where we do this is a compromise between as-early-as-possible (we don't
     * catch exceptions properly before this), and having enough infrastructure to install nice handlers (e.g. with
     * IST entries etc.). This seems like a good point to do it.
     */
    InterruptController::install_exception_handlers();

    /*
     * Gather information about the CPU we're running on and make sure it supports everything we need.
     */
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
     * Parse the static ACPI tables.
     */
    if boot_info.rsdp_address.is_none() {
        panic!("Bootloader did not pass RSDP address. Booting without ACPI is not supported.");
    }
    let acpi_info =
        match unsafe { acpi::parse_rsdp(&mut PebbleAcpiHandler, usize::from(boot_info.rsdp_address.unwrap())) } {
            Ok(acpi_info) => acpi_info,
            Err(err) => panic!("Failed to parse static ACPI tables: {:?}", err),
        };
    info!("{:#?}", acpi_info);

    /*
     * Create the topology, which also creates a TSS and per-CPU data for each processor, and loads them for the
     * boot processor.
     */
    let topology = topo::build_topology(&acpi_info);

    /*
     * Parse the DSDT.
     */
    // TODO: if we're on ACPI 1.0 - pass true as legacy mode.
    let mut aml_context = AmlContext::new(false, aml::DebugVerbosity::Scopes);
    if let Some(ref dsdt_info) = acpi_info.dsdt {
        let virtual_address = kernel_map::physical_to_virtual(PhysicalAddress::new(dsdt_info.address).unwrap());
        info!(
            "DSDT parse: {:?}",
            aml_context.parse_table(unsafe {
                core::slice::from_raw_parts(virtual_address.ptr(), dsdt_info.length as usize)
            })
        );

        // TODO: we should parse the SSDTs here. Only bother if we've managed to parse the DSDT.

        info!("----- Printing AML namespace -----");
        info!("{:#?}", aml_context.namespace);
        info!("----- Finished AML namespace -----");
    }


    /*
     * Initialise the interrupt controller, which enables interrupts, and start the per-cpu timer.
     */
    let mut interrupt_controller = InterruptController::init(&acpi_info, &mut aml_context);
    // interrupt_controller.enable_local_timer(&cpu_info, Duration::from_secs(3));

    let mut platform = PlatformImpl { kernel_page_table, topology };

    /*
     * Create kernel objects from loaded images and schedule them.
     */
    info!("Loading {} initial tasks to the ready queue", boot_info.loaded_images.num_images);
    for image in boot_info.loaded_images.images() {
        kernel::load_task(
            &mut PlatformImpl::per_cpu().scheduler(),
            image,
            platform.kernel_page_table(),
            &kernel::PHYSICAL_MEMORY_MANAGER.get(),
            &mut kernel_stack_allocator,
        );
    }
    if let Some(ref video_info) = boot_info.video_mode {
        kernel::create_framebuffer(video_info);
    }

    /*
     * Drop into userspace!
     */
    PlatformImpl::per_cpu().scheduler().drop_to_userspace()
}

/// We rely on certain processor features to be present for simplicity and sanity-retention. This
/// function checks that we support everything we need to, and enable features that we need.
fn check_support_and_enable_features(cpu_info: &CpuInfo) {
    use bit_field::BitField;
    use hal_x86_64::hw::registers::{
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

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    error!("KERNEL PANIC: {}", info);

    /*
     * If the `qemu_exit` feature is set, we use the debug port to exit.
     */
    #[cfg(feature = "qemu_exit")]
    {
        use hal_x86_64::hw::qemu::{ExitCode, ExitPort};
        unsafe { ExitPort::new() }.exit(ExitCode::Failed)
    }

    loop {
        unsafe {
            asm!("hlt");
        }
    }
}
