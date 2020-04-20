#![no_std]
#![no_main]
#![feature(const_generics, alloc_error_handler)]

#[macro_use]
extern crate alloc;

use core::{mem::MaybeUninit, panic::PanicInfo};
use libpebble::{
    caps::{CapabilitiesRepr, CAP_EARLY_LOGGING, CAP_GET_FRAMEBUFFER, CAP_PADDING},
    early_logger::EarlyLogger,
    syscall::{self, FramebufferInfo, PixelFormat},
};
use linked_list_allocator::LockedHeap;
use log::info;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

pub struct Framebuffer {
    pointer: *mut u32,
    width: usize,
    height: usize,
    stride: usize,
}

impl Framebuffer {
    pub fn new() -> Framebuffer {
        let (framebuffer_handle, framebuffer_info) = {
            let mut framebuffer_info: MaybeUninit<FramebufferInfo> = MaybeUninit::uninit();

            let framebuffer_handle = syscall::get_framebuffer(framebuffer_info.as_mut_ptr())
                .expect("Failed to get handle to framebuffer!");

            (framebuffer_handle, unsafe { framebuffer_info.assume_init() })
        };

        let mut framebuffer_address: MaybeUninit<usize> = MaybeUninit::uninit();
        syscall::map_memory_object(framebuffer_handle, libpebble::ZERO_HANDLE, framebuffer_address.as_mut_ptr())
            .unwrap();
        let framebuffer_address = unsafe { framebuffer_address.assume_init() };

        assert_eq!(framebuffer_info.pixel_format, PixelFormat::BGR32);

        Framebuffer {
            pointer: framebuffer_address as *mut u32,
            width: framebuffer_info.width as usize,
            height: framebuffer_info.height as usize,
            stride: framebuffer_info.stride as usize,
        }
    }

    pub fn draw_rect(&self, start_x: usize, start_y: usize, width: usize, height: usize, color: u32) {
        assert!((start_x + width) <= self.width);
        assert!((start_y + height) <= self.height);

        syscall::send_message(libpebble::ZERO_HANDLE, &[0xff, 0x7f, 0xdf], &[]).unwrap();

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
    // Initialise the heap
    const HEAP_START: usize = 0x600000000;
    const HEAP_SIZE: usize = 0x4000;
    let heap_memory_object = syscall::create_memory_object(HEAP_START, HEAP_SIZE, true, false).unwrap();
    syscall::map_memory_object(heap_memory_object, libpebble::ZERO_HANDLE, 0x0 as *mut usize).unwrap();
    unsafe {
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

    log::set_logger(&EarlyLogger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    info!("Simple framebuffer driver is running!");

    let framebuffer = Framebuffer::new();
    framebuffer.clear(0xffff00ff);
    framebuffer.draw_rect(100, 100, 300, 450, 0xffff0000);

    loop {}
}

#[panic_handler]
pub fn handle_panic(info: &PanicInfo) -> ! {
    log::error!("PANIC: {:?}", info);
    loop {}
}

#[alloc_error_handler]
fn alloc_error(layout: core::alloc::Layout) -> ! {
    panic!("Alloc error: {:?}", layout);
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
pub static mut CAPS: CapabilitiesRepr<4> =
    CapabilitiesRepr::new([CAP_EARLY_LOGGING, CAP_GET_FRAMEBUFFER, CAP_PADDING, CAP_PADDING]);
