#![no_std]
#![no_main]
#![feature(const_generics)]

use core::panic::PanicInfo;
use libpebble::syscall;

#[no_mangle]
pub extern "C" fn start() -> ! {
    syscall::early_log("Simple framebuffer driver is running").unwrap();

    let framebuffer_id = match syscall::request_system_object(syscall::SystemObjectId::BackupFramebuffer) {
        Ok(id) => id,
        Err(err) => panic!("Failed to get ID of framebuffer memory object: {:?}", err),
    };

    let address_space_id = syscall::my_address_space();

    // TODO: err I don't think we can do this yet. We need heap + allocator and stuff I guess
    // syscall::early_log(format!("Framebuffer id: {:?}", framebuffer_id));
    loop {}
}

#[panic_handler]
pub fn handle_panic(_: &PanicInfo) -> ! {
    // We ignore the result here because there's no point panicking in the panic handler
    let _ = syscall::early_log("Test process panicked!");
    loop {}
}

/// `N` must be a multiple of 4, and padded with zeros, so the whole descriptor is aligned to a
/// 4-byte boundary.
#[repr(C)]
pub struct Capabilities<const N: usize> {
    name_size: u32,
    desc_size: u32,
    entry_type: u32,
    name: [u8; 8],
    desc: [u8; N],
}

#[used]
#[link_section = ".caps"]
pub static mut CAPS: Capabilities<4> = Capabilities {
    name_size: 6,
    desc_size: 2,
    entry_type: 0,
    name: [b'P', b'E', b'B', b'B', b'L', b'E', b'\0', 0x00],
    desc: [0x30, 0x31, 0x00, 0x00],
};
