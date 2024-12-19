use gfxconsole::Framebuffer;
use log::info;
use std::{
    mem::MaybeUninit,
    poplar::{
        early_logger::EarlyLogger,
        syscall::{self, FramebufferInfo, PixelFormat},
        Handle,
    },
};

pub fn main() {
    log::set_logger(&EarlyLogger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    info!("Simple framebuffer driver is running!");

    let mut framebuffer = make_framebuffer();
    let mut yields = 0;

    loop {
        framebuffer.clear(0xffaaaaaa);
        framebuffer.draw_string(
            &format!("The framebuffer driver has yielded {} times!", yields),
            400,
            400,
            0xffff0000,
        );
        yields += 1;

        syscall::yield_to_kernel();
    }
}

fn make_framebuffer() -> Framebuffer {
    /*
     * This is the virtual address the framebuffer will be mapped to in our address space.
     * NOTE: this address was basically pulled out of thin air.
     */
    const FRAMEBUFFER_ADDRESS: usize = 0x00000005_00000000;

    let (framebuffer_handle, framebuffer_info) = {
        let mut framebuffer_info: MaybeUninit<FramebufferInfo> = MaybeUninit::uninit();

        let framebuffer_handle =
            syscall::get_framebuffer(framebuffer_info.as_mut_ptr()).expect("Failed to get handle to framebuffer!");

        (framebuffer_handle, unsafe { framebuffer_info.assume_init() })
    };

    unsafe {
        syscall::map_memory_object(framebuffer_handle, Handle::ZERO, Some(FRAMEBUFFER_ADDRESS), 0x0 as *mut _)
            .unwrap();
    }
    assert_eq!(framebuffer_info.pixel_format, PixelFormat::Bgr32);

    Framebuffer::new(
        FRAMEBUFFER_ADDRESS as *mut u32,
        framebuffer_info.width as usize,
        framebuffer_info.height as usize,
        framebuffer_info.stride as usize,
        16,
        8,
        0,
    )
}
