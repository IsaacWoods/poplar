#![no_std]
#![no_main]
#![feature(const_generics)]

use core::{mem::MaybeUninit, panic::PanicInfo};
use libpebble::syscall;

#[no_mangle]
pub extern "C" fn start() -> ! {
    syscall::early_log("Simple framebuffer driver is running").unwrap();

    let mut framebuffer_info: MaybeUninit<syscall::FramebufferSystemObjectInfo> = MaybeUninit::uninit();

    let framebuffer_id = match syscall::request_system_object(syscall::SystemObjectId::BackupFramebuffer {
        info_address: framebuffer_info.as_mut_ptr(),
    }) {
        Ok(id) => id,
        Err(err) => panic!("Failed to get ID of framebuffer memory object: {:?}", err),
    };

    let address_space_id = syscall::my_address_space();

    syscall::map_memory_object(framebuffer_id, address_space_id).unwrap();

    // Each pixel is a `u32` at the moment because we know the format is always either RGB32 or BGR32
    // const FRAMEBUFFER_PTR: *mut u32 = 0x00000006_00000000 as *mut u32;
    // const WIDTH: usize = 800;
    // const STRIDE: usize = 800;
    // const HEIGHT: usize = 600;

    let info: syscall::FramebufferSystemObjectInfo = unsafe { framebuffer_info.assume_init() };
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            unsafe {
                *FRAMEBUFFER_PTR.offset((y * STRIDE + x) as isize) = 0xffff00ff;
            }
        }
    }

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
