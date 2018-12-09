//! This module defines the kernel entry-point on x86_64.

mod logger;

use self::logger::KernelLogger;
use log::info;
use x86_64::boot::BootInfo;

/// This is the entry point for the kernel on x86_64. It is called from the UEFI bootloader and
/// initialises the system, then passes control into the common part of the kernel.
#[no_mangle]
pub extern "C" fn kmain(boot_info: &BootInfo) -> ! {
    /*
     * Initialise the logger.
     */
    log::set_logger(&KernelLogger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    info!("The Pebble kernel is running");

    loop {}
}
