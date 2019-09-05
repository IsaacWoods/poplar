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

    syscall::map_memory_object(framebuffer_id, address_space_id).unwrap();

    /*
     * TODO: we need a good way of getting the information about the framebuffer object we just
     * mapped from the kernel, including:
     *    - what virtual address it's mapped at
     *    - width, height, stride
     *    - pixel format
     *
     * Not sure how the best way to do that is. Maybe pass it back with the `request_system_object`
     * method? For now, we just hardcode the correct values.
     */
    // Each pixel is a `u32` at the moment because we know the format is always either RGB32 or BGR32
    const FRAMEBUFFER_PTR: *mut u32 = 0x00000006_00000000 as *mut u32;
    const WIDTH: usize = 800;
    const STRIDE: usize = 800;
    const HEIGHT: usize = 600;

    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            unsafe {
                *FRAMEBUFFER_PTR.offset((y * STRIDE + x) as isize) = 0xffff00ff;
            }
        }
    }

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
