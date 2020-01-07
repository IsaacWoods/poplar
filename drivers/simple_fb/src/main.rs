#![no_std]
#![no_main]
#![feature(const_generics)]

use core::{mem::MaybeUninit, panic::PanicInfo};
use libpebble::{
    syscall,
    syscall::system_object::{FramebufferSystemObjectInfo, SystemObjectId},
};

pub struct Framebuffer {
    pointer: *mut u32,
    width: usize,
    height: usize,
    stride: usize,
}

impl Framebuffer {
    pub fn new() -> Framebuffer {
        let (framebuffer_id, framebuffer_info) = {
            let mut framebuffer_info: MaybeUninit<FramebufferSystemObjectInfo> = MaybeUninit::uninit();

            let framebuffer_id = match syscall::request_system_object(SystemObjectId::BackupFramebuffer {
                info_address: framebuffer_info.as_mut_ptr(),
            }) {
                Ok(id) => id,
                Err(err) => panic!("Failed to get ID of framebuffer memory object: {:?}", err),
            };

            (framebuffer_id, unsafe { framebuffer_info.assume_init() })
        };

        let address_space_id = syscall::my_address_space();
        syscall::map_memory_object(framebuffer_id, address_space_id).unwrap();

        assert_eq!(framebuffer_info.pixel_format, 1);

        Framebuffer {
            pointer: framebuffer_info.address as *mut u32,
            width: framebuffer_info.width as usize,
            height: framebuffer_info.height as usize,
            stride: framebuffer_info.stride as usize,
        }
    }

    pub fn draw_rect(&self, start_x: usize, start_y: usize, width: usize, height: usize, color: u32) {
        for y in start_y..(start_y + height) {
            for x in start_x..(start_x + width) {
                unsafe {
                    *(self.pointer.offset((y * self.stride + x) as isize)) = color;
                }
            }
        }
    }

    pub fn clear(&self, color: u32) {
        self.draw_rect(0, 0, self.width, self.height, color);
    }
}

#[no_mangle]
pub extern "C" fn start() -> ! {
    syscall::early_log("Simple framebuffer driver is running").unwrap();

    let framebuffer = Framebuffer::new();
    framebuffer.clear(0xffff00ff);
    framebuffer.draw_rect(100, 100, 300, 450, 0xffff0000);

    let mailbox_id = syscall::create_mailbox().unwrap();
    let mail = syscall::wait_for_mail(mailbox_id).unwrap();
    let mail2 = syscall::wait_for_mail(mailbox_id).unwrap();

    loop {}
}

#[panic_handler]
pub fn handle_panic(info: &PanicInfo) -> ! {
    // We ignore the result here because there's no point panicking in the panic handler
    let _ = syscall::early_log("Test process panicked!");
    if let Some(location) = info.location() {
        let _ = syscall::early_log(location.file());
    }
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
