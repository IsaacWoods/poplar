#![no_std]

mod logger;

use hal::boot_info::BootInfo;
use log::info;

#[no_mangle]
pub extern "C" fn kentry(boot_info: &BootInfo) -> ! {
    log::set_logger(&logger::KernelLogger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);

    info!("Pebble kernel in kentry!");

    if boot_info.magic != hal::boot_info::BOOT_INFO_MAGIC {
        panic!("Boot info magic is not correct!");
    }

    /*
     * Initialise the heap allocator. After this, the kernel is free to use collections etc. that
     * can allocate on the heap through the global allocator.
     */
    unsafe {
        #[cfg(not(test))]
        kernel::ALLOCATOR.lock().init(boot_info.heap_address, boot_info.heap_size);
    }

    loop {}
}
