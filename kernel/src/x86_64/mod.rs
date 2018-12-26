//! This module defines the kernel entry-point on x86_64.

mod logger;
mod memory;

use self::logger::KernelLogger;
use self::memory::physical::LockedPhysicalMemoryManager;
use self::memory::{KernelPageTable, PhysicalRegionMapper};
use crate::arch::Architecture;
use log::info;
use spin::Mutex;
use x86_64::boot::BootInfo;
use x86_64::memory::kernel_map;
use x86_64::memory::paging::table::RecursiveMapping;
use x86_64::memory::paging::ActivePageTable;

pub struct Arch {
    pub physical_memory_manager: LockedPhysicalMemoryManager,

    /// This is the main set of page tables for the kernel. It is accessed through a recursive
    /// mapping, now we are in the higher-half without an identity mapping.
    pub kernel_page_table: Mutex<KernelPageTable>,
    pub physical_region_mapper: Mutex<PhysicalRegionMapper>,
}

impl Arch {
    pub fn new(boot_info: &BootInfo) -> Arch {
        /*
         * We assume the bootloader has installed a valid set of recursively-mapped page tables for
         * the kernel. This is extremely unsafe and very bad things will happen if this assumption
         * is not true.
         */
        Arch {
            physical_memory_manager: LockedPhysicalMemoryManager::new(boot_info),
            kernel_page_table: Mutex::new(unsafe { ActivePageTable::<RecursiveMapping>::new() }),
            physical_region_mapper: Mutex::new(PhysicalRegionMapper::new()),
        }
    }
}

impl Architecture for Arch {}

/// This is the entry point for the kernel on x86_64. It is called from the UEFI bootloader and
/// initialises the system, then passes control into the common part of the kernel.
#[no_mangle]
pub fn kmain() -> ! {
    /*
     * Initialise the logger.
     */
    log::set_logger(&KernelLogger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    info!("The Pebble kernel is running");

    /*
     * Initialise the heap allocator. After this, the kernel is free to use collections etc. that
     * can allocate on the heap through the global allocator.
     */
    #[cfg(not(test))]
    unsafe {
        crate::ALLOCATOR
            .lock()
            .init(kernel_map::HEAP_START, kernel_map::HEAP_END);
    }

    /*
     * Retrieve the `BootInfo` passed to us from the bootloader and make sure it has the correct
     * magic number.
     */
    let boot_info = unsafe { &mut *(kernel_map::BOOT_INFO.mut_ptr::<BootInfo>()) };
    if boot_info.magic != x86_64::boot::BOOT_INFO_MAGIC {
        panic!("Boot info magic number is not correct!");
    }

    /*
     * Initialise the physical memory manager. From this point, we can allocate physical memory
     * freely.
     */
    let mut arch = Arch::new(boot_info);


    crate::kernel_main(arch);
}
