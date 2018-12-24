//! This module defines the kernel entry-point on x86_64.

mod logger;
mod memory;

use self::logger::KernelLogger;
use self::memory::LockedMemoryController;
use log::info;
use x86_64::boot::BootInfo;
use x86_64::memory::kernel_map;

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
    let mut memory_controller = LockedMemoryController::new(boot_info);

    loop {}
}
